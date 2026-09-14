use tauri::{AppHandle, Emitter, Manager};

use dofustabs_platform::{HotkeyHandler, HotkeyId, WindowRef};

use crate::state::{PlatformHandles, SharedState};
use crate::window_layout;

/// Builds the callback handed to the platform hotkey backend. It runs on a
/// backend-owned thread; AppHandle is Send and `emit` is thread-safe, so the
/// work happens here directly under the state mutex.
pub fn make_handler(app: AppHandle) -> HotkeyHandler {
    Box::new(move |id| {
        {
            let state = app.state::<SharedState>();
            let mut s = state.0.lock().unwrap();
            match id {
                HotkeyId::Next => s.queue.push_navigation(1),
                HotkeyId::Previous => s.queue.push_navigation(-1),
                HotkeyId::Primary => match s.primary_pid() {
                    Some(pid) => s.queue.push_activation(pid),
                    None => return,
                },
                HotkeyId::Instance(pid) => s.queue.push_activation(pid),
            }
        }
        drain_and_activate(&app);
    })
}

/// Drains the activation queue: resolves the pending target, asks the platform
/// activator to focus it, and broadcasts the new active pid. Loops because a
/// hotkey may enqueue again while an activation is in flight.
pub fn drain_and_activate(app: &AppHandle) {
    loop {
        let Some((pid, window_ref)) = take_next_target(app) else {
            return;
        };

        if !activate_window(app, &window_ref) {
            continue; // target may have just died; the queue may hold more work
        }
        // The game window is full-size on the same workspace: keep the
        // switcher visible on top of it (without stealing focus back).
        window_layout::raise_own_window(app);

        let state = app.state::<SharedState>();
        let mut s = state.0.lock().unwrap();
        s.active_pid = Some(pid);
        let views = s.to_views();
        drop(s);

        if let Err(err) = app.emit("active-changed", Some(pid)) {
            log::warn!("failed to emit active-changed: {err}");
        }
        if let Err(err) = app.emit("instances-changed", views) {
            log::warn!("failed to emit instances-changed: {err}");
        }
    }
}

fn take_next_target(app: &AppHandle) -> Option<(u32, WindowRef)> {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    let enabled = s.enabled_ordered_pids();
    let active = s.active_pid;
    let pid = s.queue.take_target(&enabled, active)?;
    let window_ref = s
        .instances
        .iter()
        .find(|i| i.pid == pid)
        .map(|i| i.window_ref.clone())?;
    Some((pid, window_ref))
}

fn activate_window(app: &AppHandle, window_ref: &WindowRef) -> bool {
    let handles = app.state::<PlatformHandles>();
    let guard = handles.activator.lock().unwrap();
    match guard.as_ref() {
        Some(activator) => match activator.activate(window_ref) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("window activation failed: {err}");
                false
            }
        },
        None => {
            log::debug!("no window activator available (degraded mode)");
            false
        }
    }
}
