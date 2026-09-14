//! Orchestrates the [`WindowManager`] port: prepares the app's own window,
//! stacks game windows on the app's workspace, and keeps the switcher raised.
//! Every entry point is best effort — failures are logged, never propagated.

use tauri::{AppHandle, Manager};

use dofustabs_platform::{PlatformError, WindowManager, WindowRef};

use crate::state::{PlatformHandles, SharedState};

fn with_manager<T>(
    app: &AppHandle,
    f: impl FnOnce(&dyn WindowManager) -> Result<T, PlatformError>,
) -> Option<Result<T, PlatformError>> {
    let handles = app.state::<PlatformHandles>();
    let guard = handles.window_manager.lock().unwrap();
    guard.as_ref().map(|wm| f(wm.as_ref()))
}

/// Floats the own window and records the startup workspace/monitor area.
/// Idempotent: does nothing once the own window is known. Returns whether the
/// layout facts are available afterwards.
pub fn ensure_prepared(app: &AppHandle) -> bool {
    {
        let state = app.state::<SharedState>();
        if state.0.lock().unwrap().layout.own_window.is_some() {
            return true;
        }
    }

    let prepared = with_manager(app, |wm| {
        let own = wm.prepare_own_window()?;
        let (workspace, area) = wm.own_workspace_and_area()?;
        Ok((own, workspace, area))
    });

    match prepared {
        None => false, // degraded mode
        Some(Err(err)) => {
            log::warn!("could not prepare own window for stacking: {err}");
            false
        }
        Some(Ok((own, workspace, area))) => {
            log::info!("own window {own:?} on workspace {workspace}, usable area {area:?}");
            let state = app.state::<SharedState>();
            let mut s = state.0.lock().unwrap();
            s.layout.own_window = Some(own);
            s.layout.startup_workspace.get_or_insert(workspace);
            s.layout.monitor_area.get_or_insert(area);
            true
        }
    }
}

/// Floats and places each window on the game workspace/area. Windows already
/// arranged are skipped unless `force` is set. Lock order: state, then the
/// platform handle — never both at once.
pub fn stack_windows(app: &AppHandle, windows: &[WindowRef], force: bool) {
    if !ensure_prepared(app) {
        return;
    }

    let (targets, workspace, area) = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        let Some((workspace, area)) = s.game_target() else {
            return;
        };
        let targets: Vec<WindowRef> = windows
            .iter()
            .filter(|w| force || !s.layout.arranged.contains(w))
            .cloned()
            .collect();
        (targets, workspace, area)
    };
    if targets.is_empty() {
        return;
    }

    let mut stacked = Vec::with_capacity(targets.len());
    for win in &targets {
        match with_manager(app, |wm| wm.stack_window(win, &workspace, area)) {
            Some(Ok(())) => stacked.push(win.clone()),
            Some(Err(err)) => log::warn!("failed to stack game window {win:?}: {err}"),
            None => return,
        }
    }

    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.layout.arranged.extend(stacked);
    // Stacked windows land above the switcher; put it back on top.
    let own = s.layout.own_window.clone();
    drop(s);
    if let Some(own) = own {
        raise(app, &own);
    }
}

/// Re-stacks every tracked game window, ignoring the arranged set.
pub fn arrange_all(app: &AppHandle) {
    let windows: Vec<WindowRef> = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        s.instances.iter().map(|i| i.window_ref.clone()).collect()
    };
    stack_windows(app, &windows, true);
}

/// Brings the switcher above the game windows without taking focus.
pub fn raise_own_window(app: &AppHandle) {
    let manage = app
        .state::<SharedState>()
        .0
        .lock()
        .unwrap()
        .settings
        .manage_game_windows;
    if !manage || !ensure_prepared(app) {
        return;
    }
    let own = app
        .state::<SharedState>()
        .0
        .lock()
        .unwrap()
        .layout
        .own_window
        .clone();
    if let Some(own) = own {
        raise(app, &own);
    }
}

fn raise(app: &AppHandle, win: &WindowRef) {
    if let Some(Err(err)) = with_manager(app, |wm| wm.raise(win)) {
        log::warn!("failed to raise own window: {err}");
    }
}
