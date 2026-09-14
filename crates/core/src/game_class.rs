use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// A playable Dofus class, identified in window titles by its aliases.
#[derive(Debug)]
pub struct GameClass {
    pub canonical_name: &'static str,
    pub icon_file: &'static str,
    pub aliases: &'static [&'static str],
}

impl PartialEq for GameClass {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_name == other.canonical_name
    }
}

impl Eq for GameClass {}

/// Every alias must already be in normalized form (lowercase, no accents).
pub static ALL: &[GameClass] = &[
    GameClass {
        canonical_name: "Aniripsa",
        icon_file: "Aniripsa.png",
        aliases: &["aniripsa"],
    },
    GameClass {
        canonical_name: "Anutrof",
        icon_file: "Anutrof.png",
        aliases: &["anutrof"],
    },
    GameClass {
        canonical_name: "Feca",
        icon_file: "Feca.png",
        aliases: &["feca"],
    },
    GameClass {
        canonical_name: "Forjalanza",
        icon_file: "Forjalanza.png",
        aliases: &["forjalanza"],
    },
    GameClass {
        canonical_name: "Hipermago",
        icon_file: "Hipermago.png",
        aliases: &["hipermago"],
    },
    GameClass {
        canonical_name: "Ocra",
        icon_file: "Ocra.png",
        aliases: &["ocra"],
    },
    GameClass {
        canonical_name: "Osamodas",
        icon_file: "Osamodas.png",
        aliases: &["osamodas"],
    },
    GameClass {
        canonical_name: "Pandawa",
        icon_file: "Pandawa.png",
        aliases: &["pandawa"],
    },
    GameClass {
        canonical_name: "Sacrógrito",
        icon_file: "Sacrogrito.png",
        aliases: &["sacrogrito"],
    },
    GameClass {
        canonical_name: "Sadida",
        icon_file: "Sadida.png",
        aliases: &["sadida"],
    },
    GameClass {
        canonical_name: "Selotrop",
        icon_file: "Selotrop.png",
        aliases: &["selotrop"],
    },
    GameClass {
        canonical_name: "Sram",
        icon_file: "Sram.png",
        aliases: &["sram"],
    },
    GameClass {
        canonical_name: "Steamer",
        icon_file: "Steamer.png",
        aliases: &["steamer"],
    },
    GameClass {
        canonical_name: "Tymador",
        icon_file: "Tymador.png",
        aliases: &["tymador"],
    },
    GameClass {
        canonical_name: "Uginak",
        icon_file: "Uginak.png",
        aliases: &["uginak"],
    },
    GameClass {
        canonical_name: "Xelor",
        icon_file: "Xelor.png",
        aliases: &["xelor"],
    },
    GameClass {
        canonical_name: "Yopuka",
        icon_file: "Yopuka.png",
        aliases: &["yopuka"],
    },
    GameClass {
        canonical_name: "Zobal",
        icon_file: "Zobal.png",
        aliases: &["zobal"],
    },
    GameClass {
        canonical_name: "Zurcar",
        icon_file: "Zurcar.png",
        aliases: &["zurcar"],
    },
];

/// Lowercases, strips accents (NFD combining marks), replaces every
/// non-alphanumeric character with a space and collapses whitespace.
pub fn normalize_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.nfd() {
        if is_combining_mark(c) {
            continue;
        }
        if c.is_alphanumeric() || c.is_whitespace() {
            out.extend(c.to_lowercase());
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Resolves a class by whole-word alias match against the normalized text.
pub fn resolve_from_text(text: &str) -> Option<&'static GameClass> {
    if text.trim().is_empty() {
        return None;
    }
    let normalized = normalize_text(text);
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    ALL.iter().find(|class| {
        class
            .aliases
            .iter()
            .any(|alias| tokens.iter().any(|t| t == alias))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_accented_and_plain_alias() {
        assert_eq!(
            resolve_from_text("Sacrógrito").unwrap().canonical_name,
            "Sacrógrito"
        );
        assert_eq!(
            resolve_from_text("sacrogrito").unwrap().canonical_name,
            "Sacrógrito"
        );
    }

    #[test]
    fn resolves_case_insensitively_inside_sentence() {
        assert_eq!(
            resolve_from_text("mi FECA favorita")
                .unwrap()
                .canonical_name,
            "Feca"
        );
    }

    #[test]
    fn does_not_match_substrings() {
        assert!(resolve_from_text("fecapop").is_none());
    }

    #[test]
    fn empty_or_blank_resolves_to_none() {
        assert!(resolve_from_text("").is_none());
        assert!(resolve_from_text("   ").is_none());
    }

    #[test]
    fn no_duplicate_aliases_across_classes() {
        let mut seen = std::collections::HashSet::new();
        for class in ALL {
            for alias in class.aliases {
                assert!(seen.insert(*alias), "duplicate alias: {alias}");
            }
        }
    }

    #[test]
    fn normalize_strips_accents_and_symbols() {
        assert_eq!(normalize_text("Sacró-Grito!  (x2)"), "sacro grito x2");
    }
}
