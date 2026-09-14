//! Launching the game with a forced GPU is a Linux/PRIME feature for now.

use crate::{GameLauncher, PlatformError};
use std::path::Path;

pub struct WindowsGameLauncher;

impl GameLauncher for WindowsGameLauncher {
    fn launch(&self, _: &Path) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("game launcher".to_string()))
    }
}
