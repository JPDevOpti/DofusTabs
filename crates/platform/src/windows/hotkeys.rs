//! Global hotkeys via `RegisterHotKey` on a dedicated message-loop thread.
//!
//! Win32 requires `RegisterHotKey`/`UnregisterHotKey` to run on the thread
//! that pumps the messages, so all registration flows through a command
//! channel drained by the worker; `PostThreadMessageW(WM_APP)` is the
//! doorbell that wakes the loop. Hotkeys are registered with a null HWND,
//! which binds them to the worker thread itself — no message-only window is
//! needed, and `WM_HOTKEY` arrives as a plain thread message.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY, WM_USER,
};

use dofustabs_core::settings::Hotkey;

use crate::win32_keymap::{self, MOD_NOREPEAT};
use crate::{HotkeyBackend, HotkeyHandler, HotkeyId, PlatformError};

/// Stable Win32 hotkey ids: fixed ones for the global slots, a counter-based
/// range for per-instance hotkeys.
const ID_NEXT: i32 = 1;
const ID_PREVIOUS: i32 = 2;
const ID_PRIMARY: i32 = 3;
const ID_INSTANCE_BASE: i32 = 1000;

enum Command {
    Register {
        win_id: i32,
        modifiers: u32,
        vk: u32,
        reply: Sender<Result<(), String>>,
    },
    Unregister {
        win_id: i32,
        reply: Sender<Result<(), String>>,
    },
    Shutdown,
}

pub struct WindowsHotkeys {
    worker: Option<JoinHandle<()>>,
    worker_thread_id: u32,
    cmd_tx: Sender<Command>,
    handler: Arc<Mutex<Option<HotkeyHandler>>>,
    /// Read by the worker to translate a fired Win32 id back to a HotkeyId.
    id_to_hotkey: Arc<Mutex<HashMap<i32, HotkeyId>>>,
    assigned: HashMap<HotkeyId, i32>,
    /// Win32 modifiers/vk per assigned id, so a suspended backend can
    /// re-register everything on resume.
    combos: HashMap<i32, (u32, u32)>,
    /// While true, nothing in `assigned` is registered with Win32.
    suspended: bool,
    next_instance_id: i32,
}

impl WindowsHotkeys {
    pub fn new() -> Result<Self, PlatformError> {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel::<u32>();
        let handler: Arc<Mutex<Option<HotkeyHandler>>> = Arc::new(Mutex::new(None));
        let id_to_hotkey: Arc<Mutex<HashMap<i32, HotkeyId>>> = Arc::new(Mutex::new(HashMap::new()));

        let worker_handler = Arc::clone(&handler);
        let worker_ids = Arc::clone(&id_to_hotkey);
        let worker = std::thread::Builder::new()
            .name("win-hotkeys".to_string())
            .spawn(move || message_loop(cmd_rx, ready_tx, worker_handler, worker_ids))
            .map_err(PlatformError::Io)?;

        // The worker signals readiness only after creating its message queue;
        // posting to the thread before that would fail.
        let worker_thread_id = ready_rx.recv_timeout(Duration::from_secs(5)).map_err(|_| {
            PlatformError::CommandFailed("hotkey message loop failed to start".to_string())
        })?;

        Ok(Self {
            worker: Some(worker),
            worker_thread_id,
            cmd_tx,
            handler,
            id_to_hotkey,
            assigned: HashMap::new(),
            combos: HashMap::new(),
            suspended: false,
            next_instance_id: ID_INSTANCE_BASE,
        })
    }

    fn win_id_for(&mut self, id: HotkeyId) -> i32 {
        match id {
            HotkeyId::Next => ID_NEXT,
            HotkeyId::Previous => ID_PREVIOUS,
            HotkeyId::Primary => ID_PRIMARY,
            HotkeyId::Instance(_) => *self.assigned.entry(id).or_insert_with(|| {
                self.next_instance_id += 1;
                self.next_instance_id
            }),
        }
    }

    fn ring_doorbell(&self) -> Result<(), PlatformError> {
        let ok = unsafe { PostThreadMessageW(self.worker_thread_id, WM_APP, 0, 0) };
        if ok == 0 {
            return Err(PlatformError::CommandFailed(
                "hotkey message loop is unreachable".to_string(),
            ));
        }
        Ok(())
    }

    fn send_and_wait(
        &self,
        cmd: Command,
        reply_rx: Receiver<Result<(), String>>,
    ) -> Result<(), PlatformError> {
        self.cmd_tx
            .send(cmd)
            .map_err(|_| PlatformError::CommandFailed("hotkey message loop exited".to_string()))?;
        self.ring_doorbell()?;
        reply_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| PlatformError::CommandFailed("hotkey message loop timed out".to_string()))?
            .map_err(PlatformError::CommandFailed)
    }

    fn unregister_win_id(&self, win_id: i32) -> Result<(), PlatformError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send_and_wait(
            Command::Unregister {
                win_id,
                reply: reply_tx,
            },
            reply_rx,
        )
    }

    fn register_win_id(&self, win_id: i32, modifiers: u32, vk: u32) -> Result<(), PlatformError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send_and_wait(
            Command::Register {
                win_id,
                modifiers,
                vk,
                reply: reply_tx,
            },
            reply_rx,
        )
    }
}

impl HotkeyBackend for WindowsHotkeys {
    fn register(&mut self, id: HotkeyId, hotkey: &Hotkey) -> Result<(), PlatformError> {
        // Re-register semantics: always drop the previous combination first.
        if let Some(&win_id) = self.assigned.get(&id) {
            if !self.suspended {
                self.unregister_win_id(win_id)?;
            }
            self.id_to_hotkey.lock().unwrap().remove(&win_id);
            self.combos.remove(&win_id);
            self.assigned.remove(&id);
        }

        if hotkey.is_empty() {
            return Ok(()); // empty hotkey = clear only
        }

        let (modifiers, vk) = win32_keymap::map_hotkey(hotkey).ok_or_else(|| {
            PlatformError::InvalidData(format!(
                "key '{}' has no Win32 virtual-key code",
                hotkey.key
            ))
        })?;

        let win_id = self.win_id_for(id);
        let modifiers = modifiers | MOD_NOREPEAT;
        if !self.suspended {
            self.register_win_id(win_id, modifiers, vk)?;
        }

        self.assigned.insert(id, win_id);
        self.combos.insert(win_id, (modifiers, vk));
        self.id_to_hotkey.lock().unwrap().insert(win_id, id);
        Ok(())
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), PlatformError> {
        let Some(win_id) = self.assigned.remove(&id) else {
            return Ok(()); // idempotent, matching the Linux backend
        };
        self.id_to_hotkey.lock().unwrap().remove(&win_id);
        self.combos.remove(&win_id);
        if self.suspended {
            return Ok(());
        }
        self.unregister_win_id(win_id)
    }

    fn set_handler(&mut self, handler: HotkeyHandler) {
        *self.handler.lock().unwrap() = Some(handler);
    }

    /// `RegisterHotKey` swallows the combination system-wide, so suspending
    /// really unregisters everything and resume registers it back.
    fn set_suspended(&mut self, suspended: bool) -> Result<(), PlatformError> {
        if self.suspended == suspended {
            return Ok(());
        }
        self.suspended = suspended;

        let mut first_error = None;
        for (&win_id, &(modifiers, vk)) in &self.combos {
            let result = if suspended {
                self.unregister_win_id(win_id)
            } else {
                self.register_win_id(win_id, modifiers, vk)
            };
            if let Err(err) = result {
                first_error.get_or_insert(err);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl Drop for WindowsHotkeys {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(Command::Shutdown);
        let _ = self.ring_doorbell();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn message_loop(
    cmd_rx: Receiver<Command>,
    ready_tx: Sender<u32>,
    handler: Arc<Mutex<Option<HotkeyHandler>>>,
    id_to_hotkey: Arc<Mutex<HashMap<i32, HotkeyId>>>,
) {
    // Ids this thread actually registered, so shutdown can undo exactly them.
    let mut registered: Vec<i32> = Vec::new();

    unsafe {
        // A thread only gets a message queue after its first message API
        // call; force creation before publishing our thread id.
        let mut msg: MSG = std::mem::zeroed();
        PeekMessageW(
            &mut msg,
            std::ptr::null_mut(),
            WM_USER,
            WM_USER,
            PM_NOREMOVE,
        );
        if ready_tx.send(GetCurrentThreadId()).is_err() {
            return;
        }

        loop {
            let got = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if got == 0 || got == -1 {
                break; // WM_QUIT or hard error
            }

            match msg.message {
                WM_HOTKEY => {
                    let win_id = msg.wParam as i32;
                    let id = id_to_hotkey.lock().unwrap().get(&win_id).copied();
                    if let Some(id) = id {
                        if let Some(h) = handler.lock().unwrap().as_ref() {
                            h(id);
                        }
                    }
                }
                WM_APP => {
                    let mut shutdown = false;
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            Command::Register {
                                win_id,
                                modifiers,
                                vk,
                                reply,
                            } => {
                                let ok =
                                    RegisterHotKey(std::ptr::null_mut(), win_id, modifiers, vk);
                                if ok != 0 {
                                    registered.push(win_id);
                                    let _ = reply.send(Ok(()));
                                } else {
                                    let err = GetLastError();
                                    let _ = reply.send(Err(format!(
                                        "RegisterHotKey failed (error {err}); the combination may be taken by another app"
                                    )));
                                }
                            }
                            Command::Unregister { win_id, reply } => {
                                UnregisterHotKey(std::ptr::null_mut(), win_id);
                                registered.retain(|&r| r != win_id);
                                let _ = reply.send(Ok(()));
                            }
                            Command::Shutdown => shutdown = true,
                        }
                    }
                    if shutdown {
                        for win_id in registered.drain(..) {
                            UnregisterHotKey(std::ptr::null_mut(), win_id);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
}
