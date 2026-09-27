//! `~/.config/dofustabs/config.json`: general CLI settings (primary class,
//! mouse). Read/write preserve any other top-level key so this stays
//! forward-compatible with future settings; in particular, a leftover
//! `throttle` key from the retired background CPU limit is simply ignored
//! and left untouched on disk.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Reads `primary_class` out of `config.json`. `None` when unset.
pub fn read_primary_class(path: &Path) -> Result<Option<String>, ConfigError> {
    let root = read_root(path)?;
    Ok(root
        .get("primary_class")
        .and_then(|v| v.as_str())
        .map(str::to_string))
}

/// Sets `primary_class`, keeping every other key untouched. Atomic.
pub fn write_primary_class(path: &Path, class: Option<&str>) -> Result<(), ConfigError> {
    let mut root = read_root(path)?;
    match class {
        Some(c) => {
            root.insert("primary_class".to_string(), Value::String(c.to_string()));
        }
        None => {
            root.remove("primary_class");
        }
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&Value::Object(root))?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub const DEFAULT_SOLAAR_DEVICE: &str = "MX Master 3S";

/// Reads `mouse.solaar_device` out of `config.json`, defaulting to
/// [`DEFAULT_SOLAAR_DEVICE`] when unset.
pub fn read_solaar_device(path: &Path) -> Result<String, ConfigError> {
    let root = read_root(path)?;
    Ok(root
        .get("mouse")
        .and_then(|v| v.get("solaar_device"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_SOLAAR_DEVICE.to_string()))
}

fn read_root(path: &Path) -> Result<Map<String, Value>, ConfigError> {
    if !path.exists() {
        return Ok(Map::new());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&raw)? {
        Value::Object(map) => Ok(map),
        _ => Ok(Map::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("dofustabs-cli-config-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn primary_class_defaults_to_none_and_roundtrips() {
        let path = temp_path("primary.json");
        assert_eq!(read_primary_class(&path).unwrap(), None);
        write_primary_class(&path, Some("Yopuka")).unwrap();
        assert_eq!(
            read_primary_class(&path).unwrap(),
            Some("Yopuka".to_string())
        );
        write_primary_class(&path, None).unwrap();
        assert_eq!(read_primary_class(&path).unwrap(), None);
    }

    #[test]
    fn primary_class_preserves_other_keys() {
        let path = temp_path("primary_otherkeys.json");
        fs::write(&path, r#"{"throttle": {"enabled": false}}"#).unwrap();
        write_primary_class(&path, Some("Feca")).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["throttle"]["enabled"], false);
        assert_eq!(read_primary_class(&path).unwrap(), Some("Feca".to_string()));
    }

    #[test]
    fn write_preserves_other_keys() {
        let path = temp_path("otherkeys.json");
        fs::write(&path, r#"{"launcher_path": "/opt/ankama/launcher"}"#).unwrap();
        write_primary_class(&path, Some("Feca")).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["launcher_path"], "/opt/ankama/launcher");
        assert_eq!(value["primary_class"], "Feca");
    }
}
