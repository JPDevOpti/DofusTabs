//! Pure decision logic for "which character is currently active in this
//! client". See docs/WORKSPACES.md for the full rule.
//!
//! The active character is the `Characters/<hash>` directory whose
//! `dofus.json` has the newest mtime among those touched after the client
//! process started. When none was touched yet in this session, the character
//! played last on the account is assumed.

/// One character directory candidate: its hash (`Characters/<hash>`) and its
/// `dofus.json` mtime, both as Unix epoch seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterCandidate<'a> {
    pub hash: &'a str,
    pub mtime_epoch_secs: u64,
}

/// Resolves the active character hash for a client whose process started at
/// `process_start_epoch_secs`, given every character directory's mtime.
///
/// Returns `None` only when the account has no character directory at all.
/// Ties (identical mtimes) are broken by the lexicographically
/// smallest hash, so the result is deterministic and reproducible in tests;
/// in practice two character files sharing the exact same second is
/// vanishingly unlikely.
pub fn resolve_active_character<'a>(
    process_start_epoch_secs: u64,
    candidates: &[CharacterCandidate<'a>],
) -> Option<&'a str> {
    let newest = |pool: &mut dyn Iterator<Item = &CharacterCandidate<'a>>| {
        pool.max_by(|a, b| {
            a.mtime_epoch_secs
                .cmp(&b.mtime_epoch_secs)
                .then_with(|| b.hash.cmp(a.hash))
        })
        .map(|c| c.hash)
    };

    // The game only rewrites the character file some time after entering
    // the game (observed: minutes later, or on exit), so right after a
    // restart nothing is newer than the process. Fall back to the character
    // played last on this account instead of reporting nothing.
    newest(
        &mut candidates
            .iter()
            .filter(|c| c.mtime_epoch_secs > process_start_epoch_secs),
    )
    .or_else(|| newest(&mut candidates.iter()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(hash: &str, mtime: u64) -> CharacterCandidate<'_> {
        CharacterCandidate {
            hash,
            mtime_epoch_secs: mtime,
        }
    }

    #[test]
    fn newest_mtime_after_start_wins() {
        let candidates = [cand("a", 100), cand("b", 200), cand("c", 150)];
        assert_eq!(resolve_active_character(50, &candidates), Some("b"));
    }

    #[test]
    fn candidates_not_later_than_start_are_excluded() {
        let candidates = [cand("a", 100), cand("b", 200)];
        // Process started at 150: "a" (mtime 100) predates the process and
        // must not be chosen even though it exists.
        assert_eq!(resolve_active_character(150, &candidates), Some("b"));
    }

    #[test]
    fn no_candidate_later_than_start_falls_back_to_last_played() {
        let candidates = [cand("a", 100), cand("b", 200)];
        assert_eq!(resolve_active_character(300, &candidates), Some("b"));
    }

    #[test]
    fn candidate_touched_in_this_session_beats_a_newer_looking_leftover() {
        // "a" predates the process, "b" was written during it.
        let candidates = [cand("a", 140), cand("b", 160)];
        assert_eq!(resolve_active_character(150, &candidates), Some("b"));
    }

    #[test]
    fn empty_candidates_yields_none() {
        assert_eq!(resolve_active_character(0, &[]), None);
    }

    #[test]
    fn tie_breaks_on_lexicographically_smallest_hash() {
        let candidates = [cand("bbb", 500), cand("aaa", 500)];
        assert_eq!(resolve_active_character(0, &candidates), Some("aaa"));
    }
}
