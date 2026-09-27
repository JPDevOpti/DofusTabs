//! Reads `$XDG_RUNTIME_DIR/dofustabs/state.json`, written by the Hyprland
//! module.

use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct StateClient {
    pub slot: u32,
    pub workspace: u32,
    pub account: String,
    pub name: String,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    #[allow(dead_code)] // parsed for completeness; not shown by `status` yet
    pub icon: String,
    pub pid: u64,
    #[serde(default)]
    #[allow(dead_code)]
    pub address: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct State {
    #[serde(default)]
    #[allow(dead_code)] // kept for forward compatibility with the wire format
    pub version: u32,
    #[serde(default)]
    pub clients: Vec<StateClient>,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("state file not found: {0}")]
    NotFound(String),
    #[error("malformed state file: {0}")]
    Malformed(String),
}

/// Reads and parses the state file. A missing file (no client has ever run,
/// or the module is not installed) and a malformed file are distinguished so
/// callers can give an accurate message.
pub fn read(path: &Path) -> Result<State, StateError> {
    if !path.exists() {
        return Err(StateError::NotFound(path.display().to_string()));
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| StateError::Malformed(format!("cannot read {}: {e}", path.display())))?;
    serde_json::from_str(&raw).map_err(|e| StateError::Malformed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_state_file() {
        let json = r#"{"version":1,"clients":[{"slot":1,"workspace":11,"account":"190018816","name":"Account 190018816","class":"","icon":"/x/default.png","pid":245681,"address":"0x1"}]}"#;
        let state: State = serde_json::from_str(json).unwrap();
        assert_eq!(state.clients.len(), 1);
        assert_eq!(state.clients[0].class, "");
    }

    #[test]
    fn missing_file_is_reported_as_not_found() {
        let err = read(Path::new("/nonexistent/dofustabs-cli-test/state.json")).unwrap_err();
        assert!(matches!(err, StateError::NotFound(_)));
    }

    #[test]
    fn malformed_json_is_reported_as_malformed() {
        let dir =
            std::env::temp_dir().join(format!("dofustabs-cli-state-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        fs::write(&path, "{ not json").unwrap();
        let err = read(&path).unwrap_err();
        assert!(matches!(err, StateError::Malformed(_)));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_client_list_parses_fine() {
        let state: State = serde_json::from_str(r#"{"version":1,"clients":[]}"#).unwrap();
        assert!(state.clients.is_empty());
    }
}
