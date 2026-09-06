//! Win32 backend: EnumWindows discovery, foreground activation, and
//! RegisterHotKey global hotkeys.
//!
//! This version is a switcher, like the Linux backend — the SetParent-based
//! embedding of the original C# app (with window-style snapshots) is a
//! possible future Windows-only feature, not ported here.

pub mod activator;
pub mod discovery;
pub mod hotkeys;

use crate::{GameDiscovery, HotkeyBackend, PlatformError, WindowActivator};

/// Builds the backend set for Windows.
#[allow(clippy::type_complexity)]
pub fn windows_backend() -> Result<
    (
        Box<dyn GameDiscovery + Send>,
        Box<dyn WindowActivator + Send>,
        Box<dyn HotkeyBackend + Send>,
    ),
    PlatformError,
> {
    Ok((
        Box::new(discovery::WindowsDiscovery::new()),
        Box::new(activator::WindowsActivator::new()),
        Box::new(hotkeys::WindowsHotkeys::new()?),
    ))
}
