//! Pointer presentation for an editor viewport. Input handling stays with Base;
//! this region has no mouse/scroll listeners and never occludes its contents.
use gpui_kit::*;

pub(super) fn override_cursor(
    content: impl IntoElement,
    cursor: Option<CursorStyle>,
) -> AnyElement {
    let content = content.into_any_element();
    match cursor {
        Some(cursor) => CursorRegion { content, cursor }.into_any_element(),
        None => content,
    }
}
struct CursorRegion<E: Element> {
    content: E,
    cursor: CursorStyle,
}
impl<E: Element> IntoElement for CursorRegion<E> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl<E: Element> Element for CursorRegion<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = (E::PrepaintState, Hitbox);
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
        self.content.request_layout(id, inspector, window, cx)
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
        let content = self
            .content
            .prepaint(id, inspector, bounds, layout, window, cx);
        let pointer = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        (content, pointer)
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
        self.content
            .paint(id, inspector, bounds, layout, &mut prepaint.0, window, cx);
        window.set_cursor_style(self.cursor, &prepaint.1);
    }
}
