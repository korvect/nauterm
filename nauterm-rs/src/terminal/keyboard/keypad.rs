//! GPUI normalizes macOS keypad keys to ordinary characters. Preserve the
//! physical identity only while dispatching an actual terminal key-down event.

#[cfg(target_os = "macos")]
pub(super) fn current_sequence(window: &gpui_kit::Window) -> Option<&'static [u8]> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEventType};

    let main = MainThreadMarker::new()?;
    let event = NSApplication::sharedApplication(main).currentEvent()?;
    if event.r#type() != NSEventType::KeyDown {
        return None;
    }
    let native = crate::window_chrome::native_window(window)?;
    if event.windowNumber() != native.windowNumber() {
        return None;
    }
    sequence(event.keyCode())
}

/// Virtual key codes from the macOS SDK's HIToolbox/Events.h. Sequences match
/// TerminalKeyEncoder._applicationKeypadSequence in the Flutter client.
fn sequence(code: u16) -> Option<&'static [u8]> {
    Some(match code {
        0x41 => b"\x1bOn", // decimal
        0x43 => b"\x1bOj", // multiply
        0x45 => b"\x1bOk", // add
        0x4b => b"\x1bOo", // divide
        0x4c => b"\x1bOM", // enter
        0x4e => b"\x1bOm", // subtract
        0x52 => b"\x1bOp", // 0
        0x53 => b"\x1bOq", // 1
        0x54 => b"\x1bOr", // 2
        0x55 => b"\x1bOs", // 3
        0x56 => b"\x1bOt", // 4
        0x57 => b"\x1bOu", // 5
        0x58 => b"\x1bOv", // 6
        0x59 => b"\x1bOw", // 7
        0x5b => b"\x1bOx", // 8
        0x5c => b"\x1bOy", // 9
        0x5f => b"\x1bOl", // JIS comma
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::sequence;

    #[test]
    fn application_keypad_matches_the_reference_protocol() {
        for (code, expected) in [
            (0x41, "\x1bOn"),
            (0x43, "\x1bOj"),
            (0x45, "\x1bOk"),
            (0x4b, "\x1bOo"),
            (0x4c, "\x1bOM"),
            (0x4e, "\x1bOm"),
            (0x52, "\x1bOp"),
            (0x53, "\x1bOq"),
            (0x54, "\x1bOr"),
            (0x55, "\x1bOs"),
            (0x56, "\x1bOt"),
            (0x57, "\x1bOu"),
            (0x58, "\x1bOv"),
            (0x59, "\x1bOw"),
            (0x5b, "\x1bOx"),
            (0x5c, "\x1bOy"),
            (0x5f, "\x1bOl"),
        ] {
            assert_eq!(sequence(code), Some(expected.as_bytes()));
        }
    }

    #[test]
    fn main_keyboard_and_unmapped_keypad_keys_keep_the_normal_input_path() {
        // Main Return, 0–9 and punctuation must not become keypad input.
        for code in [
            0x24, 0x1d, 0x12, 0x13, 0x14, 0x15, 0x17, 0x16, 0x1a, 0x1c, 0x19, 0x2f, 0x2c, 0x1b,
            0x18, 0x47, 0x51, 0xffff,
        ] {
            assert_eq!(sequence(code), None, "virtual key code {code:#x}");
        }
    }
}
