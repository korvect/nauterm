//! Terminal protocol encoding. Ordinary text remains owned by the platform IME.
use gpui_kit::Keystroke;

#[cfg(any(target_os = "macos", test))]
mod keypad;

/// Only actual key-down dispatch may consult AppKit's current physical key.
/// Programmatic cursor movement continues to use `encode` without native events.
pub(super) fn encode_event(
    key: &Keystroke,
    mode: u32,
    option_as_meta: bool,
    window: &gpui_kit::Window,
) -> Option<Vec<u8>> {
    #[cfg(target_os = "macos")]
    if mode & 2 != 0
        && !key.modifiers.platform
        && let Some(sequence) = keypad::current_sequence(window)
    {
        return Some(sequence.to_vec());
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
    encode(key, mode, option_as_meta)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScrollKey {
    PageUp,
    PageDown,
    Top,
    Bottom,
}
pub(super) fn scroll_key(
    key: &Keystroke,
    enabled: bool,
    alternate_screen: bool,
) -> Option<ScrollKey> {
    if !enabled
        || alternate_screen
        || key.modifiers.alt
        || key.modifiers.control
        || key.modifiers.platform
    {
        return None;
    }
    match key.key.as_str() {
        "pageup" => Some(ScrollKey::PageUp),
        "pagedown" => Some(ScrollKey::PageDown),
        "home" => Some(ScrollKey::Top),
        "end" => Some(ScrollKey::Bottom),
        _ => None,
    }
}

pub(super) fn encode(key: &Keystroke, mode: u32, option_as_meta: bool) -> Option<Vec<u8>> {
    encode_for_platform(key, mode, option_as_meta, cfg!(target_os = "macos"))
}
fn encode_for_platform(
    key: &Keystroke,
    mode: u32,
    option_as_meta: bool,
    macos: bool,
) -> Option<Vec<u8>> {
    let modifiers = key.modifiers;
    if modifiers.platform {
        return None;
    }
    let with_alt = |sequence: &str| {
        if option_as_meta && modifiers.alt {
            format!("\x1b{sequence}").into_bytes()
        } else {
            sequence.as_bytes().to_vec()
        }
    };
    let modifier = 1
        + u8::from(modifiers.shift)
        + 2 * u8::from(modifiers.alt)
        + 4 * u8::from(modifiers.control);
    let csi_final = |last: char| {
        if modifier == 1 {
            format!("\x1b[{last}").into_bytes()
        } else {
            format!("\x1b[1;{modifier}{last}").into_bytes()
        }
    };
    let csi_tilde = |code: u8| {
        if modifier == 1 {
            format!("\x1b[{code}~").into_bytes()
        } else {
            format!("\x1b[{code};{modifier}~").into_bytes()
        }
    };
    if macos && modifiers.alt && !modifiers.shift && !modifiers.control {
        match key.key.as_str() {
            "left" => return Some(b"\x1bb".to_vec()),
            "right" => return Some(b"\x1bf".to_vec()),
            _ => {}
        }
    }
    let cursor = match key.key.as_str() {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        _ => None,
    };
    if let Some(cursor) = cursor {
        return Some(if modifier == 1 && mode & 1 != 0 {
            format!("\x1bO{cursor}").into_bytes()
        } else {
            csi_final(cursor)
        });
    }
    match key.key.as_str() {
        "enter" => return Some(with_alt("\r")),
        "backspace" => return Some(with_alt(if modifiers.control { "\x08" } else { "\x7f" })),
        "escape" => return Some(b"\x1b".to_vec()),
        "tab" => {
            return Some(if modifiers.shift {
                b"\x1b[Z".to_vec()
            } else {
                b"\t".to_vec()
            });
        }
        "home" => return Some(csi_final('H')),
        "end" => return Some(csi_final('F')),
        "insert" => return Some(csi_tilde(2)),
        "delete" => return Some(csi_tilde(3)),
        "pageup" => return Some(csi_tilde(5)),
        "pagedown" => return Some(csi_tilde(6)),
        _ => {}
    }
    if let Some(number) = key
        .key
        .strip_prefix('f')
        .and_then(|value| value.parse::<usize>().ok())
    {
        if (1..=4).contains(&number) {
            let last = (b'P' + number as u8 - 1) as char;
            return Some(if modifier == 1 {
                format!("\x1bO{last}").into_bytes()
            } else {
                csi_final(last)
            });
        }
        if let Some(code) = number
            .checked_sub(5)
            .and_then(|index| [15, 17, 18, 19, 20, 21, 23, 24].get(index))
            .copied()
        {
            return Some(csi_tilde(code));
        }
    }
    if modifiers.control {
        let control = match key.key.as_str() {
            " " | "space" | "2" | "@" => Some(0),
            "[" | "3" => Some(0x1b),
            "\\" | "4" => Some(0x1c),
            "]" | "5" => Some(0x1d),
            "^" | "6" => Some(0x1e),
            "/" | "7" | "-" | "_" => Some(0x1f),
            "8" => Some(0x7f),
            value if value.len() == 1 && value.as_bytes()[0].is_ascii_alphabetic() => {
                Some(value.as_bytes()[0].to_ascii_uppercase() & 0x1f)
            }
            _ => None,
        }?;
        return Some(with_alt(std::str::from_utf8(&[control]).ok()?));
    }
    if option_as_meta && modifiers.alt {
        // The key is layout-aware but excludes Option translation. Thus Meta-a
        // sends ESC+a rather than the macOS Option-a character å. With Meta off,
        // the native text-input path still owns Option characters and dead keys.
        let text = if key.key == "space" { " " } else { &key.key };
        if text.chars().count() == 1 {
            return Some(with_alt(&if modifiers.shift {
                text.to_uppercase()
            } else {
                text.into()
            }));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn key(value: &'static str) -> Keystroke {
        Keystroke::parse(value).unwrap()
    }
    #[test]
    fn special_and_control_keys_keep_terminal_protocol_modifiers() {
        for (stroke, expected) in [
            ("ctrl-c", "\x03"),
            ("ctrl-alt-c", "\x1b\x03"),
            ("ctrl-backspace", "\x08"),
            ("alt-enter", "\x1b\r"),
            ("ctrl-2", "\0"),
            ("ctrl-3", "\x1b"),
            ("ctrl-8", "\x7f"),
            ("shift-f1", "\x1b[1;2P"),
            ("ctrl-f5", "\x1b[15;5~"),
            ("alt-delete", "\x1b[3;3~"),
            ("ctrl-pageup", "\x1b[5;5~"),
        ] {
            assert_eq!(
                encode_for_platform(&key(stroke), 0, true, false),
                Some(expected.as_bytes().to_vec()),
                "{stroke}"
            );
        }
        assert_eq!(
            encode_for_platform(&key("home"), 1, true, false),
            Some(b"\x1b[H".to_vec())
        );
        assert_eq!(
            encode_for_platform(&key("up"), 1, true, false),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(encode_for_platform(&key("cmd-a"), 0, true, true), None);
    }
    #[test]
    fn option_meta_does_not_duplicate_platform_text_or_dead_key_translation() {
        let mut option_a = key("alt-a");
        option_a.key_char = Some("å".into());
        assert_eq!(
            encode_for_platform(&option_a, 0, true, true),
            Some(b"\x1ba".to_vec())
        );
        assert_eq!(encode_for_platform(&option_a, 0, false, true), None);
        assert_eq!(encode_for_platform(&key("a"), 0, true, true), None);
        assert_eq!(
            encode_for_platform(&key("alt-left"), 0, false, true),
            Some(b"\x1bb".to_vec())
        );
        assert_eq!(
            encode_for_platform(&key("alt-left"), 0, true, false),
            Some(b"\x1b[1;3D".to_vec())
        );
        assert_eq!(
            encode_for_platform(&key("ctrl-alt-c"), 0, false, false),
            Some(vec![3])
        );
    }
    #[test]
    fn application_keypad_mode_preserves_main_and_programmatic_keys() {
        for mode in [0, 2] {
            assert_eq!(
                encode_for_platform(&key("enter"), mode, true, true),
                Some(b"\r".to_vec())
            );
            assert_eq!(encode_for_platform(&key("1"), mode, true, true), None);
            assert_eq!(
                encode_for_platform(&key("ctrl-c"), mode, true, true),
                Some(vec![3])
            );
            assert_eq!(encode_for_platform(&key("cmd-1"), mode, true, true), None);
        }
        assert_eq!(
            encode_for_platform(&key("left"), 3, true, true),
            Some(b"\x1bOD".to_vec())
        );
    }
    #[test]
    fn navigation_scrolls_history_only_outside_interactive_screens() {
        for (stroke, expected) in [
            ("pageup", ScrollKey::PageUp),
            ("shift-pagedown", ScrollKey::PageDown),
            ("home", ScrollKey::Top),
            ("end", ScrollKey::Bottom),
        ] {
            assert_eq!(scroll_key(&key(stroke), true, false), Some(expected));
            assert_eq!(scroll_key(&key(stroke), true, true), None);
            assert_eq!(scroll_key(&key(stroke), false, false), None);
        }
        for stroke in ["ctrl-pageup", "alt-home", "cmd-end", "up", "left"] {
            assert_eq!(scroll_key(&key(stroke), true, false), None);
        }
    }
}
