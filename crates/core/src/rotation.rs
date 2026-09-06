/// Resolves the next process to activate when navigating by `delta` steps
/// through `enabled_ordered`. Mirrors the original TryDequeueActivationTarget
/// arithmetic: an unknown/absent active pid selects the first entry, and a
/// delta that is a whole number of laps selects nothing.
pub fn resolve_navigation(enabled_ordered: &[u32], active: Option<u32>, delta: i64) -> Option<u32> {
    if enabled_ordered.is_empty() {
        return None;
    }

    let len = enabled_ordered.len() as i64;
    let idx = active.and_then(|pid| enabled_ordered.iter().position(|&p| p == pid));
    let Some(idx) = idx else {
        return Some(enabled_ordered[0]);
    };

    let steps = delta % len;
    if steps == 0 {
        return None;
    }

    let target = (idx as i64 + steps).rem_euclid(len);
    Some(enabled_ordered[target as usize])
}

/// Coalesces bursts of hotkey requests: rapid navigation presses accumulate
/// into one delta, and an explicit activation supersedes pending navigation.
#[derive(Debug, Default)]
pub struct ActivationQueue {
    pending_delta: i64,
    pending_pid: Option<u32>,
}

impl ActivationQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_navigation(&mut self, delta: i64) {
        self.pending_delta += delta;
        self.pending_pid = None;
    }

    pub fn push_activation(&mut self, pid: u32) {
        self.pending_pid = Some(pid);
        self.pending_delta = 0;
    }

    pub fn has_pending(&self) -> bool {
        self.pending_pid.is_some() || self.pending_delta != 0
    }

    /// Consumes the pending request and resolves its target, if any.
    /// An explicit pid that is no longer enabled is consumed and yields None.
    pub fn take_target(&mut self, enabled_ordered: &[u32], active: Option<u32>) -> Option<u32> {
        if let Some(pid) = self.pending_pid.take() {
            self.pending_delta = 0;
            return enabled_ordered.contains(&pid).then_some(pid);
        }

        if self.pending_delta == 0 {
            return None;
        }

        let delta = self.pending_delta;
        self.pending_delta = 0;
        resolve_navigation(enabled_ordered, active, delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIDS: &[u32] = &[10, 20, 30];

    #[test]
    fn wraps_forward() {
        assert_eq!(resolve_navigation(PIDS, Some(30), 1), Some(10));
    }

    #[test]
    fn wraps_backward() {
        assert_eq!(resolve_navigation(PIDS, Some(10), -1), Some(30));
    }

    #[test]
    fn zero_delta_selects_nothing() {
        assert_eq!(resolve_navigation(PIDS, Some(10), 0), None);
    }

    #[test]
    fn full_lap_selects_nothing() {
        assert_eq!(resolve_navigation(PIDS, Some(20), 3), None);
        assert_eq!(resolve_navigation(PIDS, Some(20), -6), None);
    }

    #[test]
    fn unknown_active_selects_first() {
        assert_eq!(resolve_navigation(PIDS, Some(99), 1), Some(10));
        assert_eq!(resolve_navigation(PIDS, None, -5), Some(10));
    }

    #[test]
    fn empty_list_selects_nothing() {
        assert_eq!(resolve_navigation(&[], Some(10), 1), None);
    }

    #[test]
    fn queue_coalesces_navigation_presses() {
        let mut queue = ActivationQueue::new();
        queue.push_navigation(1);
        queue.push_navigation(1);
        assert_eq!(queue.take_target(PIDS, Some(10)), Some(30));
        assert!(!queue.has_pending());
    }

    #[test]
    fn explicit_activation_supersedes_navigation() {
        let mut queue = ActivationQueue::new();
        queue.push_navigation(5);
        queue.push_activation(20);
        assert_eq!(queue.take_target(PIDS, Some(10)), Some(20));
        assert!(!queue.has_pending());
    }

    #[test]
    fn navigation_after_activation_clears_pid() {
        let mut queue = ActivationQueue::new();
        queue.push_activation(30);
        queue.push_navigation(1);
        assert_eq!(queue.take_target(PIDS, Some(10)), Some(20));
    }

    #[test]
    fn missing_pid_is_consumed_and_yields_none() {
        let mut queue = ActivationQueue::new();
        queue.push_activation(99);
        assert_eq!(queue.take_target(PIDS, Some(10)), None);
        assert!(!queue.has_pending());
    }

    #[test]
    fn empty_queue_yields_none() {
        let mut queue = ActivationQueue::new();
        assert_eq!(queue.take_target(PIDS, Some(10)), None);
    }
}
