use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use dofustabs_platform::{DiscoveredWindow, HotkeyId, PlatformError};

use crate::state::{sync_instances, PlatformHandles, SharedState};

const SCAN_INTERVAL: Duration = Duration::from_secs(1);

/// Spawns the discovery loop: an immediate first scan, then one per second.
/// Emits `instances-changed` only when the scan actually changed something.
/// The discovery backend lives in [`PlatformHandles`] so a manual refresh can
/// run the same scan on demand.
pub fn start(app: AppHandle) {
    std::thread::Builder::new()
        .name("game-watcher".to_string())
        .spawn(move || {
            let mut scan_failed = false;
            loop {
                match scan_now(&app) {
                    Ok(()) => scan_failed = false,
                    Err(err) => {
                        // Log the transition only, not every failing tick.
                        if !scan_failed {
                            log::warn!("game discovery failed: {err}");
                            scan_failed = true;
                        }
                    }
                }
                std::thread::sleep(SCAN_INTERVAL);
            }
        })
        .expect("failed to spawn watcher thread");
}

/// Runs one discovery scan and reconciles the tracked instances with it.
/// Without a discovery backend (degraded mode) this is a no-op.
pub fn scan_now(app: &AppHandle) -> Result<(), PlatformError> {
    let windows = {
        let handles = app.state::<PlatformHandles>();
        let guard = handles.discovery.lock().unwrap();
        match guard.as_ref() {
            Some(discovery) => discovery.scan()?,
            None => return Ok(()),
        }
    };
    apply_scan(app, &windows);
    Ok(())
}

fn apply_scan(app: &AppHandle, windows: &[DiscoveredWindow]) {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();

    let persisted = s.settings.instances.clone();
    let outcome = sync_instances(&mut s.instances, windows, &persisted);
    if !outcome.changed {
        return;
    }

    let active_died = s
        .active_pid
        .is_some_and(|pid| outcome.disappeared.contains(&pid));
    if active_died {
        s.active_pid = None;
    }

    // Collect hotkey (de)registrations while holding state, apply after.
    let to_register: Vec<(u32, dofustabs_core::settings::Hotkey)> = s
        .instances
        .iter()
        .filter(|i| outcome.appeared.contains(&i.pid) && i.enabled)
        .filter_map(|i| i.hotkey.clone().map(|hk| (i.pid, hk)))
        .collect();
    let views = s.to_views();
    drop(s);

    let handles = app.state::<PlatformHandles>();
    let mut hotkeys = handles.hotkeys.lock().unwrap();
    if let Some(backend) = hotkeys.as_mut() {
        for pid in &outcome.disappeared {
            if let Err(err) = backend.unregister(HotkeyId::Instance(*pid)) {
                log::warn!("failed to unregister hotkey for pid {pid}: {err}");
            }
        }
        for (pid, hotkey) in &to_register {
            if let Err(err) = backend.register(HotkeyId::Instance(*pid), hotkey) {
                log::warn!("failed to register hotkey for pid {pid}: {err}");
            }
        }
    }
    drop(hotkeys);

    if let Err(err) = app.emit("instances-changed", views) {
        log::warn!("failed to emit instances-changed: {err}");
    }
    if active_died {
        if let Err(err) = app.emit("active-changed", Option::<u32>::None) {
            log::warn!("failed to emit active-changed: {err}");
        }
    }
}
