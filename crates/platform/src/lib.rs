//! OS-agnostic ports for window discovery, activation, and global hotkeys,
//! plus per-OS backend implementations behind `cfg` gates.

use dofustabs_core::settings::Hotkey;
use thiserror::Error;

#[cfg(target_os = "linux")]
pub mod linux;
pub mod win_keymap;
#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use windows::windows_backend;

/// Opaque, backend-specific handle to a top-level window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowRef {
    Hwnd(isize),
    WaylandAddress(String),
    X11Window(u64),
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
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Identifies which registered hotkey fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyId {
    Next,
    Previous,
    Primary,
    Instance(u32),
}

impl HotkeyId {
    /// Stable textual form used on IPC transports (e.g. the hotkey FIFO).
    pub fn wire_name(&self) -> String {
        match self {
            HotkeyId::Next => "next".to_string(),
            HotkeyId::Previous => "previous".to_string(),
            HotkeyId::Primary => "primary".to_string(),
            HotkeyId::Instance(pid) => format!("instance:{pid}"),
        }
    }

    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            "next" => Some(Self::Next),
            "previous" => Some(Self::Previous),
            "primary" => Some(Self::Primary),
            _ => s.strip_prefix("instance:")?.parse().ok().map(Self::Instance),
        }
    }
}

/// Lists the game windows currently open on the system.
pub trait GameDiscovery {
    fn scan(&self) -> Result<Vec<DiscoveredWindow>, PlatformError>;
}

/// Brings a game window to the foreground.
pub trait WindowActivator {
    fn activate(&self, win: &WindowRef) -> Result<(), PlatformError>;
}

/// Callback invoked (from a backend-owned thread) when a hotkey fires.
pub type HotkeyHandler = Box<dyn Fn(HotkeyId) + Send + Sync>;

/// Registers system-wide hotkeys. Events are delivered to the handler set via
/// [`HotkeyBackend::set_handler`]; the handler runs on a backend thread, so it
/// must hand work off to the app's own event loop.
pub trait HotkeyBackend {
    /// Registering an empty hotkey only clears any existing binding for `id`.
    fn register(&mut self, id: HotkeyId, hotkey: &Hotkey) -> Result<(), PlatformError>;
    fn unregister(&mut self, id: HotkeyId) -> Result<(), PlatformError>;
    fn set_handler(&mut self, handler: HotkeyHandler);
    /// Temporarily releases every registered combination at the OS level so
    /// the keys reach the focused app again (used while the UI captures a new
    /// hotkey); `false` restores them. Registrations made while suspended are
    /// remembered and applied on resume.
    fn set_suspended(&mut self, suspended: bool) -> Result<(), PlatformError>;
}

/// True for process/class names that belong to a game client ("Dofus",
/// "Dofus.x64", "DofusTouch", ...) but not to this app itself.
pub fn is_game_process_name(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    n.starts_with("dofus") && !n.starts_with("dofustabs")
}

/// Builds the backend set for the current Linux session. Only Hyprland
/// (Wayland) is implemented; other sessions are reported as unsupported.
#[cfg(target_os = "linux")]
#[allow(clippy::type_complexity)]
pub fn linux_backend() -> Result<
    (
        Box<dyn GameDiscovery + Send>,
        Box<dyn WindowActivator + Send>,
        Box<dyn HotkeyBackend + Send>,
    ),
    PlatformError,
> {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".to_string());
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return Err(PlatformError::UnsupportedSession(format!(
            "session type '{session}' without Hyprland; only Hyprland (Wayland) is supported for now"
        )));
    }

    Ok((
        Box::new(linux::discovery::HyprlandDiscovery::new()),
        Box::new(linux::switcher::HyprlandActivator::new()),
        Box::new(linux::hotkeys::HyprlandHotkeys::new()?),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_roundtrip() {
        for id in [
            HotkeyId::Next,
            HotkeyId::Previous,
            HotkeyId::Primary,
            HotkeyId::Instance(4832),
        ] {
            assert_eq!(HotkeyId::from_wire(&id.wire_name()), Some(id));
        }
    }

    #[test]
    fn invalid_wire_names_are_rejected() {
        assert_eq!(HotkeyId::from_wire(""), None);
        assert_eq!(HotkeyId::from_wire("nexT"), None);
        assert_eq!(HotkeyId::from_wire("instance:"), None);
        assert_eq!(HotkeyId::from_wire("instance:abc"), None);
    }

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
