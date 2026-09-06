//! Pure mapping from [`Hotkey`] to Win32 `RegisterHotKey` arguments.
//!
//! Compiled on every platform so the mapping stays unit-testable on Linux,
//! where the actual Win32 backend cannot even be built. Constants are local
//! copies of the Win32 values rather than `windows-sys` re-exports for the
//! same reason.

use dofustabs_core::settings::Hotkey;

pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;
/// Suppresses auto-repeat while the combination is held, matching the
/// fire-once-per-press behavior of a plain Hyprland `bind`.
pub const MOD_NOREPEAT: u32 = 0x4000;

/// Win32 modifier flags for the hotkey (without [`MOD_NOREPEAT`]).
pub fn modifier_flags(hotkey: &Hotkey) -> u32 {
    let mut flags = 0;
    if hotkey.alt {
        flags |= MOD_ALT;
    }
    if hotkey.ctrl {
        flags |= MOD_CONTROL;
    }
    if hotkey.shift {
        flags |= MOD_SHIFT;
    }
    if hotkey.super_key {
        flags |= MOD_WIN;
    }
    flags
}

/// Virtual-key code for a key name as produced by the frontend's hotkey
/// capture (`KeyboardEvent.key` style: "F9", "Tab", "ArrowUp", "A", "1").
/// Case-insensitive; `None` for keys Win32 hotkeys cannot express.
pub fn vk_code(key: &str) -> Option<u32> {
    // A literal space (KeyboardEvent.key for the spacebar) must be matched
    // before trimming, which would otherwise erase it.
    if key == " " {
        return Some(0x20);
    }

    let k = key.trim();

    if k.len() == 1 {
        let c = k.chars().next().unwrap();
        return match c {
            'a'..='z' => Some(c.to_ascii_uppercase() as u32),
            'A'..='Z' => Some(c as u32),
            '0'..='9' => Some(c as u32),
            ' ' => Some(0x20),
            _ => None,
        };
    }

    let upper = k.to_ascii_uppercase();

    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        // VK_F1 (0x70) .. VK_F24 (0x87)
        return (1..=24).contains(&n).then(|| 0x6F + n);
    }

    match upper.as_str() {
        "TAB" => Some(0x09),
        "BACKSPACE" => Some(0x08),
        "ENTER" | "RETURN" => Some(0x0D),
        "ESCAPE" | "ESC" => Some(0x1B),
        "SPACE" => Some(0x20),
        "PAGEUP" => Some(0x21),
        "PAGEDOWN" => Some(0x22),
        "END" => Some(0x23),
        "HOME" => Some(0x24),
        "ARROWLEFT" | "LEFT" => Some(0x25),
        "ARROWUP" | "UP" => Some(0x26),
        "ARROWRIGHT" | "RIGHT" => Some(0x27),
        "ARROWDOWN" | "DOWN" => Some(0x28),
        "INSERT" => Some(0x2D),
        "DELETE" | "DEL" => Some(0x2E),
        _ => None,
    }
}

/// Full `(modifier_flags, vk_code)` pair for `RegisterHotKey`, or `None` when
/// the hotkey is empty or its key has no virtual-key equivalent.
pub fn map_hotkey(hotkey: &Hotkey) -> Option<(u32, u32)> {
    if hotkey.is_empty() {
        return None;
    }
    Some((modifier_flags(hotkey), vk_code(&hotkey.key)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hotkey(ctrl: bool, alt: bool, shift: bool, super_key: bool, key: &str) -> Hotkey {
        Hotkey {
            ctrl,
            alt,
            shift,
            super_key,
            key: key.to_string(),
        }
    }

    #[test]
    fn letters_map_case_insensitively() {
        assert_eq!(vk_code("a"), Some(0x41));
        assert_eq!(vk_code("A"), Some(0x41));
        assert_eq!(vk_code("z"), Some(0x5A));
    }

    #[test]
    fn digits_map_to_their_ascii_codes() {
        assert_eq!(vk_code("0"), Some(0x30));
        assert_eq!(vk_code("9"), Some(0x39));
    }

    #[test]
    fn function_keys_map_within_f1_to_f24() {
        assert_eq!(vk_code("F1"), Some(0x70));
        assert_eq!(vk_code("f9"), Some(0x78));
        assert_eq!(vk_code("F10"), Some(0x79));
        assert_eq!(vk_code("F24"), Some(0x87));
        assert_eq!(vk_code("F0"), None);
        assert_eq!(vk_code("F25"), None);
    }

    #[test]
    fn named_keys_and_aliases_map() {
        assert_eq!(vk_code("Tab"), Some(0x09));
        assert_eq!(vk_code("Home"), Some(0x24));
        assert_eq!(vk_code("ArrowUp"), Some(0x26));
        assert_eq!(vk_code("up"), Some(0x26));
        assert_eq!(vk_code("Delete"), Some(0x2E));
        assert_eq!(vk_code("del"), Some(0x2E));
        assert_eq!(vk_code("escape"), Some(0x1B));
        assert_eq!(vk_code("Space"), Some(0x20));
        assert_eq!(vk_code(" "), Some(0x20));
    }

    #[test]
    fn unmappable_keys_are_rejected() {
        assert_eq!(vk_code(""), None);
        assert_eq!(vk_code("Ñ"), None);
        assert_eq!(vk_code("MediaPlay"), None);
        assert_eq!(vk_code("F1x"), None);
    }

    #[test]
    fn modifier_flags_combine() {
        assert_eq!(modifier_flags(&hotkey(false, true, false, false, "x")), MOD_ALT);
        assert_eq!(
            modifier_flags(&hotkey(true, true, true, true, "x")),
            MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN
        );
        assert_eq!(modifier_flags(&hotkey(false, false, false, false, "x")), 0);
    }

    #[test]
    fn map_hotkey_combines_or_rejects() {
        assert_eq!(
            map_hotkey(&hotkey(false, true, false, false, "F9")),
            Some((MOD_ALT, 0x78))
        );
        assert_eq!(map_hotkey(&hotkey(false, true, false, false, "")), None);
        assert_eq!(map_hotkey(&hotkey(false, true, false, false, "Ñ")), None);
    }
}
