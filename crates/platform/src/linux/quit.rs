//! Backend for `dofustabs quit`: discovers game clients and launcher
//! processes, asks the compositor to close game windows, and sends signals.
//!
//! Only ever touches processes owned by the calling user whose `comm` is
//! exactly `Dofus.x64`, `zaap` or `Dofus.AppImage` (never a pattern match).

use std::fs;
use std::process::Command;
use std::time::Duration;

use crate::linux::discovery::HyprlandDiscovery;
use crate::{GameDiscovery, PlatformError};

use super::run_command_with_timeout;

const KILL_TIMEOUT: Duration = Duration::from_secs(5);

/// One running game client this command may close: its pid and the
/// compositor address of its window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameClient {
    pub pid: u32,
    pub address: String,
}

/// The launcher's processes, split as `quit` needs them: the one main
/// `zaap` process (the one whose parent is not itself `zaap`), every other
/// `zaap` process (zygote/gpu/renderer/utility children), and the
/// `Dofus.AppImage` process(es).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LauncherProcesses {
    pub main_zaap: Option<u32>,
    pub other_zaap: Vec<u32>,
    pub appimage: Vec<u32>,
}

impl LauncherProcesses {
    /// Every launcher pid, main first.
    pub fn all_pids(&self) -> Vec<u32> {
        let mut pids: Vec<u32> = self.main_zaap.into_iter().collect();
        pids.extend(&self.other_zaap);
        pids.extend(&self.appimage);
        pids
    }
}

/// Every currently running game client, via the same Hyprland window
/// discovery `identify`/placement use (already filtered to game classes
/// with a valid pid).
pub fn discover_game_clients() -> Result<Vec<GameClient>, PlatformError> {
    let windows = HyprlandDiscovery::new().scan()?;
    Ok(windows
        .into_iter()
        .map(|w| {
            let crate::WindowRef::WaylandAddress(address) = w.window_ref;
            GameClient {
                pid: w.pid,
                address,
            }
        })
        .collect())
}

/// Asks the compositor to close the window at `address` (equivalent to the
/// user closing it themselves); does not wait or escalate.
pub fn request_close_window(address: &str) -> Result<(), PlatformError> {
    // With a Lua config, `hyprctl dispatch` evaluates a Lua expression; the
    // legacy `closewindow address:...` form is a syntax error there.
    let target = super::lua_string(&format!("address:{address}"));
    let expr = format!("hl.dsp.window.close({{ window = \"{target}\" }})");
    let stdout = super::run_hyprctl(&["dispatch", &expr])?;
    super::expect_ok(&format!("close window {address}"), &stdout)
}

/// The launcher's `zaap`/`Dofus.AppImage` processes owned by `uid`, split
/// into main/other/appimage.
pub fn discover_launcher_processes(uid: u32) -> LauncherProcesses {
    let zaap_pids = pids_by_comm_and_uid("zaap", uid);
    let appimage = pids_by_comm_and_uid("Dofus.AppImage", uid);

    let mut main_zaap = None;
    let mut other_zaap = Vec::new();
    for pid in zaap_pids {
        let parent_is_zaap = read_ppid(pid)
            .and_then(read_comm)
            .map(|c| c == "zaap")
            .unwrap_or(false);
        if parent_is_zaap || main_zaap.is_some() {
            other_zaap.push(pid);
        } else {
            main_zaap = Some(pid);
        }
    }

    LauncherProcesses {
        main_zaap,
        other_zaap,
        appimage,
    }
}

/// True when `/proc/<pid>` still exists.
pub fn pid_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

/// Sends `SIGTERM` or `SIGKILL` to `pid` via the `kill` binary (no `libc`
/// dependency in this workspace). A pid that has already exited is treated
/// as success, not an error.
pub fn send_signal(pid: u32, sig: &str) -> Result<(), PlatformError> {
    if !pid_alive(pid) {
        return Ok(());
    }
    let mut command = Command::new("kill");
    command.args([sig, &pid.to_string()]);
    let result = run_command_with_timeout(&mut command, KILL_TIMEOUT)
        .map_err(|e| PlatformError::CommandFailed(format!("failed to spawn kill: {e}")))?;
    match result.success {
        Some(true) | None => Ok(()),
        Some(false) if !pid_alive(pid) => Ok(()), // exited between the check and the signal
        Some(false) => Err(PlatformError::CommandFailed(format!(
            "kill {sig} {pid}: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ))),
    }
}

fn pids_by_comm_and_uid(comm: &str, uid: u32) -> Vec<u32> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut pids = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if read_comm(pid).as_deref() == Some(comm) && process_uid(pid) == Some(uid) {
            pids.push(pid);
        }
    }
    pids
}

fn read_comm(pid: u32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_ppid(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let last_paren = stat.rfind(')')?;
    let rest = stat.get(last_paren + 1..)?;
    // rest = " state ppid ...": ppid is the second token.
    rest.split_whitespace().nth(1)?.parse().ok()
}

fn process_uid(pid: u32) -> Option<u32> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    status
        .lines()
        .find(|l| l.starts_with("Uid:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
}

/// The real uid of this process.
pub fn current_uid() -> u32 {
    process_uid(std::process::id()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_processes_all_pids_orders_main_first() {
        let p = LauncherProcesses {
            main_zaap: Some(1),
            other_zaap: vec![2, 3],
            appimage: vec![4],
        };
        assert_eq!(p.all_pids(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn dead_pid_is_not_alive() {
        assert!(!pid_alive(u32::MAX));
    }

    #[test]
    fn sending_signal_to_dead_pid_is_ok() {
        assert!(send_signal(u32::MAX, "TERM").is_ok());
    }
}
