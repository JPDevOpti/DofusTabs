pub mod activator;
pub mod discovery;
pub mod gpu;
pub mod hotkeys;
pub mod launcher;
pub mod window_manager;

use crate::PlatformError;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long a single `hyprctl` invocation is given before it is killed and
/// reported as failed. `hyprctl` normally answers in a few milliseconds; a
/// compositor wedged or a stalled IPC socket must not hang the watcher or
/// hotkey threads forever.
const HYPRCTL_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Runs `hyprctl` with the given arguments and returns its stdout. Bounded by
/// [`HYPRCTL_TIMEOUT`]: the child is polled non-blockingly and killed if it
/// has not finished by the deadline.
pub(crate) fn run_hyprctl(args: &[&str]) -> Result<String, PlatformError> {
    let mut child = Command::new("hyprctl")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PlatformError::CommandFailed(format!("failed to spawn hyprctl: {e}")))?;

    let deadline = Instant::now() + HYPRCTL_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(PlatformError::CommandFailed(format!(
                        "hyprctl {}: timed out after {HYPRCTL_TIMEOUT:?}",
                        args.join(" ")
                    )));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(e) => {
                return Err(PlatformError::CommandFailed(format!(
                    "failed to wait for hyprctl: {e}"
                )))
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| PlatformError::CommandFailed(format!("failed to read hyprctl output: {e}")))?;

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

/// Escapes a string for interpolation into a Lua double-quoted literal
/// (backslash, double quote, newline). Window titles and classes are
/// attacker-controlled in the sense that a game client picks its own window
/// title, so every string embedded in a Lua chunk handed to `hyprctl eval`
/// must go through this first.
pub(crate) fn lua_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_string_escapes_backslashes_quotes_and_newlines() {
        assert_eq!(lua_string("plain"), "plain");
        assert_eq!(lua_string(r#"say "hi""#), r#"say \"hi\""#);
        assert_eq!(lua_string(r"C:\path"), r"C:\\path");
        assert_eq!(lua_string("line1\nline2"), "line1\\nline2");
    }
}
