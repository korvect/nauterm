//! Retain shaped rows across cursor blinks, hover, resizing surrounding panels
//! and output updates that leave those rows unchanged. No session/IO in here.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Overlay {
    selection: Option<Selection>,
    cursor: Option<u32>,
}
impl Overlay {
    fn row(
        frame: &Frame,
        row: usize,
        selection: Option<Selection>,
        cursor: Option<(u32, u32)>,
    ) -> Self {
        let base = (i64::from(frame.history_lines) - i64::from(frame.display_offset) + row as i64)
            * i64::from(frame.columns);
        let selection = selection.and_then(|range| {
            let start = range.start.max(base) - base;
            let end = range.end.min(base + i64::from(frame.columns)) - base;
            (start < end).then_some(Selection { start, end })
        });
        Self {
            selection,
            cursor: cursor
                .filter(|(column, y)| *y as usize == row && *column < frame.columns)
                .map(|(column, _)| column),
        }
    }
}

struct Rows<T, K> {
    frame: Option<Arc<Frame>>,
    key: Option<K>,
    selection: Option<Selection>,
    cursor: Option<(u32, u32)>,
    overlays: Vec<Overlay>,
    rows: Arc<[Arc<T>]>,
}
impl<T, K> Default for Rows<T, K> {
    fn default() -> Self {
        Self {
            frame: None,
            key: None,
            selection: None,
            cursor: None,
            overlays: Vec::new(),
            rows: Arc::from([]),
        }
    }
}
impl<T, K: PartialEq> Rows<T, K> {
    fn prepare(
        &mut self,
        frame: &Arc<Frame>,
        key: K,
        selection: Option<Selection>,
        cursor: Option<(u32, u32)>,
        mut prepare: impl FnMut(&[crate::native::Cell], Overlay) -> T,
    ) -> Arc<[Arc<T>]> {
        let same_frame = self
            .frame
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, frame));
        let same_key = self.key.as_ref() == Some(&key);
        if same_frame && same_key && self.selection == selection && self.cursor == cursor {
            return self.rows.clone();
        }
        let reusable = same_key
            && self
                .frame
                .as_ref()
                .is_some_and(|old| old.columns == frame.columns && old.rows == frame.rows);
        let columns = frame.columns as usize;
        let mut overlays = Vec::new();
        let mut rows = Vec::new();
        let mut shift = self.frame.as_ref().and_then(|old| {
            let old_base = i64::from(old.history_lines) - i64::from(old.display_offset);
            let new_base = i64::from(frame.history_lines) - i64::from(frame.display_offset);
            (old_base != new_base)
                .then(|| isize::try_from(new_base - old_base).ok())
                .flatten()
        });
        let mut inferred = shift.is_some();
        if columns > 0 {
            for (row, cells) in frame
                .cells
                .chunks(columns)
                .take(frame.rows as usize)
                .enumerate()
            {
                let overlay = Overlay::row(frame, row, selection, cursor);
                let same_cells = same_frame
                    || self.frame.as_ref().is_some_and(|old| {
                        old.cells.get(row * columns..row * columns + cells.len()) == Some(cells)
                    });
                let direct = reusable && same_cells && self.overlays.get(row) == Some(&overlay);
                let mut source = direct.then_some(row);
                if source.is_none() && reusable && !same_frame {
                    let old = self.frame.as_ref().unwrap();
                    // Once scrollback is full, history_lines stops increasing.
                    // Infer one shift from the first changed row, then validate
                    // every candidate. Never search all old rows for every row.
                    if !same_cells && !inferred {
                        inferred = true;
                        shift = old
                            .cells
                            .chunks(columns)
                            .take(old.rows as usize)
                            .position(|old_cells| old_cells == cells)
                            .map(|old_row| old_row as isize - row as isize);
                    }
                    source = shift
                        .and_then(|shift| row.checked_add_signed(shift))
                        .filter(|old_row| {
                            self.overlays.get(*old_row) == Some(&overlay)
                                && old
                                    .cells
                                    .get(old_row * columns..old_row * columns + cells.len())
                                    == Some(cells)
                        });
                }
                rows.push(if let Some(source) = source {
                    self.rows[source].clone()
                } else {
                    Arc::new(prepare(cells, overlay))
                });
                overlays.push(overlay);
            }
        }
        self.frame = Some(frame.clone());
        self.key = Some(key);
        self.selection = selection;
        self.cursor = cursor;
        self.rows = rows.into();
        self.overlays = overlays;
        self.rows.clone()
    }
}

#[derive(Clone, PartialEq)]
struct Style {
    font: Font,
    size: f32,
    bold_weight: u16,
    cell: Size<Pixels>,
    palette: TerminalPalette,
}
struct Row {
    background: Vec<background::Run>,
    selection: Option<background::Run>,
    text: Vec<text::PreparedRun>,
    decorations: decorations::Prepared,
}
pub(super) struct Scene {
    rows: Arc<[Arc<Row>]>,
    cell: Size<Pixels>,
    metrics: Option<decorations::Metrics>,
}
#[derive(Default)]
pub(super) struct Cache {
    rows: Rows<Row, Style>,
    scene: Option<Arc<Scene>>,
}
pub(super) struct Request<'a> {
    pub frame: &'a Arc<Frame>,
    pub appearance: &'a Appearance,
    pub font: &'a Font,
    pub cell: Size<Pixels>,
    pub selection: Option<Selection>,
    pub cursor: Option<(u32, u32)>,
}
impl Cache {
    pub fn prepare(&mut self, request: Request<'_>, window: &Window) -> Arc<Scene> {
        let Request {
            frame,
            appearance,
            font,
            cell,
            selection,
            cursor,
        } = request;
        let palette = appearance.palette();
        let style = Style {
            font: font.clone(),
            size: appearance.size,
            bold_weight: appearance.bold_weight,
            cell,
            palette,
        };
        let rows = self
            .rows
            .prepare(frame, style, selection, cursor, |cells, overlay| Row {
                background: background::runs(cells, None, palette),
                selection: overlay.selection.map(|range| background::Run {
                    start: range.start as usize,
                    end: range.end as usize,
                    color: palette.selection,
                }),
                text: text::prepare_row(
                    cells,
                    overlay.selection,
                    overlay.cursor,
                    appearance,
                    font,
                    cell,
                    window,
                ),
                decorations: decorations::Prepared::row(cells),
            });
        if let Some(scene) = &self.scene
            && Arc::ptr_eq(&scene.rows, &rows)
        {
            return scene.clone();
        }
        let metrics = rows
            .iter()
            .any(|row| !row.decorations.is_empty())
            .then(|| decorations::Metrics::measure(font, appearance.size, cell.height, window));
        let scene = Arc::new(Scene {
            rows,
            cell,
            metrics,
        });
        self.scene = Some(scene.clone());
        scene
    }
}
impl Scene {
    pub fn paint_selection(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        for (index, row) in self.rows.iter().enumerate() {
            if let Some(span) = &row.selection {
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin
                            + point(
                                self.cell.width * span.start as f32,
                                self.cell.height * index as f32,
                            ),
                        size(
                            self.cell.width * (span.end - span.start) as f32,
                            self.cell.height,
                        ),
                    ),
                    rgb(span.color),
                ));
            }
        }
    }
    pub fn paint_background(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        // Paint every background first so wide/spacer cells cannot erase glyphs.
        for (index, row) in self.rows.iter().enumerate() {
            let top = bounds.top() + self.cell.height * index as f32;
            for span in &row.background {
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.left() + self.cell.width * span.start as f32, top),
                        size(
                            self.cell.width * (span.end - span.start) as f32,
                            self.cell.height,
                        ),
                    ),
                    rgb(span.color),
                ));
            }
        }
    }
    pub fn paint_text(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        for (index, row) in self.rows.iter().enumerate() {
            if row.text.is_empty() {
                continue;
            }
            let top = bounds.top() + self.cell.height * index as f32;
            let row_bounds = Bounds::new(
                point(bounds.left(), top),
                size(bounds.size.width, self.cell.height),
            );
            window.with_content_mask(Some(ContentMask { bounds: row_bounds }), |window| {
                for run in &row.text {
                    run.paint(
                        point(bounds.left() + self.cell.width * run.column as f32, top),
                        self.cell.height,
                        window,
                        cx,
                    );
                }
            });
        }
        if let Some(metrics) = &self.metrics {
            for (index, row) in self.rows.iter().enumerate() {
                let top = bounds.top() + self.cell.height * index as f32;
                let row_bounds = Bounds::new(
                    point(bounds.left(), top),
                    size(bounds.size.width, self.cell.height),
                );
                row.decorations
                    .paint(row_bounds, self.cell, metrics, window);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Overlay, Rows, Style};
    use crate::{
        native::{Cell, Frame},
        terminal_appearance::Appearance,
        terminal_search::Selection,
    };
    use gpui_kit::{FontFallbacks, font, px, size};
    use std::{cell::Cell as Counter, sync::Arc};

    fn frame(columns: u32, rows: u32) -> Arc<Frame> {
        Arc::new(Frame {
            columns,
            rows,
            cells: vec![
                Cell {
                    text: String::new(),
                    foreground: 0xffffff,
                    background: 0,
                    flags: 0,
                    hyperlink: None
                };
                columns as usize * rows as usize
            ],
            ..Default::default()
        })
    }

    #[test]
    fn large_unchanged_grid_reuses_prepared_rows_and_blink_changes_only_cursor_row() {
        let frame = frame(500, 160);
        let calls = Counter::new(0);
        let prepare = |_: &[Cell], _: Overlay| {
            calls.set(calls.get() + 1);
            calls.get()
        };
        let mut cache = Rows::default();
        let initial = cache.prepare(&frame, 0, None, None, prepare);
        assert_eq!(calls.get(), 160);
        for _ in 0..20 {
            let next = cache.prepare(&frame, 0, None, None, prepare);
            assert!(Arc::ptr_eq(&initial, &next));
        }
        assert_eq!(calls.get(), 160);
        let blink = cache.prepare(&frame, 0, None, Some((3, 100)), prepare);
        assert_eq!(calls.get(), 161);
        assert!(!Arc::ptr_eq(&initial[100], &blink[100]));
        assert!(Arc::ptr_eq(&initial[99], &blink[99]));
        cache.prepare(&frame, 0, None, None, prepare);
        assert_eq!(calls.get(), 162);
    }

    #[test]
    fn output_rebuilds_only_changed_rows_and_selection_tracks_scrollback_coordinates() {
        let mut first = frame(100, 20);
        Arc::make_mut(&mut first).history_lines = 80;
        Arc::make_mut(&mut first).display_offset = 10;
        let calls = Counter::new(0);
        let prepare = |_: &[Cell], _: Overlay| {
            calls.set(calls.get() + 1);
            calls.get()
        };
        let mut cache = Rows::default();
        let initial = cache.prepare(&first, 0, None, None, prepare);
        let mut second = (*first).clone();
        second.cells[1203].text = "中".into();
        second.cells[1203].flags = 0x20;
        second.cells[1204].flags = 0x40;
        let second = Arc::new(second);
        let output = cache.prepare(&second, 0, None, None, prepare);
        assert_eq!(calls.get(), 21);
        assert!(!Arc::ptr_eq(&initial[12], &output[12]));
        assert!(Arc::ptr_eq(&initial[11], &output[11]));
        let selected = cache.prepare(
            &second,
            0,
            Some(Selection {
                start: 7005,
                end: 7110,
            }),
            None,
            prepare,
        );
        assert_eq!(calls.get(), 23);
        assert!(!Arc::ptr_eq(&output[0], &selected[0]));
        assert!(!Arc::ptr_eq(&output[1], &selected[1]));
        assert!(Arc::ptr_eq(&output[2], &selected[2]));
        assert_eq!(
            cache.overlays[0].selection,
            Some(Selection { start: 5, end: 100 })
        );
        assert_eq!(
            cache.overlays[1].selection,
            Some(Selection { start: 0, end: 10 })
        );
        let mut scrolled = (*second).clone();
        scrolled.display_offset += 1;
        cache.prepare(
            &Arc::new(scrolled),
            0,
            Some(Selection {
                start: 7005,
                end: 7110,
            }),
            None,
            prepare,
        );
        assert_eq!(cache.overlays[0].selection, None);
        assert_eq!(
            cache.overlays[1].selection,
            Some(Selection { start: 5, end: 100 })
        );
    }

    #[test]
    fn fonts_palettes_and_cell_metrics_invalidate_shaped_rows() {
        let frame = frame(80, 3);
        let style = Style {
            font: font("Synthetic Mono"),
            size: 12.,
            bold_weight: 700,
            cell: size(px(8.), px(16.)),
            palette: Appearance::default().palette(),
        };
        let calls = Counter::new(0);
        let prepare = |_: &[Cell], _: Overlay| {
            calls.set(calls.get() + 1);
            calls.get()
        };
        let mut cache = Rows::default();
        cache.prepare(&frame, style.clone(), None, None, prepare);
        let mut cjk = style.clone();
        cjk.font.fallbacks = Some(FontFallbacks(Arc::new(vec!["Synthetic CJK".into()])));
        let mut theme = cjk.clone();
        theme.palette.selection_text ^= 0xffffff;
        let mut resized = theme.clone();
        resized.cell.width = px(9.);
        let mut bold = resized.clone();
        bold.bold_weight = 800;
        for (index, changed) in [cjk, theme, resized, bold].into_iter().enumerate() {
            cache.prepare(&frame, changed, None, None, prepare);
            assert_eq!(calls.get(), (index + 2) * 3);
        }
    }

    #[test]
    fn scrolling_reuses_moved_rows_even_after_history_reaches_its_limit() {
        for capped in [false, true] {
            let mut first = (*frame(20, 4)).clone();
            first.history_lines = 10_000;
            for (row, cells) in first.cells.chunks_mut(20).enumerate() {
                cells[0].text = row.to_string();
            }
            let first = Arc::new(first);
            let calls = Counter::new(0);
            let prepare = |_: &[Cell], _: Overlay| {
                calls.set(calls.get() + 1);
                calls.get()
            };
            let mut cache = Rows::default();
            let original = cache.prepare(&first, 0, None, None, prepare);
            let mut second = (*first).clone();
            second.cells[..60].clone_from_slice(&first.cells[20..80]);
            second.cells[60].text = "N".into();
            if !capped {
                second.history_lines += 1;
            }
            let scrolled = cache.prepare(&Arc::new(second), 0, None, None, prepare);
            assert_eq!(calls.get(), 5);
            for row in 0..3 {
                assert!(Arc::ptr_eq(&original[row + 1], &scrolled[row]));
            }
        }
    }
}
