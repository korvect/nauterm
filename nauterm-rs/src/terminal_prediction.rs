//! Shared speculative glyph geometry; SSH predictions never mutate the VT or
//! recorded frame. Mosh uses its own screen-commit batch confirmation policy.
use crate::native::Frame;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use unicode_segmentation::UnicodeSegmentation;
use zeroize::Zeroizing;

pub const CELL_BUDGET: usize = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Adaptive,
    Always,
    Never,
}
impl Mode {
    pub const ALL: [Self; 3] = [Self::Adaptive, Self::Always, Self::Never];
    pub fn label(self) -> &'static str {
        match self {
            Self::Adaptive => "Adaptive",
            Self::Always => "Always",
            Self::Never => "Off",
        }
    }
    pub fn enabled(self, adaptive: bool) -> bool {
        match self {
            Self::Adaptive => adaptive,
            Self::Always => true,
            Self::Never => false,
        }
    }
}

#[derive(Default)]
pub struct Adaptive(bool);
impl Adaptive {
    pub const fn new(enabled: bool) -> Self {
        Self(enabled)
    }
    pub fn update(&mut self, latency: Option<u32>) -> bool {
        if let Some(latency) = latency {
            if latency >= 80 {
                self.0 = true;
            } else if latency <= 40 {
                self.0 = false;
            }
        }
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub row: u32,
    pub column: u32,
}
impl Position {
    pub fn cursor(frame: &Frame) -> Self {
        Self {
            column: frame.cursor.0,
            row: frame.cursor.1,
        }
    }
    pub fn fit(self, width: u32, columns: u32) -> Self {
        if self.column + width > columns {
            Self {
                row: self.row + 1,
                column: 0,
            }
        } else {
            self
        }
    }
    pub fn after(self, width: u32, columns: u32) -> Self {
        let start = self.fit(width, columns);
        if start.column + width >= columns {
            Self {
                row: start.row + 1,
                column: 0,
            }
        } else {
            Self {
                column: start.column + width,
                ..start
            }
        }
    }
}
pub struct Unit {
    pub text: Zeroizing<String>,
    pub width: u32,
    pub start: Position,
    pub end: Position,
}

#[derive(Default)]
pub struct Prediction {
    units: VecDeque<Unit>,
    paused: bool,
    size: Option<(u32, u32)>,
}
impl Prediction {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
    pub fn units(&self) -> impl Iterator<Item = &Unit> {
        self.units.iter()
    }
    pub fn start(&self) -> Option<Position> {
        self.units.front().map(|unit| unit.start)
    }
    pub fn clear(&mut self) -> bool {
        let changed = !self.units.is_empty();
        self.units.clear();
        changed
    }
    pub fn suspend(&mut self) -> bool {
        let changed = self.clear();
        self.paused = true;
        changed
    }
    pub fn allows(frame: &Frame) -> bool {
        frame.input_echo_enabled
            && !frame.alternate_screen
            && frame.display_offset == 0
            && frame.cursor_visible
            && frame.keyboard_mode & 0x70 == 0
            && frame.columns > 0
            && frame.rows > 0
            && frame.cursor.0 < frame.columns
            && frame.cursor.1 < frame.rows
    }
    pub fn add(&mut self, text: &str, frame: &Frame) -> bool {
        if self.paused || !Self::allows(frame) {
            return self.clear();
        }
        if text.is_empty()
            || text.len() > 16 * 1024
            || text.chars().any(|ch| ch < ' ' && ch != '\u{8}')
        {
            return self.suspend();
        }
        self.size = Some((frame.columns, frame.rows));
        let before = Zeroizing::new(
            self.units
                .iter()
                .map(|unit| unit.text.as_str())
                .collect::<String>(),
        );
        for grapheme in text.graphemes(true) {
            if matches!(grapheme, "\u{8}" | "\u{7f}") {
                if self.units.pop_back().is_none() {
                    // An erase of confirmed remote text has unknown geometry
                    // until the next real screen update.
                    self.suspend();
                    break;
                }
            } else {
                let width = cell_width(grapheme);
                if width == 0 {
                    continue;
                }
                let start = self
                    .units
                    .back()
                    .map(|unit| unit.end)
                    .unwrap_or_else(|| Position::cursor(frame))
                    .fit(width, frame.columns);
                self.units.push_back(Unit {
                    text: Zeroizing::new(grapheme.into()),
                    width,
                    start,
                    end: start.after(width, frame.columns),
                });
                if self
                    .units
                    .iter()
                    .map(|unit| unit.width as usize)
                    .sum::<usize>()
                    > CELL_BUDGET
                {
                    self.suspend();
                    break;
                }
            }
        }
        before.as_str()
            != self
                .units
                .iter()
                .map(|unit| unit.text.as_str())
                .collect::<String>()
    }
    /// Called only for an authoritative snapshot change, never an idle paint.
    pub fn reconcile(&mut self, frame: &Frame) -> bool {
        let changed_size = self
            .size
            .replace((frame.columns, frame.rows))
            .is_some_and(|old| old != (frame.columns, frame.rows));
        if std::mem::take(&mut self.paused) || changed_size || !Self::allows(frame) {
            return self.clear();
        }
        let before = self.units.len();
        while let Some(unit) = self.units.front() {
            let actual = frame
                .cells
                .get(unit.start.row as usize * frame.columns as usize + unit.start.column as usize)
                .map(|cell| cell.text.as_str())
                .unwrap_or(" ");
            let cursor = Position::cursor(frame);
            // VT cursors stay in the last cell until another character forces
            // wrapping. A matching right-edge glyph is already echoed there.
            let delayed_wrap = unit.end.column == 0
                && cursor.row == unit.start.row
                && cursor.column + 1 == frame.columns;
            let passed = cursor >= unit.end || (delayed_wrap && actual == unit.text.as_str());
            if actual == unit.text.as_str() && passed {
                self.units.pop_front();
            } else if (actual != " " && !actual.is_empty() && actual != unit.text.as_str())
                || passed
            {
                self.clear();
                break;
            } else {
                break;
            }
        }
        before != self.units.len()
    }
}

/// Same per-grapheme max-rune width rule as Flutter's terminal_text_width.dart.
pub fn cell_width(text: &str) -> u32 {
    text.chars()
        .map(|ch| {
            let rune = ch as u32;
            if rune == 0
                || rune < 0x20
                || (0x7f..0xa0).contains(&rune)
                || matches!(rune, 0x200c | 0x200d | 0xfe0e | 0xfe0f)
                || (0x300..=0x36f).contains(&rune)
                || (0x1ab0..=0x1aff).contains(&rune)
                || (0x1dc0..=0x1dff).contains(&rune)
                || (0x20d0..=0x20ff).contains(&rune)
                || (0xfe00..=0xfe0f).contains(&rune)
                || (0xfe20..=0xfe2f).contains(&rune)
                || (0x1f3fb..=0x1f3ff).contains(&rune)
                || (0xe0100..=0xe01ef).contains(&rune)
            {
                0
            } else if rune >= 0x1100
                && (rune <= 0x115f
                    || matches!(rune, 0x2329 | 0x232a)
                    || ((0x2e80..=0xa4cf).contains(&rune) && rune != 0x303f)
                    || (0xac00..=0xd7a3).contains(&rune)
                    || (0xf900..=0xfaff).contains(&rune)
                    || (0xfe10..=0xfe19).contains(&rune)
                    || (0xfe30..=0xfe6f).contains(&rune)
                    || (0xff00..=0xff60).contains(&rune)
                    || (0xffe0..=0xffe6).contains(&rune)
                    || (0x1f000..=0x1faff).contains(&rune)
                    || (0x20000..=0x3fffd).contains(&rune))
            {
                2
            } else {
                1
            }
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::Cell;
    fn screen(columns: u32, rows: u32, cursor: (u32, u32)) -> Frame {
        Frame {
            columns,
            rows,
            cursor,
            cursor_visible: true,
            input_echo_enabled: true,
            cells: vec![
                Cell {
                    text: " ".into(),
                    hyperlink: None,
                    foreground: 0,
                    background: 0,
                    flags: 0
                };
                (columns * rows) as usize
            ],
            ..Default::default()
        }
    }
    fn text(prediction: &Prediction) -> String {
        prediction.units().map(|unit| unit.text.as_str()).collect()
    }
    #[test]
    fn unicode_batches_confirm_only_the_echoed_prefix_and_pending_backspace() {
        let mut frame = screen(8, 2, (1, 0));
        let mut prediction = Prediction::default();
        assert!(prediction.add("ab中e\u{301}", &frame));
        assert_eq!(
            prediction
                .units()
                .map(|unit| unit.width)
                .collect::<Vec<_>>(),
            [1, 1, 2, 1]
        );
        assert!(!prediction.reconcile(&frame));
        assert!(prediction.add("\u{7f}", &frame));
        assert_eq!(text(&prediction), "ab中");
        frame.cells[1].text = "a".into();
        frame.cells[2].text = "b".into();
        frame.cursor = (3, 0);
        assert!(prediction.reconcile(&frame));
        assert_eq!(text(&prediction), "中");
        frame.cells[3].text = "中".into();
        frame.cells[4].text = "".into();
        frame.cursor = (5, 0);
        assert!(prediction.reconcile(&frame));
        assert!(prediction.is_empty());
    }
    #[test]
    fn controls_conflicts_sensitive_screens_and_cell_budget_discard_speculation() {
        let frame = screen(80, 24, (0, 0));
        let mut prediction = Prediction::default();
        assert!(prediction.add("abc", &frame));
        assert!(prediction.add("\r", &frame));
        assert!(!prediction.add("x", &frame));
        prediction.reconcile(&frame);
        assert!(prediction.add("x", &frame));
        let mut conflict = frame.clone();
        conflict.cells[0].text = "y".into();
        assert!(prediction.reconcile(&conflict));
        assert!(prediction.is_empty());
        let mut blocked = frame.clone();
        blocked.input_echo_enabled = false;
        assert!(!prediction.add("secret", &blocked));
        for flag in [0, 1, 2] {
            let mut blocked = frame.clone();
            match flag {
                0 => blocked.alternate_screen = true,
                1 => blocked.display_offset = 1,
                _ => blocked.keyboard_mode = 0x20,
            }
            assert!(!prediction.add("x", &blocked));
        }
        assert!(!prediction.add(&"中".repeat(129), &frame));
        assert!(prediction.is_empty());
        assert!(!prediction.add("x", &frame));
    }
    #[test]
    fn wide_graphemes_wrap_as_units_and_resize_invalidates_old_coordinates() {
        let mut frame = screen(4, 2, (3, 0));
        let mut prediction = Prediction::default();
        assert!(prediction.add("中B", &frame));
        assert_eq!(
            prediction.units.front().unwrap().start,
            Position { column: 0, row: 1 }
        );
        frame.cells[4].text = "中".into();
        frame.cells[5].text = "".into();
        frame.cells[6].text = "B".into();
        frame.cursor = (3, 1);
        assert!(prediction.reconcile(&frame));
        assert!(prediction.is_empty());
        assert!(prediction.add("x", &frame));
        assert!(prediction.reconcile(&screen(2, 2, (0, 0))));
        assert_eq!(cell_width("👩🏽‍💻"), 2);
        assert_eq!(cell_width("e\u{301}"), 1);
    }
    #[test]
    fn delayed_wrap_confirms_the_last_cell_without_repainting_the_next_unit_over_it() {
        let mut frame = screen(4, 2, (3, 0));
        let mut prediction = Prediction::default();
        assert!(prediction.add("ab", &frame));
        assert!(!prediction.reconcile(&frame));
        frame.cells[3].text = "a".into();
        // The terminal has echoed a, but its delayed-wrap cursor is still at
        // column 3. b belongs at the start of the next line.
        assert!(prediction.reconcile(&frame));
        assert_eq!(text(&prediction), "b");
        assert_eq!(prediction.start(), Some(Position { column: 0, row: 1 }));
        frame.cells[4].text = "b".into();
        frame.cursor = (1, 1);
        assert!(prediction.reconcile(&frame));
        assert!(prediction.is_empty());
    }
    #[test]
    fn adaptive_latency_uses_the_reference_hysteresis() {
        let mut adaptive = Adaptive::default();
        assert!(!adaptive.update(Some(79)));
        assert!(adaptive.update(Some(80)));
        assert!(adaptive.update(Some(60)));
        assert!(adaptive.update(None));
        assert!(!adaptive.update(Some(40)));
        assert!(!adaptive.update(Some(60)));
        assert!(Mode::Always.enabled(false));
        assert!(!Mode::Never.enabled(true));
    }
}
