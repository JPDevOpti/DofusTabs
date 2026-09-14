use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use dofustabs_core::gpu::GpuKind;
use dofustabs_platform::{DiscoveredWindow, HotkeyId, PlatformError, WindowRef};

use crate::state::{sync_instances, AppState, PlatformHandles, SharedState};
use crate::window_layout;

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
                // The own window maps shortly after setup, and again after
                // being hidden to the tray (it gets a new address once
                // re-shown); keep trying to float and stretch it whenever
                // it is not currently known.
                if !window_layout::is_prepared(&app) {
                    window_layout::ensure_prepared(&app);
                }
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

/// Classifies the GPU of each pid through the platform inspector. Only
/// resolved kinds are returned; `Unknown` (client not rendering yet) and
/// inspection errors are retried on the next scan.
fn inspect_gpus(app: &AppHandle, pids: &[u32]) -> Vec<(u32, GpuKind)> {
    if pids.is_empty() {
        return Vec::new();
    }
    let handles = app.state::<PlatformHandles>();
    let guard = handles.gpu.lock().unwrap();
    let Some(inspector) = guard.as_ref() else {
        return Vec::new();
    };
    pids.iter()
        .filter_map(|&pid| match inspector.gpu_of(pid) {
            Ok(GpuKind::Unknown) => None,
            Ok(kind) => Some((pid, kind)),
            Err(err) => {
                log::debug!("gpu inspection failed for pid {pid}: {err}");
                None
            }
        })
        .collect()
}

/// Pure decision logic for what a changed scan should do: which pids need
/// their active-hotkey (de)registered, which need stacking, and whether the
/// active pid must be cleared. Kept free of any platform/state types so it
/// can be tested with plain inputs.
#[derive(Debug, Default, PartialEq, Eq)]
struct ScanEffects {
    /// True when the previously active pid died in this scan.
    active_pid_cleared: bool,
    /// Pids that appeared this scan and should have their hotkey registered.
    to_register: Vec<(u32, dofustabs_core::settings::Hotkey)>,
    /// Pids to stack, when window management is enabled.
    to_stack: Vec<u32>,
}

/// One instance's facts relevant to [`plan_scan_effects`].
struct InstanceFacts {
    pid: u32,
    enabled: bool,
    hotkey: Option<dofustabs_core::settings::Hotkey>,
}

fn plan_scan_effects(
    outcome: &crate::state::SyncOutcome,
    active_pid: Option<u32>,
    instances: &[InstanceFacts],
    manage_game_windows: bool,
) -> ScanEffects {
    let active_pid_cleared = active_pid.is_some_and(|pid| outcome.disappeared.contains(&pid));

    let to_register: Vec<(u32, dofustabs_core::settings::Hotkey)> = instances
        .iter()
        .filter(|i| outcome.appeared.contains(&i.pid) && i.enabled)
        .filter_map(|i| i.hotkey.clone().map(|hk| (i.pid, hk)))
        .collect();

    let to_stack: Vec<u32> = if manage_game_windows {
        instances
            .iter()
            .filter(|i| outcome.appeared.contains(&i.pid))
            .map(|i| i.pid)
            .collect()
    } else {
        Vec::new()
    };

    ScanEffects {
        active_pid_cleared,
        to_register,
        to_stack,
    }
}

fn apply_scan(app: &AppHandle, windows: &[DiscoveredWindow]) {
    let state = app.state::<SharedState>();
    let (outcome, unresolved) = {
        let mut s = state.lock();
        // `sync_instances` only reads `persisted` to seed brand-new
        // instances; borrowing it instead of cloning avoids a full
        // Vec<InstanceSettings> copy on every tick (this runs once a
        // second). The split-borrow is safe because `sync_instances`
        // takes `instances` and `persisted` as two disjoint parameters.
        let AppState {
            instances,
            settings,
            ..
        } = &mut *s;
        let outcome = sync_instances(instances, windows, &settings.instances);
        (outcome, s.pids_with_unknown_gpu())
    };

    // GPU classification reads /proc; keep it outside the state lock and
    // respect the lock order (state, then platform — never nested).
    let resolved = inspect_gpus(app, &unresolved);

    // The pids gathered above (`outcome`, `unresolved`) were matched under
    // the fresh lock just taken here; a pid that disappeared in between is
    // simply absent from `s.instances` now and every lookup below no-ops for
    // it, so this two-lock window is harmless (TOCTOU without a hazard).
    let mut s = state.lock();
    let mut changed = outcome.changed;
    for (pid, gpu) in resolved {
        if s.set_gpu(pid, gpu) {
            log::info!("pid {pid} renders on the {} GPU", gpu.wire_name());
            changed = true;
        }
    }
    if !changed {
        return;
    }

    let facts: Vec<InstanceFacts> = s
        .instances
        .iter()
        .map(|i| InstanceFacts {
            pid: i.pid,
            enabled: i.enabled,
            hotkey: i.hotkey.clone(),
        })
        .collect();
    let effects = plan_scan_effects(
        &outcome,
        s.active_pid,
        &facts,
        s.settings.manage_game_windows,
    );
    if effects.active_pid_cleared {
        s.active_pid = None;
    }

    let views = s.to_views();
    let window_ref_of: std::collections::HashMap<u32, WindowRef> = s
        .instances
        .iter()
        .map(|i| (i.pid, i.window_ref.clone()))
        .collect();
    let to_stack: Vec<WindowRef> = effects
        .to_stack
        .iter()
        .filter_map(|pid| window_ref_of.get(pid).cloned())
        .collect();
    drop(s);

    // New game windows are stacked once; later manual moves are respected.
    if !to_stack.is_empty() {
        window_layout::stack_windows(app, &to_stack, false);
    }

    let handles = app.state::<PlatformHandles>();
    let mut hotkeys = handles.hotkeys.lock().unwrap();
    if let Some(backend) = hotkeys.as_mut() {
        for pid in &outcome.disappeared {
            if let Err(err) = backend.unregister(HotkeyId::Instance(*pid)) {
                log::warn!("failed to unregister hotkey for pid {pid}: {err}");
            }
        }
        for (pid, hotkey) in &effects.to_register {
            if let Err(err) = backend.register(HotkeyId::Instance(*pid), hotkey) {
                log::warn!("failed to register hotkey for pid {pid}: {err}");
            }
        }
    }
    drop(hotkeys);

    if let Err(err) = app.emit("instances-changed", views) {
        log::warn!("failed to emit instances-changed: {err}");
    }
    if effects.active_pid_cleared {
        if let Err(err) = app.emit("active-changed", Option::<u32>::None) {
            log::warn!("failed to emit active-changed: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dofustabs_core::settings::Hotkey;

    fn facts(pid: u32, enabled: bool, hotkey: Option<Hotkey>) -> InstanceFacts {
        InstanceFacts {
            pid,
            enabled,
            hotkey,
        }
    }

    #[test]
    fn appeared_enabled_instances_with_a_hotkey_are_registered() {
        let outcome = crate::state::SyncOutcome {
            appeared: vec![10, 20],
            disappeared: vec![],
            changed: true,
        };
        let instances = vec![
            facts(10, true, Some(Hotkey::alt("1"))),
            facts(20, false, Some(Hotkey::alt("2"))), // disabled: not registered
            facts(30, true, Some(Hotkey::alt("3"))),  // not appeared: not registered
        ];

        let effects = plan_scan_effects(&outcome, None, &instances, false);

        assert_eq!(effects.to_register, vec![(10, Hotkey::alt("1"))]);
        assert!(!effects.active_pid_cleared);
        assert!(effects.to_stack.is_empty());
    }

    #[test]
    fn active_pid_is_cleared_only_when_it_disappeared() {
        let outcome = crate::state::SyncOutcome {
            appeared: vec![],
            disappeared: vec![10],
            changed: true,
        };
        assert!(plan_scan_effects(&outcome, Some(10), &[], false).active_pid_cleared);
        assert!(!plan_scan_effects(&outcome, Some(20), &[], false).active_pid_cleared);
        assert!(!plan_scan_effects(&outcome, None, &[], false).active_pid_cleared);
    }

    #[test]
    fn appeared_pids_are_stacked_only_when_management_is_enabled() {
        let outcome = crate::state::SyncOutcome {
            appeared: vec![10, 20],
            disappeared: vec![],
            changed: true,
        };
        let instances = vec![facts(10, true, None), facts(20, false, None)];

        let disabled = plan_scan_effects(&outcome, None, &instances, false);
        assert!(disabled.to_stack.is_empty());

        let enabled = plan_scan_effects(&outcome, None, &instances, true);
        assert_eq!(enabled.to_stack, vec![10, 20]);
    }
}
