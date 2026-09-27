//! Pure decision logic for `dofustabs quit`: given which pids are still
//! alive after each wait step, decide which signal goes to whom next.
//!
//! This module has no I/O: callers in `cli` gather the pids, wait, re-check
//! liveness through `platform`, and apply the resulting signals.

/// One escalation step: which pids to signal, and with what.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Term,
    Kill,
}

/// Given the pids that were asked to close (via a window-close request) and
/// which of them are still alive, decide the next signal step: `SIGTERM` for
/// every survivor. Pids not in `still_alive` need nothing further.
pub fn escalate_to_term(still_alive: &[u32]) -> Vec<(u32, Signal)> {
    still_alive.iter().map(|&pid| (pid, Signal::Term)).collect()
}

/// Given the pids that were sent `SIGTERM` and which of them are still
/// alive, decide the final step: `SIGKILL` for every survivor.
pub fn escalate_to_kill(still_alive: &[u32]) -> Vec<(u32, Signal)> {
    still_alive.iter().map(|&pid| (pid, Signal::Kill)).collect()
}

/// Summarizes one escalation pass over `all_pids`: how many needed no
/// signal beyond the initial close request, how many needed `SIGTERM`, and
/// how many needed `SIGKILL`. `still_alive_after_close` and
/// `still_alive_after_term` are the liveness snapshots taken after each
/// wait; both must be subsets of `all_pids`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EscalationSummary {
    pub closed_cleanly: usize,
    pub needed_term: usize,
    pub needed_kill: usize,
}

pub fn summarize(
    all_pids: &[u32],
    still_alive_after_close: &[u32],
    still_alive_after_term: &[u32],
) -> EscalationSummary {
    let after_close: std::collections::BTreeSet<u32> =
        still_alive_after_close.iter().copied().collect();
    let after_term: std::collections::BTreeSet<u32> =
        still_alive_after_term.iter().copied().collect();

    let total = all_pids.len();
    let needed_kill = after_term.len();
    let needed_term = after_close.len().saturating_sub(needed_kill);
    let closed_cleanly = total.saturating_sub(after_close.len());

    EscalationSummary {
        closed_cleanly,
        needed_term,
        needed_kill,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_exit_after_window_close() {
        let summary = summarize(&[1, 2, 3], &[], &[]);
        assert_eq!(
            summary,
            EscalationSummary {
                closed_cleanly: 3,
                needed_term: 0,
                needed_kill: 0,
            }
        );
        assert!(escalate_to_term(&[]).is_empty());
    }

    #[test]
    fn some_need_sigterm() {
        let summary = summarize(&[1, 2, 3], &[2, 3], &[]);
        assert_eq!(
            summary,
            EscalationSummary {
                closed_cleanly: 1,
                needed_term: 2,
                needed_kill: 0,
            }
        );
        assert_eq!(
            escalate_to_term(&[2, 3]),
            vec![(2, Signal::Term), (3, Signal::Term)]
        );
    }

    #[test]
    fn some_need_sigkill() {
        let summary = summarize(&[1, 2, 3], &[1, 2, 3], &[2, 3]);
        assert_eq!(
            summary,
            EscalationSummary {
                closed_cleanly: 0,
                needed_term: 1,
                needed_kill: 2,
            }
        );
        assert_eq!(
            escalate_to_kill(&[2, 3]),
            vec![(2, Signal::Kill), (3, Signal::Kill)]
        );
    }

    #[test]
    fn nothing_running_is_a_no_op() {
        let summary = summarize(&[], &[], &[]);
        assert_eq!(summary, EscalationSummary::default());
    }
}
