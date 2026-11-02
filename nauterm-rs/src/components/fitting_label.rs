//! Prefer the complete caption when it fits, using native shaped text widths.
use gpui_kit::{prelude::*, *};

pub struct FittingLabel {
    full: SharedString,
    compact: SharedString,
}
impl FittingLabel {
    pub fn new(full: impl Into<SharedString>, compact: impl Into<SharedString>) -> Self {
        Self {
            full: full.into(),
            compact: compact.into(),
        }
    }
}
impl IntoElement for FittingLabel {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for FittingLabel {
    type RequestLayoutState = (Pixels, Pixels);
    type PrepaintState = AnyElement;
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
        let text = window.text_style();
        let font_size = text.font_size.to_pixels(window.rem_size());
        let line_height = window.pixel_snap(
            text.line_height
                .to_pixels(font_size.into(), window.rem_size()),
        );
        let line = window.text_system().shape_line(
            self.full.clone(),
            font_size,
            &[text.to_run(self.full.len())],
            None,
        );
        let mut style = Style {
            size: size(line.width.into(), line_height.into()),
            flex_shrink: 1.,
            ..Default::default()
        };
        style.min_size.width = px(0.).into();
        (
            window.request_layout(style, [], cx),
            (line.width, line_height),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (full_width, height): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let caption = if *full_width <= bounds.size.width + px(0.01) {
            self.full.clone()
        } else {
            self.compact.clone()
        };
        let mut label = div()
            .min_w_0()
            .whitespace_nowrap()
            .text_align(TextAlign::Right)
            .text_ellipsis()
            .child(caption)
            .into_any_element();
        label.layout_as_root(
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::Definite(*height),
            ),
            window,
            cx,
        );
        label.prepaint_at(bounds.origin, window, cx);
        label
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        label: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        label.paint(window, cx);
    }
}
