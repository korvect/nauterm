//! Single-line caption tracking applied after native text shaping. Shaping
//! characters separately loses kerning, fallback runs and fractional advances.
use gpui_kit::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    LineLayout, Pixels, ShapedLine, SharedString, Style, TextAlign, Window, size,
};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) struct TrackedText {
    text: SharedString,
    spacing: Pixels,
}

impl TrackedText {
    pub(crate) fn new(text: impl Into<SharedString>, spacing: Pixels) -> Self {
        Self {
            text: text.into(),
            spacing,
        }
    }
}

fn tracked_layout(line: &LineLayout, spacing: Pixels) -> LineLayout {
    let mut clusters = BTreeMap::new();
    for glyph in line.runs.iter().flat_map(|run| &run.glyphs) {
        clusters
            .entry(glyph.index)
            .and_modify(|x: &mut Pixels| *x = (*x).min(glyph.position.x))
            .or_insert(glyph.position.x);
    }
    let mut visual_order: Vec<_> = clusters.into_iter().collect();
    visual_order.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let offsets: BTreeMap<_, _> = visual_order
        .iter()
        .enumerate()
        .map(|(rank, (index, _))| (*index, spacing * rank as f32))
        .collect();
    let mut runs = line.runs.clone();
    for glyph in runs.iter_mut().flat_map(|run| &mut run.glyphs) {
        // Glyphs in the same shaped cluster move together, even across runs.
        glyph.position.x += offsets[&glyph.index];
    }
    LineLayout {
        font_size: line.font_size,
        width: line.width + spacing * visual_order.len().saturating_sub(1) as f32,
        ascent: line.ascent,
        descent: line.descent,
        runs,
        len: line.len,
    }
}

impl IntoElement for TrackedText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for TrackedText {
    type RequestLayoutState = (ShapedLine, Pixels);
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.pixel_snap(
            text_style
                .line_height
                .to_pixels(font_size.into(), window.rem_size()),
        );
        let mut line = window.text_system().shape_line(
            self.text.clone(),
            font_size,
            &[text_style.to_run(self.text.len())],
            None,
        );
        let layout = tracked_layout(&line, self.spacing);
        *line = Arc::new(layout);
        let style = Style {
            size: size(line.width.into(), line_height.into()),
            flex_shrink: 0.,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), (line, line_height))
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) {
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (line, line_height): &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _ = line.paint(
            bounds.origin,
            *line_height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::tracked_layout;
    use gpui_kit::{FontId, GlyphId, LineLayout, ShapedGlyph, ShapedRun, point, px};

    #[test]
    fn tracking_preserves_kerning_and_moves_a_cluster_as_one_unit() {
        let glyph = |index, x| ShapedGlyph {
            id: GlyphId(0),
            position: point(px(x), px(0.)),
            index,
            is_emoji: false,
        };
        let original = LineLayout {
            width: px(19.),
            runs: vec![
                ShapedRun {
                    font_id: FontId(0),
                    glyphs: vec![glyph(0, 0.), glyph(1, 9.)],
                },
                ShapedRun {
                    font_id: FontId(1),
                    glyphs: vec![glyph(1, 10.)],
                },
            ],
            ..Default::default()
        };
        let tracked = tracked_layout(&original, px(0.8));
        assert_eq!(tracked.runs[0].glyphs[0].position.x, px(0.));
        assert_eq!(tracked.runs[0].glyphs[1].position.x, px(9.8));
        assert_eq!(tracked.runs[1].glyphs[0].position.x, px(10.8));
        assert_eq!(tracked.width, px(19.8));
        assert_eq!(original.runs[0].glyphs[1].position.x, px(9.));
        assert_eq!(
            tracked_layout(&LineLayout::default(), px(0.8)).width,
            px(0.)
        );
        let single = LineLayout {
            width: px(10.),
            runs: vec![ShapedRun {
                font_id: FontId(0),
                glyphs: vec![glyph(0, 0.), glyph(0, 1.)],
            }],
            ..Default::default()
        };
        assert_eq!(tracked_layout(&single, px(0.8)).width, px(10.));
    }
}
