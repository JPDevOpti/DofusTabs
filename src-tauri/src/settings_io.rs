use std::fs;
use std::path::Path;

use dofustabs_core::settings::AppSettings;

/// Loads settings, falling back to defaults on any error (a broken settings
/// file must never prevent startup).
pub fn load(path: &Path) -> AppSettings {
    if !path.exists() {
        return AppSettings::default();
    }

    match fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|json| {
        serde_json::from_str::<AppSettings>(&json).map_err(|e| e.to_string())
    }) {
        Ok(settings) => settings,
        Err(err) => {
            log::error!("failed to load settings from {}: {err}; using defaults", path.display());
            AppSettings::default()
        }
    }
}

/// Saves settings atomically: write to `.tmp`, keep the previous file as
/// `.bak`, then rename into place (same scheme as the original C# app).
pub fn save(path: &Path, settings: &AppSettings) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;

    if path.exists() {
        fs::rename(path, path.with_extension("json.bak"))?;
    }
    fs::rename(&tmp, path)
}

/// Save wrapper that only logs on failure; persistence problems must not
/// break interactive flows.
pub fn save_logged(path: &Path, settings: &AppSettings) {
    if let Err(err) = save(path, settings) {
        log::error!("failed to save settings to {}: {err}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dofustabs_core::settings::{Hotkey, InstanceSettings};

    fn temp_settings_path(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join("dofustabs-tests")
            .join(format!("{tag}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    #[test]
    fn roundtrips_settings_through_disk() {
        let path = temp_settings_path("roundtrip");
        let mut settings = AppSettings::default();
        settings.primary_character_name = "Ana".to_string();
        settings.instances.push(InstanceSettings {
            process_id: 42,
            title: "Ana - Feca - Dofus".to_string(),
            enabled: false,
            display_order: 1,
            individual_hotkey: Some(Hotkey::alt("1")),
        });

        save(&path, &settings).unwrap();
        assert_eq!(load(&path), settings);

        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn keeps_previous_file_as_backup() {
        let path = temp_settings_path("backup");
        let first = AppSettings::default();
        let mut second = AppSettings::default();
        second.primary_character_name = "Beto".to_string();

        save(&path, &first).unwrap();
        save(&path, &second).unwrap();

        assert_eq!(load(&path), second);
        let backup = path.with_extension("json.bak");
        let restored: AppSettings =
            serde_json::from_str(&fs::read_to_string(backup).unwrap()).unwrap();
        assert_eq!(restored, first);

        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn missing_or_corrupt_files_fall_back_to_defaults() {
        let path = temp_settings_path("corrupt");
        assert_eq!(load(&path), AppSettings::default());

        fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), AppSettings::default());

        fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
