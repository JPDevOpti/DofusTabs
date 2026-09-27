//! `dofustabs identify`: resolves the active character of every running
//! Dofus client and persists it to `~/.config/dofustabs/identity.json` for
//! the Hyprland module (stage 2) to read. Idempotent and safe to run
//! concurrently (short-lived exclusive lock).
//!
//! Resolution: for each running client, its account id and game release
//! come from `/proc/<pid>/cmdline`
//! (`dofustabs_core::client_identity::parse_cmdline`); its process start
//! time comes from `/proc/<pid>/stat` (`starttime`, converted with
//! `/proc/stat`'s `btime`); every `Characters/<hash>/dofus.json` under that
//! account+release is then scanned for its mtime, and the pure decision in
//! `dofustabs_core::identify::resolve_active_character` picks the one
//! touched most recently after the process started, if any.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use dofustabs_core::client_identity::parse_cmdline;
use dofustabs_core::identify::{resolve_active_character, CharacterCandidate};
use dofustabs_platform::linux::proc;

use crate::paths;

const LOCK_WAIT: Duration = Duration::from_secs(3);
const LOCK_POLL: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedAccount {
    pub character: String,
    pub release: String,
}

/// `identity.json`'s content: one entry per account whose active character
/// is currently known. An account with no entry means "not resolved yet"
/// (login/character-selection screen, or the client is not running).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    #[serde(default)]
    pub accounts: BTreeMap<String, ResolvedAccount>,
}

impl Identity {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// Writes both `identity.json` (this method's own format) and, at
    /// `lua_path`, a Lua-readable twin the Hyprland module `dofile`s:
    /// `return { ["<account>"] = { character = "<hash>", release = "..." } }`.
    /// Both writes are atomic (write to `.tmp`, rename into place).
    fn save(&self, path: &Path, lua_path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        fs::rename(&tmp, path)?;

        if let Some(dir) = lua_path.parent() {
            fs::create_dir_all(dir)?;
        }
        let lua_tmp = lua_path.with_extension("lua.tmp");
        fs::write(&lua_tmp, render_identity_lua(self))?;
        fs::rename(&lua_tmp, lua_path)?;
        Ok(())
    }

    pub fn active_character(&self, account_id: &str) -> Option<&str> {
        self.accounts.get(account_id).map(|r| r.character.as_str())
    }
}

fn render_identity_lua(identity: &Identity) -> String {
    let mut out = String::from("return {\n");
    for (account_id, resolved) in &identity.accounts {
        out.push_str(&format!(
            "  [\"{}\"] = {{ character = \"{}\", release = \"{}\" }},\n",
            crate::accounts::lua_escape(account_id),
            crate::accounts::lua_escape(&resolved.character),
            crate::accounts::lua_escape(&resolved.release),
        ));
    }
    out.push_str("}\n");
    out
}

/// A short-lived exclusive lock: backed by `create_new`, breaks a stale lock
/// after `LOCK_WAIT` since nothing here holds it longer than one `identify`
/// pass.
struct Lock {
    path: std::path::PathBuf,
}

impl Lock {
    fn acquire(path: &Path) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let deadline = Instant::now() + LOCK_WAIT;
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(_) => {
                    return Ok(Self {
                        path: path.to_path_buf(),
                    })
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if Instant::now() >= deadline {
                        let _ = fs::remove_file(path);
                        continue;
                    }
                    std::thread::sleep(LOCK_POLL);
                }
                Err(e) => return Err(e),
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct ClientReport {
    pid: u32,
    account_id: String,
    release: String,
    resolved: Option<String>,
}

pub fn run(dry_run: bool) -> anyhow::Result<()> {
    let _lock = if dry_run {
        None
    } else {
        Some(Lock::acquire(&paths::identify_lock_path())?)
    };

    let backend = dofustabs_platform::linux_backend()?;
    let windows = backend.discovery.scan()?;

    let mut new_map: BTreeMap<String, ResolvedAccount> = BTreeMap::new();
    let mut reports = Vec::new();

    for window in &windows {
        let report = match resolve_one(window.pid) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: pid {} could not be identified: {e}", window.pid);
                continue;
            }
        };
        if let Some(hash) = &report.resolved {
            new_map.insert(
                report.account_id.clone(),
                ResolvedAccount {
                    character: hash.clone(),
                    release: report.release.clone(),
                },
            );
        }
        reports.push(report);
    }

    let new_identity = Identity { accounts: new_map };

    if dry_run {
        print_reports(&reports);
        return Ok(());
    }

    let identity_path = paths::identity_json_path();
    let identity_lua_path = paths::identity_lua_path();
    let previous = Identity::load(&identity_path).unwrap_or_default();
    let changed = previous != new_identity;
    // Only write when the resolution actually changed: this runs every
    // ~10s while any client is up, and a rewrite-every-time would advance
    // identity.json/.lua's mtime constantly even when nothing changed,
    // which is misleading and needlessly touches the runtime tmpfs. A
    // no-op run is still cheap to observe via `log::debug!` if needed.
    if changed {
        new_identity.save(&identity_path, &identity_lua_path)?;
        log::debug!("identity changed, wrote {}", identity_path.display());
    } else {
        log::debug!("identify: no change");
    }
    print_reports(&reports);

    if changed {
        notify_hyprland();
    }
    Ok(())
}

fn resolve_one(pid: u32) -> anyhow::Result<ClientReport> {
    let args = proc::read_cmdline(pid)?;
    let identity = parse_cmdline(&args).ok_or_else(|| {
        anyhow::anyhow!("could not find account id / game release in /proc/{pid}/cmdline")
    })?;
    let process_start = proc::process_start_epoch_secs(pid)?;

    let release_upper = identity.release.to_uppercase();
    let dir = paths::characters_dir(&release_upper, &identity.account_id);

    let mut candidates_owned: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let hash_dir = entry.path();
            if !hash_dir.is_dir() {
                continue;
            }
            let hash = match entry.file_name().into_string() {
                Ok(h) => h,
                Err(_) => continue,
            };
            let dofus_json = hash_dir.join("dofus.json");
            let Ok(metadata) = fs::metadata(&dofus_json) else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let epoch = modified
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            candidates_owned.push((hash, epoch));
        }
    }
    let candidates: Vec<CharacterCandidate> = candidates_owned
        .iter()
        .map(|(hash, mtime)| CharacterCandidate {
            hash: hash.as_str(),
            mtime_epoch_secs: *mtime,
        })
        .collect();

    let resolved = resolve_active_character(process_start, &candidates).map(str::to_string);

    Ok(ClientReport {
        pid,
        account_id: identity.account_id,
        release: identity.release,
        resolved,
    })
}

fn print_reports(reports: &[ClientReport]) {
    if reports.is_empty() {
        println!("no running clients found");
        return;
    }
    println!(
        "{:<8} {:<14} {:<14} CHARACTER (resolved via file mtime > process start)",
        "PID", "ACCOUNT", "RELEASE"
    );
    for r in reports {
        match &r.resolved {
            Some(hash) => println!("{:<8} {:<14} {:<14} {hash}", r.pid, r.account_id, r.release),
            None => println!(
                "{:<8} {:<14} {:<14} not in game yet (no character file touched since client start)",
                r.pid, r.account_id, r.release
            ),
        }
    }
}

/// Best-effort `hyprctl eval 'dofustabs.refresh()'`, same idiom as the one
/// in `main.rs`. `refresh()` itself must never call back into `identify`
/// (the module is not wired to it yet in this stage), so this notification
/// direction stays one-way.
fn notify_hyprland() {
    let status = std::process::Command::new("hyprctl")
        .args(["eval", "dofustabs.refresh()"])
        .status();
    match status {
        Ok(s) if s.success() => {}
        _ => eprintln!(
            "hint: could not refresh the running module (is Hyprland running with dofustabs installed?); \
             identity.json was updated and will apply on the next window event or reload"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_roundtrips_through_json() {
        let mut identity = Identity::default();
        identity.accounts.insert(
            "190018816".to_string(),
            ResolvedAccount {
                character: "92d60605292e2c0cd060f461d152d096".to_string(),
                release: "experimental".to_string(),
            },
        );
        let json = serde_json::to_string(&identity).unwrap();
        let back: Identity = serde_json::from_str(&json).unwrap();
        assert_eq!(back, identity);
    }

    #[test]
    fn active_character_looks_up_by_account() {
        let mut identity = Identity::default();
        identity.accounts.insert(
            "1".to_string(),
            ResolvedAccount {
                character: "hash1".to_string(),
                release: "main".to_string(),
            },
        );
        assert_eq!(identity.active_character("1"), Some("hash1"));
        assert_eq!(identity.active_character("2"), None);
    }

    #[test]
    fn missing_identity_file_loads_as_empty() {
        let identity =
            Identity::load(Path::new("/nonexistent/dofustabs-test/identity.json")).unwrap();
        assert!(identity.accounts.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir =
            std::env::temp_dir().join(format!("dofustabs-identify-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("identity.json");
        let lua_path = dir.join("identity.lua");

        let mut identity = Identity::default();
        identity.accounts.insert(
            "1".to_string(),
            ResolvedAccount {
                character: "hash1".to_string(),
                release: "main".to_string(),
            },
        );
        identity.save(&path, &lua_path).unwrap();
        assert_eq!(Identity::load(&path).unwrap(), identity);
        let lua = fs::read_to_string(&lua_path).unwrap();
        assert_eq!(
            lua,
            "return {\n  [\"1\"] = { character = \"hash1\", release = \"main\" },\n}\n"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn render_identity_lua_escapes_values() {
        let mut identity = Identity::default();
        identity.accounts.insert(
            "1".to_string(),
            ResolvedAccount {
                character: "ha\"sh".to_string(),
                release: "main".to_string(),
            },
        );
        let lua = render_identity_lua(&identity);
        assert!(lua.contains(r#"character = "ha\"sh""#));
    }

    #[test]
    fn empty_identity_renders_empty_lua_table() {
        assert_eq!(render_identity_lua(&Identity::default()), "return {\n}\n");
    }
}
