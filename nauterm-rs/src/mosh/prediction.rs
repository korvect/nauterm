//! Mosh ACKs acknowledge delivery, not rendered echo. Retire batches only after
//! a native screen commit, using the resulting authoritative snapshot.
use crate::{
    native::{Frame, SessionEvent},
    terminal_prediction::{
        Adaptive, CELL_BUDGET, Position, Prediction as Screen, Unit, cell_width,
    },
};
use std::collections::VecDeque;
use unicode_segmentation::UnicodeSegmentation;
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
struct Batch {
    id: u64,
    start: Position,
    erase: Option<Position>,
}
struct Pending {
    batch: u64,
    unit: Unit,
}
enum Snapshot {
    Confirmed(Option<Position>),
    Pending,
    Mismatch,
}
pub struct Prediction {
    units: VecDeque<Pending>,
    batches: VecDeque<Batch>,
    next_batch: u64,
    confirmed: Option<Position>,
    echo: bool,
    adaptive: Adaptive,
    paused: bool,
    commit_pending: bool,
    size: Option<(u32, u32)>,
}
impl Default for Prediction {
    fn default() -> Self {
        Self {
            units: VecDeque::new(),
            batches: VecDeque::new(),
            next_batch: 0,
            confirmed: None,
            echo: false,
            // Flutter starts Mosh adaptive prediction enabled until the first
            // latency sample. SSH's adaptive baseline is deliberately different.
            adaptive: Adaptive::new(true),
            paused: false,
            commit_pending: false,
            size: None,
        }
    }
}
impl Prediction {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
    pub fn units(&self) -> impl Iterator<Item = &Unit> {
        self.units.iter().map(|pending| &pending.unit)
    }
    pub fn start(&self) -> Option<Position> {
        self.units.front().map(|pending| pending.unit.start)
    }
    pub fn clear(&mut self) -> bool {
        let changed = !self.is_empty();
        self.units.clear();
        self.batches.clear();
        self.confirmed = None;
        self.commit_pending = false;
        changed
    }
    pub fn suspend(&mut self) -> bool {
        let changed = self.clear();
        self.paused = true;
        changed
    }
    pub fn event(&mut self, event: &SessionEvent) -> bool {
        match event.kind.as_str() {
            "mosh_echo_enabled" => self.echo = true,
            "mosh_echo_disabled" => {
                self.echo = false;
                self.paused = false;
                return self.clear();
            }
            "mosh_latency_updated" => {
                self.adaptive.update(event.latency_ms);
            }
            "mosh_screen_committed" => self.commit_pending = true,
            "connect_start" | "retry" | "error" | "connection_lost" | "exit_status"
            | "session_closed" => {
                let changed = !self.is_empty();
                *self = Self::default();
                return changed;
            }
            // Neither delivery ACKs nor queued input state numbers prove that
            // the terminal engine contains the echo. Network roaming retains
            // the same transport and therefore also retains its predictions.
            _ => {}
        }
        false
    }
    fn cursor(&self, frame: &Frame) -> Position {
        self.batches
            .back()
            .filter(|batch| self.units.back().is_none_or(|unit| unit.batch <= batch.id))
            .and_then(|batch| batch.erase)
            .or_else(|| self.units.back().map(|pending| pending.unit.end))
            .unwrap_or_else(|| Position::cursor(frame))
    }
    pub fn input(&mut self, text: &str, frame: &Frame) -> bool {
        if !self.echo || self.paused || !Screen::allows(frame) {
            return self.clear();
        }
        if text.is_empty()
            || text.len() > 16 * 1024
            || text.chars().any(|ch| ch < ' ' && ch != '\u{8}')
        {
            return self.suspend();
        }
        let erase = text
            .graphemes(true)
            .all(|g| matches!(g, "\u{8}" | "\u{7f}"));
        let start = self.cursor(frame);
        if !self.adaptive.update(None) && (erase || self.confirmed != Some(start)) {
            return false;
        }
        self.size = Some((frame.columns, frame.rows));
        let Some(id) = self.next_batch.checked_add(1) else {
            return self.suspend();
        };
        self.next_batch = id;
        let mut changed = false;
        let base = self.batches.front().map_or(start, |batch| batch.start);
        let mut cursor = start;
        for text in text.graphemes(true) {
            if matches!(text, "\u{8}" | "\u{7f}") {
                if self.units.pop_back().is_none() {
                    // Geometry of already-confirmed text is unknown until the
                    // next screen. Never guess an erase into a password/prompt.
                    return self.suspend() || changed;
                }
                changed = true;
                cursor = self.units.back().map_or(base, |pending| pending.unit.end);
            } else {
                let width = cell_width(text);
                if width == 0 {
                    continue;
                }
                let start = cursor.fit(width, frame.columns);
                cursor = start.after(width, frame.columns);
                self.units.push_back(Pending {
                    batch: id,
                    unit: Unit {
                        text: Zeroizing::new(text.into()),
                        width,
                        start,
                        end: cursor,
                    },
                });
                changed = true;
            }
        }
        let expected = erase.then_some(cursor);
        self.batches.push_back(Batch {
            id,
            start,
            erase: expected,
        });
        self.batches.retain(|batch| {
            batch.erase.is_some() || self.units.iter().any(|unit| unit.batch == batch.id)
        });
        if self
            .units
            .iter()
            .map(|pending| pending.unit.width as usize)
            .sum::<usize>()
            > CELL_BUDGET
            || self.batches.len() > CELL_BUDGET * 2
        {
            self.suspend();
        }
        changed
    }
    fn actual<'a>(frame: &'a Frame, unit: &Unit) -> &'a str {
        frame
            .cells
            .get(unit.start.row as usize * frame.columns as usize + unit.start.column as usize)
            .map_or(" ", |cell| cell.text.as_str())
    }
    fn conflict(frame: &Frame, unit: &Unit) -> bool {
        let actual = Self::actual(frame, unit);
        !actual.is_empty() && actual != " " && actual != unit.text.as_str()
    }
    fn snapshot(&self, batch: Batch, frame: &Frame) -> Snapshot {
        if let Some(cursor) = batch.erase {
            let blank = frame
                .cells
                .get(cursor.row as usize * frame.columns as usize + cursor.column as usize)
                .is_some_and(|cell| cell.text.is_empty() || cell.text == " ");
            return if Position::cursor(frame) == cursor && blank {
                Snapshot::Confirmed(None)
            } else {
                Snapshot::Pending
            };
        }
        let mut end = None;
        let mut matches = true;
        for pending in self.units.iter().filter(|unit| unit.batch == batch.id) {
            end = Some(pending.unit.end);
            matches &= Self::actual(frame, &pending.unit) == pending.unit.text.as_str();
            if Self::conflict(frame, &pending.unit) {
                return Snapshot::Mismatch;
            }
        }
        match end {
            Some(end) if matches => Snapshot::Confirmed(Some(end)),
            Some(end) if Position::cursor(frame) >= end => Snapshot::Mismatch,
            _ => Snapshot::Pending,
        }
    }
    /// Use screen commits even when their cells equal the previous frame; an ACK
    /// by itself must never retire or relocate a predicted batch.
    pub fn reconcile(&mut self, frame: &Frame, changed: bool) -> bool {
        let resized = self
            .size
            .replace((frame.columns, frame.rows))
            .is_some_and(|old| old != (frame.columns, frame.rows));
        let committed = std::mem::take(&mut self.commit_pending);
        if resized || !self.echo || !Screen::allows(frame) {
            return self.clear();
        }
        if !changed && !committed {
            return false;
        }
        self.paused = false;
        let before = self.units.len();
        if committed {
            while let Some(batch) = self.batches.front().copied() {
                match self.snapshot(batch, frame) {
                    Snapshot::Confirmed(next) => {
                        self.confirmed = next;
                        self.units.retain(|unit| unit.batch != batch.id);
                        self.batches.pop_front();
                    }
                    Snapshot::Mismatch => {
                        self.clear();
                        break;
                    }
                    Snapshot::Pending => break,
                }
            }
        }
        // A remote cursor preceding the first batch invalidates its origin.
        // Later batches naturally lie ahead of the cursor while input is queued.
        if self
            .batches
            .front()
            .is_some_and(|batch| Position::cursor(frame) < batch.start)
        {
            self.clear();
        } else if let Some(batch) = self
            .units
            .iter()
            .find(|unit| Self::conflict(frame, &unit.unit))
            .map(|unit| unit.batch)
        {
            self.units.retain(|unit| unit.batch < batch);
            self.batches.retain(|pending| pending.id < batch);
            self.confirmed = None;
        }
        if self.confirmed != Some(Position::cursor(frame)) {
            self.confirmed = None;
        }
        before != self.units.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::Cell;
    fn frame(cursor: u32, text: &str) -> Frame {
        let mut frame = Frame {
            columns: 80,
            rows: 24,
            cursor: (cursor, 0),
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
                80 * 24
            ],
            ..Default::default()
        };
        for (cell, ch) in frame.cells.iter_mut().zip(text.chars()) {
            cell.text = ch.to_string();
        }
        frame
    }
    fn event(kind: &str) -> SessionEvent {
        SessionEvent {
            kind: kind.into(),
            ..Default::default()
        }
    }
    fn enabled() -> Prediction {
        let mut model = Prediction::default();
        model.event(&event("mosh_echo_enabled"));
        model
    }
    fn text(model: &Prediction) -> String {
        model.units().map(|unit| unit.text.as_str()).collect()
    }
    fn commit(model: &mut Prediction, frame: &Frame) {
        model.event(&event("mosh_screen_committed"));
        model.reconcile(frame, true);
    }
    #[test]
    fn mosh_ack_waits_for_matching_commits_and_retires_whole_batches_in_order() {
        let mut model = enabled();
        model.input("abc", &frame(0, ""));
        model.input("def", &frame(0, ""));
        for kind in ["mosh_input_state_queued", "mosh_prediction_confirmed"] {
            model.event(&event(kind));
            assert!(!model.reconcile(&frame(0, ""), false));
            assert_eq!(text(&model), "abcdef");
        }
        commit(&mut model, &frame(2, "ab"));
        assert_eq!(text(&model), "abcdef");
        commit(&mut model, &frame(3, "abc"));
        assert_eq!(text(&model), "def");
        assert_eq!(model.start(), Some(Position { row: 0, column: 3 }));
        commit(&mut model, &frame(6, "abcdef"));
        assert!(model.is_empty());
        assert!(model.batches.is_empty());
    }
    #[test]
    fn mosh_commits_do_not_require_an_ack_and_a_snapshot_can_confirm_many_batches() {
        for mismatch in [false, true] {
            let mut model = enabled();
            model.input("abc", &frame(0, ""));
            model.input("def", &frame(0, ""));
            commit(
                &mut model,
                &frame(6, if mismatch { "abcXef" } else { "abcdef" }),
            );
            assert!(model.is_empty());
            assert!(model.batches.is_empty());
        }
    }
    #[test]
    fn mosh_roaming_and_cursor_at_next_origin_retain_uncommitted_input() {
        let mut model = enabled();
        model.input("ab", &frame(3, ""));
        model.input("cd", &frame(3, ""));
        for kind in [
            "mosh_network_switching",
            "mosh_network_degraded",
            "mosh_network_restored",
        ] {
            model.event(&event(kind));
            model.reconcile(&frame(5, ""), true);
            assert_eq!(text(&model), "abcd");
        }
        model.reconcile(&frame(2, ""), true);
        assert!(model.is_empty());
    }
    #[test]
    fn mosh_backspace_removes_graphemes_and_preserves_the_next_input_origin() {
        let mut model = enabled();
        model.input("e\u{301}🙂", &frame(0, ""));
        model.input("\u{8}", &frame(0, ""));
        assert_eq!(text(&model), "e\u{301}");
        model.input("a", &frame(0, ""));
        assert_eq!(text(&model), "e\u{301}a");
        assert_eq!(model.units().last().unwrap().start.column, 1);
        model.input("\u{7f}\u{7f}", &frame(0, ""));
        assert!(model.is_empty());
        model.input("x", &frame(0, ""));
        assert_eq!(text(&model), "x");
        assert_eq!(model.start(), Some(Position { row: 0, column: 0 }));
    }
    #[test]
    fn mosh_sensitive_controls_resize_and_budgets_clear_the_visual_layer() {
        let mut model = Prediction::default();
        assert!(!model.input("secret", &frame(0, "")));
        model.event(&event("mosh_echo_enabled"));
        model.input("abc", &frame(0, ""));
        assert!(model.event(&event("mosh_echo_disabled")));
        model.event(&event("mosh_echo_enabled"));
        for control in ["\r", "\u{1b}[A", "\t"] {
            model.input("abc", &frame(0, ""));
            model.input(control, &frame(0, ""));
            assert!(model.is_empty());
            assert!(!model.input("x", &frame(0, "")));
            commit(&mut model, &frame(0, ""));
        }
        model.input("中", &frame(79, ""));
        assert_eq!(model.start(), Some(Position { row: 1, column: 0 }));
        let resized = Frame {
            columns: 40,
            ..frame(0, "")
        };
        model.reconcile(&resized, true);
        assert!(model.is_empty());
        model.input(&"中".repeat(129), &frame(0, ""));
        assert!(model.is_empty());
        model.event(&event("session_closed"));
        assert!(!model.input("x", &frame(0, "")));
    }
    #[test]
    fn mosh_low_latency_continues_a_confirmed_line_but_not_an_unrelated_prompt() {
        let mut model = enabled();
        model.input("abc", &frame(0, ""));
        commit(&mut model, &frame(3, "abc"));
        model.event(&SessionEvent {
            kind: "mosh_latency_updated".into(),
            latency_ms: Some(40),
            ..Default::default()
        });
        assert!(model.input("d", &frame(3, "abc")));
        assert_eq!(text(&model), "d");
        commit(&mut model, &frame(4, "abcd"));
        model.reconcile(&frame(0, ""), true);
        assert!(!model.input("e", &frame(0, "")));
        model.event(&SessionEvent {
            kind: "mosh_latency_updated".into(),
            latency_ms: Some(80),
            ..Default::default()
        });
        assert!(model.input("e", &frame(0, "")));
    }
}
