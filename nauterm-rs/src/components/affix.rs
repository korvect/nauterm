//! Shared field affordances: a filled circular clear button and a persistent,
//! rotating arrow. Workspace and Settings retain their own palettes.
use super::{
    ButtonCustomVariant,
    button::Button,
    motion::{self, Transition},
};
use crate::palette::Palette;
use gpui_kit::{assets::IconName, component::Icon, prelude::*, *};
use std::time::{Duration, Instant};

pub fn clear_button<const S: bool>(
    id: impl Into<ElementId>,
    p: Palette,
    themed: bool,
    cx: &App,
) -> Button<S> {
    let dark = p.background.r < 0.5;
    let (background, hover, active) = if themed {
        (
            p.control.blend(p.text.alpha(0.06)),
            p.control.blend(p.text.alpha(0.10)),
            p.control.blend(p.text.alpha(0.16)),
        )
    } else {
        (
            rgb(if dark { 0x3e3e3e } else { 0xf0f4f5 }),
            rgb(if dark { 0x484848 } else { 0xe9f1f3 }),
            rgb(if dark { 0x444444 } else { 0xc7d5d8 }),
        )
    };
    Button::<S>::new(id)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(background.into())
                .foreground(p.muted.into())
                .hover(hover.into())
                .active(active.into()),
        )
        .size(px(16.))
        .p_0()
        .rounded_full()
        .icon(IconName::X)
        .icon_size(px(10.))
        .focusable(false)
        .tab_stop(false)
}

pub fn rotating_arrow(
    id: EntityId,
    open: bool,
    color: Rgba,
    size: f32,
    window: &mut Window,
    cx: &mut App,
) -> Icon {
    let state = window.use_keyed_state(("field-arrow", id), cx, |_, _| {
        Transition::new(f32::from(open))
    });
    let reduced = cx.reduce_motion();
    let (angle, moving) = state.update(cx, |motion, _| {
        motion.set(f32::from(open), Instant::now(), motion::OVERLAY, reduced)
    });
    if moving {
        window.request_animation_frame();
    }
    Icon::new(IconName::ChevronDown)
        .size(px(size))
        .text_color(color)
        .rotate(percentage(angle * 0.5))
}

pub fn clear_transition(id: EntityId, visible: bool, window: &mut Window, cx: &mut App) -> f32 {
    let state = window.use_keyed_state(("field-clear", id), cx, |_, _| {
        Transition::ease_in_out(f32::from(visible))
    });
    let reduced = cx.reduce_motion();
    let (amount, moving) = state.update(cx, |motion, _| {
        motion.set(
            f32::from(visible),
            Instant::now(),
            Duration::from_millis(150),
            reduced,
        )
    });
    if moving {
        window.request_animation_frame();
    }
    amount
}
