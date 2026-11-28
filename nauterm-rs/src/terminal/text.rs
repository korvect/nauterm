//! Shape same-style ASCII spans together without losing terminal cell positions.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Emphasis {
    Normal,
    Selection,
    Cursor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Style {
    foreground: u32,
    bold: bool,
    italic: bool,
    emphasis: Emphasis,
}

#[derive(Debug, PartialEq, Eq)]
struct Run {
    row: usize,
    column: usize,
    text: String,
    style: Style,
    ascii: bool,
}

#[cfg(test)]
fn runs(
    frame: &Frame,
    selection: Option<Selection>,
    cursor: Option<(u32, u32)>,
    palette: TerminalPalette,
) -> Vec<Run> {
    if frame.columns == 0 {
        return Vec::new();
    }
    let base = (i64::from(frame.history_lines) - i64::from(frame.display_offset))
        * i64::from(frame.columns);
    frame
        .cells
        .chunks(frame.columns as usize)
        .take(frame.rows as usize)
        .enumerate()
        .flat_map(|(row, cells)| {
            row_runs(
                cells,
                row,
                base + row as i64 * i64::from(frame.columns),
                selection,
                cursor,
                palette,
            )
        })
        .collect()
}
fn row_runs(
    cells: &[crate::native::Cell],
    row: usize,
    base: i64,
    selection: Option<Selection>,
    cursor: Option<(u32, u32)>,
    palette: TerminalPalette,
) -> Vec<Run> {
    let mut result = Vec::new();
    let mut pending: Option<Run> = None;
    let mut pending_gap = 0;
    for (column, cell) in cells.iter().enumerate() {
        if cell.flags & (0x0040 | 0x0100 | 0x0400) != 0 {
            result.extend(pending.take());
            pending_gap = 0;
            continue;
        }
        let absolute = base + column as i64;
        let (foreground, emphasis) = if cursor == Some((column as u32, row as u32)) {
            (palette.cursor_text, Emphasis::Cursor)
        } else if selection.is_some_and(|selection| selection.contains(absolute)) {
            (palette.selection_text, Emphasis::Selection)
        } else {
            (
                cell_colors(
                    cell.foreground,
                    cell.background,
                    if powerline::supports(&cell.text) {
                        cell.flags & !0x80
                    } else {
                        cell.flags
                    },
                )
                .0,
                Emphasis::Normal,
            )
        };
        let style = Style {
            foreground,
            bold: cell.flags & 2 != 0,
            italic: cell.flags & 4 != 0,
            emphasis,
        };
        let text = if cell.text.is_empty() {
            " "
        } else {
            &cell.text
        };
        let ascii = cell.flags & 0x20 == 0
            && text.len() == 1
            && (b' '..=b'~').contains(&text.as_bytes()[0]);
        let joins = ascii && pending.as_ref().is_some_and(|run| run.style == style);
        if text.trim().is_empty() {
            if joins && cell.text.is_empty() {
                // Empty cells may be the entire unused right side of a large
                // viewport. Materialize them only if more text follows.
                pending_gap += 1;
            } else if joins {
                let pending = &mut pending.as_mut().unwrap().text;
                pending.extend(std::iter::repeat_n(' ', pending_gap));
                pending_gap = 0;
                pending.push(' ');
            } else {
                result.extend(pending.take());
                pending_gap = 0;
            }
            continue;
        }
        if joins {
            let pending = &mut pending.as_mut().unwrap().text;
            pending.extend(std::iter::repeat_n(' ', pending_gap));
            pending_gap = 0;
            pending.push_str(text);
            continue;
        }
        result.extend(pending.take());
        pending_gap = 0;
        let run = Run {
            row,
            column,
            text: text.to_owned(),
            style,
            ascii,
        };
        if ascii {
            pending = Some(run);
        } else {
            result.push(run);
        }
    }
    // Preserve written spaces (including trailing ones) and their shaping
    // context; only unwritten right padding is omitted.
    result.extend(pending);
    result
}

/// ASCII byte indices are terminal columns. A ligature may consume several
/// columns but produce one glyph; counting glyphs would shift following text.
/// Keep a cluster's internal offsets, including multiple glyphs at one index.
fn grid_layout(layout: &LineLayout, columns: usize, width: Pixels) -> LineLayout {
    let mut origins = vec![None::<Pixels>; columns];
    for glyph in layout.runs.iter().flat_map(|run| &run.glyphs) {
        if let Some(origin) = origins.get_mut(glyph.index) {
            *origin = Some(origin.map_or(glyph.position.x, |x| x.min(glyph.position.x)));
        }
    }
    let mut runs = layout.runs.clone();
    for glyph in runs.iter_mut().flat_map(|run| &mut run.glyphs) {
        if let Some(Some(origin)) = origins.get(glyph.index) {
            glyph.position.x = width * glyph.index as f32 + (glyph.position.x - *origin);
        }
    }
    LineLayout {
        font_size: layout.font_size,
        width: width * columns as f32,
        ascent: layout.ascent,
        descent: layout.descent,
        runs,
        len: layout.len,
    }
}

pub(super) struct PreparedRun {
    pub column: usize,
    content: Content,
}
enum Content {
    Text(Box<ShapedLine>),
    Powerline(Box<powerline::Prepared>),
}
impl PreparedRun {
    pub fn paint(&self, origin: Point<Pixels>, height: Pixels, window: &mut Window, cx: &mut App) {
        match &self.content {
            Content::Text(line) => {
                let _ = line.paint(origin, height, TextAlign::Left, None, window, cx);
            }
            Content::Powerline(glyph) => glyph.paint(origin, window),
        }
    }
}

pub(super) fn prepare_row(
    cells: &[crate::native::Cell],
    selection: Option<Selection>,
    cursor: Option<u32>,
    appearance: &Appearance,
    font: &Font,
    cell: Size<Pixels>,
    window: &Window,
) -> Vec<PreparedRun> {
    row_runs(
        cells,
        0,
        0,
        selection,
        cursor.map(|column| (column, 0)),
        appearance.palette(),
    )
    .into_iter()
    .map(|run| {
        if let Some(glyph) =
            powerline::Prepared::new(&run.text, cell, appearance.size, run.style.foreground)
        {
            return PreparedRun {
                column: run.column,
                content: Content::Powerline(Box::new(glyph)),
            };
        }
        let mut face = font.clone();
        if run.style.bold {
            face.weight = FontWeight(f32::from(appearance.bold_weight));
        }
        if run.style.italic {
            face.style = FontStyle::Italic;
        }
        let text: SharedString = run.text.into();
        let style = TextRun {
            len: text.len(),
            font: face,
            color: rgb(run.style.foreground).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let mut line = window
            .text_system()
            .shape_line(text, px(appearance.size), &[style], None);
        if run.ascii {
            *line = Arc::new(grid_layout(&line, line.len(), cell.width));
        }
        PreparedRun {
            column: run.column,
            content: Content::Text(Box::new(line)),
        }
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::Cell;
    use core::prelude::v1::test;

    fn cell(text: &str, flags: u16) -> Cell {
        Cell {
            text: text.into(),
            foreground: 0xeeeeee,
            background: 0,
            flags,
            hyperlink: None,
        }
    }

    #[test]
    fn powerline_uses_explicit_colors_but_selection_and_cursor_take_precedence() {
        let frame = Frame {
            columns: 3,
            rows: 1,
            cells: vec![
                cell("\u{e0b0}", 0x80),
                cell("\u{e0b2}", 0x81),
                cell("x", 0x80),
            ],
            ..Default::default()
        };
        let palette = Appearance::default().palette();
        let normal = runs(&frame, None, None, palette);
        assert_eq!(normal[0].style.foreground, 0xeeeeee);
        assert_eq!(normal[1].style.foreground, 0);
        assert_eq!(normal[2].style.foreground, cell_colors(0xeeeeee, 0, 0x80).0);
        let highlighted = runs(
            &frame,
            Some(Selection { start: 0, end: 1 }),
            Some((1, 0)),
            palette,
        );
        assert_eq!(highlighted[0].style.foreground, palette.selection_text);
        assert_eq!(highlighted[1].style.foreground, palette.cursor_text);
    }

    #[test]
    fn right_padding_does_not_enter_text_layout_but_interior_spaces_keep_columns() {
        let mut cells = vec![cell("", 0); 1000];
        for (index, character) in "foo  bar".chars().enumerate() {
            cells[index] = cell(&character.to_string(), 0);
        }
        let frame = Frame {
            columns: 1000,
            rows: 1,
            cells,
            ..Default::default()
        };
        let prepared = runs(&frame, None, None, Appearance::default().palette());
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].column, 0);
        assert_eq!(prepared[0].text, "foo  bar");
    }

    #[test]
    fn unwritten_gaps_and_explicit_trailing_spaces_retain_shaping_context() {
        let frame = Frame {
            columns: 10,
            rows: 1,
            cells: vec![
                cell("=", 0),
                cell("", 0),
                cell("", 0),
                cell(">", 0),
                cell(" ", 0),
                cell(" ", 0),
                cell("", 0),
                cell("", 0),
                cell("", 0),
                cell("", 0),
            ],
            ..Default::default()
        };
        let prepared = runs(&frame, None, None, Appearance::default().palette());
        assert_eq!(prepared[0].text, "=  >  ");
    }

    #[test]
    fn ascii_runs_keep_grid_gaps_rows_and_graphemes_independent() {
        let frame = Frame {
            columns: 8,
            rows: 2,
            cells: vec![
                cell("", 0),
                cell("-", 0),
                cell(">", 0),
                cell(" ", 0),
                cell("中", 0x20),
                cell("", 0x40),
                cell("=", 0),
                cell(">", 0),
                cell("!", 0),
                cell("=", 0),
                cell("secret", 0x100),
                cell("e\u{301}", 0),
                cell("👩‍💻", 0x20),
                cell("", 0x40),
                cell("x", 0),
                cell("y", 2),
            ],
            ..Default::default()
        };
        let runs = runs(&frame, None, None, Appearance::default().palette());
        let text: Vec<_> = runs
            .iter()
            .map(|run| (run.row, run.column, run.text.as_str(), run.ascii))
            .collect();
        assert_eq!(
            text,
            [
                (0, 1, "-> ", true),
                (0, 4, "中", false),
                (0, 6, "=>", true),
                (1, 0, "!=", true),
                (1, 3, "e\u{301}", false),
                (1, 4, "👩‍💻", false),
                (1, 6, "x", true),
                (1, 7, "y", true),
            ]
        );
        assert!(runs.last().unwrap().style.bold);
    }

    #[test]
    fn selection_and_cursor_break_ligatures_even_when_text_colors_match() {
        let frame = Frame {
            columns: 5,
            rows: 1,
            history_lines: 20,
            display_offset: 3,
            cells: vec![cell("=", 0); 5],
            ..Default::default()
        };
        let mut palette = Appearance::default().palette();
        palette.selection_text = 0xeeeeee;
        palette.cursor_text = 0xeeeeee;
        let selected = Selection { start: 86, end: 89 };
        let runs = runs(&frame, Some(selected), Some((2, 0)), palette);
        let spans: Vec<_> = runs
            .iter()
            .map(|run| (run.column, run.text.as_str(), run.style.emphasis))
            .collect();
        assert_eq!(
            spans,
            [
                (0, "=", Emphasis::Normal),
                (1, "=", Emphasis::Selection),
                (2, "=", Emphasis::Cursor),
                (3, "=", Emphasis::Selection),
                (4, "=", Emphasis::Normal),
            ]
        );
    }

    #[test]
    fn grid_placement_uses_source_indices_instead_of_ligature_glyph_count() {
        let glyph = |index, x, y| ShapedGlyph {
            id: GlyphId(1),
            position: point(px(x), px(y)),
            index,
            is_emoji: false,
        };
        let original = LineLayout {
            font_size: px(12.),
            width: px(28.8),
            ascent: px(9.),
            descent: px(3.),
            runs: vec![
                ShapedRun {
                    font_id: FontId(0),
                    // One glyph consumes source bytes 0 and 1.
                    glyphs: vec![glyph(0, 0., 0.), glyph(2, 14.4, 0.)],
                },
                ShapedRun {
                    font_id: FontId(1),
                    // A second glyph belongs to the same source cluster.
                    glyphs: vec![glyph(2, 15.2, -2.), glyph(3, 21.6, 0.)],
                },
            ],
            len: 4,
        };
        let placed = grid_layout(&original, 4, px(8.));
        assert_eq!(placed.width, px(32.));
        assert_eq!(placed.runs[0].glyphs[1].position.x, px(16.));
        assert!((f32::from(placed.runs[1].glyphs[0].position.x) - 16.8).abs() < 0.001);
        assert_eq!(placed.runs[1].glyphs[0].position.y, px(-2.));
        assert_eq!(placed.runs[1].glyphs[1].position.x, px(24.));
        assert_eq!(original.runs[0].glyphs[1].position.x, px(14.4));
    }
}
