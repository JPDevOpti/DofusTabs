use crate::{PlatformError, WindowManager, WindowRef};
use dofustabs_core::geometry::Rect;
use serde::Deserialize;

/// Own window class as reported by Hyprland (the Tauri app id).
const OWN_CLASS: &str = "dofustabs";

/// Places windows through Hyprland's Lua `hl` API (`hyprctl eval`).
pub struct HyprlandWindowManager;

impl HyprlandWindowManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HyprlandWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

fn eval(lua: &str) -> Result<(), PlatformError> {
    let stdout = super::run_hyprctl(&["eval", lua])?;
    super::expect_ok(&format!("eval {lua}"), &stdout)
}

fn address_of(win: &WindowRef) -> Result<&str, PlatformError> {
    match win {
        WindowRef::WaylandAddress(address) => Ok(address),
        other => Err(PlatformError::InvalidData(format!(
            "Hyprland window manager requires a Wayland address, got {other:?}"
        ))),
    }
}

fn own_client() -> Result<HyprClient, PlatformError> {
    let json = super::run_hyprctl(&["clients", "-j"])?;
    let clients = parse_clients(&json)?;
    find_own_client(clients, std::process::id()).ok_or_else(|| {
        PlatformError::InvalidData("own window not found among Hyprland clients".to_string())
    })
}

impl WindowManager for HyprlandWindowManager {
    fn prepare_own_window(&self) -> Result<WindowRef, PlatformError> {
        // The rule only applies to windows mapped after it, so the already
        // mapped own window is floated explicitly as well.
        eval(&own_float_rule_lua())?;
        let own = own_client()?;
        if !own.floating {
            eval(&float_lua(&own.address))?;
        }
        Ok(WindowRef::WaylandAddress(own.address))
    }

    fn stack_window(
        &self,
        win: &WindowRef,
        workspace: &str,
        rect: Rect,
    ) -> Result<(), PlatformError> {
        let address = address_of(win)?;
        eval(&float_lua(address))?;
        eval(&move_to_workspace_lua(address, workspace))?;
        eval(&move_xy_lua(address, rect.x, rect.y))?;
        eval(&resize_lua(address, rect.width, rect.height))
    }

    fn raise(&self, win: &WindowRef) -> Result<(), PlatformError> {
        eval(&raise_lua(address_of(win)?))
    }

    fn own_workspace_and_area(&self) -> Result<(String, Rect), PlatformError> {
        let own = own_client()?;
        let monitors = parse_monitors(&super::run_hyprctl(&["monitors", "-j"])?)?;
        let area = monitor_area(&monitors, own.monitor).ok_or_else(|| {
            PlatformError::InvalidData(format!("monitor {} not found", own.monitor))
        })?;
        Ok((own.workspace.id.to_string(), area))
    }
}

// ── Lua builders ─────────────────────────────────────────────────────────────

fn own_float_rule_lua() -> String {
    format!("hl.window_rule({{ match = {{ class = \"^{OWN_CLASS}$\" }}, float = true }})")
}

fn float_lua(address: &str) -> String {
    format!(
        "hl.dispatch(hl.dsp.window.float({{ action = \"on\", window = \"address:{address}\" }}))"
    )
}

fn move_to_workspace_lua(address: &str, workspace: &str) -> String {
    format!(
        "hl.dispatch(hl.dsp.window.move({{ workspace = \"{workspace}\", window = \"address:{address}\", follow = false }}))"
    )
}

fn move_xy_lua(address: &str, x: i32, y: i32) -> String {
    format!(
        "hl.dispatch(hl.dsp.window.move({{ x = {x}, y = {y}, window = \"address:{address}\" }}))"
    )
}

fn resize_lua(address: &str, width: i32, height: i32) -> String {
    format!(
        "hl.dispatch(hl.dsp.window.resize({{ x = {width}, y = {height}, window = \"address:{address}\" }}))"
    )
}

fn raise_lua(address: &str) -> String {
    format!("hl.dispatch(hl.dsp.window.bring_to_top({{ window = \"address:{address}\" }}))")
}

// ── hyprctl JSON ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct HyprClient {
    address: String,
    #[serde(default)]
    pid: i64,
    #[serde(default)]
    class: String,
    #[serde(default)]
    workspace: HyprWorkspaceRef,
    #[serde(default)]
    monitor: i64,
    #[serde(default)]
    floating: bool,
}

#[derive(Debug, Default, Deserialize)]
struct HyprWorkspaceRef {
    #[serde(default)]
    id: i64,
}

#[derive(Debug, Deserialize)]
struct HyprMonitor {
    id: i64,
    #[serde(default)]
    x: i32,
    #[serde(default)]
    y: i32,
    #[serde(default)]
    width: i32,
    #[serde(default)]
    height: i32,
    #[serde(default = "one")]
    scale: f64,
    #[serde(default)]
    reserved: [i32; 4],
}

fn one() -> f64 {
    1.0
}

fn parse_clients(json: &str) -> Result<Vec<HyprClient>, PlatformError> {
    serde_json::from_str(json)
        .map_err(|e| PlatformError::InvalidData(format!("hyprctl clients JSON: {e}")))
}

fn parse_monitors(json: &str) -> Result<Vec<HyprMonitor>, PlatformError> {
    serde_json::from_str(json)
        .map_err(|e| PlatformError::InvalidData(format!("hyprctl monitors JSON: {e}")))
}

/// The own window is matched by pid first (unambiguous), falling back to the
/// app class for the rare case where Hyprland reports no pid.
fn find_own_client(clients: Vec<HyprClient>, own_pid: u32) -> Option<HyprClient> {
    let by_pid = clients.iter().position(|c| c.pid == i64::from(own_pid));
    let index = by_pid.or_else(|| clients.iter().position(|c| c.class == OWN_CLASS))?;
    clients.into_iter().nth(index)
}

fn monitor_area(monitors: &[HyprMonitor], monitor_id: i64) -> Option<Rect> {
    let m = monitors.iter().find(|m| m.id == monitor_id)?;
    Some(Rect::usable_area(
        m.x, m.y, m.width, m.height, m.scale, m.reserved,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENTS: &str = r#"[
        {
            "address": "0x55d2e8a0b100",
            "pid": 4832,
            "class": "Dofus.x64",
            "workspace": {"id": 2, "name": "2"},
            "monitor": 0,
            "floating": false,
            "at": [0, 0], "size": [1920, 1040], "pinned": false
        },
        {
            "address": "0x55d2e8a0b300",
            "pid": 6001,
            "class": "dofustabs",
            "workspace": {"id": 3, "name": "3"},
            "monitor": 1,
            "floating": true,
            "at": [1930, 20], "size": [300, 800], "pinned": false
        }
    ]"#;

    const MONITORS: &str = r#"[
        {
            "id": 0, "name": "eDP-1", "x": 0, "y": 0, "width": 3840, "height": 2160,
            "scale": 2.0, "reserved": [0, 40, 0, 0], "activeWorkspace": {"id": 2, "name": "2"}
        },
        {
            "id": 1, "name": "DP-1", "x": 1920, "y": 0, "width": 2560, "height": 1440,
            "scale": 1.0, "reserved": [10, 20, 30, 40], "activeWorkspace": {"id": 3, "name": "3"}
        }
    ]"#;

    #[test]
    fn own_client_is_found_by_pid() {
        let own = find_own_client(parse_clients(CLIENTS).unwrap(), 6001).unwrap();
        assert_eq!(own.address, "0x55d2e8a0b300");
        assert_eq!(own.workspace.id, 3);
        assert_eq!(own.monitor, 1);
        assert!(own.floating);
    }

    #[test]
    fn own_client_falls_back_to_class_when_pid_is_unknown() {
        let own = find_own_client(parse_clients(CLIENTS).unwrap(), 99999).unwrap();
        assert_eq!(own.class, "dofustabs");
        assert!(find_own_client(Vec::new(), 99999).is_none());
    }

    #[test]
    fn monitor_area_uses_scale_and_reserved_margins() {
        let monitors = parse_monitors(MONITORS).unwrap();
        assert_eq!(
            monitor_area(&monitors, 0),
            Some(Rect::new(0, 40, 1920, 1040))
        );
        assert_eq!(
            monitor_area(&monitors, 1),
            Some(Rect::new(1930, 20, 2520, 1380))
        );
        assert_eq!(monitor_area(&monitors, 7), None);
    }

    #[test]
    fn monitor_without_scale_or_reserved_defaults_to_full_size() {
        let monitors =
            parse_monitors(r#"[{"id": 0, "x": 0, "y": 0, "width": 1280, "height": 720}]"#).unwrap();
        assert_eq!(monitor_area(&monitors, 0), Some(Rect::new(0, 0, 1280, 720)));
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse_clients("nope").is_err());
        assert!(parse_monitors("nope").is_err());
    }

    #[test]
    fn lua_builders_target_the_window_by_address() {
        assert_eq!(
            float_lua("0x1"),
            "hl.dispatch(hl.dsp.window.float({ action = \"on\", window = \"address:0x1\" }))"
        );
        assert_eq!(
            move_to_workspace_lua("0x1", "3"),
            "hl.dispatch(hl.dsp.window.move({ workspace = \"3\", window = \"address:0x1\", follow = false }))"
        );
        assert_eq!(
            move_xy_lua("0x1", 100, -5),
            "hl.dispatch(hl.dsp.window.move({ x = 100, y = -5, window = \"address:0x1\" }))"
        );
        assert_eq!(
            resize_lua("0x1", 1500, 900),
            "hl.dispatch(hl.dsp.window.resize({ x = 1500, y = 900, window = \"address:0x1\" }))"
        );
        assert_eq!(
            raise_lua("0x1"),
            "hl.dispatch(hl.dsp.window.bring_to_top({ window = \"address:0x1\" }))"
        );
        assert_eq!(
            own_float_rule_lua(),
            "hl.window_rule({ match = { class = \"^dofustabs$\" }, float = true })"
        );
    }

    #[test]
    fn non_wayland_refs_are_rejected() {
        assert!(address_of(&WindowRef::Hwnd(1)).is_err());
        assert_eq!(
            address_of(&WindowRef::WaylandAddress("0x9".into())).unwrap(),
            "0x9"
        );
    }
}
