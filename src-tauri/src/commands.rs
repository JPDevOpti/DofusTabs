use tauri::{AppHandle, Emitter, State};

use dofustabs_core::geometry::SIDEBAR_WIDTH;
use dofustabs_platform::HotkeyId;

use crate::hotkey_dispatch::drain_and_activate;
use crate::settings_io;
use crate::state::{
    AppState, HotkeyView, InstanceView, LayoutConstantsView, PlatformHandles, SettingsView,
    SharedState,
};
use crate::watcher;
use crate::window_layout;

fn emit_instances(app: &AppHandle, views: &[InstanceView]) {
    if let Err(err) = app.emit("instances-changed", views) {
        log::warn!("failed to emit instances-changed: {err}");
    }
}

/// Persists the current state and returns the refreshed views. Shared by
/// every command that just changed the persisted settings shape.
fn persist_and_views(state: &State<'_, SharedState>) -> Vec<InstanceView> {
    let mut s = state.lock();
    let path = s.settings_path.clone();
    let settings = s.snapshot_settings().clone();
    settings_io::save_logged(&path, &settings);
    s.to_views()
}

/// Persists and emits without a preceding mutation; used when the mutation
/// already happened under its own lock (e.g. after a conditional check).
fn persist_and_emit(app: &AppHandle, state: &State<'_, SharedState>) -> Vec<InstanceView> {
    let views = persist_and_views(state);
    emit_instances(app, &views);
    views
}

/// Locks state, runs `f`, persists, then emits `instances-changed` with the
/// refreshed views. This is the shared shape behind most mutating commands:
/// lock -> mutate -> persist -> emit.
fn mutate_and_emit<R>(
    app: &AppHandle,
    state: &State<'_, SharedState>,
    f: impl FnOnce(&mut AppState) -> R,
) -> (R, Vec<InstanceView>) {
    let result = {
        let mut s = state.lock();
        f(&mut s)
    };
    let views = persist_and_views(state);
    emit_instances(app, &views);
    (result, views)
}

/// Same shape as [`mutate_and_emit`] but for settings-only commands that
/// don't need the mutation's return value threaded back to the caller.
fn mutate_settings_and_emit(
    app: &AppHandle,
    state: &State<'_, SharedState>,
    f: impl FnOnce(&mut AppState),
) -> Vec<InstanceView> {
    mutate_and_emit(app, state, f).1
}

fn register_hotkey(
    handles: &State<'_, PlatformHandles>,
    id: HotkeyId,
    hotkey: Option<&dofustabs_core::settings::Hotkey>,
) -> Result<(), String> {
    let mut guard = handles.hotkeys.lock().unwrap();
    let Some(backend) = guard.as_mut() else {
        return Ok(()); // degraded mode: persist-only
    };
    match hotkey {
        Some(hk) if !hk.is_empty() => backend.register(id, hk).map_err(|e| e.to_string()),
        _ => backend.unregister(id).map_err(|e| e.to_string()),
    }
}

/// Forces a discovery scan right now (the watcher also scans every second)
/// and answers with the resulting snapshot.
#[tauri::command]
pub fn refresh(app: AppHandle, state: State<'_, SharedState>) -> Result<Vec<InstanceView>, String> {
    watcher::scan_now(&app).map_err(|e| e.to_string())?;
    Ok(state.lock().to_views())
}

#[tauri::command]
pub fn activate(app: AppHandle, state: State<'_, SharedState>, pid: u32) -> Result<(), String> {
    state.lock().queue.push_activation(pid);
    drain_and_activate(&app);
    Ok(())
}

#[tauri::command]
pub fn set_enabled(
    app: AppHandle,
    state: State<'_, SharedState>,
    handles: State<'_, PlatformHandles>,
    pid: u32,
    enabled: bool,
) -> Result<Vec<InstanceView>, String> {
    if !state.lock().instances.iter().any(|i| i.pid == pid) {
        return Err(format!("unknown pid {pid}"));
    }
    // The instance may still disappear between the check above and the
    // mutation below (the watcher runs on its own thread); treat that as a
    // benign no-op rather than a hard error.
    let ((hotkey, was_active), views) = mutate_and_emit(&app, &state, |s| {
        let Some(instance) = s.instances.iter_mut().find(|i| i.pid == pid) else {
            return (None, false);
        };
        instance.enabled = enabled;
        let hotkey = instance.hotkey.clone();
        let was_active = s.active_pid == Some(pid);
        if !enabled && was_active {
            s.active_pid = None;
        }
        (hotkey, was_active)
    });

    if enabled {
        register_hotkey(&handles, HotkeyId::Instance(pid), hotkey.as_ref())?;
    } else {
        register_hotkey(&handles, HotkeyId::Instance(pid), None)?;
        if was_active {
            if let Err(err) = app.emit("active-changed", Option::<u32>::None) {
                log::warn!("failed to emit active-changed: {err}");
            }
        }
    }

    Ok(views)
}

#[tauri::command]
pub fn reorder(
    app: AppHandle,
    state: State<'_, SharedState>,
    pids: Vec<u32>,
) -> Result<Vec<InstanceView>, String> {
    Ok(mutate_settings_and_emit(&app, &state, |s| {
        s.apply_order(&pids)
    }))
}

fn move_instance(
    app: &AppHandle,
    state: &State<'_, SharedState>,
    pid: u32,
    delta: i32,
) -> Result<Vec<InstanceView>, String> {
    let moved = state.lock().move_instance(pid, delta);
    if !moved {
        // Already at the edge of its group: nothing to persist or announce.
        return Ok(state.lock().to_views());
    }
    Ok(persist_and_emit(app, state))
}

#[tauri::command]
pub fn move_up(
    app: AppHandle,
    state: State<'_, SharedState>,
    pid: u32,
) -> Result<Vec<InstanceView>, String> {
    move_instance(&app, &state, pid, -1)
}

#[tauri::command]
pub fn move_down(
    app: AppHandle,
    state: State<'_, SharedState>,
    pid: u32,
) -> Result<Vec<InstanceView>, String> {
    move_instance(&app, &state, pid, 1)
}

#[tauri::command]
pub fn set_primary(
    app: AppHandle,
    state: State<'_, SharedState>,
    pid: u32,
) -> Result<Vec<InstanceView>, String> {
    if !state.lock().instances.iter().any(|i| i.pid == pid) {
        return Err(format!("unknown pid {pid}"));
    }
    let (_, views) = mutate_and_emit(&app, &state, |s| {
        if let Some(identity) = s
            .instances
            .iter()
            .find(|i| i.pid == pid)
            .map(|i| i.primary_identity().to_string())
        {
            s.settings.primary_character_name = identity;
        }
    });
    Ok(views)
}

#[tauri::command]
pub fn get_settings(state: State<'_, SharedState>) -> SettingsView {
    state.lock().settings_view()
}

/// Layout facts the frontend needs but must not hardcode, sourced from the
/// same constant the platform layer uses to place windows.
#[tauri::command]
pub fn layout_constants() -> LayoutConstantsView {
    LayoutConstantsView {
        sidebar_width: SIDEBAR_WIDTH as u32,
    }
}

#[tauri::command]
pub fn set_global_hotkey(
    state: State<'_, SharedState>,
    handles: State<'_, PlatformHandles>,
    slot: String,
    hotkey: Option<HotkeyView>,
) -> Result<(), String> {
    let hotkey = hotkey.map(HotkeyView::into_core).unwrap_or_default();

    let id = {
        let mut s = state.lock();
        let (slot_ref, id) = match slot.as_str() {
            "next" => (&mut s.settings.next_hotkey, HotkeyId::Next),
            "previous" => (&mut s.settings.previous_hotkey, HotkeyId::Previous),
            "primary" => (&mut s.settings.primary_hotkey, HotkeyId::Primary),
            other => return Err(format!("unknown hotkey slot '{other}'")),
        };
        *slot_ref = hotkey.clone();
        id
    };

    register_hotkey(&handles, id, Some(&hotkey))?;

    persist_and_views(&state);
    Ok(())
}

#[tauri::command]
pub fn set_instance_hotkey(
    app: AppHandle,
    state: State<'_, SharedState>,
    handles: State<'_, PlatformHandles>,
    pid: u32,
    hotkey: Option<HotkeyView>,
) -> Result<(), String> {
    let hotkey = hotkey
        .map(HotkeyView::into_core)
        .filter(|hk| !hk.is_empty());

    if !state.lock().instances.iter().any(|i| i.pid == pid) {
        return Err(format!("unknown pid {pid}"));
    }
    let (enabled, _views) = mutate_and_emit(&app, &state, |s| {
        let Some(instance) = s.instances.iter_mut().find(|i| i.pid == pid) else {
            return false;
        };
        instance.hotkey = hotkey.clone();
        instance.enabled
    });

    if enabled {
        register_hotkey(&handles, HotkeyId::Instance(pid), hotkey.as_ref())?;
    }

    Ok(())
}

/// Releases (or restores) every OS-level hotkey while the UI captures a new
/// combination, so the pressed keys reach the capture widget instead of
/// triggering a switch. Degraded mode has nothing to suspend.
#[tauri::command]
pub fn set_hotkeys_suspended(
    handles: State<'_, PlatformHandles>,
    suspended: bool,
) -> Result<(), String> {
    let mut guard = handles.hotkeys.lock().unwrap();
    match guard.as_mut() {
        Some(backend) => backend.set_suspended(suspended).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Toggles compositor-level management of the game windows. Turning it on
/// stacks every current game window right away; turning it off leaves the
/// windows where they are.
#[tauri::command]
pub fn set_manage_game_windows(
    app: AppHandle,
    state: State<'_, SharedState>,
    manage: bool,
) -> Result<(), String> {
    state.lock().settings.manage_game_windows = manage;
    persist_and_views(&state);
    if manage {
        window_layout::arrange_all(&app);
    }
    Ok(())
}

/// Re-applies the stacking to every current game window, including the ones
/// the user moved by hand since they appeared.
#[tauri::command]
pub fn arrange_windows(app: AppHandle) -> Result<(), String> {
    window_layout::arrange_all(&app);
    Ok(())
}

/// Widens the switcher column to fit an overlay (settings panel, account
/// popover) rendered to its right. `extra` is the width in logical pixels
/// the overlay needs beyond the column; 0 collapses it back.
#[tauri::command]
pub fn set_sidebar_extension(app: AppHandle, extra: u32) -> Result<(), String> {
    window_layout::set_own_width(&app, SIDEBAR_WIDTH + extra as i32);
    Ok(())
}

/// Sets the launcher path used by [`launch_game`]; blank clears the
/// override so the default location applies again.
#[tauri::command]
pub fn set_launcher_path(
    state: State<'_, SharedState>,
    path: Option<String>,
) -> Result<SettingsView, String> {
    state.lock().set_launcher_path(path);
    persist_and_views(&state);
    Ok(state.lock().settings_view())
}

/// Starts the game launcher detached from this app, on the dedicated GPU
/// where the platform supports forcing it. The new client shows up through
/// the regular discovery scan once its window is mapped.
#[tauri::command]
pub fn launch_game(
    state: State<'_, SharedState>,
    handles: State<'_, PlatformHandles>,
) -> Result<(), String> {
    let path = state.lock().launcher_path();
    let guard = handles.launcher.lock().unwrap();
    let Some(launcher) = guard.as_ref() else {
        return Err("platform backend unavailable: cannot launch the game".to_string());
    };
    launcher.launch(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_show_names(
    app: AppHandle,
    state: State<'_, SharedState>,
    show: bool,
) -> Result<(), String> {
    mutate_settings_and_emit(&app, &state, |s| {
        s.settings.show_sidebar_names = show;
    });
    Ok(())
}
