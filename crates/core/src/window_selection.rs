//! Picks one representative window per process. A game client can own
//! several top-level windows (IME helpers, splash screens, an empty
//! "Release" frame); the one with the most informative title wins so each
//! PID maps to exactly one stable instance.

/// Titles some clients expose on helper windows that never identify a
/// character. They lose against any other title.
const NOISE_TITLES: &[&str] = &["MSCTFIME UI", "Default IME", "Release"];

pub fn is_noise_title(title: &str) -> bool {
    NOISE_TITLES
        .iter()
        .any(|noise| noise.eq_ignore_ascii_case(title.trim()))
}

/// Higher is better. Rewards the `Name - Class - Dofus` shape, then pipes,
/// then any mention of the game; blank and noise titles rank lowest.
pub fn score_title(title: &str) -> i32 {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return i32::MIN;
    }
    if is_noise_title(trimmed) {
        return i32::MIN + 100;
    }

    let mut score = trimmed.chars().count().min(120) as i32;
    if trimmed.contains(" - ") {
        score += 180;
    }
    if trimmed.contains('|') {
        score += 60;
    }
    if trimmed.to_lowercase().contains("dofus") {
        score += 20;
    }
    score
}

/// Keeps the best-titled item per PID (highest [`score_title`], then the
/// longest title, then the first seen). Output preserves the order in which
/// each PID first appeared, so repeated scans stay stable.
pub fn best_per_pid<T>(
    items: Vec<T>,
    pid_of: impl Fn(&T) -> u32,
    title_of: impl Fn(&T) -> &str,
) -> Vec<T> {
    let mut best: Vec<T> = Vec::new();
    for item in items {
        let pid = pid_of(&item);
        match best.iter().position(|b| pid_of(b) == pid) {
            Some(index) => {
                if ranks_higher(title_of(&item), title_of(&best[index])) {
                    best[index] = item;
                }
            }
            None => best.push(item),
        }
    }
    best
}

fn ranks_higher(candidate: &str, current: &str) -> bool {
    let key = |t: &str| (score_title(t), t.trim().chars().count());
    key(candidate) > key(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_titles_are_case_insensitive_and_trimmed() {
        assert!(is_noise_title("Default IME"));
        assert!(is_noise_title("  release "));
        assert!(is_noise_title("msctfime ui"));
        assert!(!is_noise_title("Ana - Feca - Dofus"));
        assert!(!is_noise_title("Release notes"));
    }

    #[test]
    fn blank_and_noise_titles_rank_lowest() {
        assert_eq!(score_title(""), i32::MIN);
        assert_eq!(score_title("   "), i32::MIN);
        assert_eq!(score_title("Release"), i32::MIN + 100);
        assert!(score_title("Release") > score_title(""));
        assert!(score_title("x") > score_title("Release"));
    }

    #[test]
    fn full_titles_beat_partial_ones() {
        let full = score_title("Ana - Feca - Dofus 3");
        let piped = score_title("Ana | Feca | Dofus");
        let bare = score_title("Dofus");
        let other = score_title("Loading");
        assert!(full > piped);
        assert!(piped > bare);
        assert!(bare > other);
    }

    #[test]
    fn length_bonus_is_capped() {
        let long = "a".repeat(500);
        assert_eq!(score_title(&long), 120);
    }

    #[test]
    fn keeps_one_window_per_pid_with_best_title() {
        let windows = vec![
            (10, "Default IME"),
            (10, "Ana - Feca - Dofus"),
            (20, "Dofus"),
            (10, "Dofus"),
            (20, "Beto - Sram - Dofus"),
        ];
        let best = best_per_pid(windows, |w| w.0, |w| w.1);
        assert_eq!(best, vec![(10, "Ana - Feca - Dofus"), (20, "Beto - Sram - Dofus")]);
    }

    #[test]
    fn ties_prefer_longer_then_first_seen() {
        let windows = vec![(1, "Ana - Feca - Dofus"), (1, "Anabel - Feca - Dofus")];
        assert_eq!(best_per_pid(windows, |w| w.0, |w| w.1)[0].1, "Anabel - Feca - Dofus");

        let same = vec![(1, "first"), (1, "second"), (1, "seventh")];
        assert_eq!(best_per_pid(same, |w| w.0, |w| w.1)[0].1, "seventh");
        let equal = vec![(1, "aaaaa"), (1, "bbbbb")];
        assert_eq!(best_per_pid(equal, |w| w.0, |w| w.1)[0].1, "aaaaa");
    }

    #[test]
    fn output_order_follows_first_appearance() {
        let windows = vec![(3, "c"), (1, "a"), (2, "b"), (1, "a - b - dofus")];
        let pids: Vec<u32> = best_per_pid(windows, |w| w.0, |w| w.1)
            .into_iter()
            .map(|w| w.0)
            .collect();
        assert_eq!(pids, vec![3, 1, 2]);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert!(best_per_pid(Vec::<(u32, &str)>::new(), |w| w.0, |w| w.1).is_empty());
    }
}
