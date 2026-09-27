//! MX Master 3S integration: `dofustabs main` (Smart Shift button target)
//! and `dofustabs mouse enable|disable|status` (wiring Solaar to call it).
//!
//! Mechanism: the side buttons are ordinary Hyprland mouse binds, handled
//! entirely in `hyprland/dofustabs.lua` (`MOUSE_BACK`/`MOUSE_FORWARD`). The
//! Smart Shift button has no OS-visible event by default, so it is diverted
//! (`solaar config <device> divert-keys "Smart Shift" Diverted`) and a
//! Solaar rule executes `dofustabs main` on press. That requires the Solaar
//! process to run in the user session, so `mouse enable` also wires a guarded
//! autostart block into `~/.config/hypr/autostart.lua`.

use std::fs;
use std::process::Command;

use crate::paths;

pub const RULE_BEGIN: &str = "# BEGIN dofustabs-mouse";
pub const RULE_END: &str = "# END dofustabs-mouse";
pub const AUTOSTART_BEGIN: &str = "-- BEGIN dofustabs-mouse";
pub const AUTOSTART_END: &str = "-- END dofustabs-mouse";

const SMART_SHIFT_KEY: &str = "Smart Shift";

pub fn autostart_lua_path() -> std::path::PathBuf {
    paths::hypr_config_dir().join("autostart.lua")
}

pub fn solaar_rules_yaml_path() -> std::path::PathBuf {
    paths::config_home().join("solaar/rules.yaml")
}

/// The Solaar rule document that runs `<dofustabs_bin> main` on Smart Shift
/// press, delimited so `disable` can remove exactly this block. Solaar loads
/// `rules.yaml` as a sequence of YAML documents (`yaml.safe_load_all`), each
/// document itself a list of rule components (see
/// `logitech_receiver/diversion.py`: `Rule.__init__` / `Key.data` /
/// `Execute.data`), so this is a full, self-contained document.
pub fn rule_block(dofustabs_bin: &str) -> String {
    format!(
        "{RULE_BEGIN}\n\
         ---\n\
         - Key: [{SMART_SHIFT_KEY}, pressed]\n\
         - Execute: [{dofustabs_bin}, main]\n\
         {RULE_END}\n"
    )
}

/// Prepends `block` to `contents` unless an identical dofustabs block is
/// already present (in which case `contents` is returned unchanged).
pub fn insert_delimited_block(contents: &str, begin: &str, block: &str) -> String {
    if contents.contains(begin) {
        return contents.to_string();
    }
    if contents.trim().is_empty() {
        block.to_string()
    } else {
        format!("{block}{contents}")
    }
}

/// Removes the text between (and including) `begin`/`end`. `None` when the
/// markers are not found.
pub fn remove_delimited_block(contents: &str, begin: &str, end: &str) -> Option<String> {
    let start = contents.find(begin)?;
    let mut finish = contents[start..].find(end)? + start + end.len();
    if contents[finish..].starts_with('\n') {
        finish += 1;
    }
    let mut result = contents[..start].to_string();
    result.push_str(&contents[finish..]);
    Some(result)
}

fn dofustabs_bin_path() -> String {
    paths::home_dir()
        .join(".local/bin/dofustabs")
        .display()
        .to_string()
}

fn solaar_device(config_path: &std::path::Path) -> anyhow::Result<String> {
    Ok(crate::config::read_solaar_device(config_path)?)
}

fn solaar_running() -> bool {
    Command::new("pgrep")
        .args(["-x", "solaar"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn run_enable() -> anyhow::Result<()> {
    let device = solaar_device(&paths::cli_config_json_path())?;
    let bin = dofustabs_bin_path();

    // 1. Divert Smart Shift so it stops toggling the wheel internally and
    // starts sending a HID++ notification Solaar's rule can see.
    let status = Command::new("solaar")
        .args([
            "config",
            &device,
            "divert-keys",
            SMART_SHIFT_KEY,
            "Diverted",
        ])
        .status();
    match status {
        Ok(s) if s.success() => println!("diverted {SMART_SHIFT_KEY} on {device}"),
        Ok(s) => eprintln!("warning: solaar config divert-keys exited with {s}"),
        Err(e) => eprintln!("warning: failed to run solaar config: {e}"),
    }

    // 2. Solaar rule.
    let rules_path = solaar_rules_yaml_path();
    let existing = fs::read_to_string(&rules_path).unwrap_or_default();
    let updated = insert_delimited_block(&existing, RULE_BEGIN, &rule_block(&bin));
    if updated != existing {
        paths::backup_if_exists(&rules_path)?;
        if let Some(dir) = rules_path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&rules_path, updated)?;
        println!("added rule to {}", rules_path.display());
    } else {
        println!("rule already present in {}", rules_path.display());
    }

    // 3. Autostart block (hidden Solaar on Hyprland start).
    let autostart_path = autostart_lua_path();
    let existing = fs::read_to_string(&autostart_path).unwrap_or_default();
    let block = format!(
        "{AUTOSTART_BEGIN}\no.launch_on_start(\"solaar --window=hide\")\n{AUTOSTART_END}\n"
    );
    let updated = insert_delimited_block(&existing, AUTOSTART_BEGIN, &block);
    if updated != existing {
        paths::backup_if_exists(&autostart_path)?;
        if let Some(dir) = autostart_path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&autostart_path, updated)?;
        println!("added autostart entry to {}", autostart_path.display());
    } else {
        println!(
            "autostart entry already present in {}",
            autostart_path.display()
        );
    }

    // 4. Start Solaar now, hidden, if it is not already running.
    if solaar_running() {
        println!("solaar already running");
    } else {
        match Command::new("solaar").args(["--window=hide"]).spawn() {
            Ok(_) => println!("started solaar --window=hide"),
            Err(e) => eprintln!("warning: failed to start solaar: {e}"),
        }
    }

    println!("mouse integration enabled");
    Ok(())
}

pub fn run_disable() -> anyhow::Result<()> {
    let device = solaar_device(&paths::cli_config_json_path())?;

    // Reverse diversion first.
    let status = Command::new("solaar")
        .args(["config", &device, "divert-keys", SMART_SHIFT_KEY, "Regular"])
        .status();
    match status {
        Ok(s) if s.success() => println!("{SMART_SHIFT_KEY} restored to Regular on {device}"),
        Ok(s) => eprintln!("warning: solaar config divert-keys exited with {s}"),
        Err(e) => eprintln!("warning: failed to run solaar config: {e}"),
    }

    let rules_path = solaar_rules_yaml_path();
    if rules_path.exists() {
        let contents = fs::read_to_string(&rules_path)?;
        if let Some(updated) = remove_delimited_block(&contents, RULE_BEGIN, RULE_END) {
            paths::backup_if_exists(&rules_path)?;
            fs::write(&rules_path, updated)?;
            println!("removed rule from {}", rules_path.display());
        }
    }

    let autostart_path = autostart_lua_path();
    if autostart_path.exists() {
        let contents = fs::read_to_string(&autostart_path)?;
        if let Some(updated) = remove_delimited_block(&contents, AUTOSTART_BEGIN, AUTOSTART_END) {
            paths::backup_if_exists(&autostart_path)?;
            fs::write(&autostart_path, updated)?;
            println!("removed autostart entry from {}", autostart_path.display());
        }
    }

    println!(
        "mouse integration disabled (a Solaar process this tool did not start is left running)"
    );
    Ok(())
}

pub fn run_status() -> anyhow::Result<()> {
    let device = solaar_device(&paths::cli_config_json_path())?;
    println!("solaar running: {}", solaar_running());

    let divert = Command::new("solaar")
        .args(["config", &device, "divert-keys"])
        .output();
    match divert {
        Ok(o) => println!("divert-keys: {}", String::from_utf8_lossy(&o.stdout).trim()),
        Err(e) => println!("divert-keys: could not query ({e})"),
    }

    let rules_path = solaar_rules_yaml_path();
    let rule_present = fs::read_to_string(&rules_path)
        .map(|c| c.contains(RULE_BEGIN))
        .unwrap_or(false);
    println!("rule present in {}: {rule_present}", rules_path.display());

    let autostart_path = autostart_lua_path();
    let autostart_present = fs::read_to_string(&autostart_path)
        .map(|c| c.contains(AUTOSTART_BEGIN))
        .unwrap_or(false);
    println!(
        "autostart entry present in {}: {autostart_present}",
        autostart_path.display()
    );

    let binds = Command::new("hyprctl").args(["binds", "-j"]).output();
    match binds {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            let present = text.contains("mouse:275") && text.contains("mouse:276");
            println!("side-button binds present: {present}");
        }
        Err(e) => println!("side-button binds: could not query ({e})"),
    }

    Ok(())
}

/// What `dofustabs main` should do, given whether any game client is
/// currently tracked. Kept pure and separate from I/O so it is unit-tested
/// directly.
#[derive(Debug, PartialEq, Eq)]
pub enum MainAction {
    /// Focus this slot (the lowest occupied slot; 1 when it is occupied).
    FocusSlot { slot: u32 },
    /// No game running: restore the button's original function.
    ToggleWheel,
}

pub fn decide_main_action(occupied_slots: &[u32]) -> MainAction {
    if occupied_slots.is_empty() {
        return MainAction::ToggleWheel;
    }
    let slot = if occupied_slots.contains(&1) {
        1
    } else {
        *occupied_slots.iter().min().unwrap()
    };
    MainAction::FocusSlot { slot }
}

pub fn run_main() -> anyhow::Result<()> {
    let state = crate::state::read(&paths::state_json_path());
    let occupied: Vec<u32> = match state {
        Ok(s) => s.clients.iter().map(|c| c.slot).collect(),
        Err(_) => Vec::new(),
    };

    match decide_main_action(&occupied) {
        MainAction::FocusSlot { slot } => {
            let expr = format!("dofustabs.goto_slot({slot})");
            let status = Command::new("hyprctl").args(["eval", &expr]).status();
            match status {
                Ok(s) if s.success() => println!("focused slot {slot}"),
                Ok(s) => eprintln!("warning: hyprctl eval exited with {s}"),
                Err(e) => eprintln!("warning: failed to run hyprctl eval: {e}"),
            }
        }
        MainAction::ToggleWheel => {
            let device = solaar_device(&paths::cli_config_json_path())?;
            let status = Command::new("solaar")
                .args(["config", &device, "scroll-ratchet", "~"])
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("no game running: toggled scroll-ratchet on {device}")
                }
                Ok(s) => eprintln!("warning: solaar config scroll-ratchet exited with {s}"),
                Err(e) => eprintln!("warning: failed to run solaar config: {e}"),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_main_action_toggles_wheel_when_nothing_running() {
        assert_eq!(decide_main_action(&[]), MainAction::ToggleWheel);
    }

    #[test]
    fn decide_main_action_prefers_slot_1() {
        assert_eq!(
            decide_main_action(&[3, 1, 2]),
            MainAction::FocusSlot { slot: 1 }
        );
    }

    #[test]
    fn decide_main_action_falls_back_to_lowest_occupied_slot() {
        assert_eq!(
            decide_main_action(&[4, 2]),
            MainAction::FocusSlot { slot: 2 }
        );
    }

    #[test]
    fn rule_insert_is_idempotent_and_preserves_unrelated_content() {
        let base = "# my own rules\n- Rule: []\n";
        let block = rule_block("/home/user/.local/bin/dofustabs");
        let inserted = insert_delimited_block(base, RULE_BEGIN, &block);
        assert!(inserted.contains(RULE_BEGIN));
        assert!(inserted.contains("- Key: [Smart Shift, pressed]"));
        assert!(inserted.contains("- Execute: [/home/user/.local/bin/dofustabs, main]"));
        assert!(inserted.contains("# my own rules"));

        let inserted_again = insert_delimited_block(&inserted, RULE_BEGIN, &block);
        assert_eq!(inserted, inserted_again);
    }

    #[test]
    fn rule_insert_on_empty_file() {
        let block = rule_block("/home/user/.local/bin/dofustabs");
        let inserted = insert_delimited_block("", RULE_BEGIN, &block);
        assert_eq!(inserted, block);
    }

    #[test]
    fn rule_remove_strips_only_our_block() {
        let base = "# my own rules\n- Rule: []\n";
        let block = rule_block("/home/user/.local/bin/dofustabs");
        let inserted = insert_delimited_block(base, RULE_BEGIN, &block);
        let removed = remove_delimited_block(&inserted, RULE_BEGIN, RULE_END).unwrap();
        assert_eq!(removed, base);
    }

    #[test]
    fn rule_remove_is_none_when_absent() {
        assert_eq!(
            remove_delimited_block("no markers here", RULE_BEGIN, RULE_END),
            None
        );
    }

    #[test]
    fn autostart_insert_and_remove_roundtrip() {
        let base = "-- Extra autostart processes.\n-- o.launch_on_start(\"my-service\")\n";
        let block = format!(
            "{AUTOSTART_BEGIN}\no.launch_on_start(\"solaar --window=hide\")\n{AUTOSTART_END}\n"
        );
        let inserted = insert_delimited_block(base, AUTOSTART_BEGIN, &block);
        assert!(inserted.contains("solaar --window=hide"));
        assert!(inserted.contains("my-service"));

        let removed = remove_delimited_block(&inserted, AUTOSTART_BEGIN, AUTOSTART_END).unwrap();
        assert_eq!(removed, base);
    }
}
