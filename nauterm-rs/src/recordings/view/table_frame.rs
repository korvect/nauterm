//! The frame paints after the virtual list, using that list's measured bounds.
//! GPUI's rectangular content mask alone does not clip children to a radius.
use crate::palette::Palette;
use gpui_kit::*;

pub(super) struct TableFrame {
    content: AnyElement,
    palette: Palette,
}
impl TableFrame {
    pub(super) fn new(content: impl IntoElement, palette: Palette) -> Self {
        Self {
            content: content.into_any_element(),
            palette,
        }
    }
}
impl IntoElement for TableFrame {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for TableFrame {
    type RequestLayoutState = <AnyElement as Element>::RequestLayoutState;
    type PrepaintState = <AnyElement as Element>::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.content.id()
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.content.source_location()
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        Element::request_layout(&mut self.content, id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        Element::prepaint(&mut self.content, id, inspector, bounds, layout, window, cx)
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        Element::paint(
            &mut self.content,
            id,
            inspector,
            bounds,
            layout,
            prepaint,
            window,
            cx,
        );
        // Quads snap their edges internally, whereas paths keep their logical
        // coordinates. Share snapped near/far edges so the cutout and outline
        // describe exactly the same rounded rectangle at fractional sizes.
        let bounds = Bounds::from_corners(
            window.pixel_snap_point(bounds.origin),
            window.pixel_snap_point(bounds.bottom_right()),
        );
        let radius = px(7.)
            .min(bounds.size.width / 2.)
            .min(bounds.size.height / 2.);
        if radius <= px(0.) {
            return;
        }
        // Four disjoint cutouts may share a layer. The overlapping outline
        // MUST have a later layer: Scene batches Quad before Path at equal
        // draw order, regardless of their insertion order within a layer.
        let control = radius * (1. - 0.552_284_8);
        window.paint_layer(bounds, |window| {
            for (corner, sx, sy) in [
                (bounds.origin, 1., 1.),
                (point(bounds.right(), bounds.top()), -1., 1.),
                (point(bounds.left(), bounds.bottom()), 1., -1.),
                (point(bounds.right(), bounds.bottom()), -1., -1.),
            ] {
                let local = |x: Pixels, y: Pixels| corner + point(x * sx, y * sy);
                let mut path = PathBuilder::fill();
                path.move_to(corner);
                path.line_to(local(radius, px(0.)));
                path.cubic_bezier_to(
                    local(px(0.), radius),
                    local(control, px(0.)),
                    local(px(0.), control),
                );
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, self.palette.background);
                }
            }
        });
        window.paint_layer(bounds, |window| {
            let mut border = outline(bounds, self.palette.border, BorderStyle::Solid);
            border.corner_radii = Corners::all(radius);
            border.border_widths = px(1.).into();
            window.paint_quad(border);
        });
    }
}
