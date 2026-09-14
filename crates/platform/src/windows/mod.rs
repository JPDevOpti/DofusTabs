//! Win32 backend: EnumWindows discovery, foreground activation, and
//! RegisterHotKey global hotkeys.
//!
//! This version is a switcher, like the Linux backend — the SetParent-based
//! embedding of the original C# app (with window-style snapshots) is a
//! possible future Windows-only feature, not ported here.

pub mod activator;
pub mod discovery;
pub mod gpu;
pub mod hotkeys;
pub mod launcher;
pub mod window_manager;

use crate::BackendSet;
use gpu::WindowsGpuInspector;
use launcher::WindowsGameLauncher;
use window_manager::WindowsWindowManager;

use crate::PlatformError;

/// Builds the backend set for Windows.
pub fn windows_backend() -> Result<BackendSet, PlatformError> {
    Ok(BackendSet {
        discovery: Box::new(discovery::WindowsDiscovery::new()),
        activator: Box::new(activator::WindowsActivator::new()),
        hotkeys: Box::new(hotkeys::WindowsHotkeys::new()?),
        window_manager: Box::new(WindowsWindowManager),
        gpu: Box::new(WindowsGpuInspector),
        launcher: Box::new(WindowsGameLauncher),
    })
}
