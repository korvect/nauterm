//! Product modal scope and backdrop over base focus trapping and GPUI painting.
//! Business confirmation is explicit; Enter never implies a destructive action.
use super::{
    motion::{self, Transition},
    overlay,
};
use gpui_kit::{base::FocusTrapElement, prelude::*, *};
use std::{rc::Rc, time::Instant};

type Dismiss = Rc<dyn Fn(&(), &mut Window, &mut App)>;
gpui_kit::actions!(modal, [Cancel, Confirm]);
pub(super) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, Some("NautermModal")),
        KeyBinding::new("enter", Confirm, Some("NautermModal")),
    ]);
}
struct Lease {
    focus: FocusHandle,
    previous: Option<WeakFocusHandle>,
    initial: Option<WeakFocusHandle>,
    last_focus: Option<WeakFocusHandle>,
    motion: Transition,
    closing: bool,
    notified: bool,
    dismissible: bool,
    dismiss: Option<Dismiss>,
}
impl Lease {
    fn request_dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing || !self.dismissible || self.dismiss.is_none() {
            return;
        }
        self.closing = true;
        self.focus.focus(window, cx);
        let now = Instant::now();
        // Reverse from the currently painted position instead of snapping a
        // partially entered panel to its fully open state. Exit accelerates;
        // entry decelerates, matching the reference's direction-specific curve.
        self.motion = Transition::ease_in(self.motion.value(now));
        self.motion
            .set(0., now, motion::MODAL_EXIT, cx.reduce_motion());
        cx.notify();
    }
}
#[derive(IntoElement)]
pub struct Modal {
    id: ElementId,
    label: SharedString,
    content: Option<AnyElement>,
    focus: Option<FocusHandle>,
    initial: Option<FocusHandle>,
    previous: Option<FocusHandle>,
    dismiss: Option<Dismiss>,
    confirm: Option<Dismiss>,
    dismissible: bool,
    dismiss_on_backdrop: bool,
    alert: bool,
}
impl Modal {
    /// Dismiss the current modal through its guarded, animated cancel path.
    pub fn dismiss(window: &mut Window, cx: &mut App) {
        let dismiss = overlay::manager(window, cx).read(cx).modal_dismiss();
        if let Some(dismiss) = dismiss {
            dismiss(window, cx);
        }
    }
    /// Finish an accepted asynchronous operation in its own scope. Unlike
    /// cancellation this may close a busy modal, including beneath another one.
    /// It never dismisses an unrelated topmost modal.
    pub fn complete(scope: &FocusHandle, window: &mut Window, cx: &mut App) -> bool {
        let complete = overlay::manager(window, cx).read(cx).modal_complete(scope);
        if let Some(complete) = complete {
            complete(window, cx);
            true
        } else {
            false
        }
    }
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content: None,
            focus: None,
            initial: None,
            previous: None,
            dismiss: None,
            confirm: None,
            dismissible: true,
            dismiss_on_backdrop: true,
            alert: false,
        }
    }
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
    pub(super) fn alert(mut self) -> Self {
        self.alert = true;
        self
    }
    pub fn focus_scope(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }
    pub fn initial_focus(mut self, focus: &FocusHandle) -> Self {
        self.initial = Some(focus.clone());
        self
    }
    pub fn return_focus(mut self, focus: &FocusHandle) -> Self {
        self.previous = Some(focus.clone());
        self
    }
    pub fn dismissible(mut self, value: bool) -> Self {
        self.dismissible = value;
        self
    }
    pub fn dismiss_on_backdrop(mut self, value: bool) -> Self {
        self.dismiss_on_backdrop = value;
        self
    }
    pub fn on_dismiss(mut self, callback: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self {
        self.dismiss = Some(Rc::new(callback));
        self
    }
    pub fn on_confirm(mut self, callback: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self {
        self.confirm = Some(Rc::new(callback));
        self
    }
}
impl RenderOnce for Modal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let manager = overlay::manager(window, cx);
        let weak = manager.downgrade();
        let lease = window.use_keyed_state((self.id.clone(), "modal-scope"), cx, |window, cx| {
            let focus = self
                .focus
                .unwrap_or_else(|| cx.focus_handle())
                .tab_stop(false);
            let previous = manager
                .read(cx)
                .modal_focus()
                .or(self.previous)
                .or_else(|| window.focused(cx));
            let id = cx.entity_id();
            let request = cx.weak_entity();
            let completed = request.clone();
            let dismiss = Rc::new(move |window: &mut Window, cx: &mut App| {
                let _ = request.update(cx, |lease: &mut Lease, cx| {
                    lease.request_dismiss(window, cx)
                });
            });
            let complete = Rc::new(move |window: &mut Window, cx: &mut App| {
                let _ = completed.update(cx, |lease: &mut Lease, cx| {
                    lease.dismissible = true;
                    lease.request_dismiss(window, cx);
                });
            });
            cx.on_release_in(window, move |lease: &mut Lease, window, cx| {
                let was_top = weak
                    .update(cx, |manager, cx| manager.end_modal(id, window, cx))
                    .unwrap_or(false);
                let current = window.focused(cx);
                let owned = lease
                    .initial
                    .as_ref()
                    .into_iter()
                    .chain(lease.last_focus.as_ref())
                    .filter_map(WeakFocusHandle::upgrade)
                    .any(|focus| Some(focus) == current);
                if was_top
                    && (current.is_none()
                        || owned
                        || lease.focus.is_focused(window)
                        || lease.focus.contains_focused(window, cx))
                    && let Some(previous) =
                        lease.previous.as_ref().and_then(WeakFocusHandle::upgrade)
                {
                    previous.focus(window, cx);
                }
            })
            .detach();
            manager.update(cx, |manager, cx| {
                manager.begin_modal(id, focus.clone(), dismiss, complete, window, cx)
            });
            self.initial.as_ref().unwrap_or(&focus).focus(window, cx);
            Lease {
                focus,
                previous: previous.as_ref().map(FocusHandle::downgrade),
                initial: self.initial.as_ref().map(FocusHandle::downgrade),
                last_focus: None,
                motion: Transition::new(0.),
                closing: false,
                notified: false,
                dismissible: true,
                dismiss: None,
            }
        });
        let priority = manager.read(cx).modal_priority(lease.entity_id());
        let focus = lease.read(cx).focus.clone();
        let reduced = cx.reduce_motion();
        let dismiss = self.dismiss;
        let (progress, moving) = lease.update(cx, |lease, cx| {
            lease.dismiss = dismiss;
            lease.dismissible = self.dismissible;
            if lease.focus.contains_focused(window, cx) {
                lease.last_focus = window.focused(cx).as_ref().map(FocusHandle::downgrade);
            }
            lease.motion.set(
                if lease.closing { 0. } else { 1. },
                Instant::now(),
                if lease.closing {
                    motion::MODAL_EXIT
                } else {
                    motion::MODAL_ENTER
                },
                reduced,
            )
        });
        if moving {
            window.request_animation_frame();
        }
        let closing = lease.read(cx).closing;
        if closing && !moving && progress <= 0.001 && !lease.read(cx).notified {
            let callback = lease.read(cx).dismiss.clone();
            lease.update(cx, |lease, _| lease.notified = true);
            let retained = lease.downgrade();
            window.defer(cx, move |window, cx| {
                if let Some(callback) = callback {
                    callback(&(), window, cx);
                }
                // A save/validation failure may keep the dialog mounted.
                let _ = retained.update(cx, |lease, cx| {
                    lease.closing = false;
                    lease.notified = false;
                    lease.motion = Transition::new(0.);
                    cx.notify();
                });
            });
        }
        let confirm = self.confirm;
        let scope = focus.clone();
        let dismiss_manager = manager.downgrade();
        let confirm_manager = manager.downgrade();
        let id = lease.entity_id();
        let request = lease.downgrade();
        let confirm_lease = lease.downgrade();
        let backdrop_lease = lease.downgrade();
        let backdrop_manager = manager.downgrade();
        let viewport = window.viewport_size();
        deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id(self.id.clone())

                    .role(if self.alert { Role::AlertDialog } else { Role::Dialog })
                    .aria_label(crate::localization::text(cx, &self.label))
                    .w(viewport.width)
                    .h(viewport.height)
                    .p(px(24.))
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .key_context("NautermModal")
                    .tab_group()
                    .tab_stop(false)
                    .on_action(move |_: &Cancel, window, cx| {
                        let top = dismiss_manager.upgrade().is_some_and(|manager| manager.read(cx).modal_top() == Some(id));
                        if top {
                            let _ = request.update(cx, |lease, cx| lease.request_dismiss(window, cx));
                        }
                        cx.stop_propagation();
                    })
                    .on_action(move |_: &Confirm, window, cx| {
                        let topmost = confirm_manager.upgrade().is_some_and(|manager| manager.read(cx).modal_top() == Some(id));
                        if topmost
                            && confirm_lease.upgrade().is_some_and(|lease| !lease.read(cx).closing)
                            && scope.is_focused(window)
                            && let Some(confirm) = &confirm
                        {
                            confirm(&(), window, cx);
                        }
                        cx.stop_propagation();
                    })
                    .when(closing, |host| host.capture_any_mouse_down(|_, _, cx| cx.stop_propagation()))
                    .child(
                        div()
                            .id((self.id.clone(), "backdrop"))

                            .absolute()
                            .inset_0()
                            .occlude()
                            .bg(rgba(0x0000008a))
                            .opacity(progress)
                            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                            .on_click(move |event, window, cx| {
                                let primary = !matches!(event, ClickEvent::Mouse(event) if event.up.button != MouseButton::Left);
                                let top = backdrop_manager.upgrade().is_some_and(|manager| manager.read(cx).modal_top() == Some(id));
                                if self.dismiss_on_backdrop && primary && top {
                                    let _ = backdrop_lease.update(cx, |lease, cx| lease.request_dismiss(window, cx));
                                }
                                cx.stop_propagation();
                            })
                            .on_scroll(|_, _, cx| cx.stop_propagation()),
                    )
                    .child(
                        div()
                            .id((self.id.clone(), "surface"))

                            .relative()
                            .max_w_full()
                            .max_h_full()
                            // GPUI applies opacity to individual primitives,
                            // not an isolated composited dialog. Fading the
                            // surface would let the black scrim tint its colors.
                            // Keep its ink opaque and animate its position only.
                            .top(px(12. * (1. - progress)))
                            .occlude()
                            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                            .on_scroll(|_, _, cx| cx.stop_propagation())
                            .children(self.content),
                    )
                    .focus_trap(self.id, &focus),
            ),
        )
        .with_priority(priority)
    }
}
