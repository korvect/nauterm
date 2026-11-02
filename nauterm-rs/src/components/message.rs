//! One transient feedback surface per window, shared by all child views.
use super::{Button, ButtonCustomVariant, motion::Transition, overlay};
use crate::{palette::Palette, typography::FontWeights};
use gpui_kit::{
    assets::IconName,
    base::{POPUP_PRIORITY, Root, RootPlugin},
    component::{ActiveTheme, Sizable},
    prelude::*,
    *,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    hash::Hash,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Clone, PartialEq, Eq)]
pub struct Message {
    text: String,
    error: bool,
}
impl Message {
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: true,
        }
    }
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: false,
        }
    }
}
#[derive(Default)]
struct Slot {
    host: Option<WeakEntity<Host>>,
    pending: Option<Message>,
    settings: bool,
}
#[derive(Default)]
struct Registry(HashMap<WindowId, Slot>);
impl Global for Registry {}

pub(super) fn init(cx: &mut App) {
    cx.set_global(Registry::default());
    Root::register_plugin(cx, |window, cx| {
        let owner = cx.weak_entity();
        let slot = cx
            .global_mut::<Registry>()
            .0
            .entry(window.window_handle().window_id())
            .or_default();
        slot.host = Some(owner);
        let pending = slot.pending.take();
        let overlays = overlay::manager(window, cx);
        cx.observe(&overlays, |_, _, cx| cx.notify()).detach();
        let mut host = Host {
            message: None,
            closing: false,
            motion: Transition::new(0.),
            timer: None,
        };
        if let Some(message) = pending {
            host.show(message, cx);
        }
        host
    });
    cx.on_window_closed(|cx, id| {
        cx.global_mut::<Registry>().0.remove(&id);
    })
    .detach();
}

pub fn settings_window(window: &Window, cx: &mut App) {
    cx.global_mut::<Registry>()
        .0
        .entry(window.window_handle().window_id())
        .or_default()
        .settings = true;
}

pub fn show_for_window(id: WindowId, message: Message, cx: &mut App) {
    if message.text.trim().is_empty() {
        return;
    }
    let Some(slot) = cx.global_mut::<Registry>().0.get_mut(&id) else {
        return;
    };
    let Some(host) = slot.host.as_ref().and_then(WeakEntity::upgrade) else {
        slot.pending = Some(message);
        return;
    };
    host.update(cx, |host, cx| host.show(message, cx));
}

pub fn show(window: &Window, message: Message, cx: &mut App) {
    show_for_window(window.window_handle().window_id(), message, cx);
}

/// Observe operational feedback without coupling child views to their parent.
/// Clearing a source rearms identical future failures; redraws do not reshow it.
pub fn watch<T: 'static>(
    window: &Window,
    cx: &mut Context<T>,
    read: impl Fn(&T) -> Option<Message> + 'static,
) {
    let id = window.window_handle().window_id();
    watch_window(id, cx, read);
}

pub fn watch_window<T: 'static>(
    id: WindowId,
    cx: &mut Context<T>,
    read: impl Fn(&T) -> Option<Message> + 'static,
) {
    watch_window_many(id, cx, move |view| {
        read(view)
            .into_iter()
            .map(|message| ((), message))
            .collect()
    });
}

/// Track independent operations by stable source, including retained drawers.
/// An unchanged older failure cannot mask a new failure from another source.
pub fn watch_many<T: 'static, K: Eq + Hash + 'static>(
    window: &Window,
    cx: &mut Context<T>,
    read: impl Fn(&T) -> Vec<(K, Message)> + 'static,
) {
    watch_window_many(window.window_handle().window_id(), cx, read);
}

pub fn watch_window_many<T: 'static, K: Eq + Hash + 'static>(
    id: WindowId,
    cx: &mut Context<T>,
    read: impl Fn(&T) -> Vec<(K, Message)> + 'static,
) {
    cx.global_mut::<Registry>().0.entry(id).or_default();
    let previous = RefCell::new(Sources {
        messages: HashMap::new(),
    });
    let update = Rc::new(move |view: &T, cx: &mut App| {
        let changes = previous.borrow_mut().update(read(view));
        for message in changes {
            show_for_window(id, message, cx);
        }
    });
    let initial = update.clone();
    cx.observe_self(move |view, cx| update(view, cx)).detach();
    let view = cx.weak_entity();
    cx.defer(move |cx| {
        let _ = view.update(cx, |view, cx| initial(view, cx));
    });
}

struct Sources<K> {
    messages: HashMap<K, Message>,
}
impl<K: Eq + Hash> Sources<K> {
    fn update(&mut self, values: Vec<(K, Message)>) -> Vec<Message> {
        let mut next = HashMap::with_capacity(values.len());
        let mut changes = vec![];
        for (key, message) in values {
            if self.messages.get(&key) != Some(&message) {
                changes.push(message.clone());
            }
            next.insert(key, message);
        }
        self.messages = next;
        changes
    }
}

struct Host {
    message: Option<Message>,
    closing: bool,
    motion: Transition,
    timer: Option<Task<()>>,
}
impl RootPlugin for Host {}

impl Host {
    fn show(&mut self, message: Message, cx: &mut Context<Self>) {
        if self.message.as_ref() != Some(&message) || self.closing {
            self.motion = Transition::new(0.);
        }
        self.message = Some(message);
        self.closing = false;
        self.timer = Some(cx.spawn(async move |host, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = host.update(cx, |host, cx| host.dismiss(cx));
        }));
        cx.notify();
    }
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.closing = true;
        cx.notify();
    }
}
impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().absolute().inset_0();
        let Some(message) = self.message.clone() else {
            return root;
        };
        let message_text = crate::localization::message(cx, &message.text);
        let (amount, moving) = self.motion.set(
            f32::from(!self.closing),
            Instant::now(),
            Duration::from_millis(120),
            cx.reduce_motion(),
        );
        if moving {
            window.request_animation_frame();
        }
        if self.closing && !moving {
            self.message = None;
            return root;
        }
        let settings = cx
            .global::<Registry>()
            .0
            .get(&window.window_handle().window_id())
            .is_some_and(|slot| slot.settings);
        let dark = cx.theme().is_dark();
        let p = if settings {
            Palette::settings(dark)
        } else {
            Palette::new(dark)
        };
        let color = rgb(if message.error { 0xef4444 } else { 0x075e92 });
        let overlays = overlay::manager(window, cx).read(cx);
        let priority = overlays
            .modal_top()
            .map_or(0, |id| overlays.modal_priority(id))
            + POPUP_PRIORITY
            + 300;
        let top = if settings { 46. } else { 58. };
        let mut text_style = window.text_style();
        text_style.font_family = cx.theme().font_family.clone();
        text_style.font_weight = FontWeights::MEDIUM;
        let natural = message_text.lines().fold(0_f32, |width, line| {
            let text: SharedString = line.to_owned().into();
            width.max(f32::from(
                window
                    .text_system()
                    .shape_line(
                        text.clone(),
                        px(14.),
                        &[text_style.to_run(text.len())],
                        None,
                    )
                    .width(),
            ))
        });
        let width = (natural.ceil() + 82.)
            .min((f32::from(window.viewport_size().width) - 36.).clamp(1., 420.));
        let height = (f32::from(window.viewport_size().height) - top - 18.).max(32.);
        root = root.child(
            deferred(
                div()
                    .id("window-message")
                    .role(if message.error {
                        Role::Alert
                    } else {
                        Role::Status
                    })
                    .absolute()
                    .top(px(top - 6. * (1. - amount)))
                    .right(px(18.))
                    .w(px(width))
                    .max_h(px(height))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(color.alpha(0.36))
                    .bg(p.surface)
                    .text_color(p.text)
                    .font_family(cx.theme().font_family.clone())
                    .text_size(px(14.))
                    .line_height(px(18.9))
                    .font_weight(FontWeights::MEDIUM)
                    .shadow(vec![BoxShadow {
                        color: rgb(0).alpha(0.12).into(),
                        offset: point(px(0.), px(8.)),
                        blur_radius: px(18.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .opacity(amount)
                    .occlude()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .pl(px(12.))
                    .pr(px(6.))
                    .py(px(8.))
                    .child(
                        crate::assets::icon(if message.error {
                            IconName::CircleAlert
                        } else {
                            IconName::Info
                        })
                        .size(px(18.))
                        .mt(px(3.))
                        .text_color(color)
                        .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .id("window-message-text")
                            .flex_1()
                            .min_w_0()
                            .max_h(px((height - 18.).max(18.)))
                            .overflow_y_scroll()
                            .whitespace_normal()
                            .child(message_text),
                    )
                    .child(
                        Button::new("dismiss-window-message")
                            .focusable(false)
                            .xsmall()
                            .ghost()
                            .size(px(24.))
                            .p_0()
                            .icon_size(px(14.))
                            .custom(ButtonCustomVariant::for_surface(p))
                            .icon(crate::assets::icon(IconName::X))
                            .tooltip("Dismiss")
                            .on_click(cx.listener(|host, _, _, cx| host.dismiss(cx))),
                    ),
            )
            .with_priority(priority),
        );
        root
    }
}

#[cfg(test)]
mod tests {
    use super::{Message, Sources};
    use std::collections::HashMap;

    #[test]
    fn persistent_failure_does_not_hide_another_operations_failure() {
        let mut sources = Sources {
            messages: HashMap::new(),
        };
        assert_eq!(
            sources
                .update(vec![("providers", Message::error("catalog failed"))])
                .len(),
            1
        );
        let changes = sources.update(vec![
            ("providers", Message::error("catalog failed")),
            ("conversation", Message::error("stream failed")),
        ]);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].text, "stream failed");
        assert!(
            sources
                .update(vec![("providers", Message::error("catalog failed"))])
                .is_empty()
        );
        assert!(sources.update(vec![]).is_empty());
        assert_eq!(
            sources
                .update(vec![("providers", Message::error("catalog failed"))])
                .len(),
            1
        );
    }

    #[test]
    fn retained_drawers_are_independent_of_observation_order() {
        let mut sources = Sources {
            messages: HashMap::new(),
        };
        assert_eq!(
            sources
                .update(vec![
                    (1, Message::error("save failed")),
                    (2, Message::error("save failed"))
                ])
                .len(),
            2
        );
        assert!(
            sources
                .update(vec![
                    (2, Message::error("save failed")),
                    (1, Message::error("save failed"))
                ])
                .is_empty()
        );
        assert!(
            sources
                .update(vec![(2, Message::error("save failed"))])
                .is_empty()
        );
        assert_eq!(
            sources
                .update(vec![
                    (2, Message::error("save failed")),
                    (3, Message::error("save failed"))
                ])
                .len(),
            1
        );
    }

    #[test]
    fn feedback_severity_is_part_of_an_observed_change() {
        let mut sources = Sources {
            messages: HashMap::new(),
        };
        assert_eq!(
            sources
                .update(vec![("operation", Message::info("status"))])
                .len(),
            1
        );
        let changes = sources.update(vec![("operation", Message::error("status"))]);
        assert_eq!(changes.len(), 1);
        assert!(changes[0].error);
        assert!(
            sources
                .update(vec![("operation", Message::error("status"))])
                .is_empty()
        );
    }
}
