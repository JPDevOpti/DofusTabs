//! `doctor`: pass/fail checks for the whole integration.

use std::fs;
use std::process::Command;

use crate::assets;
use crate::paths;

struct Check {
    name: &'static str,
    ok: bool,
    detail: String,
}

pub fn run() -> bool {
    let running = has_running_clients();
    let checks = vec![
        check_hyprland_reachable(),
        check_module_installed(),
        check_loader_block(),
        check_configerrors(),
        check_widget_installed(),
        check_widget_in_bar(),
        check_icons_installed(),
        check_state_readable(),
        check_binds(running),
        check_f9_untouched(),
        check_character_dir_readable(),
        check_characters_resolved(),
        check_mouse_integration(),
    ];

    let mut all_ok = true;
    for c in &checks {
        let mark = if c.ok { "PASS" } else { "FAIL" };
        println!("[{mark}] {}: {}", c.name, c.detail);
        all_ok &= c.ok;
    }
    all_ok
}

fn hyprctl(args: &[&str]) -> Option<String> {
    let output = Command::new("hyprctl").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn check_hyprland_reachable() -> Check {
    let ok = hyprctl(&["version"]).is_some();
    Check {
        name: "hyprland reachable",
        detail: if ok {
            "hyprctl responded".to_string()
        } else {
            "hyprctl did not respond; is Hyprland running?".to_string()
        },
        ok,
    }
}

fn check_module_installed() -> Check {
    let path = paths::dofustabs_module_path();
    let matches = fs::read_to_string(&path).ok().as_deref() == Some(assets::HYPRLAND_MODULE);
    Check {
        name: "module installed and up to date",
        detail: if matches {
            path.display().to_string()
        } else if path.exists() {
            format!(
                "{} differs from the embedded module; run `dofustabs install`",
                path.display()
            )
        } else {
            format!("{} not found; run `dofustabs install`", path.display())
        },
        ok: matches,
    }
}

fn check_loader_block() -> Check {
    let path = paths::hyprland_lua_path();
    let present = fs::read_to_string(&path)
        .map(|c| c.contains(assets::LOADER_BEGIN))
        .unwrap_or(false);
    Check {
        name: "loader block present in hyprland.lua",
        detail: if present {
            path.display().to_string()
        } else {
            format!("missing in {}", path.display())
        },
        ok: present,
    }
}

fn check_configerrors() -> Check {
    match hyprctl(&["configerrors"]) {
        Some(out) if out.trim().is_empty() => Check {
            name: "hyprctl configerrors empty",
            detail: "no errors".to_string(),
            ok: true,
        },
        Some(out) => Check {
            name: "hyprctl configerrors empty",
            detail: out.trim().to_string(),
            ok: false,
        },
        None => Check {
            name: "hyprctl configerrors empty",
            detail: "hyprctl unreachable".to_string(),
            ok: false,
        },
    }
}

fn check_widget_installed() -> Check {
    let dir = paths::widget_dir();
    let manifest_ok = fs::read_to_string(dir.join("manifest.json"))
        .ok()
        .as_deref()
        == Some(assets::WIDGET_MANIFEST);
    let qml_ok = fs::read_to_string(dir.join("BarWidget.qml"))
        .ok()
        .as_deref()
        == Some(assets::WIDGET_QML);
    let ok = manifest_ok && qml_ok;
    Check {
        name: "widget installed and up to date",
        detail: if ok {
            dir.display().to_string()
        } else {
            format!(
                "{} missing or differs from the embedded widget",
                dir.display()
            )
        },
        ok,
    }
}

fn check_widget_in_bar() -> Check {
    let path = paths::shell_json_path();
    let present = fs::read_to_string(&path)
        .map(|c| c.contains("local.dofus-workspaces"))
        .unwrap_or(false);
    Check {
        name: "widget present in bar layout",
        detail: if present {
            path.display().to_string()
        } else {
            format!("not found in {}", path.display())
        },
        ok: present,
    }
}

fn check_icons_installed() -> Check {
    let dir = paths::icons_dir();
    let missing: Vec<&str> = assets::ICONS
        .iter()
        .filter(|(name, bytes)| fs::read(dir.join(name)).ok().as_deref() != Some(*bytes))
        .map(|(name, _)| *name)
        .collect();
    Check {
        name: "icons installed",
        detail: if missing.is_empty() {
            dir.display().to_string()
        } else {
            format!("missing or stale: {}", missing.join(", "))
        },
        ok: missing.is_empty(),
    }
}

fn check_state_readable() -> Check {
    let path = paths::state_json_path();
    match crate::state::read(&path) {
        Ok(_) => Check {
            name: "state file readable",
            detail: path.display().to_string(),
            ok: true,
        },
        Err(crate::state::StateError::NotFound(_)) => Check {
            name: "state file readable",
            detail: format!("{} not found (no client has run yet)", path.display()),
            ok: true,
        },
        Err(e) => Check {
            name: "state file readable",
            detail: e.to_string(),
            ok: false,
        },
    }
}

fn has_running_clients() -> bool {
    crate::state::read(&paths::state_json_path())
        .map(|s| !s.clients.is_empty())
        .unwrap_or(false)
}

fn check_binds(running: bool) -> Check {
    let binds = hyprctl(&["binds", "-j"]).unwrap_or_default();
    let has = |key: &str| {
        binds.contains(&format!("\"key\":\"{key}\""))
            || binds.contains(&format!("\"key\": \"{key}\""))
    };
    let f1 = has("F1");
    let f2 = has("F2");
    let f3 = has("F3");
    let ok = if running {
        f1 && f2 && f3
    } else {
        !f1 && !f2 && !f3
    };
    Check {
        name: "F1/F2/F3 binds present iff clients are running",
        detail: format!("running_clients={running} F1={f1} F2={f2} F3={f3}"),
        ok,
    }
}

/// Informational only: mouse integration is opt-in (`dofustabs mouse
/// enable`), so its absence is never a failure. Only an inconsistent
/// half-enabled state (rule present but Solaar not running, say) is worth
/// flagging, and even then as a hint, not a doctor failure.
fn check_mouse_integration() -> Check {
    let rule_present = fs::read_to_string(crate::mouse::solaar_rules_yaml_path())
        .map(|c| c.contains(crate::mouse::RULE_BEGIN))
        .unwrap_or(false);
    let autostart_present = fs::read_to_string(crate::mouse::autostart_lua_path())
        .map(|c| c.contains(crate::mouse::AUTOSTART_BEGIN))
        .unwrap_or(false);
    let solaar_running = Command::new("pgrep")
        .args(["-x", "solaar"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    Check {
        name: "mouse integration (opt-in; informational)",
        detail: format!(
            "rule_present={rule_present} autostart_present={autostart_present} solaar_running={solaar_running}"
        ),
        ok: true,
    }
}

/// The Unity3d save root the game writes character files into
/// (`~/.config/unity3d/Ankama/Dofus`). Not created until the launcher has
/// been run at least once, so a missing directory is reported informationally,
/// not as a failure; an existing-but-unreadable one is a real failure.
fn check_character_dir_readable() -> Check {
    let dir = paths::unity3d_dofus_dir();
    if !dir.exists() {
        return Check {
            name: "character directory root readable",
            detail: format!("{} not created yet (no client has run)", dir.display()),
            ok: true,
        };
    }
    match fs::read_dir(&dir) {
        Ok(_) => Check {
            name: "character directory root readable",
            detail: dir.display().to_string(),
            ok: true,
        },
        Err(e) => Check {
            name: "character directory root readable",
            detail: format!("{}: {e}", dir.display()),
            ok: false,
        },
    }
}

/// Informational only: whether each currently running client's active
/// character was resolved by the last `dofustabs identify` run. Never fails
/// doctor, since "not in game yet" is an expected, benign state (login /
/// character-selection screen).
fn check_characters_resolved() -> Check {
    let identity =
        crate::identify::Identity::load(&paths::identity_json_path()).unwrap_or_default();
    let state = match crate::state::read(&paths::state_json_path()) {
        Ok(s) => s,
        Err(_) => {
            return Check {
                name: "active character resolved per running client",
                detail: "no running clients".to_string(),
                ok: true,
            }
        }
    };
    if state.clients.is_empty() {
        return Check {
            name: "active character resolved per running client",
            detail: "no running clients".to_string(),
            ok: true,
        };
    }
    let mut parts = Vec::new();
    for c in &state.clients {
        match identity.active_character(&c.account) {
            Some(hash) => parts.push(format!(
                "account {} resolved {}",
                c.account,
                &hash[..hash.len().min(8)]
            )),
            None => parts.push(format!("account {} not in game yet", c.account)),
        }
    }
    Check {
        name: "active character resolved per running client",
        detail: parts.join("; "),
        ok: true,
    }
}

fn check_f9_untouched() -> Check {
    let binds = hyprctl(&["binds", "-j"]).unwrap_or_default();
    let has_f9 = binds.contains("\"key\":\"F9\"") || binds.contains("\"key\": \"F9\"");
    Check {
        name: "F9 binds untouched (informational)",
        detail: if has_f9 {
            "F9 is bound".to_string()
        } else {
            "F9 has no bind (informational only)".to_string()
        },
        ok: true,
    }
}
