use crate::{is_game_process_name, DiscoveredWindow, GameDiscovery, PlatformError, WindowRef};
use serde::Deserialize;

/// Discovers game windows through `hyprctl clients -j`.
pub struct HyprlandDiscovery;

impl HyprlandDiscovery {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HyprlandDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl GameDiscovery for HyprlandDiscovery {
    fn scan(&self) -> Result<Vec<DiscoveredWindow>, PlatformError> {
        let json = super::run_hyprctl(&["clients", "-j"])?;
        parse_clients(&json)
    }
}

#[derive(Deserialize)]
struct HyprClient {
    address: String,
    #[serde(default)]
    pid: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    class: String,
    #[serde(default, rename = "initialClass")]
    initial_class: String,
}

fn parse_clients(json: &str) -> Result<Vec<DiscoveredWindow>, PlatformError> {
    let clients: Vec<HyprClient> = serde_json::from_str(json)
        .map_err(|e| PlatformError::InvalidData(format!("hyprctl clients JSON: {e}")))?;

    Ok(clients
        .into_iter()
        .filter(|c| {
            c.pid > 0
                && (is_game_process_name(&c.class) || is_game_process_name(&c.initial_class))
        })
        .map(|c| DiscoveredWindow {
            pid: c.pid as u32,
            title: c.title,
            process_name: if c.class.is_empty() { c.initial_class } else { c.class },
            window_ref: WindowRef::WaylandAddress(c.address),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"[
        {
            "address": "0x55d2e8a0b100",
            "pid": 4832,
            "title": "Juan-Perez - Feca - Dofus 3",
            "class": "Dofus.x64",
            "initialClass": "Dofus.x64"
        },
        {
            "address": "0x55d2e8a0b200",
            "pid": 5120,
            "title": "Mozilla Firefox",
            "class": "firefox",
            "initialClass": "firefox"
        },
        {
            "address": "0x55d2e8a0b300",
            "pid": 6001,
            "title": "DofusTabs",
            "class": "dofustabs",
            "initialClass": "dofustabs"
        },
        {
            "address": "0x55d2e8a0b400",
            "pid": -1,
            "title": "Zombie - Dofus",
            "class": "Dofus.x64",
            "initialClass": "Dofus.x64"
        }
    ]"#;

    #[test]
    fn keeps_only_game_clients_with_valid_pid() {
        let windows = parse_clients(FIXTURE).unwrap();
        assert_eq!(windows.len(), 1);

        let w = &windows[0];
        assert_eq!(w.pid, 4832);
        assert_eq!(w.title, "Juan-Perez - Feca - Dofus 3");
        assert_eq!(w.process_name, "Dofus.x64");
        assert_eq!(
            w.window_ref,
            WindowRef::WaylandAddress("0x55d2e8a0b100".to_string())
        );
    }

    #[test]
    fn falls_back_to_initial_class_when_class_is_empty() {
        let json = r#"[{"address": "0xa", "pid": 10, "title": "t", "class": "", "initialClass": "Dofus"}]"#;
        let windows = parse_clients(json).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].process_name, "Dofus");
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse_clients("not json").is_err());
    }

    #[test]
    fn empty_client_list_is_ok() {
        assert!(parse_clients("[]").unwrap().is_empty());
    }
}
