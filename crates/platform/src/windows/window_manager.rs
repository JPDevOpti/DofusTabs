//! Window placement is not implemented on Windows yet: preparation and
//! raising are harmless no-ops, stacking reports itself as unsupported.

use crate::{PlatformError, WindowManager, WindowRef};
use dofustabs_core::geometry::Rect;

pub struct WindowsWindowManager;

impl WindowManager for WindowsWindowManager {
    fn prepare_own_window(&self) -> Result<WindowRef, PlatformError> {
        Err(PlatformError::Unsupported(
            "own window preparation".to_string(),
        ))
    }

    fn stack_window(&self, _: &WindowRef, _: &str, _: Rect) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("window stacking".to_string()))
    }

    fn raise(&self, _: &WindowRef) -> Result<(), PlatformError> {
        Ok(())
    }

    fn own_workspace_and_area(&self) -> Result<(String, Rect), PlatformError> {
        Err(PlatformError::Unsupported("workspace lookup".to_string()))
    }
}
