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

    fn own_monitor_name(&self) -> Result<String, PlatformError> {
        Err(PlatformError::Unsupported("monitor lookup".to_string()))
    }

    fn set_left_reserved(&self, _: &str, _: i32) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported(
            "reserved area".to_string(),
        ))
    }

    fn group_bar_visible(&self) -> Result<bool, PlatformError> {
        Err(PlatformError::Unsupported("group bar".to_string()))
    }

    fn set_group_bar_visible(&self, _: bool) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("group bar".to_string()))
    }

    fn group_window(
        &self,
        _: &WindowRef,
        _: &str,
        _: Option<&WindowRef>,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("window grouping".to_string()))
    }
}
