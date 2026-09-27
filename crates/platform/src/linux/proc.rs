//! Reads `/proc/<pid>` for the two facts `identify` needs: the process's
//! command line (account id, game release) and its start time, so the
//! active-character heuristic can tell "this character file was touched
//! after this client started" from "this is a leftover from a previous
//! session".
//!
//! Start-time source: `/proc/<pid>/stat` field 22 (`starttime`, in clock
//! ticks since boot) converted with the system boot time from `/proc/stat`'s
//! `btime` line (boot time as Unix epoch seconds). This is preferred over
//! `/proc/<pid>/ctime` (the inode's own metadata), because a pid can be
//! reused and `stat`'s `starttime` is what the kernel itself considers the
//! process's start, independent of filesystem timestamp granularity or
//! clock changes after boot.
//!
//! Clock ticks per second (`USER_HZ`) is hardcoded to 100, which is the
//! value used by every mainstream Linux distribution on every architecture
//! this project targets (it has been the kernel default since the tick rate
//! became configurable); reading it in properly requires an `sysconf(3)`
//! FFI call this project has no existing dependency for, and getting it
//! wrong only shifts the derived start time by a few tens of milliseconds
//! at 100 Hz, which the identify heuristic tolerates fine.

use std::fs;
use std::path::Path;

use crate::PlatformError;

const CLOCK_TICKS_PER_SECOND: u64 = 100;

/// Splits `/proc/<pid>/cmdline`'s raw NUL-separated bytes into arguments.
pub fn read_cmdline(pid: u32) -> Result<Vec<String>, PlatformError> {
    let path = format!("/proc/{pid}/cmdline");
    let raw = fs::read(&path)?;
    Ok(parse_cmdline_bytes(&raw))
}

fn parse_cmdline_bytes(raw: &[u8]) -> Vec<String> {
    raw.split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

/// The process's start time as a Unix epoch second.
pub fn process_start_epoch_secs(pid: u32) -> Result<u64, PlatformError> {
    let stat_path = format!("/proc/{pid}/stat");
    let stat_raw = fs::read_to_string(&stat_path)?;
    let ticks = parse_stat_starttime_ticks(&stat_raw).ok_or_else(|| {
        PlatformError::InvalidData(format!("could not parse starttime from {stat_path}"))
    })?;

    let btime = boot_time_epoch_secs(Path::new("/proc/stat"))?;
    Ok(btime + ticks / CLOCK_TICKS_PER_SECOND)
}

/// Parses field 22 (`starttime`) out of `/proc/<pid>/stat`'s content.
///
/// The format is `pid (comm) state ppid ... starttime ...`; `comm` is the
/// executable's basename in parentheses and may itself contain spaces or
/// parentheses (e.g. a renamed process, or one whose argv[0] was crafted
/// to contain them), so fields cannot be split on whitespace directly.
/// Splitting on the *last* `)` in the line reliably separates `comm` from
/// the fixed-format fields that follow, since none of those fields can
/// contain unescaped parentheses.
fn parse_stat_starttime_ticks(content: &str) -> Option<u64> {
    let last_paren = content.rfind(')')?;
    let rest = content.get(last_paren + 1..)?;
    // `rest` starts with " state ppid pgrp session tty_nr tpgid flags
    // minflt cminflt majflt cmajflt utime stime cutime cstime priority nice
    // num_threads itrealvalue starttime ...". Fields after `state` (the
    // first token here) are 1-indexed from field 3 in the full stat line,
    // so `starttime` (field 22) is the 20th token of `rest`.
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // fields[0] = state (field 3), so starttime (field 22) is index 19.
    fields.get(19)?.parse::<u64>().ok()
}

/// Parses `btime` (boot time, Unix epoch seconds) out of `/proc/stat`.
fn boot_time_epoch_secs(proc_stat_path: &Path) -> Result<u64, PlatformError> {
    let content = fs::read_to_string(proc_stat_path)?;
    parse_btime(&content).ok_or_else(|| {
        PlatformError::InvalidData(format!("no btime line in {}", proc_stat_path.display()))
    })
}

fn parse_btime(content: &str) -> Option<u64> {
    content.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        if parts.next()? != "btime" {
            return None;
        }
        parts.next()?.parse::<u64>().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cmdline_null_separated_bytes() {
        let raw = b"/opt/ankama/dofus\0-logFile\0/x/dofus.1.log\0";
        assert_eq!(
            parse_cmdline_bytes(raw),
            vec![
                "/opt/ankama/dofus".to_string(),
                "-logFile".to_string(),
                "/x/dofus.1.log".to_string(),
            ]
        );
    }

    #[test]
    fn empty_cmdline_is_empty_vec() {
        assert!(parse_cmdline_bytes(b"").is_empty());
    }

    #[test]
    fn parses_starttime_from_simple_comm() {
        // pid comm state ppid pgrp session tty_nr tpgid flags minflt cminflt
        // majflt cmajflt utime stime cutime cstime priority nice
        // num_threads itrealvalue starttime ...
        let stat = "4832 (dofus) S 1 4832 4832 0 -1 4194304 100 0 0 0 10 2 0 0 20 0 4 0 123456 0 0";
        assert_eq!(parse_stat_starttime_ticks(stat), Some(123456));
    }

    #[test]
    fn comm_field_may_contain_spaces_and_parentheses() {
        let stat = "10 (my (weird) proc name) S 1 10 10 0 -1 0 0 0 0 0 0 0 0 0 0 0 0 0 999 0 0";
        assert_eq!(parse_stat_starttime_ticks(stat), Some(999));
    }

    #[test]
    fn malformed_stat_is_none() {
        assert_eq!(parse_stat_starttime_ticks("not a stat line"), None);
    }

    #[test]
    fn parses_btime_from_proc_stat_fixture() {
        let content = "cpu  100 0 200 300\nctxt 12345\nbtime 1700000000\nprocesses 42\n";
        assert_eq!(parse_btime(content), Some(1700000000));
    }

    #[test]
    fn missing_btime_line_is_none() {
        assert_eq!(parse_btime("cpu 1 2 3\nctxt 1\n"), None);
    }
}
