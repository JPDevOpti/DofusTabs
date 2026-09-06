use crate::{HotkeyBackend, HotkeyHandler, HotkeyId, PlatformError};
use dofustabs_core::settings::Hotkey;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

// Linux-only module, so hardcoding the Linux O_NONBLOCK value avoids a libc
// dependency for a single constant.
const O_NONBLOCK: i32 = 0o4000;

/// Global hotkeys via Hyprland binds. Each bind runs `sh` to echo the hotkey's
/// wire name into a FIFO owned by this backend; a reader thread dispatches
/// those lines to the handler. A FIFO is used because it can be written from a
/// bind with plain shell built-ins — no socat/nc required on the user's system.
///
/// The XDG GlobalShortcuts portal is the compositor-agnostic replacement for
/// this mechanism and is the intended future upgrade.
pub struct HyprlandHotkeys {
    fifo_path: PathBuf,
    binds: HashMap<HotkeyId, BindSpec>,
    handler: Arc<Mutex<Option<HotkeyHandler>>>,
    running: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
}

struct BindSpec {
    unbind_value: String,
}

impl HyprlandHotkeys {
    pub fn new() -> Result<Self, PlatformError> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let fifo_path = dir.join("dofustabs.fifo");

        let _ = fs::remove_file(&fifo_path);
        let status = Command::new("mkfifo")
            .arg(&fifo_path)
            .status()
            .map_err(|e| PlatformError::CommandFailed(format!("failed to spawn mkfifo: {e}")))?;
        if !status.success() {
            return Err(PlatformError::CommandFailed(format!(
                "mkfifo {} failed",
                fifo_path.display()
            )));
        }

        let handler: Arc<Mutex<Option<HotkeyHandler>>> = Arc::new(Mutex::new(None));
        let running = Arc::new(AtomicBool::new(true));
        let reader = spawn_reader(fifo_path.clone(), Arc::clone(&handler), Arc::clone(&running));

        Ok(Self {
            fifo_path,
            binds: HashMap::new(),
            handler,
            running,
            reader: Some(reader),
        })
    }
}

impl HotkeyBackend for HyprlandHotkeys {
    fn register(&mut self, id: HotkeyId, hotkey: &Hotkey) -> Result<(), PlatformError> {
        self.unregister(id)?;
        if hotkey.is_empty() {
            return Ok(());
        }

        let bind_value = build_bind_value(hotkey, id, &self.fifo_path);
        let stdout = super::run_hyprctl(&["keyword", "bind", &bind_value])?;
        super::expect_ok(&format!("bind {bind_value}"), &stdout)?;

        self.binds.insert(
            id,
            BindSpec {
                unbind_value: build_unbind_value(hotkey),
            },
        );
        Ok(())
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), PlatformError> {
        let Some(spec) = self.binds.remove(&id) else {
            return Ok(());
        };
        let stdout = super::run_hyprctl(&["keyword", "unbind", &spec.unbind_value])?;
        super::expect_ok(&format!("unbind {}", spec.unbind_value), &stdout)
    }

    fn set_handler(&mut self, handler: HotkeyHandler) {
        *self.handler.lock().unwrap() = Some(handler);
    }
}

impl Drop for HyprlandHotkeys {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);

        for spec in self.binds.values() {
            let _ = super::run_hyprctl(&["keyword", "unbind", &spec.unbind_value]);
        }

        // Unblock the reader thread: a non-blocking write open fails with
        // ENXIO when no reader is waiting, which is fine — it means the
        // thread already exited.
        if let Ok(mut fifo) = OpenOptions::new()
            .write(true)
            .custom_flags(O_NONBLOCK)
            .open(&self.fifo_path)
        {
            let _ = fifo.write_all(b"\n");
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let _ = fs::remove_file(&self.fifo_path);
    }
}

fn spawn_reader(
    path: PathBuf,
    handler: Arc<Mutex<Option<HotkeyHandler>>>,
    running: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while running.load(Ordering::SeqCst) {
            // Blocks until a bind (or Drop) opens the FIFO for writing; EOF
            // arrives when all writers close, then the FIFO is reopened.
            let Ok(fifo) = File::open(&path) else { return };
            for line in BufReader::new(fifo).lines() {
                if !running.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(line) = line else { break };
                if let Some(id) = HotkeyId::from_wire(line.trim()) {
                    if let Some(handler) = handler.lock().unwrap().as_ref() {
                        handler(id);
                    }
                }
            }
        }
    })
}

/// Hyprland `bind` keyword value: `MODS,key,exec,command`. The exec command
/// must not contain commas, and the FIFO lives under XDG_RUNTIME_DIR, which
/// contains neither commas nor spaces.
fn build_bind_value(hotkey: &Hotkey, id: HotkeyId, fifo: &Path) -> String {
    format!(
        "{},{},exec,sh -c 'echo {} > {}'",
        hypr_mods(hotkey),
        hypr_key(hotkey),
        id.wire_name(),
        fifo.display()
    )
}

fn build_unbind_value(hotkey: &Hotkey) -> String {
    format!("{},{}", hypr_mods(hotkey), hypr_key(hotkey))
}

fn hypr_mods(hotkey: &Hotkey) -> String {
    let mut parts = Vec::new();
    if hotkey.ctrl {
        parts.push("CTRL");
    }
    if hotkey.alt {
        parts.push("ALT");
    }
    if hotkey.shift {
        parts.push("SHIFT");
    }
    if hotkey.super_key {
        parts.push("SUPER");
    }
    parts.join(" ")
}

fn hypr_key(hotkey: &Hotkey) -> String {
    hotkey.key.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mods_render_in_fixed_order() {
        assert_eq!(hypr_mods(&Hotkey::alt("Tab")), "ALT");
        assert_eq!(hypr_mods(&Hotkey::alt_shift("Tab")), "ALT SHIFT");
        assert_eq!(hypr_mods(&Hotkey::new(true, true, true, true, "k")), "CTRL ALT SHIFT SUPER");
        assert_eq!(hypr_mods(&Hotkey::default()), "");
    }

    #[test]
    fn bind_value_targets_the_fifo() {
        let value = build_bind_value(
            &Hotkey::alt("Tab"),
            HotkeyId::Next,
            Path::new("/run/user/1000/dofustabs.fifo"),
        );
        assert_eq!(
            value,
            "ALT,tab,exec,sh -c 'echo next > /run/user/1000/dofustabs.fifo'"
        );
    }

    #[test]
    fn instance_bind_value_carries_the_pid() {
        let value = build_bind_value(
            &Hotkey::new(true, false, false, false, "1"),
            HotkeyId::Instance(4832),
            Path::new("/run/user/1000/dofustabs.fifo"),
        );
        assert_eq!(
            value,
            "CTRL,1,exec,sh -c 'echo instance:4832 > /run/user/1000/dofustabs.fifo'"
        );
    }

    #[test]
    fn unbind_value_matches_bind_prefix() {
        assert_eq!(build_unbind_value(&Hotkey::alt_shift("Tab")), "ALT SHIFT,tab");
    }
}
