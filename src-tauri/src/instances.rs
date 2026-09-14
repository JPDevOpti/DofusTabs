//! Tracked game instances: their identity, per-instance options, and the
//! reconciliation/ordering logic that keeps them in sync with the live
//! discovery scan and the sidebar's display order.

use std::collections::HashMap;

use dofustabs_core::gpu::GpuKind;
use dofustabs_core::settings::{Hotkey, InstanceSettings};
use dofustabs_core::window_title::parse_title;
use dofustabs_platform::{DiscoveredWindow, WindowRef};

use crate::state::AppState;

/// One tracked game client with its parsed identity and per-instance options.
pub struct TrackedInstance {
    pub pid: u32,
    pub title: String,
    pub character_name: String,
    pub class_name: Option<&'static str>,
    pub icon_file: Option<&'static str>,
    pub window_ref: WindowRef,
    pub enabled: bool,
    pub display_order: i32,
    pub hotkey: Option<Hotkey>,
    /// GPU the client renders on; `Unknown` until the watcher resolves it.
    pub gpu: GpuKind,
}

impl TrackedInstance {
    pub fn from_discovered(win: &DiscoveredWindow) -> Self {
        let parsed = parse_title(&win.title);
        Self {
            pid: win.pid,
            title: win.title.clone(),
            character_name: parsed.character_name,
            class_name: parsed.class.map(|c| c.canonical_name),
            icon_file: parsed.class.map(|c| c.icon_file),
            window_ref: win.window_ref.clone(),
            enabled: true,
            display_order: 0,
            hotkey: None,
            gpu: GpuKind::Unknown,
        }
    }

    /// Stable identity used for the "primary character" setting: the parsed
    /// character name when available, otherwise the raw window title.
    pub fn primary_identity(&self) -> &str {
        if self.character_name.trim().is_empty() {
            &self.title
        } else {
            &self.character_name
        }
    }
}

/// Result of reconciling the tracked list against a fresh discovery scan.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncOutcome {
    pub appeared: Vec<u32>,
    pub disappeared: Vec<u32>,
    pub changed: bool,
}

/// Reconciles `instances` with `discovered`, preserving enabled/order/hotkey
/// for surviving pids and seeding new instances from `persisted` (matched by
/// exact title first, then by parsed character name). New unmatched instances
/// go to the end of the display order.
pub fn sync_instances(
    instances: &mut Vec<TrackedInstance>,
    discovered: &[DiscoveredWindow],
    persisted: &[InstanceSettings],
) -> SyncOutcome {
    let mut outcome = SyncOutcome::default();

    let alive: Vec<u32> = discovered.iter().map(|d| d.pid).collect();
    let before = instances.len();
    instances.retain(|i| {
        let keep = alive.contains(&i.pid);
        if !keep {
            outcome.disappeared.push(i.pid);
        }
        keep
    });
    outcome.changed |= instances.len() != before;

    for win in discovered {
        match instances.iter_mut().find(|i| i.pid == win.pid) {
            Some(existing) => {
                if existing.title != win.title {
                    let parsed = parse_title(&win.title);
                    existing.title = win.title.clone();
                    existing.character_name = parsed.character_name;
                    existing.class_name = parsed.class.map(|c| c.canonical_name);
                    existing.icon_file = parsed.class.map(|c| c.icon_file);
                    outcome.changed = true;
                }
            }
            None => {
                let mut instance = TrackedInstance::from_discovered(win);
                let matched = persisted
                    .iter()
                    .find(|p| !p.title.trim().is_empty() && p.title == instance.title)
                    .or_else(|| {
                        let name = instance.character_name.trim();
                        if name.is_empty() {
                            return None;
                        }
                        persisted
                            .iter()
                            .find(|p| parse_title(&p.title).character_name == name)
                    });

                match matched {
                    Some(p) => {
                        instance.enabled = p.enabled;
                        instance.display_order = p.display_order;
                        instance.hotkey = p.individual_hotkey.clone();
                    }
                    None => {
                        instance.display_order = instances
                            .iter()
                            .map(|i| i.display_order)
                            .max()
                            .map_or(0, |m| m + 1);
                    }
                }

                outcome.appeared.push(instance.pid);
                outcome.changed = true;
                instances.push(instance);
            }
        }
    }

    outcome
}

impl AppState {
    /// Pids whose GPU is still unresolved, for the watcher to inspect.
    pub fn pids_with_unknown_gpu(&self) -> Vec<u32> {
        self.instances
            .iter()
            .filter(|i| i.gpu == GpuKind::Unknown)
            .map(|i| i.pid)
            .collect()
    }

    /// Records the GPU classification for `pid`; true when it changed.
    pub fn set_gpu(&mut self, pid: u32, gpu: GpuKind) -> bool {
        match self.instances.iter_mut().find(|i| i.pid == pid) {
            Some(instance) if instance.gpu != gpu => {
                instance.gpu = gpu;
                true
            }
            _ => false,
        }
    }

    pub fn enabled_ordered_pids(&self) -> Vec<u32> {
        let mut enabled: Vec<&TrackedInstance> =
            self.instances.iter().filter(|i| i.enabled).collect();
        enabled.sort_by_key(|i| i.display_order);
        enabled.iter().map(|i| i.pid).collect()
    }

    /// Pids in the order the sidebar shows them (enabled first, then by
    /// display order within each group).
    pub fn visual_order(&self) -> Vec<u32> {
        let mut ordered: Vec<&TrackedInstance> = self.instances.iter().collect();
        ordered.sort_by_key(|i| (!i.enabled, i.display_order));
        ordered.iter().map(|i| i.pid).collect()
    }

    /// Listed pids take positions 0..n in the given order; unlisted ones keep
    /// their relative order after them. O(n) via a single pid→index map
    /// instead of a linear find per pid.
    pub fn apply_order(&mut self, pids: &[u32]) {
        let mut index_by_pid: HashMap<u32, usize> = HashMap::with_capacity(self.instances.len());
        for (index, instance) in self.instances.iter().enumerate() {
            index_by_pid.insert(instance.pid, index);
        }

        let mut next = pids.len() as i32;
        let known: Vec<u32> = self.instances.iter().map(|i| i.pid).collect();
        for pid in &known {
            if !pids.contains(pid) {
                if let Some(&index) = index_by_pid.get(pid) {
                    self.instances[index].display_order = next;
                    next += 1;
                }
            }
        }
        for (order, pid) in pids.iter().enumerate() {
            if let Some(&index) = index_by_pid.get(pid) {
                self.instances[index].display_order = order as i32;
            }
        }
    }

    /// Moves `pid` one step up (`-1`) or down (`+1`) among the accounts in
    /// its own enabled/disabled group. Returns false when it cannot move
    /// (unknown pid or already at the edge of its group).
    pub fn move_instance(&mut self, pid: u32, delta: i32) -> bool {
        let enabled_by_pid: HashMap<u32, bool> =
            self.instances.iter().map(|i| (i.pid, i.enabled)).collect();
        let Some(&enabled) = enabled_by_pid.get(&pid) else {
            return false;
        };
        let mut order = self.visual_order();
        let group: Vec<usize> = order
            .iter()
            .enumerate()
            .filter(|(_, p)| enabled_by_pid.get(p) == Some(&enabled))
            .map(|(index, _)| index)
            .collect();
        let Some(slot) = group.iter().position(|&index| order[index] == pid) else {
            return false;
        };
        let target = slot as i32 + delta;
        if target < 0 || target >= group.len() as i32 {
            return false;
        }
        order.swap(group[slot], group[target as usize]);
        self.apply_order(&order);
        true
    }

    pub(crate) fn is_primary(&self, instance: &TrackedInstance) -> bool {
        let primary = self.settings.primary_character_name.trim();
        !primary.is_empty() && primary == instance.primary_identity()
    }

    pub fn primary_pid(&self) -> Option<u32> {
        self.instances
            .iter()
            .find(|i| i.enabled && self.is_primary(i))
            .map(|i| i.pid)
    }

    /// Snapshots the live instance list into the persisted settings shape.
    pub fn snapshot_settings(&mut self) -> &dofustabs_core::settings::AppSettings {
        self.settings.instances = self
            .instances
            .iter()
            .map(|i| InstanceSettings {
                process_id: i.pid,
                title: i.title.clone(),
                enabled: i.enabled,
                display_order: i.display_order,
                individual_hotkey: i.hotkey.clone(),
            })
            .collect();
        &self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use dofustabs_core::settings::AppSettings;
    use std::path::PathBuf;

    fn discovered(pid: u32, title: &str) -> DiscoveredWindow {
        DiscoveredWindow {
            pid,
            title: title.to_string(),
            process_name: "Dofus.x64".to_string(),
            window_ref: WindowRef::WaylandAddress(format!("0x{pid:x}")),
        }
    }

    fn state_with(instances: Vec<TrackedInstance>) -> AppState {
        let mut state = AppState::new(
            AppSettings::default(),
            PathBuf::from("/tmp/unused.json"),
            PathBuf::from("/home/ana"),
        );
        state.instances = instances;
        state
    }

    #[test]
    fn sync_adopts_new_instances_at_end_of_order() {
        let mut instances = Vec::new();
        let outcome = sync_instances(
            &mut instances,
            &[
                discovered(10, "Ana - Feca - Dofus"),
                discovered(20, "Beto - Sram - Dofus"),
            ],
            &[],
        );

        assert!(outcome.changed);
        assert_eq!(outcome.appeared, vec![10, 20]);
        assert_eq!(instances[0].display_order, 0);
        assert_eq!(instances[1].display_order, 1);
        assert_eq!(instances[0].character_name, "Ana");
        assert_eq!(instances[0].class_name, Some("Feca"));
    }

    #[test]
    fn sync_preserves_survivor_options_and_drops_dead_pids() {
        let mut instances = Vec::new();
        sync_instances(
            &mut instances,
            &[
                discovered(10, "Ana - Feca - Dofus"),
                discovered(20, "Beto - Sram - Dofus"),
            ],
            &[],
        );
        instances[0].enabled = false;
        instances[0].display_order = 7;
        instances[0].hotkey = Some(Hotkey::alt("1"));

        let outcome = sync_instances(&mut instances, &[discovered(10, "Ana - Feca - Dofus")], &[]);

        assert!(outcome.changed);
        assert_eq!(outcome.disappeared, vec![20]);
        assert_eq!(instances.len(), 1);
        assert!(!instances[0].enabled);
        assert_eq!(instances[0].display_order, 7);
        assert_eq!(instances[0].hotkey, Some(Hotkey::alt("1")));
    }

    #[test]
    fn sync_matches_persisted_settings_by_exact_title() {
        let persisted = vec![InstanceSettings {
            process_id: 999, // stale pid: matching is by title, never by pid
            title: "Ana - Feca - Dofus".to_string(),
            enabled: false,
            display_order: 3,
            individual_hotkey: Some(Hotkey::alt("2")),
        }];

        let mut instances = Vec::new();
        sync_instances(
            &mut instances,
            &[discovered(10, "Ana - Feca - Dofus")],
            &persisted,
        );

        assert!(!instances[0].enabled);
        assert_eq!(instances[0].display_order, 3);
        assert_eq!(instances[0].hotkey, Some(Hotkey::alt("2")));
    }

    #[test]
    fn sync_falls_back_to_character_name_when_title_changed() {
        let persisted = vec![InstanceSettings {
            process_id: 999,
            title: "Ana - Feca - Dofus 2.71".to_string(),
            enabled: false,
            display_order: 5,
            individual_hotkey: None,
        }];

        let mut instances = Vec::new();
        sync_instances(
            &mut instances,
            &[discovered(10, "Ana - Feca - Dofus 3.0")],
            &persisted,
        );

        assert!(!instances[0].enabled);
        assert_eq!(instances[0].display_order, 5);
    }

    #[test]
    fn sync_reparses_identity_when_title_changes_in_place() {
        let mut instances = Vec::new();
        sync_instances(&mut instances, &[discovered(10, "Connecting - Dofus")], &[]);

        let outcome = sync_instances(&mut instances, &[discovered(10, "Ana - Feca - Dofus")], &[]);

        assert!(outcome.changed);
        assert!(outcome.appeared.is_empty());
        assert_eq!(instances[0].character_name, "Ana");
        assert_eq!(instances[0].class_name, Some("Feca"));
    }

    #[test]
    fn unchanged_scan_reports_no_change() {
        let mut instances = Vec::new();
        sync_instances(&mut instances, &[discovered(10, "Ana - Feca - Dofus")], &[]);

        let outcome = sync_instances(&mut instances, &[discovered(10, "Ana - Feca - Dofus")], &[]);

        assert_eq!(outcome, SyncOutcome::default());
    }

    #[test]
    fn disabled_accounts_sort_last_keeping_their_relative_order() {
        let mut instances = Vec::new();
        sync_instances(
            &mut instances,
            &[
                discovered(10, "Ana - Feca - Dofus"),
                discovered(20, "Beto - Sram - Dofus"),
                discovered(30, "Caro - Xelor - Dofus"),
                discovered(40, "Dani - Osamodas - Dofus"),
            ],
            &[],
        );
        // display order: 10=0, 20=1, 30=2, 40=3; disable 10 and 30
        instances[0].enabled = false;
        instances[2].enabled = false;

        let state = state_with(instances);
        assert_eq!(state.visual_order(), vec![20, 40, 10, 30]);
    }

    #[test]
    fn apply_order_places_listed_pids_first_and_keeps_the_rest_after() {
        let mut instances = Vec::new();
        sync_instances(
            &mut instances,
            &[
                discovered(10, "Ana - Feca - Dofus"),
                discovered(20, "Beto - Sram - Dofus"),
                discovered(30, "Caro - Xelor - Dofus"),
            ],
            &[],
        );
        let mut state = state_with(instances);

        state.apply_order(&[30, 10]);

        let order = |pid: u32| {
            state
                .instances
                .iter()
                .find(|i| i.pid == pid)
                .unwrap()
                .display_order
        };
        assert_eq!(order(30), 0);
        assert_eq!(order(10), 1);
        assert_eq!(order(20), 2);
    }

    #[test]
    fn move_instance_swaps_with_neighbour_in_same_group() {
        let mut instances = Vec::new();
        sync_instances(
            &mut instances,
            &[
                discovered(10, "Ana - Feca - Dofus"),
                discovered(20, "Beto - Sram - Dofus"),
                discovered(30, "Caro - Xelor - Dofus"),
                discovered(40, "Dani - Osamodas - Dofus"),
            ],
            &[],
        );
        instances[1].enabled = false; // 20 disabled
        let mut state = state_with(instances);
        assert_eq!(state.visual_order(), vec![10, 30, 40, 20]);

        assert!(state.move_instance(40, -1));
        assert_eq!(state.visual_order(), vec![10, 40, 30, 20]);

        assert!(state.move_instance(10, 1));
        assert_eq!(state.visual_order(), vec![40, 10, 30, 20]);

        // Edges of the enabled group and of the disabled group do not move.
        assert!(!state.move_instance(40, -1));
        assert!(!state.move_instance(30, 1));
        assert!(!state.move_instance(20, -1));
        assert!(!state.move_instance(20, 1));
        assert!(!state.move_instance(999, 1));
        assert_eq!(state.visual_order(), vec![40, 10, 30, 20]);
    }

    #[test]
    fn gpu_starts_unknown_and_only_reports_real_changes() {
        let mut instances = Vec::new();
        sync_instances(
            &mut instances,
            &[
                discovered(10, "Ana - Feca - Dofus"),
                discovered(20, "Beto - Sram - Dofus"),
            ],
            &[],
        );
        let mut state = state_with(instances);
        assert_eq!(state.pids_with_unknown_gpu(), vec![10, 20]);

        assert!(state.set_gpu(10, GpuKind::Dedicated));
        assert!(!state.set_gpu(10, GpuKind::Dedicated));
        assert!(!state.set_gpu(999, GpuKind::Integrated));
        assert_eq!(state.pids_with_unknown_gpu(), vec![20]);
        assert_eq!(state.to_views()[0].gpu, GpuKind::Dedicated);

        // A rescan keeps the resolved GPU for surviving pids.
        let persisted = state.settings.instances.clone();
        sync_instances(
            &mut state.instances,
            &[discovered(10, "Ana - Feca - Dofus")],
            &persisted,
        );
        assert_eq!(state.instances[0].gpu, GpuKind::Dedicated);
    }

    #[test]
    fn enabled_pids_follow_display_order() {
        let mut instances = Vec::new();
        sync_instances(
            &mut instances,
            &[
                discovered(10, "Ana - Feca - Dofus"),
                discovered(20, "Beto - Sram - Dofus"),
                discovered(30, "Caro - Xelor - Dofus"),
            ],
            &[],
        );
        instances[0].display_order = 2;
        instances[2].display_order = 0;
        instances[1].enabled = false;

        let state = state_with(instances);
        assert_eq!(state.enabled_ordered_pids(), vec![30, 10]);
    }
}
