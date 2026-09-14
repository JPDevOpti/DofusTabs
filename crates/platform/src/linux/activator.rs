use crate::{PlatformError, WindowActivator, WindowRef};

/// Activates game windows through Hyprland's focus dispatcher.
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

        // Hyprland >= 0.56 exposes dispatchers through the Lua `hl` API and
        // rejects the legacy `dispatch focuswindow` form; try Lua first and
        // fall back to the legacy dispatcher on older compositors.
        let lua = focus_lua(address);
        if let Ok(stdout) = super::run_hyprctl(&["eval", &lua]) {
            if stdout.trim() == "ok" {
                return Ok(());
            }
        }

        let target = format!("address:{address}");
        let stdout = super::run_hyprctl(&["dispatch", "focuswindow", &target])?;
        super::expect_ok(&format!("focuswindow {address}"), &stdout)
    }
}

fn focus_lua(address: &str) -> String {
    format!("hl.dispatch(hl.dsp.focus({{ window = \"address:{address}\" }}))")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_lua_targets_the_window_address() {
        assert_eq!(
            focus_lua("0x1234"),
            "hl.dispatch(hl.dsp.focus({ window = \"address:0x1234\" }))"
        );
    }
}
