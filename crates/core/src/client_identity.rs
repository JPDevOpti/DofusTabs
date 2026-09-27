//! Pure parsing of a Dofus client's command line into the account id and
//! game release it belongs to.
//!
//! Verified fact: the command line contains
//! `-logFile .../gamesLogs/dofus-<release>/dofus.<accountId>.log` and
//! `--gameRelease <release>`. Both are parsed here so callers (which read
//! `/proc/<pid>/cmdline`) never touch string parsing directly.

/// Account id and release extracted from a client's argv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    pub account_id: String,
    pub release: String,
}

/// Parses `args` (as split from `/proc/<pid>/cmdline`, one argument per
/// element) into a [`ClientIdentity`]. Returns `None` when either the
/// account id or the release cannot be found, since a partial identity is
/// not actionable.
pub fn parse_cmdline(args: &[String]) -> Option<ClientIdentity> {
    let release = find_game_release(args)?;
    let account_id = find_account_id(args)?;
    Some(ClientIdentity {
        account_id,
        release,
    })
}

/// Looks for `--gameRelease <value>` (as two separate argv entries, or as
/// `--gameRelease=<value>` in one).
fn find_game_release(args: &[String]) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        if let Some(v) = arg.strip_prefix("--gameRelease=") {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
        if arg == "--gameRelease" {
            if let Some(v) = args.get(i + 1) {
                if !v.is_empty() {
                    return Some(v.clone());
                }
            }
        }
    }
    None
}

/// Extracts the account id out of the `-logFile
/// .../gamesLogs/dofus-<release>/dofus.<accountId>.log` argument.
fn find_account_id(args: &[String]) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        let candidate = if let Some(v) = arg.strip_prefix("-logFile=") {
            Some(v.to_string())
        } else if arg == "-logFile" {
            args.get(i + 1).cloned()
        } else {
            None
        };
        if let Some(path) = candidate {
            if let Some(id) = account_id_from_log_path(&path) {
                return Some(id);
            }
        }
    }
    None
}

/// `.../dofus.<accountId>.log` -> `<accountId>`.
fn account_id_from_log_path(path: &str) -> Option<String> {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let rest = file_name.strip_prefix("dofus.")?;
    let id = rest.strip_suffix(".log")?;
    if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
        Some(id.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_separate_arguments() {
        let a = args(&[
            "/opt/ankama/dofus",
            "-logFile",
            "/home/u/.config/gamesLogs/dofus-experimental/dofus.190018816.log",
            "--gameRelease",
            "experimental",
        ]);
        let id = parse_cmdline(&a).unwrap();
        assert_eq!(id.account_id, "190018816");
        assert_eq!(id.release, "experimental");
    }

    #[test]
    fn parses_equals_form() {
        let a = args(&[
            "/opt/ankama/dofus",
            "-logFile=/home/u/.config/gamesLogs/dofus-main/dofus.190231219.log",
            "--gameRelease=main",
        ]);
        let id = parse_cmdline(&a).unwrap();
        assert_eq!(id.account_id, "190231219");
        assert_eq!(id.release, "main");
    }

    #[test]
    fn missing_release_is_none() {
        let a = args(&["-logFile", "/x/gamesLogs/dofus-main/dofus.190018816.log"]);
        assert!(parse_cmdline(&a).is_none());
    }

    #[test]
    fn missing_log_file_is_none() {
        let a = args(&["--gameRelease", "main"]);
        assert!(parse_cmdline(&a).is_none());
    }

    #[test]
    fn non_numeric_account_id_is_rejected() {
        let a = args(&[
            "-logFile",
            "/x/gamesLogs/dofus-main/dofus.notanumber.log",
            "--gameRelease",
            "main",
        ]);
        assert!(parse_cmdline(&a).is_none());
    }

    #[test]
    fn empty_args_is_none() {
        assert!(parse_cmdline(&[]).is_none());
    }
}
