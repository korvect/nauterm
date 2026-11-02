use super::*;

pub(super) fn padded_bounds(
    bounds: Bounds<Pixels>,
    padding: crate::terminal_padding::Padding,
) -> Bounds<Pixels> {
    let fitted = padding.fitted(f32::from(bounds.size.width), f32::from(bounds.size.height));
    Bounds::new(
        point(
            bounds.left() + px(fitted.left),
            bounds.top() + px(fitted.top),
        ),
        size(
            (bounds.size.width - px(fitted.left) - px(fitted.right)).max(px(0.)),
            (bounds.size.height - px(fitted.top) - px(fitted.bottom)).max(px(0.)),
        ),
    )
}

pub(super) fn terminal_font(appearance: &Appearance) -> Font {
    let mut face = font(appearance.resolved_family().to_owned());
    face.weight = FontWeight(f32::from(appearance.weight));
    face.features = if appearance.enable_ligatures {
        FontFeatures::default()
    } else {
        FontFeatures(Arc::new(vec![("liga".into(), 0), ("calt".into(), 0)]))
    };
    face.fallbacks = Some(FontFallbacks(Arc::new(appearance.fallback_families())));
    face
}
pub(super) fn cursor_column(frame: &Frame) -> u32 {
    let index = frame.cursor.1 as usize * frame.columns as usize + frame.cursor.0 as usize;
    if frame.cursor.0 > 0
        && frame
            .cells
            .get(index)
            .is_some_and(|cell| cell.flags & 0x40 != 0)
    {
        frame.cursor.0 - 1
    } else {
        frame.cursor.0
    }
}
pub(super) fn cursor_rect(
    bounds: Bounds<Pixels>,
    frame: &Frame,
    width: Pixels,
    height: Pixels,
) -> Option<Bounds<Pixels>> {
    if frame.cursor.0 >= frame.columns || frame.cursor.1 >= frame.rows {
        return None;
    }
    let column = cursor_column(frame);
    let index = frame.cursor.1 as usize * frame.columns as usize + column as usize;
    let span = if frame
        .cells
        .get(index)
        .is_some_and(|cell| cell.flags & 0x20 != 0)
    {
        2.
    } else {
        1.
    };
    Some(Bounds::new(
        point(
            bounds.left() + width * column as f32,
            bounds.top() + height * frame.cursor.1 as f32,
        ),
        size(width * span, height),
    ))
}
pub(super) fn paint_cursor(
    rect: Bounds<Pixels>,
    shape: u32,
    focused: bool,
    color: u32,
    window: &mut Window,
) {
    let color = rgba((color << 8) | if focused { 255 } else { 153 });
    match shape {
        0 | 3 => {
            if shape == 0 && focused {
                return;
            } // Filled before text.
            let stroke = px(1.);
            for edge in [
                Bounds::new(rect.origin, size(rect.size.width, stroke)),
                Bounds::new(
                    point(rect.left(), rect.bottom() - stroke),
                    size(rect.size.width, stroke),
                ),
                Bounds::new(rect.origin, size(stroke, rect.size.height)),
                Bounds::new(
                    point(rect.right() - stroke, rect.top()),
                    size(stroke, rect.size.height),
                ),
            ] {
                window.paint_quad(fill(edge, color));
            }
        }
        1 => {
            let thickness = px((f32::from(rect.size.height) * 0.16).clamp(1., 3.));
            window.paint_quad(fill(
                Bounds::new(
                    point(rect.left(), rect.bottom() - thickness),
                    size(rect.size.width, thickness),
                ),
                color,
            ));
        }
        2 => {
            let thickness = px((f32::from(rect.size.width) * 0.18).clamp(1., 2.));
            window.paint_quad(fill(
                Bounds::new(rect.origin, size(thickness, rect.size.height)),
                color,
            ));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn ligatures_require_opt_in_and_keep_the_selected_fallback_family() {
        let mut appearance = Appearance {
            cjk_family: Some("Example CJK".into()),
            ..Appearance::default()
        };
        let disabled = terminal_font(&appearance);
        assert_ne!(disabled.features, FontFeatures::default());
        appearance.enable_ligatures = true;
        let enabled = terminal_font(&appearance);
        assert_eq!(enabled.features, FontFeatures::default());
        assert_eq!(enabled.fallbacks, disabled.fallbacks);
        assert_eq!(enabled.fallbacks.unwrap().0[0], "Example CJK");
    }
    #[test]
    fn padding_uses_one_grid_rectangle_and_never_produces_negative_sizes() {
        let outer = Bounds::new(point(px(10.), px(20.)), size(px(100.), px(80.)));
        let padding = crate::terminal_padding::Padding::parse("5, 10, 15, 20").unwrap();
        assert_eq!(
            padded_bounds(outer, padding),
            Bounds::new(point(px(30.), px(25.)), size(px(70.), px(60.)))
        );
        let huge = crate::terminal_padding::Padding::parse("3e38").unwrap();
        let inner = padded_bounds(outer, huge);
        assert_eq!(inner.origin, point(outer.right(), outer.bottom()));
        assert_eq!(inner.size, size(px(0.), px(0.)));
    }
    #[test]
    fn wide_cursor_covers_the_glyph_even_on_its_spacer() {
        let mut frame = Frame {
            columns: 4,
            rows: 1,
            cursor: (1, 0),
            cells: vec![
                crate::native::Cell {
                    text: String::new(),
                    hyperlink: None,
                    foreground: 0,
                    background: 0,
                    flags: 0
                };
                4
            ],
            ..Default::default()
        };
        frame.cells[0].flags = 0x20;
        frame.cells[1].flags = 0x40;
        let rect = cursor_rect(Bounds::default(), &frame, px(8.), px(15.)).unwrap();
        assert_eq!(rect.origin.x, px(0.));
        assert_eq!(rect.size, size(px(16.), px(15.)));
        frame.cursor = (4, 0);
        assert!(cursor_rect(Bounds::default(), &frame, px(8.), px(15.)).is_none());
    }
}
