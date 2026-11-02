//! Typed, product-owned drag feedback. Capture a payload when dragging really
//! starts, and revalidate it when hovering and dropping, not just at render.
use super::Button;
use crate::palette::Palette;
use crate::typography::FontWeights;
use gpui_kit::{prelude::*, *};
use std::{cell::RefCell, rc::Rc};

pub struct Payload<T> {
    pub value: T,
    pub label: String,
    pub count: usize,
    pub dark: bool,
    pub icon: gpui_kit::assets::IconName,
}
struct Value<T>(Rc<RefCell<Option<T>>>);
impl<T> Clone for Value<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
struct Preview {
    label: String,
    count: usize,
    dark: bool,
    icon: gpui_kit::assets::IconName,
    cursor_offset: Point<Pixels>,
}
impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.dark);
        let label: SharedString = if self.count > 1 {
            crate::localization::text(cx, &format!("{} items", self.count))
        } else {
            self.label.clone().into()
        };
        let offset = self.cursor_offset + point(px(10.), px(12.));
        let mut card = div()
            .max_w(px(240.))
            .px(px(10.))
            .py(px(7.))
            .rounded(px(8.))
            .bg(p.surface.alpha(0.96))
            .text_color(p.text)
            .border_1()
            .border_color(p.accent.alpha(0.62))
            .shadow(vec![gpui_kit::base::box_shadow(
                px(0.),
                px(3.),
                px(4.),
                px(-1.),
                black().opacity(0.15),
            )])
            .flex()
            .items_center()
            .gap(px(7.))
            .child(
                gpui_kit::component::Icon::new(self.icon)
                    .size(px(18.))
                    .text_color(if self.icon == gpui_kit::assets::IconName::Folder {
                        rgb(0x5ec4f2)
                    } else {
                        p.muted
                    }),
            )
            .child(
                div()
                    .min_w_0()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .font_weight(FontWeights::SEMIBOLD)
                    .child(label),
            )
            .into_any_element();
        // GPUI subtracts the source-row cursor offset from this view's origin.
        // Position only the compact card back at the pointer; a transparent
        // one-pixel root avoids a row-sized drag image or source-position jump.
        canvas(
            move |bounds, window, cx| {
                card.layout_as_root(
                    size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
                    window,
                    cx,
                );
                card.prepaint_at(bounds.origin + offset, window, cx);
                card
            },
            |_, mut card, window, cx| card.paint(window, cx),
        )
        .w(px(1.))
        .h(px(1.))
    }
}
pub fn source<T: 'static>(
    button: Button,
    capture: impl Fn(&mut Window, &mut App) -> Option<Payload<T>> + 'static,
) -> Button {
    button.on_drag(
        Value(Rc::new(RefCell::new(None))),
        move |drag, cursor_offset, window, cx| {
            let payload = capture(window, cx);
            let (label, count, dark, icon, value) = match payload {
                Some(payload) => (
                    payload.label,
                    payload.count,
                    payload.dark,
                    payload.icon,
                    Some(payload.value),
                ),
                None => (
                    "Unavailable".into(),
                    0,
                    false,
                    gpui_kit::assets::IconName::File,
                    None,
                ),
            };
            *drag.0.borrow_mut() = value;
            cx.new(|_| Preview {
                label,
                count,
                dark,
                icon,
                cursor_offset,
            })
        },
    )
}
pub fn target<T: 'static>(
    id: impl Into<SharedString>,
    content: AnyElement,
    dark: bool,
    label: &'static str,
    accepts: impl Fn(&T, &App) -> bool + 'static,
    receive: impl Fn(&T, &mut Window, &mut App) + 'static,
) -> AnyElement {
    target_for(
        id,
        content,
        dark,
        label,
        move |value: &Value<T>, cx| {
            value
                .0
                .borrow()
                .as_ref()
                .is_some_and(|value| accepts(value, cx))
        },
        move |value: &Value<T>, window, cx| {
            if let Some(value) = value.0.borrow().as_ref() {
                receive(value, window, cx);
            }
        },
    )
}
/// GPUI translates native file drag events to ExternalPaths. This uses the
/// same presentation/guards as internal drags, without a copied UI payload.
pub fn external_target(
    id: impl Into<SharedString>,
    content: AnyElement,
    dark: bool,
    accepts: impl Fn(&ExternalPaths, &App) -> bool + 'static,
    receive: impl Fn(&ExternalPaths, &mut Window, &mut App) + 'static,
) -> AnyElement {
    target_for(id, content, dark, "Upload here", accepts, receive)
}
pub fn external_target_with_label(
    id: impl Into<SharedString>,
    content: AnyElement,
    dark: bool,
    label: &'static str,
    accepts: impl Fn(&ExternalPaths, &App) -> bool + 'static,
    receive: impl Fn(&ExternalPaths, &mut Window, &mut App) + 'static,
) -> AnyElement {
    target_for(id, content, dark, label, accepts, receive)
}
fn target_for<T: 'static>(
    id: impl Into<SharedString>,
    content: AnyElement,
    dark: bool,
    label: &'static str,
    accepts: impl Fn(&T, &App) -> bool + 'static,
    receive: impl Fn(&T, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let id = id.into();
    let p = Palette::new(dark);
    let accepts = Rc::new(accepts);
    let predicate = {
        let accepts = accepts.clone();
        move |value: &dyn std::any::Any, _: &mut Window, cx: &mut App| {
            value
                .downcast_ref::<T>()
                .is_some_and(|value| accepts(value, cx))
        }
    };
    div()
        .id(id.clone())
        .relative()
        .size_full()
        .can_drop(predicate.clone())
        .on_drop(move |value: &T, window, cx| {
            if accepts(value, cx) {
                receive(value, window, cx);
                cx.stop_propagation();
            }
        })
        .child(content)
        .child(
            div()
                .id(SharedString::from(format!("{id}-feedback")))
                .absolute()
                .inset_0()
                .border_2()
                .border_color(rgba(0x298df2aa))
                .bg(rgba(0x298df226))
                // Keep a hitbox for drag-over styling without changing the
                // content's layout or occluding its ordinary pointer events.
                .opacity(0.)
                .can_drop(predicate)
                .drag_over::<T>(|style, _, _, _| style.opacity(1.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .px(px(12.))
                        .py(px(7.))
                        .rounded(px(7.))
                        .border_1()
                        .border_color(Rgba {
                            a: 0.55,
                            ..p.accent
                        })
                        .bg(Rgba {
                            a: 0.94,
                            ..p.surface
                        })
                        .text_color(p.text)
                        .text_size(px(12.))
                        .font_weight(FontWeights::SEMIBOLD)
                        .child(crate::localization::view(label)),
                ),
        )
        .into_any_element()
}
