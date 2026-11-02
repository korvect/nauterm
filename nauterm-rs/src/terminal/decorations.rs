//! Cell decorations are row runs, including spaces and wide-cell spacers.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Underline {
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Style {
    underline: Underline,
    strike: bool,
}
impl Style {
    fn from_flags(flags: u16) -> Option<Self> {
        if flags & 0x0100 != 0 {
            return None;
        }
        let underline = if flags & 0x1000 != 0 {
            Underline::Curly
        } else if flags & 0x2000 != 0 {
            Underline::Dotted
        } else if flags & 0x4000 != 0 {
            Underline::Dashed
        } else if flags & 0x0800 != 0 {
            Underline::Double
        } else if flags & 0x0008 != 0 {
            Underline::Single
        } else {
            Underline::None
        };
        let strike = flags & 0x0200 != 0;
        (underline != Underline::None || strike).then_some(Self { underline, strike })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Run {
    row: usize,
    start: usize,
    end: usize,
    style: Style,
    color: u32,
}
#[cfg(test)]
fn runs(frame: &Frame) -> Vec<Run> {
    if frame.columns == 0 {
        return Vec::new();
    }
    frame
        .cells
        .chunks(frame.columns as usize)
        .take(frame.rows as usize)
        .enumerate()
        .flat_map(|(row, cells)| row_runs(cells, row))
        .collect()
}
fn row_runs(cells: &[crate::native::Cell], row: usize) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut current: Option<Run> = None;
    for (column, cell) in cells.iter().enumerate() {
        let style = Style::from_flags(cell.flags);
        let color = cell_colors(cell.foreground, cell.background, cell.flags).0;
        if current
            .as_ref()
            .is_some_and(|run| Some(run.style) == style && run.color == color)
        {
            current.as_mut().unwrap().end = column + 1;
            continue;
        }
        if let Some(run) = current.take() {
            runs.push(run);
        }
        if let Some(style) = style {
            current = Some(Run {
                row,
                start: column,
                end: column + 1,
                style,
                color,
            });
        }
    }
    if let Some(run) = current {
        runs.push(run);
    }
    runs
}
pub(super) struct Prepared {
    runs: Vec<Run>,
}
impl Prepared {
    pub fn row(cells: &[crate::native::Cell]) -> Self {
        Self {
            runs: row_runs(cells, 0),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        cell: Size<Pixels>,
        metrics: &Metrics,
        window: &mut Window,
    ) {
        paint_runs(&self.runs, bounds, cell, metrics, window);
    }
}
pub(super) struct Metrics {
    pub(super) thickness: Pixels,
    pub(super) underline: Pixels,
    strike: Pixels,
}
impl Metrics {
    pub fn measure(font: &Font, font_size: f32, height: Pixels, window: &Window) -> Self {
        let system = window.text_system();
        let id = system.resolve_font(font);
        let ascent = system.ascent(id, px(font_size));
        let baseline = system.baseline_offset(id, px(font_size), height);
        let thickness = px((font_size / 14.).round().max(1.));
        Self {
            thickness,
            underline: (height - thickness).min(baseline + px(2.)),
            strike: thickness.max(baseline - ascent * 0.36),
        }
    }
}
fn stroke(
    window: &mut Window,
    color: u32,
    left: Pixels,
    right: Pixels,
    y: Pixels,
    thickness: Pixels,
) {
    window.paint_quad(fill(
        Bounds::new(
            point(left - thickness / 2., y - thickness / 2.),
            size(right - left + thickness, thickness),
        ),
        rgb(color),
    ));
}

fn paint_runs(
    runs: &[Run],
    bounds: Bounds<Pixels>,
    cell: Size<Pixels>,
    metrics: &Metrics,
    window: &mut Window,
) {
    if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
        return;
    }
    for run in runs {
        let left = bounds.left() + cell.width * run.start as f32;
        let right = bounds.left() + cell.width * run.end as f32;
        let top = bounds.top() + cell.height * run.row as f32;
        let y = top + metrics.underline;
        let thickness = metrics.thickness;
        match run.style.underline {
            Underline::None => {}
            Underline::Single => stroke(window, run.color, left, right, y, thickness),
            Underline::Double => {
                let gap = px(2.).max(thickness * 2.);
                stroke(window, run.color, left, right, y - gap / 2., thickness);
                stroke(window, run.color, left, right, y + gap / 2., thickness);
            }
            Underline::Dotted => {
                let radius = thickness * 0.55;
                let step = (radius * 3.).max(cell.width * 0.45);
                let mut x = left + radius;
                while x < right {
                    let mut dot = fill(
                        Bounds::new(
                            point(x - radius, y - radius),
                            size(radius * 2., radius * 2.),
                        ),
                        rgb(run.color),
                    );
                    dot.corner_radii = Corners::all(radius);
                    window.paint_quad(dot);
                    x += step;
                }
            }
            Underline::Dashed => {
                let dash = px(3.).max(cell.width * 0.7);
                let gap = px(2.).max(cell.width * 0.35);
                let mut x = left;
                while x < right {
                    stroke(window, run.color, x, (x + dash).min(right), y, thickness);
                    x += dash + gap;
                }
            }
            Underline::Curly => {
                let amplitude = px(1.).max(thickness * 1.3);
                let wavelength = px(6.).max(cell.width * 1.5);
                let mut path = PathBuilder::stroke(thickness);
                path.move_to(point(left, y));
                let mut x = left;
                while x < right {
                    x = (x + px(1.5)).min(right);
                    let phase = (x - left) / wavelength * std::f32::consts::TAU;
                    path.line_to(point(x, y + amplitude * phase.sin()));
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(run.color));
                }
            }
        }
        if run.style.strike {
            stroke(
                window,
                run.color,
                left,
                right,
                top + metrics.strike,
                thickness,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn all_native_underline_flags_and_hidden_text_follow_reference_precedence() {
        for (flag, underline) in [
            (8, Underline::Single),
            (0x800, Underline::Double),
            (0x1000, Underline::Curly),
            (0x2000, Underline::Dotted),
            (0x4000, Underline::Dashed),
        ] {
            assert_eq!(
                Style::from_flags(flag | 0x200),
                Some(Style {
                    underline,
                    strike: true
                })
            );
            assert_eq!(Style::from_flags(flag | 0x100), None);
        }
        assert_eq!(
            Style::from_flags(0x1000 | 0x2000 | 0x4000 | 0x800 | 8)
                .unwrap()
                .underline,
            Underline::Curly
        );
        assert_eq!(
            Style::from_flags(0x200),
            Some(Style {
                underline: Underline::None,
                strike: true
            })
        );
        assert_eq!(Style::from_flags(2 | 4 | 0x40), None);
    }
    #[test]
    fn row_runs_include_spaces_and_wide_spacers_but_split_hidden_and_color_changes() {
        let cell = |text: &str, flags, foreground| crate::native::Cell {
            text: text.into(),
            flags,
            foreground,
            background: 0,
            hyperlink: None,
        };
        let frame = Frame {
            columns: 6,
            rows: 2,
            cells: vec![
                cell("A", 8, 0xffffff),
                cell(" ", 8, 0xffffff),
                cell("中", 8 | 0x20, 0xffffff),
                cell("", 8 | 0x40, 0xffffff),
                cell("hidden", 8 | 0x100, 0xffffff),
                cell("B", 8, 0xffffff),
                cell("C", 8, 0xffffff),
                cell("D", 8, 0xff0000),
                cell(" ", 0x200, 0xff0000),
                cell("", 0, 0xffffff),
                cell("E", 0x1000, 0xffffff),
                cell("F", 0x1000, 0xffffff),
            ],
            ..Frame::default()
        };
        let ranges: Vec<_> = runs(&frame)
            .into_iter()
            .map(|run| {
                (
                    run.row,
                    run.start,
                    run.end,
                    run.style.underline,
                    run.style.strike,
                )
            })
            .collect();
        assert_eq!(
            ranges,
            vec![
                (0, 0, 4, Underline::Single, false),
                (0, 5, 6, Underline::Single, false),
                (1, 0, 1, Underline::Single, false),
                (1, 1, 2, Underline::Single, false),
                (1, 2, 3, Underline::None, true),
                (1, 4, 6, Underline::Curly, false)
            ]
        );
    }
}
