//! Cell-sized Powerline sprites, matching the Flutter terminal painter.
//! Tessellate with the row cache; painting only translates the cached vertices.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Glyph {
    Triangle(bool),
    Chevron(bool),
    Round { right: bool, outline: bool },
    Corner(u8),
    Diagonal(bool),
    Flame(bool),
}
impl Glyph {
    fn from_text(text: &str) -> Option<Self> {
        let mut chars = text.chars();
        let ch = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        Some(match ch {
            '\u{e0b0}' => Self::Triangle(true),
            '\u{e0b1}' => Self::Chevron(true),
            '\u{e0b2}' => Self::Triangle(false),
            '\u{e0b3}' => Self::Chevron(false),
            '\u{e0b4}' => Self::Round {
                right: true,
                outline: false,
            },
            '\u{e0b5}' => Self::Round {
                right: true,
                outline: true,
            },
            '\u{e0b6}' => Self::Round {
                right: false,
                outline: false,
            },
            '\u{e0b7}' => Self::Round {
                right: false,
                outline: true,
            },
            '\u{e0b8}' => Self::Corner(0),
            '\u{e0ba}' => Self::Corner(1),
            '\u{e0bc}' => Self::Corner(2),
            '\u{e0be}' => Self::Corner(3),
            '\u{e0b9}' | '\u{e0bf}' => Self::Diagonal(true),
            '\u{e0bb}' | '\u{e0bd}' => Self::Diagonal(false),
            '\u{e0d2}' => Self::Flame(true),
            '\u{e0d4}' => Self::Flame(false),
            _ => return None,
        })
    }
    fn outline(self) -> bool {
        matches!(
            self,
            Self::Chevron(_) | Self::Diagonal(_) | Self::Round { outline: true, .. }
        )
    }
    fn build(self, cell: Size<Pixels>, thickness: Pixels) -> Option<Path<Pixels>> {
        if cell.width <= px(0.) || cell.height <= px(0.) {
            return None;
        }
        let mut path = if self.outline() {
            PathBuilder::stroke(thickness)
        } else {
            PathBuilder::fill()
        };
        let width = cell.width;
        let height = cell.height;
        let mid = height / 2.;
        let polygon = |path: &mut PathBuilder, points: &[Point<Pixels>]| {
            path.move_to(points[0]);
            for point in &points[1..] {
                path.line_to(*point);
            }
            path.close();
        };
        match self {
            Self::Triangle(right) | Self::Chevron(right) => {
                let (flat, tip) = if right {
                    (px(0.), width)
                } else {
                    (width, px(0.))
                };
                path.move_to(point(flat, px(0.)));
                path.line_to(point(tip, mid));
                path.line_to(point(flat, height));
                if !self.outline() {
                    path.close();
                }
            }
            Self::Round { right, outline } => {
                const K: f32 = 0.552_284_8;
                let radius = width.min(mid);
                let direction = if right { 1. } else { -1. };
                let flat = if right { px(0.) } else { width };
                let tip = flat + radius * direction;
                let control = flat + radius * (direction * K);
                let endpoint = if outline { px(1.).min(width) } else { px(0.) };
                path.move_to(point(flat, px(0.)));
                if outline {
                    path.line_to(point(flat + endpoint * direction, px(0.)));
                }
                path.cubic_bezier_to(
                    point(tip, radius),
                    point(control, px(0.)),
                    point(tip, radius * (1. - K)),
                );
                path.line_to(point(tip, height - radius));
                path.cubic_bezier_to(
                    point(flat + endpoint * direction, height),
                    point(tip, height - radius * (1. - K)),
                    point(control, height),
                );
                if outline {
                    path.line_to(point(flat, height));
                } else {
                    path.close();
                }
            }
            Self::Corner(corner) => {
                let tl = point(px(0.), px(0.));
                let tr = point(width, px(0.));
                let bl = point(px(0.), height);
                let br = point(width, height);
                let points = match corner {
                    0 => [tl, br, bl],
                    1 => [tr, br, bl],
                    2 => [tl, tr, bl],
                    _ => [tl, tr, br],
                };
                polygon(&mut path, &points);
            }
            Self::Diagonal(right) => {
                path.move_to(point(if right { px(0.) } else { width }, px(0.)));
                path.line_to(point(if right { width } else { px(0.) }, height));
            }
            Self::Flame(right) => {
                let (flat, tip) = if right {
                    (px(0.), width)
                } else {
                    (width, px(0.))
                };
                let upper = (mid - thickness / 2.).max(px(0.));
                let lower = (mid + thickness / 2.).min(height);
                polygon(
                    &mut path,
                    &[
                        point(flat, px(0.)),
                        point(tip, px(0.)),
                        point(width / 2., upper),
                        point(flat, upper),
                    ],
                );
                polygon(
                    &mut path,
                    &[
                        point(flat, height),
                        point(tip, height),
                        point(width / 2., lower),
                        point(flat, lower),
                    ],
                );
            }
        }
        path.build().ok()
    }
}

pub(super) fn supports(text: &str) -> bool {
    Glyph::from_text(text).is_some()
}

pub(super) struct Prepared {
    path: Path<Pixels>,
    color: u32,
}
impl Prepared {
    pub fn new(text: &str, cell: Size<Pixels>, size: f32, color: u32) -> Option<Self> {
        Some(Self {
            path: Glyph::from_text(text)?.build(cell, px((size / 14.).round().max(1.)))?,
            color,
        })
    }
    pub fn paint(&self, origin: Point<Pixels>, window: &mut Window) {
        let mut path = self.path.clone();
        path.bounds.origin += origin;
        for vertex in &mut path.vertices {
            vertex.xy_position += origin;
        }
        window.paint_path(path, rgb(self.color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn reference_sprites_tessellate_at_narrow_wide_and_fractional_cell_sizes() {
        for code in (0xe0b0..=0xe0bf).chain([0xe0d2, 0xe0d4]) {
            let text = char::from_u32(code).unwrap().to_string();
            for (width, height) in [(8., 18.), (18., 18.), (7.5, 17.25)] {
                let glyph =
                    Prepared::new(&text, size(px(width), px(height)), 13., 0xff6600).unwrap();
                assert!(!glyph.path.vertices.is_empty(), "{code:x}");
                // Miter joins can extend up to 2px beyond a 1px stroke's
                // centerline; filled sprites must stay inside the cell.
                let margin = if Glyph::from_text(&text).unwrap().outline() {
                    2.
                } else {
                    0.001
                };
                assert!(
                    glyph.path.vertices.iter().all(|vertex| {
                        let x = f32::from(vertex.xy_position.x);
                        let y = f32::from(vertex.xy_position.y);
                        x.is_finite()
                            && y.is_finite()
                            && x >= -margin
                            && x <= width + margin
                            && y >= -margin
                            && y <= height + margin
                    }),
                    "{code:x}"
                );
            }
        }
        for text in [
            "",
            "a",
            "\u{f07b}",
            "\u{f024b}",
            "\u{e0b0}x",
            "\u{e0b0}\u{fe0f}",
        ] {
            assert!(!supports(text));
        }
    }

    #[test]
    fn triangle_fills_the_cell_and_preserves_the_separator_color() {
        let glyph = Prepared::new("\u{e0b0}", size(px(8.), px(18.)), 13., 0x123456).unwrap();
        assert_eq!(
            glyph.path.bounds,
            Bounds::new(point(px(0.), px(0.)), size(px(8.), px(18.)))
        );
        assert_eq!(glyph.color, 0x123456);
    }
}
