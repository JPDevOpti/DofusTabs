//! Launches the Ankama Launcher AppImage detached from this app, with the
//! NVIDIA PRIME render-offload environment so the game uses the dGPU.

use crate::{GameLauncher, PlatformError};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

/// Environment that makes GLX/Vulkan pick the NVIDIA GPU on a PRIME setup.
pub const PRIME_OFFLOAD_ENV: [(&str, &str); 4] = [
    ("__NV_PRIME_RENDER_OFFLOAD", "1"),
    ("__NV_PRIME_RENDER_OFFLOAD_PROVIDER", "NVIDIA-G0"),
    ("__GLX_VENDOR_LIBRARY_NAME", "nvidia"),
    ("__VK_LAYER_NV_optimus", "NVIDIA_only"),
];

/// Electron inside the AppImage needs this without a setuid chrome-sandbox.
const LAUNCHER_ARGS: [&str; 1] = ["--no-sandbox"];

pub struct LinuxGameLauncher;

impl LinuxGameLauncher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxGameLauncher {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds the detached command: no inherited stdio and its own process
/// group, so closing DofusTabs never takes the launcher down with it.
pub(crate) fn launch_command(launcher: &Path) -> Command {
    let mut command = Command::new(launcher);
    command
        .args(LAUNCHER_ARGS)
        .envs(PRIME_OFFLOAD_ENV)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    command
}

impl GameLauncher for LinuxGameLauncher {
    fn launch(&self, launcher: &Path) -> Result<(), PlatformError> {
        if !launcher.is_file() {
            return Err(PlatformError::InvalidData(format!(
                "launcher not found: {}",
                launcher.display()
            )));
        }
        launch_command(launcher).spawn().map_err(|e| {
            PlatformError::CommandFailed(format!("failed to start {}: {e}", launcher.display()))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn command_carries_prime_env_and_no_sandbox() {
        let command = launch_command(Path::new("/opt/Dofus.AppImage"));
        assert_eq!(command.get_program(), OsStr::new("/opt/Dofus.AppImage"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![OsStr::new("--no-sandbox")]
        );
        for (key, value) in PRIME_OFFLOAD_ENV {
            assert_eq!(
                command.get_envs().find(|(k, _)| *k == OsStr::new(key)),
                Some((OsStr::new(key), Some(OsStr::new(value))))
            );
        }
    }

    #[test]
    fn missing_launcher_is_reported_without_spawning() {
        let err = LinuxGameLauncher::new()
            .launch(Path::new("/nonexistent/dofustabs/Dofus.AppImage"))
            .unwrap_err();
        assert!(matches!(err, PlatformError::InvalidData(_)), "{err}");
    }
}
