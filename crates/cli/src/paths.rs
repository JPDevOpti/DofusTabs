//! XDG-aware locations for every file the CLI reads or writes.

use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// `$XDG_CONFIG_HOME`, or `~/.config` when unset.
pub fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
}

/// `$XDG_DATA_HOME`, or `~/.local/share` when unset.
pub fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".local/share"))
}

/// `$XDG_RUNTIME_DIR`, or `/tmp` when unset (matches the Hyprland module).
pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn dofustabs_config_dir() -> PathBuf {
    config_home().join("dofustabs")
}

pub fn accounts_json_path() -> PathBuf {
    dofustabs_config_dir().join("accounts.json")
}

pub fn accounts_lua_path() -> PathBuf {
    dofustabs_config_dir().join("accounts.lua")
}

pub fn cli_config_json_path() -> PathBuf {
    dofustabs_config_dir().join("config.json")
}

/// Runtime state (like `state.json`), not configuration: rewritten every
/// ~10s by the periodic `identify` poll, so it lives under
/// `$XDG_RUNTIME_DIR/dofustabs/`, not `~/.config/dofustabs/`.
pub fn identity_json_path() -> PathBuf {
    dofustabs_runtime_dir().join("identity.json")
}

pub fn identity_lua_path() -> PathBuf {
    dofustabs_runtime_dir().join("identity.lua")
}

/// Where `identity.json`/`identity.lua` used to live, in stage 2. `install`
/// removes any leftover copy there so a stale file is never read by
/// mistake.
pub fn legacy_identity_json_path() -> PathBuf {
    dofustabs_config_dir().join("identity.json")
}

pub fn legacy_identity_lua_path() -> PathBuf {
    dofustabs_config_dir().join("identity.lua")
}

pub fn classes_json_path() -> PathBuf {
    data_home().join("dofustabs/classes.json")
}

/// The Unity3d save root Ankama's launcher writes into: always under the
/// real `$HOME/.config` (this is the game's own convention, not ours, so it
/// is not affected by `$XDG_CONFIG_HOME` overrides used to sandbox
/// dofustabs's own config in tests).
pub fn unity3d_dofus_dir() -> PathBuf {
    home_dir().join(".config/unity3d/Ankama/Dofus")
}

/// `Characters/` directory for one account under one release. `release`
/// must be upper-cased by the caller (the on-disk directory is uppercase,
/// e.g. `EXPERIMENTAL`).
pub fn characters_dir(release_upper: &str, account_id: &str) -> PathBuf {
    unity3d_dofus_dir()
        .join(release_upper)
        .join("Accounts")
        .join(account_id)
        .join("Characters")
}

pub fn hypr_config_dir() -> PathBuf {
    config_home().join("hypr")
}

pub fn hyprland_lua_path() -> PathBuf {
    hypr_config_dir().join("hyprland.lua")
}

pub fn dofustabs_module_path() -> PathBuf {
    hypr_config_dir().join("dofustabs.lua")
}

pub fn omarchy_plugins_dir() -> PathBuf {
    config_home().join("omarchy/plugins")
}

pub fn widget_dir() -> PathBuf {
    omarchy_plugins_dir().join("local.dofus-workspaces")
}

pub fn shell_json_path() -> PathBuf {
    config_home().join("omarchy/shell.json")
}

pub fn icons_dir() -> PathBuf {
    data_home().join("dofustabs/icons")
}

pub fn state_json_path() -> PathBuf {
    runtime_dir().join("dofustabs/state.json")
}

pub fn dofustabs_runtime_dir() -> PathBuf {
    runtime_dir().join("dofustabs")
}

pub fn throttle_cache_path() -> PathBuf {
    dofustabs_runtime_dir().join("throttle.json")
}

pub fn throttle_lock_path() -> PathBuf {
    dofustabs_runtime_dir().join("throttle.lock")
}

pub fn identify_lock_path() -> PathBuf {
    dofustabs_runtime_dir().join("identify.lock")
}

/// Timestamped backup path for `path`, following the project's convention:
/// `<file>.bak.<unix-timestamp>`.
pub fn backup_path(path: &std::path::Path) -> PathBuf {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".bak.{ts}"));
    PathBuf::from(name)
}

/// Copies `path` to a timestamped backup if it exists. No-op otherwise.
pub fn backup_if_exists(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() {
        std::fs::copy(path, backup_path(path))?;
    }
    Ok(())
}
