use crate::{PlatformError, WindowManager, WindowRef};
use dofustabs_core::geometry::Rect;
use serde::Deserialize;
use std::time::Duration;

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
        // All four dispatches run as one Lua chunk in a single `hyprctl
        // eval` call instead of four separate spawns. `hyprctl --batch`
        // cannot carry these safely: each dispatch is itself a Lua literal
        // full of quotes and braces that `--batch`'s `;`-split command line
        // would mangle, so one chunk executing all four statements in order
        // is the safe way to cut this to one process per stack.
        eval(&stack_window_lua(address, workspace, rect))
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

    fn own_monitor_name(&self) -> Result<String, PlatformError> {
        let own = own_client()?;
        let monitors = parse_monitors(&super::run_hyprctl(&["monitors", "-j"])?)?;
        monitors
            .iter()
            .find(|m| m.id == own.monitor)
            .map(|m| m.name.clone())
            .ok_or_else(|| PlatformError::InvalidData(format!("monitor {} not found", own.monitor)))
    }

    fn set_left_reserved(&self, monitor: &str, px: i32) -> Result<(), PlatformError> {
        eval(&reserve_lua(monitor, px))
    }

    fn group_bar_visible(&self) -> Result<bool, PlatformError> {
        let stdout = super::run_hyprctl(&["getoption", GROUP_BAR_OPTION])?;
        parse_bool_option(&stdout)
    }

    fn set_group_bar_visible(&self, visible: bool) -> Result<(), PlatformError> {
        eval(&group_bar_lua(visible))
    }

    fn group_window(
        &self,
        win: &WindowRef,
        workspace: &str,
        anchor: Option<&WindowRef>,
    ) -> Result<(), PlatformError> {
        // Each step is a separate `hyprctl eval`: dispatchers batched in one
        // chunk are applied asynchronously, and moving a window to another
        // workspace dissolves its group membership, so the move must be
        // observed as complete before any grouping happens.
        let address = address_of(win)?;
        let clients = parse_clients(&super::run_hyprctl(&["clients", "-j"])?)?;
        let client = client_by_address(&clients, address).ok_or_else(|| {
            PlatformError::InvalidData(format!("window {address} not found among Hyprland clients"))
        })?;
        if client.floating {
            eval(&float_off_lua(address))?;
        }
        if !client.workspace.matches(workspace) {
            eval(&move_to_workspace_lua(address, workspace))?;
            wait_for_workspace(address, workspace)?;
        }

        // Membership is re-read after the move: it may have just been reset.
        let clients = parse_clients(&super::run_hyprctl(&["clients", "-j"])?)?;
        let grouped = client_by_address(&clients, address)
            .map(|c| !c.grouped.is_empty())
            .unwrap_or(false);
        match anchor {
            None => {
                if !grouped {
                    eval(&group_toggle_lua(address))?;
                }
            }
            Some(anchor) => {
                let anchor_address = address_of(anchor)?;
                let anchor_client = client_by_address(&clients, anchor_address).ok_or_else(|| {
                    PlatformError::InvalidData(format!(
                        "group anchor {anchor_address} not found among Hyprland clients"
                    ))
                })?;
                if anchor_client.grouped.iter().any(|m| m == address) {
                    return Ok(());
                }
                if grouped {
                    eval(&group_remove_lua(address))?;
                }
                eval(&group_add_lua(anchor_address, address))?;
            }
        }
        Ok(())
    }
}

// ── Lua builders ─────────────────────────────────────────────────────────────

/// The switcher column floats without compositor decorations: no border,
/// no corner rounding and no shadow, so it reads as part of the game area.
fn own_float_rule_lua() -> String {
    format!(
        "hl.window_rule({{ match = {{ class = \"^{OWN_CLASS}$\" }}, float = true, border_size = 0, rounding = 0, no_shadow = true }})"
    )
}

fn float_lua(address: &str) -> String {
    let address = super::lua_string(address);
    format!(
        "hl.dispatch(hl.dsp.window.float({{ action = \"on\", window = \"address:{address}\" }}))"
    )
}

fn move_to_workspace_lua(address: &str, workspace: &str) -> String {
    let workspace = super::lua_string(workspace);
    format!(
        "hl.dispatch(hl.dsp.window.move({{ workspace = \"{workspace}\", window = \"address:{address}\", follow = false }}))"
    )
}

fn move_xy_lua(address: &str, x: i32, y: i32) -> String {
    let address = super::lua_string(address);
    format!(
        "hl.dispatch(hl.dsp.window.move({{ x = {x}, y = {y}, window = \"address:{address}\" }}))"
    )
}

fn resize_lua(address: &str, width: i32, height: i32) -> String {
    let address = super::lua_string(address);
    format!(
        "hl.dispatch(hl.dsp.window.resize({{ x = {width}, y = {height}, window = \"address:{address}\" }}))"
    )
}

fn float_off_lua(address: &str) -> String {
    let address = super::lua_string(address);
    format!(
        "hl.dispatch(hl.dsp.window.float({{ action = \"off\", window = \"address:{address}\" }}))"
    )
}

/// Hyprland option that toggles the tab bar drawn on grouped windows.
const GROUP_BAR_OPTION: &str = "group:groupbar:enabled";

fn group_bar_lua(visible: bool) -> String {
    format!("hl.config({{ group = {{ groupbar = {{ enabled = {visible} }} }} }})")
}

/// Parses `hyprctl getoption` output for a boolean option (`bool: true`).
fn parse_bool_option(stdout: &str) -> Result<bool, PlatformError> {
    let value = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("bool:"))
        .map(str::trim);
    match value {
        Some("true") | Some("1") => Ok(true),
        Some("false") | Some("0") => Ok(false),
        _ => Err(PlatformError::InvalidData(format!(
            "unexpected hyprctl getoption output: {}",
            stdout.trim()
        ))),
    }
}

fn reserve_lua(monitor: &str, px: i32) -> String {
    let monitor = super::lua_string(monitor);
    format!(
        "hl.monitor({{ output = \"{monitor}\", reserved = {{ top = 0, right = 0, bottom = 0, left = {px} }} }})"
    )
}

fn group_toggle_lua(address: &str) -> String {
    let address = super::lua_string(address);
    format!("hl.dispatch(hl.dsp.group.toggle({{ window = \"address:{address}\" }}))")
}

fn group_add_lua(anchor_address: &str, address: &str) -> String {
    let anchor_address = super::lua_string(anchor_address);
    let address = super::lua_string(address);
    format!(
        "local a = hl.get_window(\"address:{anchor_address}\"); local w = hl.get_window(\"address:{address}\"); if a == nil or w == nil or a.group == nil then error(\"group anchor unavailable\") end; a.group:add(w)"
    )
}

fn group_remove_lua(address: &str) -> String {
    let address = super::lua_string(address);
    format!(
        "local w = hl.get_window(\"address:{address}\"); if w ~= nil and w.group ~= nil then w.group:remove(w) end"
    )
}

fn raise_lua(address: &str) -> String {
    let address = super::lua_string(address);
    format!("hl.dispatch(hl.dsp.window.bring_to_top({{ window = \"address:{address}\" }}))")
}

/// Single Lua chunk performing every `stack_window` dispatch in order, so the
/// whole sequence runs through one `hyprctl eval` call. Hyprland resizes
/// floating windows around their centre, so the size is applied before the
/// absolute move to pin the top-left; that ordering constraint is preserved
/// by simply keeping the statements in the same order the four separate
/// `eval` calls used to run in.
fn stack_window_lua(address: &str, workspace: &str, rect: Rect) -> String {
    [
        float_lua(address),
        move_to_workspace_lua(address, workspace),
        resize_lua(address, rect.width, rect.height),
        move_xy_lua(address, rect.x, rect.y),
    ]
    .join("; ")
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
    /// Addresses of every member of this window's group (including itself);
    /// empty when the window is not grouped.
    #[serde(default)]
    grouped: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct HyprWorkspaceRef {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    name: String,
}

impl HyprWorkspaceRef {
    /// A workspace selector as used by the app is either the numeric id or
    /// the workspace name.
    fn matches(&self, selector: &str) -> bool {
        self.id.to_string() == selector || self.name == selector
    }
}

#[derive(Debug, Deserialize)]
struct HyprMonitor {
    id: i64,
    #[serde(default)]
    name: String,
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

fn client_by_address<'a>(clients: &'a [HyprClient], address: &str) -> Option<&'a HyprClient> {
    clients.iter().find(|c| c.address == address)
}

/// Workspace moves are applied asynchronously by the compositor; polls the
/// client list (up to ~500ms) until `address` reports `workspace`.
fn wait_for_workspace(address: &str, workspace: &str) -> Result<(), PlatformError> {
    const ATTEMPTS: usize = 10;
    const INTERVAL: Duration = Duration::from_millis(50);
    for _ in 0..ATTEMPTS {
        let clients = parse_clients(&super::run_hyprctl(&["clients", "-j"])?)?;
        if client_by_address(&clients, address).is_some_and(|c| c.workspace.matches(workspace)) {
            return Ok(());
        }
        std::thread::sleep(INTERVAL);
    }
    Err(PlatformError::InvalidData(format!(
        "window {address} did not reach workspace {workspace}"
    )))
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
        },
        {
            "address": "0x55d2e8a0b400",
            "pid": 7000,
            "class": "Dofus.x64",
            "workspace": {"id": 2, "name": "2"},
            "monitor": 0,
            "floating": false,
            "grouped": ["0x55d2e8a0b400", "0x55d2e8a0b500"],
            "at": [0, 0], "size": [1920, 1040], "pinned": false
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
            "hl.window_rule({ match = { class = \"^dofustabs$\" }, float = true, border_size = 0, rounding = 0, no_shadow = true })"
        );
    }

    #[test]
    fn stack_window_lua_chains_all_four_dispatches_in_order() {
        let lua = stack_window_lua("0x1", "3", Rect::new(10, 20, 300, 400));
        let parts: Vec<&str> = lua.split("; ").collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], float_lua("0x1"));
        assert_eq!(parts[1], move_to_workspace_lua("0x1", "3"));
        assert_eq!(parts[2], resize_lua("0x1", 300, 400));
        assert_eq!(parts[3], move_xy_lua("0x1", 10, 20));
    }

    #[test]
    fn grouped_addresses_parse_and_default_to_empty() {
        let clients = parse_clients(CLIENTS).unwrap();
        let ungrouped = client_by_address(&clients, "0x55d2e8a0b100").unwrap();
        assert!(ungrouped.grouped.is_empty());
        let grouped = client_by_address(&clients, "0x55d2e8a0b400").unwrap();
        assert_eq!(
            grouped.grouped,
            vec!["0x55d2e8a0b400".to_string(), "0x55d2e8a0b500".to_string()]
        );
        assert!(client_by_address(&clients, "0xdead").is_none());
    }

    #[test]
    fn monitor_name_parses() {
        let monitors = parse_monitors(MONITORS).unwrap();
        assert_eq!(monitors.iter().find(|m| m.id == 0).unwrap().name, "eDP-1");
        assert_eq!(monitors.iter().find(|m| m.id == 1).unwrap().name, "DP-1");
    }

    #[test]
    fn group_lua_builders_use_the_documented_forms() {
        assert_eq!(
            float_off_lua("0x1"),
            "hl.dispatch(hl.dsp.window.float({ action = \"off\", window = \"address:0x1\" }))"
        );
        assert_eq!(
            reserve_lua("eDP-1", 64),
            "hl.monitor({ output = \"eDP-1\", reserved = { top = 0, right = 0, bottom = 0, left = 64 } })"
        );
        assert_eq!(
            group_toggle_lua("0x1"),
            "hl.dispatch(hl.dsp.group.toggle({ window = \"address:0x1\" }))"
        );
        assert_eq!(
            group_add_lua("0x1", "0x2"),
            "local a = hl.get_window(\"address:0x1\"); local w = hl.get_window(\"address:0x2\"); if a == nil or w == nil or a.group == nil then error(\"group anchor unavailable\") end; a.group:add(w)"
        );
        assert_eq!(
            group_remove_lua("0x2"),
            "local w = hl.get_window(\"address:0x2\"); if w ~= nil and w.group ~= nil then w.group:remove(w) end"
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
