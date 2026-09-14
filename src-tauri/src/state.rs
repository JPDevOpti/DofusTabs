use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use dofustabs_core::geometry::Rect;
use dofustabs_core::gpu::GpuKind;
use dofustabs_core::rotation::ActivationQueue;
use dofustabs_core::settings::{AppSettings, Hotkey, InstanceSettings};
use dofustabs_core::window_title::parse_title;
use dofustabs_platform::{
    DiscoveredWindow, GameDiscovery, GameLauncher, GpuInspector, HotkeyBackend, WindowActivator,
    WindowManager, WindowRef,
};
use serde::{Deserialize, Serialize};

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

/// Runtime facts about window placement, resolved once the compositor has
/// mapped the app's own window (see `window_layout`).
#[derive(Debug, Default)]
pub struct LayoutState {
    /// The app's own window, once prepared; `None` until preparation works.
    pub own_window: Option<WindowRef>,
    /// Workspace the app was on when first prepared (startup default).
    pub startup_workspace: Option<String>,
    /// Usable area of the app's monitor when first prepared.
    pub monitor_area: Option<Rect>,
    /// Game windows already stacked once; the user's later manual moves are
    /// left alone until an explicit re-arrange.
    pub arranged: HashSet<WindowRef>,
}

pub struct AppState {
    pub instances: Vec<TrackedInstance>,
    pub active_pid: Option<u32>,
    pub settings: AppSettings,
    pub queue: ActivationQueue,
    pub settings_path: PathBuf,
    /// User's home directory, the base for the default launcher path.
    pub home_dir: PathBuf,
    pub layout: LayoutState,
}

pub struct SharedState(pub Mutex<AppState>);

/// Platform handles live outside the state mutex; lock order is always
/// state first, then platform, never the reverse.
#[derive(Default)]
pub struct PlatformHandles {
    pub discovery: Mutex<Option<Box<dyn GameDiscovery + Send>>>,
    pub activator: Mutex<Option<Box<dyn WindowActivator + Send>>>,
    pub hotkeys: Mutex<Option<Box<dyn HotkeyBackend + Send>>>,
    pub window_manager: Mutex<Option<Box<dyn WindowManager + Send>>>,
    pub gpu: Mutex<Option<Box<dyn GpuInspector + Send>>>,
    pub launcher: Mutex<Option<Box<dyn GameLauncher + Send>>>,
}

impl AppState {
    pub fn new(settings: AppSettings, settings_path: PathBuf, home_dir: PathBuf) -> Self {
        Self {
            instances: Vec::new(),
            active_pid: None,
            settings,
            queue: ActivationQueue::new(),
            settings_path,
            home_dir,
            layout: LayoutState::default(),
        }
    }

    /// Launcher the "launch game" action starts: the configured path, or
    /// the default AppImage location under the user's home.
    pub fn launcher_path(&self) -> PathBuf {
        self.settings.launcher_path_or_default(&self.home_dir)
    }

    /// Stores the launcher path; blank input clears the override so the
    /// default applies again.
    pub fn set_launcher_path(&mut self, path: Option<String>) {
        self.settings.launcher_path = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    }

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

    /// Workspace and area the game windows are stacked into: the settings
    /// override when set, otherwise what was resolved at startup. `None`
    /// until the own window has been prepared.
    pub fn game_target(&self) -> Option<(String, Rect)> {
        let workspace = self
            .settings
            .game_workspace
            .clone()
            .or_else(|| self.layout.startup_workspace.clone())?;
        let area = self.settings.game_area.or(self.layout.monitor_area)?;
        Some((workspace, area))
    }

    pub fn enabled_ordered_pids(&self) -> Vec<u32> {
        let mut enabled: Vec<&TrackedInstance> =
            self.instances.iter().filter(|i| i.enabled).collect();
        enabled.sort_by_key(|i| i.display_order);
        enabled.iter().map(|i| i.pid).collect()
    }

    pub fn to_views(&self) -> Vec<InstanceView> {
        let mut views: Vec<InstanceView> = self
            .instances
            .iter()
            .map(|i| InstanceView {
                pid: i.pid,
                character_name: i.character_name.clone(),
                class_name: i.class_name.map(str::to_string),
                icon_file: i.icon_file.map(str::to_string),
                enabled: i.enabled,
                display_order: i.display_order,
                hotkey: i.hotkey.as_ref().map(HotkeyView::from_core),
                is_primary: self.is_primary(i),
                is_active: self.active_pid == Some(i.pid),
                gpu: i.gpu,
            })
            .collect();
        // Disabled accounts sink to the bottom; the sort is stable so each
        // group keeps its display order.
        views.sort_by_key(|v| (!v.enabled, v.display_order));
        views
    }

    /// Pids in the order the sidebar shows them (enabled first, then by
    /// display order within each group).
    pub fn visual_order(&self) -> Vec<u32> {
        let mut ordered: Vec<&TrackedInstance> = self.instances.iter().collect();
        ordered.sort_by_key(|i| (!i.enabled, i.display_order));
        ordered.iter().map(|i| i.pid).collect()
    }

    /// Listed pids take positions 0..n in the given order; unlisted ones keep
    /// their relative order after them.
    pub fn apply_order(&mut self, pids: &[u32]) {
        let known: Vec<u32> = self.instances.iter().map(|i| i.pid).collect();
        let mut next = pids.len() as i32;
        for pid in &known {
            if !pids.contains(pid) {
                if let Some(i) = self.instances.iter_mut().find(|i| i.pid == *pid) {
                    i.display_order = next;
                    next += 1;
                }
            }
        }
        for (index, pid) in pids.iter().enumerate() {
            if let Some(i) = self.instances.iter_mut().find(|i| i.pid == *pid) {
                i.display_order = index as i32;
            }
        }
    }

    /// Moves `pid` one step up (`-1`) or down (`+1`) among the accounts in
    /// its own enabled/disabled group. Returns false when it cannot move
    /// (unknown pid or already at the edge of its group).
    pub fn move_instance(&mut self, pid: u32, delta: i32) -> bool {
        let Some(enabled) = self
            .instances
            .iter()
            .find(|i| i.pid == pid)
            .map(|i| i.enabled)
        else {
            return false;
        };
        let mut order = self.visual_order();
        let group: Vec<usize> = order
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                self.instances
                    .iter()
                    .any(|i| i.pid == **p && i.enabled == enabled)
            })
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

    pub fn settings_view(&self) -> SettingsView {
        SettingsView {
            next_hotkey: HotkeyView::from_core(&self.settings.next_hotkey),
            previous_hotkey: HotkeyView::from_core(&self.settings.previous_hotkey),
            primary_hotkey: HotkeyView::from_core(&self.settings.primary_hotkey),
            show_names: self.settings.show_sidebar_names,
            manage_game_windows: self.settings.manage_game_windows,
            launcher_path: self.launcher_path().to_string_lossy().into_owned(),
        }
    }

    fn is_primary(&self, instance: &TrackedInstance) -> bool {
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
    pub fn snapshot_settings(&mut self) -> &AppSettings {
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

// ── Wire types (camelCase per the fixed IPC contract) ────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyView {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub key: String,
}

impl HotkeyView {
    pub fn from_core(hotkey: &Hotkey) -> Self {
        Self {
            ctrl: hotkey.ctrl,
            alt: hotkey.alt,
            shift: hotkey.shift,
            super_key: hotkey.super_key,
            key: hotkey.key.clone(),
        }
    }

    pub fn into_core(self) -> Hotkey {
        Hotkey {
            ctrl: self.ctrl,
            alt: self.alt,
            shift: self.shift,
            super_key: self.super_key,
            key: self.key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceView {
    pub pid: u32,
    pub character_name: String,
    pub class_name: Option<String>,
    pub icon_file: Option<String>,
    pub enabled: bool,
    pub display_order: i32,
    pub hotkey: Option<HotkeyView>,
    pub is_primary: bool,
    pub is_active: bool,
    pub gpu: GpuKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub next_hotkey: HotkeyView,
    pub previous_hotkey: HotkeyView,
    pub primary_hotkey: HotkeyView,
    pub show_names: bool,
    pub manage_game_windows: bool,
    /// Effective launcher path (configured or default).
    pub launcher_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn views_are_sorted_by_display_order_and_flag_active_and_primary() {
        let mut instances = Vec::new();
        sync_instances(
            &mut instances,
            &[
                discovered(10, "Ana - Feca - Dofus"),
                discovered(20, "Beto - Sram - Dofus"),
            ],
            &[],
        );
        instances[0].display_order = 1;
        instances[1].display_order = 0;

        let mut state = state_with(instances);
        state.active_pid = Some(10);
        state.settings.primary_character_name = "Beto".to_string();

        let views = state.to_views();
        assert_eq!(views[0].pid, 20);
        assert!(views[0].is_primary);
        assert!(!views[0].is_active);
        assert_eq!(views[1].pid, 10);
        assert!(views[1].is_active);
        assert_eq!(views[1].icon_file.as_deref(), Some("Feca.png"));
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
        let pids: Vec<u32> = state.to_views().iter().map(|v| v.pid).collect();
        assert_eq!(pids, vec![20, 40, 10, 30]);
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
    fn wire_shapes_are_camel_case() {
        let view = InstanceView {
            pid: 1,
            character_name: "Ana".to_string(),
            class_name: None,
            icon_file: None,
            enabled: true,
            display_order: 0,
            hotkey: Some(HotkeyView::from_core(&Hotkey::new(
                false, false, false, true, "F1",
            ))),
            is_primary: false,
            is_active: true,
            gpu: GpuKind::Dedicated,
        };

        let json = serde_json::to_value(&view).unwrap();
        assert!(json.get("characterName").is_some());
        assert!(json.get("isActive").is_some());
        assert!(json["hotkey"].get("superKey").is_some());
        assert_eq!(json["gpu"], "dedicated");
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
    fn launcher_path_falls_back_to_default_and_blank_clears_override() {
        let mut state = state_with(Vec::new());
        assert_eq!(
            state.launcher_path(),
            PathBuf::from("/home/ana/Applications/Dofus.AppImage")
        );
        assert_eq!(
            state.settings_view().launcher_path,
            "/home/ana/Applications/Dofus.AppImage"
        );

        state.set_launcher_path(Some("  /opt/Dofus.AppImage ".to_string()));
        assert_eq!(
            state.settings.launcher_path.as_deref(),
            Some("/opt/Dofus.AppImage")
        );
        assert_eq!(state.launcher_path(), PathBuf::from("/opt/Dofus.AppImage"));

        state.set_launcher_path(Some("   ".to_string()));
        assert_eq!(state.settings.launcher_path, None);
        state.set_launcher_path(None);
        assert_eq!(
            state.launcher_path(),
            PathBuf::from("/home/ana/Applications/Dofus.AppImage")
        );
    }

    #[test]
    fn game_target_prefers_settings_over_startup_values() {
        let mut state = state_with(Vec::new());
        assert_eq!(state.game_target(), None);

        state.layout.startup_workspace = Some("2".to_string());
        state.layout.monitor_area = Some(Rect::new(0, 0, 1920, 1080));
        assert_eq!(
            state.game_target(),
            Some(("2".to_string(), Rect::new(0, 0, 1920, 1080)))
        );

        state.settings.game_workspace = Some("5".to_string());
        state.settings.game_area = Some(Rect::new(10, 10, 100, 100));
        assert_eq!(
            state.game_target(),
            Some(("5".to_string(), Rect::new(10, 10, 100, 100)))
        );
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
