//! IPC-facing DTOs (camelCase wire shapes) and the conversions from the
//! internal [`AppState`]/[`TrackedInstance`] model into them.

use dofustabs_core::gpu::GpuKind;
use dofustabs_core::settings::Hotkey;
use serde::{Deserialize, Serialize};

use crate::instances::TrackedInstance;
use crate::state::AppState;

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

/// Response shape for the [`crate::commands::layout_constants`] command:
/// layout facts the frontend needs but must not hardcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutConstantsView {
    pub sidebar_width: u32,
}

fn instance_view(state: &AppState, instance: &TrackedInstance) -> InstanceView {
    InstanceView {
        pid: instance.pid,
        character_name: instance.character_name.clone(),
        class_name: instance.class_name.map(str::to_string),
        icon_file: instance.icon_file.map(str::to_string),
        enabled: instance.enabled,
        display_order: instance.display_order,
        hotkey: instance.hotkey.as_ref().map(HotkeyView::from_core),
        is_primary: state.is_primary(instance),
        is_active: state.active_pid == Some(instance.pid),
        gpu: instance.gpu,
    }
}

impl AppState {
    pub fn to_views(&self) -> Vec<InstanceView> {
        let mut views: Vec<InstanceView> = self
            .instances
            .iter()
            .map(|i| instance_view(self, i))
            .collect();
        // Disabled accounts sink to the bottom; the sort is stable so each
        // group keeps its display order.
        views.sort_by_key(|v| (!v.enabled, v.display_order));
        views
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::sync_instances;
    use dofustabs_core::settings::AppSettings;
    use dofustabs_platform::{DiscoveredWindow, WindowRef};
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
        instances[0].enabled = false;
        instances[2].enabled = false;

        let state = state_with(instances);
        let pids: Vec<u32> = state.to_views().iter().map(|v| v.pid).collect();
        assert_eq!(pids, vec![20, 40, 10, 30]);
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
    fn layout_constants_view_is_camel_case() {
        let view = LayoutConstantsView { sidebar_width: 64 };
        let json = serde_json::to_value(view).unwrap();
        assert_eq!(json["sidebarWidth"], 64);
    }
}
