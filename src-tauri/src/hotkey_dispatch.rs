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
            let mut s = state.lock();
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
    drain_with(
        || take_next_target(app),
        |window_ref| activate_window(app, window_ref),
        |pid| {
            // The game window sits next to the switcher column, not under
            // it: keep the column on top without stealing focus back.
            window_layout::raise_own_window(app);

            let state = app.state::<SharedState>();
            let mut s = state.lock();
            s.active_pid = Some(pid);
            let views = s.to_views();
            drop(s);

            if let Err(err) = app.emit("active-changed", Some(pid)) {
                log::warn!("failed to emit active-changed: {err}");
            }
            if let Err(err) = app.emit("instances-changed", views) {
                log::warn!("failed to emit instances-changed: {err}");
            }
        },
    );
}

/// Pure drain loop: pulls targets from `next` until it yields `None`, tries
/// `activate` on each, and calls `on_activated` only for the ones that
/// succeeded. A failed activation (the target may have just died) is skipped
/// without stopping the drain, since the queue may hold more work. Returns
/// the pids that were successfully activated, in order.
fn drain_with(
    mut next: impl FnMut() -> Option<(u32, WindowRef)>,
    mut activate: impl FnMut(&WindowRef) -> bool,
    mut on_activated: impl FnMut(u32),
) -> Vec<u32> {
    let mut activated = Vec::new();
    while let Some((pid, window_ref)) = next() {
        if !activate(&window_ref) {
            continue;
        }
        on_activated(pid);
        activated.push(pid);
    }
    activated
}

fn take_next_target(app: &AppHandle) -> Option<(u32, WindowRef)> {
    let state = app.state::<SharedState>();
    let mut s = state.lock();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn wr(n: u32) -> WindowRef {
        WindowRef::WaylandAddress(format!("0x{n:x}"))
    }

    /// Fake activator: fails for pids listed in `failing`, succeeds for the
    /// rest, recording every attempted address.
    struct FakeActivator {
        failing: Vec<WindowRef>,
        attempted: RefCell<Vec<WindowRef>>,
    }

    impl FakeActivator {
        fn activate(&self, win: &WindowRef) -> bool {
            self.attempted.borrow_mut().push(win.clone());
            !self.failing.contains(win)
        }
    }

    #[test]
    fn drain_continues_past_a_failed_activation() {
        let mut queue = vec![(10, wr(10)), (20, wr(20)), (30, wr(30))];
        let activator = FakeActivator {
            failing: vec![wr(20)],
            attempted: RefCell::new(Vec::new()),
        };

        let activated = drain_with(
            || {
                if queue.is_empty() {
                    None
                } else {
                    Some(queue.remove(0))
                }
            },
            |win| activator.activate(win),
            |_pid| {},
        );

        assert_eq!(activated, vec![10, 30]);
        assert_eq!(
            activator.attempted.into_inner(),
            vec![wr(10), wr(20), wr(30)]
        );
    }

    #[test]
    fn drain_calls_on_activated_only_for_successes() {
        let mut queue = vec![(1, wr(1)), (2, wr(2))];
        let activator = FakeActivator {
            failing: vec![wr(1)],
            attempted: RefCell::new(Vec::new()),
        };
        let notified = RefCell::new(Vec::new());

        drain_with(
            || {
                if queue.is_empty() {
                    None
                } else {
                    Some(queue.remove(0))
                }
            },
            |win| activator.activate(win),
            |pid| notified.borrow_mut().push(pid),
        );

        assert_eq!(notified.into_inner(), vec![2]);
    }

    #[test]
    fn empty_queue_activates_nothing() {
        let activated = drain_with(|| None, |_win| true, |_pid| {});
        assert!(activated.is_empty());
    }
}
