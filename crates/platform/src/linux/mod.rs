pub mod activator;
pub mod discovery;
pub mod gpu;
pub mod launcher;
pub mod proc;
pub mod quit;

use crate::PlatformError;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
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
    run_command_with_timeout(Command::new("hyprctl").args(args), HYPRCTL_TIMEOUT)
        .map_err(|e| PlatformError::CommandFailed(format!("failed to spawn hyprctl: {e}")))
        .and_then(|result| finish_command(&format!("hyprctl {}", args.join(" ")), result))
}

/// Outcome of running a child to completion (or to a timeout): its raw
/// stdout/stderr bytes plus whether it exited successfully, or `None` for
/// `success` when it had to be killed after the deadline.
pub(crate) struct CommandOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    success: Option<bool>,
}

/// Spawns `command` with piped stdout/stderr, drains both concurrently on
/// reader threads while waiting for it to exit, and kills it if it is still
/// running after `timeout`.
///
/// Draining on separate threads (rather than polling `try_wait` while the
/// pipes fill up) matters once the child writes more than the OS pipe buffer
/// (typically ~64 KiB): without a reader, the child blocks on the second
/// `write()` and never reaches the deadline check, so `try_wait` alone would
/// hang until the timeout regardless of how much output was actually
/// produced.
pub(crate) fn run_command_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> std::io::Result<CommandOutput> {
    let mut child: Child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout_pipe = child.stdout.take().expect("stdout was piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr was piped");

    let stdout_reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let success = loop {
        match child.try_wait()? {
            Some(status) => break Some(status.success()),
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    };

    // The child has exited (or been killed), so both pipes are at EOF and
    // these joins return promptly.
    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();

    Ok(CommandOutput {
        stdout,
        stderr,
        success,
    })
}

fn finish_command(context: &str, result: CommandOutput) -> Result<String, PlatformError> {
    let stdout = String::from_utf8_lossy(&result.stdout).into_owned();
    match result.success {
        None => Err(PlatformError::CommandFailed(format!(
            "{context}: timed out after {HYPRCTL_TIMEOUT:?}"
        ))),
        Some(true) => Ok(stdout),
        Some(false) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            Err(PlatformError::CommandFailed(format!(
                "{context}: {} {}",
                stdout.trim(),
                stderr.trim()
            )))
        }
    }
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

    /// Regression test for a defect where polling `try_wait` without draining
    /// the piped stdout/stderr blocks a child writing more than the OS pipe
    /// buffer (~64 KiB) until the timeout, even though it would otherwise
    /// exit immediately. `run_command_with_timeout` is the factored helper
    /// used by both `run_hyprctl` and this test so the same code path is
    /// exercised with an arbitrary command instead of relying on `hyprctl`
    /// being installed.
    #[test]
    fn drains_large_output_without_blocking_on_the_timeout() {
        // Print more than 200 KiB to stdout: large enough to fill the
        // default pipe buffer several times over.
        let script = "python3 -c \"import sys; sys.stdout.write('x' * 220000)\" \
            || perl -e \"print 'x' x 220000\" \
            || yes x | head -c 220000";
        let mut command = Command::new("sh");
        command.arg("-c").arg(script);

        let result =
            run_command_with_timeout(&mut command, Duration::from_secs(10)).expect("spawn sh");

        assert_eq!(result.success, Some(true));
        assert!(
            result.stdout.len() >= 200_000,
            "expected large output, got {} bytes",
            result.stdout.len()
        );
    }
}
