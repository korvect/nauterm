use super::Tooltip;
use gpui_kit::{
    base::{
        POPUP_PRIORITY, Placement, Root, RootPlugin, TooltipDefaults, TooltipOverlay,
        TooltipRequest,
    },
    prelude::*,
    *,
};
use std::{collections::HashMap, time::Duration};

#[derive(Default)]
struct Registry(HashMap<WindowId, WeakEntity<Host>>);
impl Global for Registry {}

struct Host {
    overlay: Entity<TooltipOverlay>,
    owner: Option<EntityId>,
    anchor: Bounds<Pixels>,
}
impl RootPlugin for Host {}

pub(crate) fn init(cx: &mut App) {
    cx.set_global(Registry::default());
    TooltipDefaults::new()
        .with_show_delay(Duration::from_millis(350))
        .with_grace_period(Duration::ZERO)
        .install(cx);
    Root::register_plugin(cx, |window, cx| {
        let owner = cx.weak_entity();
        cx.global_mut::<Registry>()
            .0
            .insert(window.window_handle().window_id(), owner);
        let overlays = crate::components::overlay::manager(window, cx);
        cx.observe(&overlays, |host, _, cx| {
            host.hide(cx);
            cx.notify();
        })
        .detach();
        cx.observe_window_activation(window, |host, window, cx| {
            if !window.is_window_active() {
                host.hide(cx);
            }
        })
        .detach();
        cx.observe_window_bounds(window, |host, _, cx| host.hide(cx))
            .detach();
        Host {
            overlay: cx.new(|_| {
                TooltipOverlay::new().render_with(|view, _, _, _| {
                    // Symmetric transparent gutters keep a 6px gap on either side
                    // of a flipped tooltip and an 8px visible viewport inset.
                    div().px(px(4.)).py(px(6.)).child(view).into_any_element()
                })
            }),
            owner: None,
            anchor: Bounds::default(),
        }
    });
    cx.on_window_closed(|cx, id| {
        cx.global_mut::<Registry>().0.remove(&id);
    })
    .detach();
    cx.intercept_keystrokes(|_, window, cx| hide(window.window_handle().window_id(), cx))
        .detach();
}

fn manager(id: WindowId, cx: &App) -> Option<Entity<Host>> {
    cx.try_global::<Registry>()?.0.get(&id)?.upgrade()
}

pub(crate) fn hover(
    owner: EntityId,
    text: SharedString,
    anchor: Bounds<Pixels>,
    hovered: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let id = window.window_handle().window_id();
    if !hovered {
        hide_owner(id, owner, cx);
        return;
    }
    let Some(host) = manager(id, cx) else {
        return;
    };
    host.update(cx, |host, cx| {
        host.owner = Some(owner);
        host.anchor = anchor;
        host.overlay.update(cx, |overlay, cx| {
            overlay.request_show(
                TooltipRequest::new(anchor, move |_, cx| {
                    cx.new(|_| Tooltip::new(text.clone())).into()
                })
                .with_placement(Placement::Bottom),
                window,
                cx,
            );
        });
    });
}

pub(crate) fn hide_owner(id: WindowId, owner: EntityId, cx: &mut App) {
    if let Some(host) = manager(id, cx) {
        host.update(cx, |host, cx| {
            if host.owner == Some(owner) {
                host.hide(cx);
            }
        });
    }
}

pub(crate) fn anchor_changed(id: WindowId, owner: EntityId, anchor: Bounds<Pixels>, cx: &mut App) {
    if let Some(host) = manager(id, cx) {
        host.update(cx, |host, cx| {
            if host.owner == Some(owner) && host.anchor != anchor {
                host.hide(cx);
            }
        });
    }
}

fn hide(id: WindowId, cx: &mut App) {
    if let Some(host) = manager(id, cx) {
        host.update(cx, |host, cx| host.hide(cx));
    }
}

impl Host {
    fn hide(&mut self, cx: &mut Context<Self>) {
        if self.owner.take().is_some() {
            self.overlay.update(cx, |overlay, cx| overlay.hide(cx));
        }
    }
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !window.is_window_active() || !window.is_window_hovered() {
            self.hide(cx);
        }
        let overlays = crate::components::overlay::manager(window, cx).read(cx);
        let priority = overlays
            .modal_top()
            .map_or(0, |id| overlays.modal_priority(id))
            + POPUP_PRIORITY
            + 200;
        div()
            .absolute()
            .inset_0()
            .child(
                canvas(
                    |_, _, _| {},
                    |_, _, window, _| {
                        window.on_mouse_event(|_: &MouseDownEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                hide(window.window_handle().window_id(), cx);
                            }
                        });
                        window.on_mouse_event(|_: &ScrollEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                hide(window.window_handle().window_id(), cx);
                            }
                        });
                        window.on_mouse_event(|_: &MouseExitEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                hide(window.window_handle().window_id(), cx);
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(deferred(self.overlay.clone()).with_priority(priority))
    }
}
