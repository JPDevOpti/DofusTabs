//! OS-agnostic ports for window discovery, activation and GPU inspection,
//! plus the Linux (Hyprland) backend implementation.
//!
//! This crate is Hyprland-only: the Windows backend, the window-manager
//! stacking/grouping port and the FIFO/runtime hotkey backend that used to
//! live here only served the retired sidebar UI and were removed along with
//! it. See `README.md` for details.

use dofustabs_core::gpu::GpuKind;
use std::path::Path;
use thiserror::Error;

#[cfg(target_os = "linux")]
pub mod linux;

/// Opaque, backend-specific handle to a top-level window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowRef {
    WaylandAddress(String),
}

/// One running game client, as reported by a [`GameDiscovery`] backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredWindow {
    pub pid: u32,
    pub title: String,
    pub process_name: String,
    pub window_ref: WindowRef,
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("unsupported session: {0}")]
    UnsupportedSession(String),
    #[error("command failed: {0}")]
    CommandFailed(String),
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("unsupported on this platform: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Lists the game windows currently open on the system.
pub trait GameDiscovery {
    fn scan(&self) -> Result<Vec<DiscoveredWindow>, PlatformError>;
}

/// Brings a game window to the foreground.
pub trait WindowActivator {
    fn activate(&self, win: &WindowRef) -> Result<(), PlatformError>;
}

/// Tells which GPU a running process renders on.
pub trait GpuInspector {
    /// Best-effort classification for `pid`; `Unknown` when the process has
    /// not opened any render node yet or cannot be inspected.
    fn gpu_of(&self, pid: u32) -> Result<GpuKind, PlatformError>;
    /// Batched form of [`GpuInspector::gpu_of`] for `pids`; backends should
    /// override this to build any shared lookup (e.g. the DRM node to PCI
    /// map) once for the whole batch instead of once per pid.
    fn gpu_of_many(&self, pids: &[u32]) -> Vec<(u32, Result<GpuKind, PlatformError>)> {
        pids.iter().map(|&pid| (pid, self.gpu_of(pid))).collect()
    }
}

/// Starts the game launcher detached from this app, forcing the dedicated
/// GPU where the platform supports it.
pub trait GameLauncher {
    fn launch(&self, launcher: &Path) -> Result<(), PlatformError>;
}

/// True for process/class names that belong to a game client ("Dofus",
/// "Dofus.x64", "DofusTouch", ...) but not to this app itself.
pub fn is_game_process_name(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    n.starts_with("dofus") && !n.starts_with("dofustabs")
}

/// Everything a platform provides, built together by `*_backend()`.
pub struct BackendSet {
    pub discovery: Box<dyn GameDiscovery + Send>,
    pub activator: Box<dyn WindowActivator + Send>,
    pub gpu: Box<dyn GpuInspector + Send>,
    pub launcher: Box<dyn GameLauncher + Send>,
}

/// Builds the backend set for the current Linux session. Only Hyprland
/// (Wayland) is implemented; other sessions are reported as unsupported.
#[cfg(target_os = "linux")]
pub fn linux_backend() -> Result<BackendSet, PlatformError> {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".to_string());
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return Err(PlatformError::UnsupportedSession(format!(
            "session type '{session}' without Hyprland; only Hyprland (Wayland) is supported for now"
        )));
    }

    Ok(BackendSet {
        discovery: Box::new(linux::discovery::HyprlandDiscovery::new()),
        activator: Box::new(linux::activator::HyprlandActivator::new()),
        gpu: Box::new(linux::gpu::LinuxGpuInspector::new()),
        launcher: Box::new(linux::launcher::LinuxGameLauncher::new()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_process_names_match_by_prefix() {
        assert!(is_game_process_name("dofus"));
        assert!(is_game_process_name("Dofus.x64"));
        assert!(is_game_process_name("DofusTouch"));
        assert!(is_game_process_name("  Dofus  "));
    }

    #[test]
    fn non_game_and_own_process_names_do_not_match() {
        assert!(!is_game_process_name("firefox"));
        assert!(!is_game_process_name("dofustabs"));
        assert!(!is_game_process_name("DofusTabs"));
        assert!(!is_game_process_name(""));
    }
}
