//! Orchestrates the [`WindowManager`] port: prepares the app's own window,
//! stacks game windows on the app's workspace, and keeps the switcher raised.
//! Every entry point is best effort — failures are logged, never propagated.

use std::collections::HashSet;

use tauri::{AppHandle, Manager};

use dofustabs_core::geometry::{Rect, SIDEBAR_WIDTH};
use dofustabs_platform::{PlatformError, WindowManager, WindowRef};

use crate::state::{PlatformHandles, SharedState};

/// Runtime facts about window placement, resolved once the compositor has
/// mapped the app's own window (see [`ensure_prepared`]).
#[derive(Debug, Default)]
pub struct LayoutState {
    /// The app's own window, once prepared; `None` until preparation works.
    pub own_window: Option<WindowRef>,
    /// Workspace the app was on when first prepared (startup default).
    pub startup_workspace: Option<String>,
    /// Usable area of the app's monitor when first prepared.
    pub monitor_area: Option<Rect>,
    /// Name of the monitor the app's own window is on, once resolved; used
    /// to release the reserved sidebar column on exit.
    pub monitor_name: Option<String>,
    /// Whether the compositor's group tab bar was visible before the app
    /// hid it; restored on release. `None` when it was never touched.
    pub group_bar_was_visible: Option<bool>,
    /// Game windows already placed in the group once; the user's later
    /// manual moves are left alone until an explicit re-arrange.
    pub arranged: HashSet<WindowRef>,
}

fn with_manager<T>(
    app: &AppHandle,
    f: impl FnOnce(&dyn WindowManager) -> Result<T, PlatformError>,
) -> Option<Result<T, PlatformError>> {
    let handles = app.state::<PlatformHandles>();
    let guard = handles.window_manager.lock().unwrap();
    guard.as_ref().map(|wm| f(wm.as_ref()))
}

/// Floats the own window and places it as the switcher column on the left
/// edge of the usable monitor area, then records the startup
/// workspace/monitor area. Idempotent: does nothing once the own window is
/// known. Returns whether the layout facts are available afterwards.
pub fn ensure_prepared(app: &AppHandle) -> bool {
    {
        let state = app.state::<SharedState>();
        if state.lock().layout.own_window.is_some() {
            return true;
        }
    }

    let prepared = with_manager(app, |wm| {
        let own = wm.prepare_own_window()?;
        // A previous instance killed without cleanup leaves its column
        // reserved; clear it before measuring, otherwise the usable area
        // would already exclude the column and everything shifts right.
        let monitor = match wm.own_monitor_name() {
            Ok(monitor) => {
                if let Err(err) = wm.set_left_reserved(&monitor, 0) {
                    log::warn!("failed to clear a stale reservation on {monitor}: {err}");
                }
                Some(monitor)
            }
            Err(err) => {
                log::warn!("could not resolve own monitor name: {err}");
                None
            }
        };
        let (workspace, area) = wm.own_workspace_and_area()?;
        // The own window is only the switcher column on the left edge of
        // the usable area; game windows are grouped into a tile to its right.
        wm.stack_window(&own, &workspace, area.left_column(SIDEBAR_WIDTH))?;
        // Reserve the sidebar column on the own monitor so the game group
        // never covers it. A reservation failure is logged, not fatal: the
        // own window still stacks correctly, it may just get briefly
        // overlapped by a tile until the user moves it.
        if let Some(monitor) = &monitor {
            if let Err(err) = wm.set_left_reserved(monitor, SIDEBAR_WIDTH) {
                log::warn!("failed to reserve sidebar column on {monitor}: {err}");
            }
        }
        // The switcher column replaces the compositor's tab bar, which would
        // otherwise take vertical space from the game. Best effort as well.
        let group_bar = match wm.group_bar_visible() {
            Ok(visible) => {
                if let Err(err) = wm.set_group_bar_visible(false) {
                    log::warn!("failed to hide the group tab bar: {err}");
                }
                Some(visible)
            }
            Err(err) => {
                log::warn!("could not read the group tab bar setting: {err}");
                None
            }
        };
        Ok((own, workspace, area, monitor, group_bar))
    });

    match prepared {
        None => false, // degraded mode
        Some(Err(err)) => {
            log::warn!("could not prepare own window for stacking: {err}");
            false
        }
        Some(Ok((own, workspace, area, monitor, group_bar))) => {
            log::info!("own window {own:?} on workspace {workspace}, usable area {area:?}");
            let state = app.state::<SharedState>();
            let mut s = state.lock();
            s.layout.own_window = Some(own);
            s.layout.startup_workspace.get_or_insert(workspace);
            s.layout.monitor_area.get_or_insert(area);
            if let Some(monitor) = monitor {
                s.layout.monitor_name.get_or_insert(monitor);
            }
            if let Some(visible) = group_bar {
                s.layout.group_bar_was_visible.get_or_insert(visible);
            }
            true
        }
    }
}

/// Windows that still need grouping: `force` regroups everything, otherwise
/// windows already in `arranged` (the user may have moved them by hand) are
/// left alone. Kept pure so the dedup rule can be tested without a platform
/// backend or Tauri state.
fn windows_to_stack(
    windows: &[WindowRef],
    arranged: &HashSet<WindowRef>,
    force: bool,
) -> Vec<WindowRef> {
    windows
        .iter()
        .filter(|w| force || !arranged.contains(w))
        .cloned()
        .collect()
}

/// Picks the group anchor: the first window in `alive` (the currently
/// tracked instances, in order) that is already in `arranged`, i.e. already a
/// member of the group. `None` means no group exists yet and the first
/// successfully grouped window becomes it.
fn pick_anchor(arranged: &HashSet<WindowRef>, alive: &[WindowRef]) -> Option<WindowRef> {
    alive.iter().find(|w| arranged.contains(w)).cloned()
}

/// Groups every window in `targets` through `manager`, one at a time,
/// skipping (and logging) the ones that fail. The first target grouped with
/// no anchor becomes the anchor for the rest. Returns the windows that were
/// grouped successfully, so the caller can update the arranged set.
fn group_all(
    manager: &dyn WindowManager,
    targets: &[WindowRef],
    workspace: &str,
    mut anchor: Option<WindowRef>,
) -> Vec<WindowRef> {
    let mut grouped = Vec::with_capacity(targets.len());
    for win in targets {
        match manager.group_window(win, workspace, anchor.as_ref()) {
            Ok(()) => {
                grouped.push(win.clone());
                anchor.get_or_insert_with(|| win.clone());
            }
            Err(err) => log::warn!("failed to group game window {win:?}: {err}"),
        }
    }
    grouped
}

/// Joins each window into the native Hyprland game group on the game
/// workspace. Windows already arranged are skipped unless `force` is set.
/// Lock order: state, then the platform handle — never both at once.
pub fn stack_windows(app: &AppHandle, windows: &[WindowRef], force: bool) {
    if !ensure_prepared(app) {
        return;
    }

    let (targets, workspace, anchor) = {
        let state = app.state::<SharedState>();
        let s = state.lock();
        let Some((workspace, _area)) = s.game_target() else {
            return;
        };
        let alive: Vec<WindowRef> = s.instances.iter().map(|i| i.window_ref.clone()).collect();
        (
            windows_to_stack(windows, &s.layout.arranged, force),
            workspace,
            pick_anchor(&s.layout.arranged, &alive),
        )
    };
    if targets.is_empty() {
        return;
    }

    let Some(grouped) = with_manager(app, |wm| Ok(group_all(wm, &targets, &workspace, anchor)))
    else {
        return; // degraded mode
    };
    let grouped = grouped.unwrap_or_default();

    let state = app.state::<SharedState>();
    let mut s = state.lock();
    s.layout.arranged.extend(grouped);
    // Newly grouped windows may be raised above the switcher; keep it on top.
    let own = s.layout.own_window.clone();
    drop(s);
    if let Some(own) = own {
        raise(app, &own);
    }
}

/// Re-groups every tracked game window, ignoring the arranged set.
pub fn arrange_all(app: &AppHandle) {
    let windows: Vec<WindowRef> = {
        let state = app.state::<SharedState>();
        let s = state.lock();
        s.instances.iter().map(|i| i.window_ref.clone()).collect()
    };
    stack_windows(app, &windows, true);
}

/// Releases the reserved sidebar column on the own monitor. Called on app
/// exit so the next run (or the user) gets the full monitor width back.
/// Best effort, like the other entry points.
pub fn release(app: &AppHandle) {
    let (monitor, group_bar) = {
        let state = app.state::<SharedState>();
        let s = state.lock();
        (
            s.layout.monitor_name.clone(),
            s.layout.group_bar_was_visible,
        )
    };
    if let Some(visible) = group_bar {
        if let Some(Err(err)) = with_manager(app, |wm| wm.set_group_bar_visible(visible)) {
            log::warn!("failed to restore the group tab bar: {err}");
        }
    }
    let Some(monitor) = monitor else {
        return;
    };
    if let Some(Err(err)) = with_manager(app, |wm| wm.set_left_reserved(&monitor, 0)) {
        log::warn!("failed to release reserved column on {monitor}: {err}");
    }
}

/// Releases the reserved sidebar column and forgets the own window, called
/// when the own window is hidden to the tray. The window is unmapped by
/// Hyprland while hidden and gets a new address once re-shown, so the
/// stale [`WindowRef`] must be dropped; the next [`ensure_prepared`] call
/// re-floats, re-stacks and re-reserves once the window is mapped again.
pub fn suspend(app: &AppHandle) {
    release(app);
    let state = app.state::<SharedState>();
    let mut s = state.lock();
    s.layout.own_window = None;
}

/// Whether the own window is currently known/prepared.
pub fn is_prepared(app: &AppHandle) -> bool {
    let state = app.state::<SharedState>();
    let known = state.lock().layout.own_window.is_some();
    known
}

/// Brings the switcher above the game windows without taking focus.
pub fn raise_own_window(app: &AppHandle) {
    let own = {
        let state = app.state::<SharedState>();
        let s = state.lock();
        if !s.settings.manage_game_windows {
            return;
        }
        s.layout.own_window.clone()
    };
    // `ensure_prepared` re-locks state itself; the own-window snapshot above
    // is taken and dropped first so this never nests two state locks.
    if own.is_none() && !ensure_prepared(app) {
        return;
    }
    let own = own.or_else(|| app.state::<SharedState>().lock().layout.own_window.clone());
    if let Some(own) = own {
        raise(app, &own);
    }
}

/// Widens or shrinks the switcher column. Used to widen it while an overlay
/// (settings panel, account popover) extends past the 64px column, and to
/// shrink it back once every overlay is closed. Best effort, like the other
/// entry points.
pub fn set_own_width(app: &AppHandle, width: i32) {
    if !ensure_prepared(app) {
        return;
    }

    let (own, workspace, area) = {
        let state = app.state::<SharedState>();
        let s = state.lock();
        let (Some(own), Some(workspace), Some(area)) = (
            s.layout.own_window.clone(),
            s.layout.startup_workspace.clone(),
            s.layout.monitor_area,
        ) else {
            return;
        };
        (own, workspace, area)
    };

    match with_manager(app, |wm| {
        wm.stack_window(&own, &workspace, area.left_column(width))
    }) {
        Some(Ok(())) => {}
        Some(Err(err)) => log::warn!("failed to resize own window: {err}"),
        None => return,
    }
    raise(app, &own);
}

fn raise(app: &AppHandle, win: &WindowRef) {
    if let Some(Err(err)) = with_manager(app, |wm| wm.raise(win)) {
        log::warn!("failed to raise own window: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn wr(n: u32) -> WindowRef {
        WindowRef::WaylandAddress(format!("0x{n:x}"))
    }

    #[test]
    fn dedup_skips_already_arranged_windows_unless_forced() {
        let arranged: HashSet<WindowRef> = [wr(1)].into_iter().collect();
        let windows = vec![wr(1), wr(2)];

        assert_eq!(windows_to_stack(&windows, &arranged, false), vec![wr(2)]);
        assert_eq!(
            windows_to_stack(&windows, &arranged, true),
            vec![wr(1), wr(2)]
        );
        assert!(windows_to_stack(&[], &arranged, false).is_empty());
    }

    /// Fake [`WindowManager`]: fails to stack windows listed in `failing`,
    /// recording every attempted stack call.
    struct FakeWindowManager {
        failing: Vec<WindowRef>,
        attempts: RefCell<Vec<WindowRef>>,
    }

    impl WindowManager for FakeWindowManager {
        fn prepare_own_window(&self) -> Result<WindowRef, PlatformError> {
            unimplemented!("not exercised by this test")
        }

        fn stack_window(
            &self,
            win: &WindowRef,
            _workspace: &str,
            _rect: Rect,
        ) -> Result<(), PlatformError> {
            self.attempts.borrow_mut().push(win.clone());
            if self.failing.contains(win) {
                Err(PlatformError::CommandFailed("boom".to_string()))
            } else {
                Ok(())
            }
        }

        fn raise(&self, _win: &WindowRef) -> Result<(), PlatformError> {
            unimplemented!("not exercised by this test")
        }

        fn own_workspace_and_area(&self) -> Result<(String, Rect), PlatformError> {
            unimplemented!("not exercised by this test")
        }

        fn own_monitor_name(&self) -> Result<String, PlatformError> {
            unimplemented!("not exercised by this test")
        }

        fn set_left_reserved(&self, _monitor: &str, _px: i32) -> Result<(), PlatformError> {
            unimplemented!("not exercised by this test")
        }

        fn group_bar_visible(&self) -> Result<bool, PlatformError> {
            unimplemented!("not exercised by this test")
        }

        fn set_group_bar_visible(&self, _visible: bool) -> Result<(), PlatformError> {
            unimplemented!("not exercised by this test")
        }

        fn group_window(
            &self,
            win: &WindowRef,
            _workspace: &str,
            _anchor: Option<&WindowRef>,
        ) -> Result<(), PlatformError> {
            self.attempts.borrow_mut().push(win.clone());
            if self.failing.contains(win) {
                Err(PlatformError::CommandFailed("boom".to_string()))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn group_all_skips_failures_and_returns_only_successes() {
        let manager = FakeWindowManager {
            failing: vec![wr(2)],
            attempts: RefCell::new(Vec::new()),
        };
        let targets = vec![wr(1), wr(2), wr(3)];

        let grouped = group_all(&manager, &targets, "2", None);

        assert_eq!(grouped, vec![wr(1), wr(3)]);
        assert_eq!(manager.attempts.into_inner(), targets);
    }

    #[test]
    fn group_all_over_empty_targets_groups_nothing() {
        let manager = FakeWindowManager {
            failing: Vec::new(),
            attempts: RefCell::new(Vec::new()),
        };
        assert!(group_all(&manager, &[], "1", None).is_empty());
    }

    #[test]
    fn pick_anchor_finds_first_already_arranged_alive_window() {
        let arranged: HashSet<WindowRef> = [wr(2), wr(3)].into_iter().collect();
        let alive = vec![wr(1), wr(2), wr(3)];

        assert_eq!(pick_anchor(&arranged, &alive), Some(wr(2)));
        assert_eq!(pick_anchor(&HashSet::new(), &alive), None);
        assert_eq!(pick_anchor(&arranged, &[]), None);
    }
}
