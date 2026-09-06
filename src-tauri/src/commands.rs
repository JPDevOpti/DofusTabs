use tauri::{AppHandle, Emitter, State};

use dofustabs_platform::HotkeyId;

use crate::hotkey_dispatch::drain_and_activate;
use crate::settings_io;
use crate::state::{HotkeyView, InstanceView, PlatformHandles, SettingsView, SharedState};

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

#[tauri::command]
pub fn refresh(state: State<'_, SharedState>) -> Vec<InstanceView> {
    // The watcher owns discovery and pushes changes every second; refresh
    // just answers with the current snapshot.
    state.0.lock().unwrap().to_views()
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
    {
        let mut s = state.0.lock().unwrap();
        let known: Vec<u32> = s.instances.iter().map(|i| i.pid).collect();
        // Listed pids take positions 0..n in the given order; unlisted ones
        // keep their relative order after them.
        let mut next = pids.len() as i32;
        for pid in &known {
            if !pids.contains(pid) {
                if let Some(i) = s.instances.iter_mut().find(|i| i.pid == *pid) {
                    i.display_order = next;
                    next += 1;
                }
            }
        }
        for (index, pid) in pids.iter().enumerate() {
            if let Some(i) = s.instances.iter_mut().find(|i| i.pid == *pid) {
                i.display_order = index as i32;
            }
        }
    }

    let views = persist_and_views(&state);
    emit_instances(&app, &views);
    Ok(views)
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
    let hotkey = hotkey.map(HotkeyView::into_core).filter(|hk| !hk.is_empty());

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
