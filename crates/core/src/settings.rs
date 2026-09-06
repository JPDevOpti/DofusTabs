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
    pub fn new(ctrl: bool, alt: bool, shift: bool, super_key: bool, key: impl Into<String>) -> Self {
        Self { ctrl, alt, shift, super_key, key: key.into() }
    }

    pub fn alt(key: impl Into<String>) -> Self {
        Self { alt: true, key: key.into(), ..Self::default() }
    }

    pub fn alt_shift(key: impl Into<String>) -> Self {
        Self { alt: true, shift: true, key: key.into(), ..Self::default() }
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
    pub instances: Vec<InstanceSettings>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            next_hotkey: Hotkey::alt("Tab"),
            previous_hotkey: Hotkey::alt_shift("Tab"),
            primary_hotkey: Hotkey::alt("Home"),
            primary_character_name: String::new(),
            show_sidebar_names: false,
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
    fn instance_enabled_defaults_to_true() {
        let instance: InstanceSettings = serde_json::from_str("{\"process_id\": 7}").unwrap();
        assert!(instance.enabled);
        assert_eq!(instance.process_id, 7);
    }

    #[test]
    fn settings_roundtrip_through_json() {
        let mut settings = AppSettings::default();
        settings.primary_character_name = "Juan-Perez".to_string();
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
