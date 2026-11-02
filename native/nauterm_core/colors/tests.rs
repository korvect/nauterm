use crate::terminal::{TerminalDefaultColors, TerminalEmulator, TerminalOptions};

fn palette(seed: u32) -> ([u32; 19], TerminalDefaultColors) {
    let values = std::array::from_fn(|index| seed + index as u32 * 0x030201);
    (
        values,
        TerminalDefaultColors::from_palette(&values).unwrap(),
    )
}
fn engine(backend: u32) -> Box<dyn TerminalEmulator> {
    crate::session::create_terminal_emulator(
        40,
        4,
        TerminalOptions {
            emulator_backend: crate::terminal::TerminalEmulatorBackend::from_u32(backend),
            ..Default::default()
        },
    )
    .unwrap()
}
fn query(terminal: &mut dyn TerminalEmulator, prefix: &str, expected: u32) {
    terminal.write_bytes(format!("\x1b]{prefix};?\x07").as_bytes());
    let response = terminal.drain_transport_writes().concat();
    let rgb = response
        .split(";rgb:")
        .nth(1)
        .unwrap_or_else(|| panic!("No RGB query response: {response:?}"));
    let rgb = rgb.split(['\x07', '\x1b']).next().unwrap();
    let channels: Vec<_> = rgb
        .split('/')
        .map(|channel| {
            let value = u32::from_str_radix(channel, 16).unwrap();
            (value * 255 / ((1u32 << (channel.len() * 4)) - 1)) as u8
        })
        .collect();
    assert_eq!(channels.len(), 3, "{response:?}");
    let actual =
        (u32::from(channels[0]) << 16) | (u32::from(channels[1]) << 8) | u32::from(channels[2]);
    assert_eq!(actual, expected, "Query {prefix}: {response:?}");
}

#[test]
fn constructor_colors_are_available_before_the_first_output_query() {
    for backend in [0, 1] {
        let (values, colors) = palette(0x102030);
        let mut terminal = crate::session::create_terminal_emulator(
            40,
            4,
            TerminalOptions {
                emulator_backend: crate::terminal::TerminalEmulatorBackend::from_u32(backend),
                default_colors: colors,
                ..Default::default()
            },
        )
        .unwrap();
        query(terminal.as_mut(), "11", values[1]);
        query(terminal.as_mut(), "4;1", values[4]);
        terminal.write_bytes(b"\x1b[31mR");
        let frame = terminal.snapshot();
        assert_eq!(
            (frame.cells[0].foreground, frame.cells[0].background),
            (values[4], values[1])
        );
    }
}

#[test]
fn rgb_aliases_remain_distinct_from_native_palette_and_default_roles() {
    for backend in [0, 1] {
        let mut terminal = engine(backend);
        terminal.write_bytes(b"\x1b[30mA\x1b[31mB\x1b[0mC\x1b[38;2;52;56;66mD\x1b[38;2;217;95;86mE\x1b[48;2;251;251;248mF\x1b[0m\x1b[6 q");
        let before = terminal.snapshot();
        let (values, colors) = palette(0x102030);
        assert!(terminal.rs_set_palette(colors));
        let after = terminal.snapshot();
        assert_eq!(after.text, before.text);
        assert_eq!(after.history_lines, before.history_lines);
        assert_eq!(
            (after.cursor_column, after.cursor_row, after.cursor_shape),
            (before.cursor_column, before.cursor_row, 2)
        );
        assert_eq!(after.cursor_color, values[2]);
        assert_eq!(
            after.cells[0].foreground, values[3],
            "ANSI black backend {backend}"
        );
        assert_eq!(after.cells[1].foreground, values[4]);
        assert_eq!(after.cells[2].foreground, values[0]);
        assert_eq!(after.cells[3].foreground, 0x343842);
        assert_eq!(after.cells[4].foreground, 0xd95f56);
        assert_eq!(after.cells[5].background, 0xfbfbf8);
        assert_eq!(after.cells[2].background, values[1]);
    }
}

#[test]
fn osc_queries_overrides_resets_and_theme_updates_use_the_same_native_colors() {
    for backend in [0, 1] {
        let mut terminal = engine(backend);
        let (first, colors) = palette(0x102030);
        assert!(terminal.rs_set_palette(colors));
        for (prefix, expected) in [
            ("10", first[0]),
            ("11", first[1]),
            ("12", first[2]),
            ("4;0", first[3]),
            ("4;1", first[4]),
        ] {
            query(terminal.as_mut(), prefix, expected);
        }
        terminal.write_bytes(
            b"\x1b]4;1;#aabbcc\x07\x1b]10;#123456\x07\x1b]11;#654321\x07\x1b]12;#abcdef\x07",
        );
        let (next, colors) = palette(0x506070);
        assert!(terminal.rs_set_palette(colors));
        for (prefix, expected) in [
            ("10", 0x123456),
            ("11", 0x654321),
            ("12", 0xabcdef),
            ("4;1", 0xaabbcc),
            ("4;0", next[3]),
        ] {
            query(terminal.as_mut(), prefix, expected);
        }
        terminal.write_bytes(b"\x1b]104;1\x07\x1b]110\x07\x1b]111\x07\x1b]112\x07");
        for (prefix, expected) in [
            ("10", next[0]),
            ("11", next[1]),
            ("12", next[2]),
            ("4;1", next[4]),
        ] {
            query(terminal.as_mut(), prefix, expected);
        }
        terminal.write_bytes(b"\x1bc");
        query(terminal.as_mut(), "11", next[1]);
        query(terminal.as_mut(), "4;1", next[4]);
    }
}

#[test]
fn updates_preserve_history_alternate_screen_and_reject_invalid_colors() {
    for backend in [0, 1] {
        let mut terminal = engine(backend);
        for index in 0..12 {
            terminal.write_bytes(format!("retained {index}\r\n").as_bytes());
        }
        let text = terminal.plain_text();
        let history = terminal.snapshot().history_lines;
        assert!(history > 0);
        terminal.write_bytes(b"\x1b[?1049hALT");
        let (_, colors) = palette(0x102030);
        assert!(terminal.rs_set_palette(colors));
        assert!(terminal.is_alt_screen());
        assert!(terminal.plain_text().contains("ALT"));
        terminal.write_bytes(b"\x1b[?1049l");
        assert_eq!(terminal.plain_text(), text);
        assert_eq!(terminal.snapshot().history_lines, history);
    }
    assert!(TerminalDefaultColors::from_palette(&[0; 18]).is_none());
    assert!(TerminalDefaultColors::from_palette(&[0; 20]).is_none());
    assert!(TerminalDefaultColors::from_palette(&[0x1000000; 19]).is_none());
    assert!(TerminalDefaultColors::from_palette(&[0; 19]).is_some());
}
