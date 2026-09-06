pub mod discovery;
pub mod hotkeys;
pub mod switcher;

use crate::PlatformError;
use std::process::Command;

/// Runs `hyprctl` with the given arguments and returns its stdout.
pub(crate) fn run_hyprctl(args: &[&str]) -> Result<String, PlatformError> {
    let output = Command::new("hyprctl")
        .args(args)
        .output()
        .map_err(|e| PlatformError::CommandFailed(format!("failed to spawn hyprctl: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PlatformError::CommandFailed(format!(
            "hyprctl {}: {} {}",
            args.join(" "),
            stdout.trim(),
            stderr.trim()
        )));
    }
    Ok(stdout)
}

/// hyprctl exits 0 even when a request fails; success is reported as "ok" on
/// stdout, so callers must check the body too.
pub(crate) fn expect_ok(context: &str, stdout: &str) -> Result<(), PlatformError> {
    if stdout.trim().eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(PlatformError::CommandFailed(format!(
            "{context}: {}",
            stdout.trim()
        )))
    }
}
