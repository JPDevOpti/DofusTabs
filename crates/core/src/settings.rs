use crate::geometry::Rect;
use serde::{Deserialize, Serialize};

/// A key combination, serialized in a platform-neutral shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub key: String,
}

impl Hotkey {
    pub fn new(
        ctrl: bool,
        alt: bool,
        shift: bool,
        super_key: bool,
        key: impl Into<String>,
    ) -> Self {
        Self {
            ctrl,
            alt,
            shift,
            super_key,
            key: key.into(),
        }
    }

    pub fn alt(key: impl Into<String>) -> Self {
        Self {
            alt: true,
            key: key.into(),
            ..Self::default()
        }
    }

    pub fn alt_shift(key: impl Into<String>) -> Self {
        Self {
            alt: true,
            shift: true,
            key: key.into(),
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.key.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceSettings {
    pub process_id: u32,
    pub title: String,
    pub enabled: bool,
    pub display_order: i32,
    pub individual_hotkey: Option<Hotkey>,
}

impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            process_id: 0,
            title: String::new(),
            enabled: true,
            display_order: 0,
            individual_hotkey: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub next_hotkey: Hotkey,
    pub previous_hotkey: Hotkey,
    pub primary_hotkey: Hotkey,
    pub primary_character_name: String,
    pub show_sidebar_names: bool,
    /// Float new game windows and stack them on the app's workspace.
    pub manage_game_windows: bool,
    /// Workspace the game windows are moved to; `None` means the workspace
    /// DofusTabs was on at startup.
    pub game_workspace: Option<String>,
    /// Area the game windows are stacked into; `None` means the full usable
    /// area of the app's monitor.
    pub game_area: Option<Rect>,
    /// Path of the Ankama Launcher AppImage used by "launch game"; `None`
    /// means [`default_launcher_path`] under the user's home.
    pub launcher_path: Option<String>,
    pub instances: Vec<InstanceSettings>,
}

/// Where the launcher AppImage lives unless the user says otherwise.
pub const DEFAULT_LAUNCHER_RELATIVE_PATH: &str = "Applications/Dofus.AppImage";

/// Default launcher location: `<home>/Applications/Dofus.AppImage`.
pub fn default_launcher_path(home: &std::path::Path) -> std::path::PathBuf {
    home.join(DEFAULT_LAUNCHER_RELATIVE_PATH)
}

impl AppSettings {
    /// Effective launcher path: the configured one when non-blank, otherwise
    /// the default under `home`.
    pub fn launcher_path_or_default(&self, home: &std::path::Path) -> std::path::PathBuf {
        match self.launcher_path.as_deref().map(str::trim) {
            Some(path) if !path.is_empty() => std::path::PathBuf::from(path),
            _ => default_launcher_path(home),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            next_hotkey: Hotkey::alt("Tab"),
            previous_hotkey: Hotkey::alt_shift("Tab"),
            primary_hotkey: Hotkey::alt("Home"),
            primary_character_name: String::new(),
            show_sidebar_names: false,
            manage_game_windows: true,
            game_workspace: None,
            game_area: None,
            launcher_path: None,
            instances: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_json_deserializes_to_defaults() {
        let settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, AppSettings::default());
        assert_eq!(settings.next_hotkey, Hotkey::alt("Tab"));
        assert_eq!(settings.previous_hotkey, Hotkey::alt_shift("Tab"));
        assert_eq!(settings.primary_hotkey, Hotkey::alt("Home"));
        assert!(!settings.show_sidebar_names);
        assert!(settings.instances.is_empty());
    }

    #[test]
    fn old_settings_json_without_window_fields_gets_defaults() {
        let json = r#"{
            "next_hotkey": {"alt": true, "key": "Tab"},
            "show_sidebar_names": true,
            "instances": [{"process_id": 3, "title": "Ana - Feca - Dofus"}]
        }"#;
        let settings: AppSettings = serde_json::from_str(json).unwrap();
        assert!(settings.manage_game_windows);
        assert_eq!(settings.game_workspace, None);
        assert_eq!(settings.game_area, None);
        assert!(settings.show_sidebar_names);
        assert_eq!(settings.instances.len(), 1);
    }

    #[test]
    fn window_management_fields_roundtrip() {
        let settings = AppSettings {
            manage_game_windows: false,
            game_workspace: Some("3".to_string()),
            game_area: Some(Rect::new(10, 20, 1500, 900)),
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<AppSettings>(&json).unwrap(),
            settings
        );
    }

    #[test]
    fn old_settings_json_without_launcher_path_defaults_to_home_appimage() {
        let json = r#"{"next_hotkey": {"alt": true, "key": "Tab"}, "instances": []}"#;
        let settings: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.launcher_path, None);
        let home = std::path::Path::new("/home/ana");
        assert_eq!(
            settings.launcher_path_or_default(home),
            std::path::PathBuf::from("/home/ana/Applications/Dofus.AppImage")
        );
    }

    #[test]
    fn configured_launcher_path_wins_unless_blank() {
        let home = std::path::Path::new("/home/ana");
        let mut settings = AppSettings {
            launcher_path: Some("/opt/Dofus.AppImage".to_string()),
            ..AppSettings::default()
        };
        assert_eq!(
            settings.launcher_path_or_default(home),
            std::path::PathBuf::from("/opt/Dofus.AppImage")
        );

        settings.launcher_path = Some("   ".to_string());
        assert_eq!(
            settings.launcher_path_or_default(home),
            default_launcher_path(home)
        );

        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<AppSettings>(&json).unwrap(),
            settings
        );
    }

    #[test]
    fn instance_enabled_defaults_to_true() {
        let instance: InstanceSettings = serde_json::from_str("{\"process_id\": 7}").unwrap();
        assert!(instance.enabled);
        assert_eq!(instance.process_id, 7);
    }

    #[test]
    fn settings_roundtrip_through_json() {
        let mut settings = AppSettings {
            primary_character_name: "Juan-Perez".to_string(),
            ..AppSettings::default()
        };
        settings.instances.push(InstanceSettings {
            process_id: 1234,
            title: "Juan-Perez - Sacrógrito - Dofus".to_string(),
            enabled: false,
            display_order: 2,
            individual_hotkey: Some(Hotkey::new(true, false, false, false, "1")),
        });

        let json = serde_json::to_string(&settings).unwrap();
        let restored: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, settings);
    }

    #[test]
    fn hotkey_is_empty_only_without_key() {
        assert!(Hotkey::default().is_empty());
        assert!(!Hotkey::alt("Tab").is_empty());
    }
}
