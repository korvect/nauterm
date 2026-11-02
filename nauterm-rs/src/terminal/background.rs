//! Default cells already have the surface color; only emit non-default spans.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Run {
    pub start: usize,
    pub end: usize,
    pub color: u32,
}
pub(super) fn runs(
    cells: &[crate::native::Cell],
    selection: Option<Selection>,
    palette: TerminalPalette,
) -> Vec<Run> {
    let mut result: Vec<Run> = Vec::new();
    for (column, cell) in cells.iter().enumerate() {
        let color = if selection.is_some_and(|range| range.contains(column as i64)) {
            palette.selection
        } else {
            cell_colors(cell.foreground, cell.background, cell.flags).1
        };
        if color == palette.base.background {
            continue;
        }
        if let Some(last) = result.last_mut()
            && last.end == column
            && last.color == color
        {
            last.end += 1;
        } else {
            result.push(Run {
                start: column,
                end: column + 1,
                color,
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::runs;
    use crate::{native::Cell, terminal_appearance::Appearance, terminal_search::Selection};

    #[test]
    fn empty_large_terminal_needs_no_cell_background_quads() {
        let p = Appearance::default().palette();
        let cells = vec![
            Cell {
                text: String::new(),
                foreground: p.base.foreground,
                background: p.base.background,
                flags: 0,
                hyperlink: None
            };
            1000
        ];
        assert!(runs(&cells, None, p).is_empty());
        let selected = runs(
            &cells,
            Some(Selection {
                start: 10,
                end: 990,
            }),
            p,
        );
        assert_eq!(selected.len(), 1);
        assert_eq!(
            (selected[0].start, selected[0].end, selected[0].color),
            (10, 990, p.selection)
        );
    }
    #[test]
    fn inverse_selection_and_default_gaps_preserve_exact_colors() {
        let p = Appearance::default().palette();
        let cell = |fg, bg, flags| Cell {
            text: String::new(),
            foreground: fg,
            background: bg,
            flags,
            hyperlink: None,
        };
        let cells = vec![
            cell(p.base.foreground, 0xff0000, 0),
            cell(p.base.foreground, 0xff0000, 0),
            cell(p.base.foreground, p.base.background, 0),
            cell(0xff0000, p.base.background, 1),
            cell(p.base.foreground, p.base.background, 0),
        ];
        let spans = runs(&cells, Some(Selection { start: 4, end: 5 }), p);
        assert_eq!(
            spans
                .iter()
                .map(|span| (span.start, span.end, span.color))
                .collect::<Vec<_>>(),
            vec![(0, 2, 0xff0000), (3, 4, 0xff0000), (4, 5, p.selection)]
        );
    }
}
