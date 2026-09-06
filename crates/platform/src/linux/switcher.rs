use crate::{PlatformError, WindowActivator, WindowRef};

/// Activates game windows through `hyprctl dispatch focuswindow`.
pub struct HyprlandActivator;

impl HyprlandActivator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HyprlandActivator {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowActivator for HyprlandActivator {
    fn activate(&self, win: &WindowRef) -> Result<(), PlatformError> {
        let WindowRef::WaylandAddress(address) = win else {
            return Err(PlatformError::InvalidData(
                "Hyprland activator requires a Wayland address".to_string(),
            ));
        };

        let target = format!("address:{address}");
        let stdout = super::run_hyprctl(&["dispatch", "focuswindow", &target])?;
        super::expect_ok(&format!("focuswindow {address}"), &stdout)
    }
}
