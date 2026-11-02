//! Native terminal search coordinates, independent of GPUI and the backend.
use anyhow::{Result, ensure};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Direction {
    Next = 0,
    Previous = 1,
}

/// Half-open range in the retained grid (history begins at zero). Native
/// selection APIs instead use signed offsets relative to the active screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub start: i64,
    pub end: i64,
}
impl Selection {
    pub fn from_native(
        (start, end): (i64, i64),
        columns: u32,
        rows: u32,
        history_lines: u32,
    ) -> Result<Self> {
        let history = i64::from(columns) * i64::from(history_lines);
        ensure!(
            columns > 0
                && rows > 0
                && start >= -history
                && start < end
                && end <= i64::from(columns) * i64::from(rows),
            "Invalid native word selection"
        );
        Ok(Self {
            start: start + history,
            end: end + history,
        })
    }
    pub fn between_cells(anchor: i64, head: i64) -> Self {
        Self {
            start: anchor.min(head),
            end: anchor.max(head) + 1,
        }
    }
    pub fn contains(self, offset: i64) -> bool {
        (self.start..self.end).contains(&offset)
    }
    pub fn native_offsets(self, columns: u32, history_lines: u32) -> (i64, i64) {
        let history = i64::from(columns) * i64::from(history_lines);
        (self.start - history, self.end - history)
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct NativeResult {
    pub found: bool,
    pub match_index: Option<usize>,
    pub total_matches: Option<usize>,
    pub columns: u32,
    pub rows: u32,
    pub start_row: u32,
    pub start_column: u32,
    pub end_row: u32,
    pub end_column: u32,
    pub error: Option<String>,
}
impl NativeResult {
    pub fn validate(&self) -> Result<()> {
        if !self.found {
            ensure!(self.match_index.is_none(), "Invalid native search count");
            return Ok(());
        }
        ensure!(
            (2..=1000).contains(&self.columns) && (1..=500).contains(&self.rows),
            "Invalid native search dimensions"
        );
        ensure!(
            self.start_row < self.rows
                && self.end_row < self.rows
                && self.start_column < self.columns
                && self.end_column <= self.columns
                && (self.start_row, self.start_column) < (self.end_row, self.end_column),
            "Invalid native search selection"
        );
        ensure!(
            matches!((self.match_index, self.total_matches), (None, None))
                || matches!((self.match_index, self.total_matches), (Some(index), Some(total)) if index < total),
            "Invalid native search count"
        );
        Ok(())
    }
    pub fn selection(
        &self,
        columns: u32,
        rows: u32,
        history_lines: u32,
        display_offset: u32,
    ) -> Result<Option<Selection>> {
        self.validate()?;
        if !self.found || self.error.is_some() {
            return Ok(None);
        }
        ensure!(
            (self.columns, self.rows) == (columns, rows),
            "Terminal dimensions changed during search"
        );
        let viewport = viewport_start(columns, history_lines, display_offset);
        Ok(Some(Selection {
            start: viewport
                + i64::from(self.start_row) * i64::from(columns)
                + i64::from(self.start_column),
            end: viewport
                + i64::from(self.end_row) * i64::from(columns)
                + i64::from(self.end_column),
        }))
    }
    pub fn status(&self) -> String {
        if self.error.is_some() {
            "Error".into()
        } else if let Some(total) = self.total_matches {
            format!(
                "{} / {total}",
                self.match_index.map_or(0, |index| index + 1)
            )
        } else if self.found {
            "Found".into()
        } else {
            "No results".into()
        }
    }
}

pub fn viewport_start(columns: u32, history_lines: u32, display_offset: u32) -> i64 {
    (i64::from(history_lines) - i64::from(display_offset)) * i64::from(columns)
}

/// Native origins are viewport-clamped and inclusive. At a viewport edge,
/// reveal the adjacent row before stepping, or move to the opposite buffer
/// edge when wrapping. Otherwise a one-cell match can keep finding itself.
pub fn boundary_step(
    columns: u32,
    rows: u32,
    history_lines: u32,
    display_offset: u32,
    selection: Option<Selection>,
    direction: Direction,
) -> Option<(i32, bool)> {
    let selection = selection?;
    let viewport = viewport_start(columns, history_lines, display_offset);
    let history = history_lines.min(i32::MAX as u32) as i32;
    match direction {
        Direction::Previous if selection.start == viewport => {
            Some(if display_offset < history_lines {
                (1, false)
            } else {
                (-history, true)
            })
        }
        Direction::Next if selection.end == viewport + i64::from(columns) * i64::from(rows) => {
            Some(if display_offset > 0 {
                (-1, false)
            } else {
                (history, true)
            })
        }
        _ => None,
    }
}

pub fn origin(
    columns: u32,
    rows: u32,
    history_lines: u32,
    display_offset: u32,
    selection: Option<Selection>,
    direction: Direction,
) -> (u32, u32) {
    if columns == 0 || rows == 0 {
        return (0, 0);
    }
    let last = i64::from(columns) * i64::from(rows) - 1;
    let viewport = viewport_start(columns, history_lines, display_offset);
    let offset = match (selection, direction) {
        (Some(selection), Direction::Next) => selection.end - viewport,
        (Some(selection), Direction::Previous) => selection.start - viewport - 1,
        (None, Direction::Next) => 0,
        (None, Direction::Previous) => last,
    }
    .clamp(0, last);
    (
        (offset / i64::from(columns)) as u32,
        (offset % i64::from(columns)) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_word_ranges_map_history_and_screen_without_viewport_clamping() -> Result<()> {
        let selection = Selection::from_native((-20, 4), 10, 3, 100)?;
        assert_eq!(
            selection,
            Selection {
                start: 980,
                end: 1004
            }
        );
        assert_eq!(selection.native_offsets(10, 100), (-20, 4));
        assert_eq!(
            Selection::from_native((-1000, 30), 10, 3, 100)?,
            Selection {
                start: 0,
                end: 1030
            }
        );
        for invalid in [(-1001, 0), (0, 31), (0, 0), (3, 2), (i64::MIN, i64::MAX)] {
            assert!(Selection::from_native(invalid, 10, 3, 100).is_err());
        }
        Ok(())
    }
    fn found() -> NativeResult {
        NativeResult {
            found: true,
            match_index: Some(1),
            total_matches: Some(3),
            columns: 80,
            rows: 24,
            start_row: 2,
            start_column: 5,
            end_row: 2,
            end_column: 11,
            error: None,
        }
    }
    #[test]
    fn search_ranges_use_history_for_paint_and_active_screen_for_copy() -> Result<()> {
        let selection = found().selection(80, 24, 30, 20)?.unwrap();
        assert_eq!(
            selection,
            Selection {
                start: 965,
                end: 971
            }
        );
        assert_eq!(selection.native_offsets(80, 30), (-1435, -1429));
        assert!(selection.contains(970));
        assert!(!selection.contains(971));
        assert_eq!(
            Selection::between_cells(8, 3),
            Selection { start: 3, end: 9 }
        );
        Ok(())
    }
    #[test]
    fn next_and_previous_origins_skip_the_current_match() -> Result<()> {
        let selection = found().selection(80, 24, 30, 20)?;
        assert_eq!(origin(80, 24, 30, 20, selection, Direction::Next), (2, 11));
        assert_eq!(
            origin(80, 24, 30, 20, selection, Direction::Previous),
            (2, 4)
        );
        assert_eq!(origin(80, 24, 30, 20, None, Direction::Next), (0, 0));
        assert_eq!(origin(80, 24, 30, 20, None, Direction::Previous), (23, 79));
        assert_eq!(origin(0, 0, 0, 0, None, Direction::Next), (0, 0));
        Ok(())
    }
    #[test]
    fn malformed_coordinates_counts_and_resize_are_rejected() {
        let result = found();
        for invalid in [
            NativeResult {
                columns: 0,
                ..result.clone()
            },
            NativeResult {
                end_column: 81,
                ..result.clone()
            },
            NativeResult {
                end_row: 24,
                ..result.clone()
            },
            NativeResult {
                end_column: 5,
                ..result.clone()
            },
            NativeResult {
                match_index: Some(3),
                ..result.clone()
            },
        ] {
            assert!(invalid.validate().is_err());
        }
        assert!(result.selection(40, 24, 0, 0).is_err());
    }
    #[test]
    fn viewport_edges_reveal_adjacent_rows_or_wrap_the_whole_buffer() {
        let first = Selection { start: 0, end: 1 };
        assert_eq!(
            boundary_step(80, 24, 0, 0, Some(first), Direction::Previous),
            Some((0, true))
        );
        let first_visible = Selection {
            start: 800,
            end: 801,
        };
        assert_eq!(
            boundary_step(80, 24, 30, 20, Some(first_visible), Direction::Previous),
            Some((1, false))
        );
        assert_eq!(
            boundary_step(80, 24, 30, 30, Some(first), Direction::Previous),
            Some((-30, true))
        );
        let last = Selection {
            start: 4319,
            end: 4320,
        };
        assert_eq!(
            boundary_step(80, 24, 30, 0, Some(last), Direction::Next),
            Some((30, true))
        );
    }
    #[test]
    fn counters_are_one_based_but_not_invented_for_uncounted_backends() {
        let mut result = found();
        assert_eq!(result.status(), "2 / 3");
        result.match_index = None;
        result.total_matches = None;
        assert_eq!(result.status(), "Found");
        result.found = false;
        assert_eq!(result.status(), "No results");
        result.total_matches = Some(0);
        assert_eq!(result.status(), "0 / 0");
        result.error = Some("failed".into());
        assert_eq!(result.status(), "Error");
    }
}
