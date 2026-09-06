use crate::game_class::{self, GameClass};

/// Character name and class extracted from a game window title.
#[derive(Debug, PartialEq, Eq)]
pub struct ParsedTitle {
    pub character_name: String,
    pub class: Option<&'static GameClass>,
}

const IGNORED_TITLE_TOKENS: &[&str] = &["dofus", "touch", "ankama", "launcher"];

/// Standalone words that act as segment separators; hyphens inside names
/// (e.g. "Juan-Perez") are preserved because they are not surrounded by spaces.
const SEGMENT_SEPARATORS: &[&str] = &["-", "|", "–", "—"];

pub fn parse_title(title: &str) -> ParsedTitle {
    if title.trim().is_empty() {
        return ParsedTitle { character_name: String::new(), class: None };
    }

    let segments = split_segments(title);

    let mut class = segments.iter().find_map(|s| game_class::resolve_from_text(s));
    if class.is_none() {
        class = game_class::resolve_from_text(title);
    }

    let mut name = segments
        .iter()
        .find(|s| is_valid_name_segment(s, class))
        .cloned()
        .unwrap_or_default();
    if name.trim().is_empty() {
        if let Some(first) = segments.first() {
            name = first.clone();
        }
    }

    ParsedTitle { character_name: name, class }
}

fn split_segments(title: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for word in title.split_whitespace() {
        if SEGMENT_SEPARATORS.contains(&word) {
            if !current.is_empty() {
                segments.push(current.join(" "));
                current.clear();
            }
        } else {
            current.push(word);
        }
    }
    if !current.is_empty() {
        segments.push(current.join(" "));
    }
    segments
}

fn is_valid_name_segment(segment: &str, class: Option<&'static GameClass>) -> bool {
    if segment.trim().is_empty() {
        return false;
    }
    let normalized = game_class::normalize_text(segment);
    if IGNORED_TITLE_TOKENS.iter().any(|token| normalized.contains(token)) {
        return false;
    }
    if let Some(cls) = class {
        if game_class::resolve_from_text(segment) == Some(cls) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_class_from_standard_title() {
        let parsed = parse_title("Nombre - Feca - Dofus 3");
        assert_eq!(parsed.character_name, "Nombre");
        assert_eq!(parsed.class.unwrap().canonical_name, "Feca");
    }

    #[test]
    fn preserves_hyphenated_character_names() {
        let parsed = parse_title("Juan-Perez - Sacrógrito - Dofus");
        assert_eq!(parsed.character_name, "Juan-Perez");
        assert_eq!(parsed.class.unwrap().canonical_name, "Sacrógrito");
    }

    #[test]
    fn bare_game_title_falls_back_to_first_segment() {
        let parsed = parse_title("Dofus");
        assert_eq!(parsed.character_name, "Dofus");
        assert!(parsed.class.is_none());
    }

    #[test]
    fn supports_pipe_separator() {
        let parsed = parse_title("Alma | Pandawa | Dofus");
        assert_eq!(parsed.character_name, "Alma");
        assert_eq!(parsed.class.unwrap().canonical_name, "Pandawa");
    }

    #[test]
    fn empty_title_yields_empty_result() {
        let parsed = parse_title("   ");
        assert_eq!(parsed.character_name, "");
        assert!(parsed.class.is_none());
    }

    #[test]
    fn class_segment_is_not_used_as_name() {
        let parsed = parse_title("Feca - Dofus");
        // No valid name segment: the class token and the game token are both
        // rejected, so the first segment wins as fallback.
        assert_eq!(parsed.character_name, "Feca");
        assert_eq!(parsed.class.unwrap().canonical_name, "Feca");
    }
}
