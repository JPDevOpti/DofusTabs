//! GPU inspection is not implemented on Windows: every client is `Unknown`.

use crate::{GpuInspector, PlatformError};
use dofustabs_core::gpu::GpuKind;

pub struct WindowsGpuInspector;

impl GpuInspector for WindowsGpuInspector {
    fn gpu_of(&self, _: u32) -> Result<GpuKind, PlatformError> {
        Ok(GpuKind::Unknown)
    }
}
