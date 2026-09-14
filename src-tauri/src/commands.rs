use tauri::{AppHandle, Emitter, State};

use dofustabs_platform::HotkeyId;

use crate::hotkey_dispatch::drain_and_activate;
use crate::settings_io;
use crate::state::{HotkeyView, InstanceView, PlatformHandles, SettingsView, SharedState};
use crate::watcher;
use crate::window_layout;

fn emit_instances(app: &AppHandle, views: &[InstanceView]) {
    if let Err(err) = app.emit("instances-changed", views) {
        log::warn!("failed to emit instances-changed: {err}");
    }
}

/// Persists the current state and returns the refreshed views.
fn persist_and_views(state: &State<'_, SharedState>) -> Vec<InstanceView> {
    let mut s = state.0.lock().unwrap();
    let path = s.settings_path.clone();
    let settings = s.snapshot_settings().clone();
    settings_io::save_logged(&path, &settings);
    s.to_views()
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
    Ok(state.0.lock().unwrap().to_views())
}

#[tauri::command]
pub fn activate(app: AppHandle, state: State<'_, SharedState>, pid: u32) -> Result<(), String> {
    state.0.lock().unwrap().queue.push_activation(pid);
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
    let (hotkey, was_active) = {
        let mut s = state.0.lock().unwrap();
        let Some(instance) = s.instances.iter_mut().find(|i| i.pid == pid) else {
            return Err(format!("unknown pid {pid}"));
        };
        instance.enabled = enabled;
        let hotkey = instance.hotkey.clone();
        let was_active = s.active_pid == Some(pid);
        if !enabled && was_active {
            s.active_pid = None;
        }
        (hotkey, was_active)
    };

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

    let views = persist_and_views(&state);
    emit_instances(&app, &views);
    Ok(views)
}

#[tauri::command]
pub fn reorder(
    app: AppHandle,
    state: State<'_, SharedState>,
    pids: Vec<u32>,
) -> Result<Vec<InstanceView>, String> {
    state.0.lock().unwrap().apply_order(&pids);

    let views = persist_and_views(&state);
    emit_instances(&app, &views);
    Ok(views)
}

fn move_instance(
    app: &AppHandle,
    state: &State<'_, SharedState>,
    pid: u32,
    delta: i32,
) -> Result<Vec<InstanceView>, String> {
    let moved = state.0.lock().unwrap().move_instance(pid, delta);
    if !moved {
        // Already at the edge of its group: nothing to persist or announce.
        return Ok(state.0.lock().unwrap().to_views());
    }

    let views = persist_and_views(state);
    emit_instances(app, &views);
    Ok(views)
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
    {
        let mut s = state.0.lock().unwrap();
        let Some(identity) = s
            .instances
            .iter()
            .find(|i| i.pid == pid)
            .map(|i| i.primary_identity().to_string())
        else {
            return Err(format!("unknown pid {pid}"));
        };
        s.settings.primary_character_name = identity;
    }

    let views = persist_and_views(&state);
    emit_instances(&app, &views);
    Ok(views)
}

#[tauri::command]
pub fn get_settings(state: State<'_, SharedState>) -> SettingsView {
    state.0.lock().unwrap().settings_view()
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
        let mut s = state.0.lock().unwrap();
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

    let enabled = {
        let mut s = state.0.lock().unwrap();
        let Some(instance) = s.instances.iter_mut().find(|i| i.pid == pid) else {
            return Err(format!("unknown pid {pid}"));
        };
        instance.hotkey = hotkey.clone();
        instance.enabled
    };

    if enabled {
        register_hotkey(&handles, HotkeyId::Instance(pid), hotkey.as_ref())?;
    }

    let views = persist_and_views(&state);
    emit_instances(&app, &views);
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
    state.0.lock().unwrap().settings.manage_game_windows = manage;
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

/// Sets the launcher path used by [`launch_game`]; blank clears the
/// override so the default location applies again.
#[tauri::command]
pub fn set_launcher_path(
    state: State<'_, SharedState>,
    path: Option<String>,
) -> Result<SettingsView, String> {
    state.0.lock().unwrap().set_launcher_path(path);
    persist_and_views(&state);
    Ok(state.0.lock().unwrap().settings_view())
}

/// Starts the game launcher detached from this app, on the dedicated GPU
/// where the platform supports forcing it. The new client shows up through
/// the regular discovery scan once its window is mapped.
#[tauri::command]
pub fn launch_game(
    state: State<'_, SharedState>,
    handles: State<'_, PlatformHandles>,
) -> Result<(), String> {
    let path = state.0.lock().unwrap().launcher_path();
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
    state.0.lock().unwrap().settings.show_sidebar_names = show;
    let views = persist_and_views(&state);
    emit_instances(&app, &views);
    Ok(())
}
