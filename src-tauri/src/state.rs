//! Composition root: [`AppState`] (the mutable app model), [`SharedState`]
//! (its mutex wrapper), and [`PlatformHandles`] (the live platform backends).
//! The instance model, reconciliation logic and IPC view DTOs live in
//! [`crate::instances`] and [`crate::views`]; this module re-exports them so
//! existing `use crate::state::X` paths keep compiling.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use dofustabs_core::geometry::{Rect, SIDEBAR_WIDTH};
use dofustabs_core::rotation::ActivationQueue;
use dofustabs_core::settings::AppSettings;
use dofustabs_platform::{
    GameDiscovery, GameLauncher, GpuInspector, HotkeyBackend, WindowActivator, WindowManager,
};

#[allow(unused_imports)]
pub use crate::instances::{sync_instances, SyncOutcome, TrackedInstance};
pub use crate::views::{HotkeyView, InstanceView, LayoutConstantsView, SettingsView};
pub use crate::window_layout::LayoutState;

pub struct AppState {
    pub instances: Vec<TrackedInstance>,
    pub active_pid: Option<u32>,
    pub settings: AppSettings,
    pub queue: ActivationQueue,
    pub settings_path: PathBuf,
    /// User's home directory, the base for the default launcher path.
    pub home_dir: PathBuf,
    pub layout: LayoutState,
}

/// Wraps [`AppState`] behind a mutex managed by Tauri.
pub struct SharedState(pub Mutex<AppState>);

impl SharedState {
    /// Locks the state, recovering from poisoning instead of panicking.
    ///
    /// A panic while holding the lock (e.g. inside a command handler) would
    /// otherwise poison the mutex and take down every future lock attempt,
    /// including from the watcher thread. The app's state is plain data with
    /// no invariants that a panic mid-mutation could leave subtly broken in a
    /// way that matters here, so recovering the guard and carrying on is
    /// preferable to a cascading failure.
    pub fn lock(&self) -> MutexGuard<'_, AppState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Platform handles live outside the state mutex; lock order is always
/// state first, then platform, never the reverse.
#[derive(Default)]
pub struct PlatformHandles {
    pub discovery: Mutex<Option<Box<dyn GameDiscovery + Send>>>,
    pub activator: Mutex<Option<Box<dyn WindowActivator + Send>>>,
    pub hotkeys: Mutex<Option<Box<dyn HotkeyBackend + Send>>>,
    pub window_manager: Mutex<Option<Box<dyn WindowManager + Send>>>,
    pub gpu: Mutex<Option<Box<dyn GpuInspector + Send>>>,
    pub launcher: Mutex<Option<Box<dyn GameLauncher + Send>>>,
}

impl AppState {
    pub fn new(settings: AppSettings, settings_path: PathBuf, home_dir: PathBuf) -> Self {
        Self {
            instances: Vec::new(),
            active_pid: None,
            settings,
            queue: ActivationQueue::new(),
            settings_path,
            home_dir,
            layout: LayoutState::default(),
        }
    }

    /// Launcher the "launch game" action starts: the configured path, or
    /// the default AppImage location under the user's home.
    pub fn launcher_path(&self) -> PathBuf {
        self.settings.launcher_path_or_default(&self.home_dir)
    }

    /// Stores the launcher path; blank input clears the override so the
    /// default applies again.
    pub fn set_launcher_path(&mut self, path: Option<String>) {
        self.settings.launcher_path = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    }

    /// Workspace and area the game windows are stacked into: the settings
    /// override when set (used verbatim), otherwise the monitor area resolved
    /// at startup minus the switcher column on its left edge. `None` until
    /// the own window has been prepared.
    pub fn game_target(&self) -> Option<(String, Rect)> {
        let workspace = self
            .settings
            .game_workspace
            .clone()
            .or_else(|| self.layout.startup_workspace.clone())?;
        let area = self
            .settings
            .game_area
            .or_else(|| Some(self.layout.monitor_area?.inset_left(SIDEBAR_WIDTH)))?;
        Some((workspace, area))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_defaults() -> AppState {
        AppState::new(
            AppSettings::default(),
            PathBuf::from("/tmp/unused.json"),
            PathBuf::from("/home/ana"),
        )
    }

    #[test]
    fn launcher_path_falls_back_to_default_and_blank_clears_override() {
        let mut state = state_with_defaults();
        assert_eq!(
            state.launcher_path(),
            PathBuf::from("/home/ana/Applications/Dofus.AppImage")
        );
        assert_eq!(
            state.settings_view().launcher_path,
            "/home/ana/Applications/Dofus.AppImage"
        );

        state.set_launcher_path(Some("  /opt/Dofus.AppImage ".to_string()));
        assert_eq!(
            state.settings.launcher_path.as_deref(),
            Some("/opt/Dofus.AppImage")
        );
        assert_eq!(state.launcher_path(), PathBuf::from("/opt/Dofus.AppImage"));

        state.set_launcher_path(Some("   ".to_string()));
        assert_eq!(state.settings.launcher_path, None);
        state.set_launcher_path(None);
        assert_eq!(
            state.launcher_path(),
            PathBuf::from("/home/ana/Applications/Dofus.AppImage")
        );
    }

    #[test]
    fn game_target_prefers_settings_over_startup_values() {
        let mut state = state_with_defaults();
        assert_eq!(state.game_target(), None);

        state.layout.startup_workspace = Some("2".to_string());
        state.layout.monitor_area = Some(Rect::new(0, 0, 1920, 1080));
        // The startup area reserves the switcher column on the left.
        assert_eq!(
            state.game_target(),
            Some((
                "2".to_string(),
                Rect::new(SIDEBAR_WIDTH, 0, 1920 - SIDEBAR_WIDTH, 1080)
            ))
        );

        state.settings.game_workspace = Some("5".to_string());
        state.settings.game_area = Some(Rect::new(10, 10, 100, 100));
        assert_eq!(
            state.game_target(),
            Some(("5".to_string(), Rect::new(10, 10, 100, 100)))
        );
    }
}
