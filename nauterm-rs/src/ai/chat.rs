//! Shared workspace/terminal AI presentation, built from Nauterm's own controls.
use super::{
    Preferences, Service, Summary,
    history::{Scope, Session},
};
use crate::{
    components::{
        Button, ButtonCustomVariant, Input, MarkdownDocument, ScrollbarsExt, Spinner,
        menu::{DropdownMenu, MenuItem},
    },
    palette::Palette,
    terminal::Terminal,
    terminal_appearance::Palette as TerminalPalette,
};
use gpui_kit::{
    assets::IconName,
    base::input::{InputEvent, InputState, TextareaState},
    component::{Disableable, Sizable},
    prelude::*,
    *,
};
use std::{sync::Arc, time::Duration};
mod attachments;
mod commands;
mod composer;
mod conversations;
mod history_list;
mod messages;
mod provider;
mod scroll;
mod source;
mod toolbar;
mod windowed;
pub use source::Source;

pub struct Configure;
pub struct Chat {
    service: Option<Service>,
    source: Source,
    scope: Scope,
    host_uuid: Option<String>,
    session: Option<Arc<Session>>,
    input: Entity<TextareaState>,
    attachments: Vec<super::attachment::Attachment>,
    picking_attachments: bool,
    model: Entity<InputState>,
    history_search: Entity<InputState>,
    providers: Vec<Summary>,
    provider: Option<String>,
    provider_loading: bool,
    provider_revision: Option<u64>,
    provider_error: Option<String>,
    history_loading: bool,
    history: Option<history_list::Catalog>,
    error: Option<String>,
    defaults: Preferences,
    context: Preferences,
    active: bool,
    scroll: ScrollHandle,
    message_rows: windowed::Rows,
    selecting_messages: bool,
    selection_drag: bool,
    selection_pending: bool,
    history_scroll: UniformListScrollHandle,
    follow: bool,
    last_offset: Pixels,
    scroll_motion: scroll::Latest,
    selection_focus: FocusHandle,
    hovered_message: Option<i64>,
    copied_text: Option<String>,
    copy_reset: Option<Task<()>>,
    generation: u64,
    updates: Option<Task<()>>,
    documents: std::collections::BTreeMap<String, messages::Document>,
    command_scroll: std::collections::BTreeMap<String, ScrollHandle>,
    command_clock: Option<Task<()>>,
    editor: Option<messages::Editor>,
    delete_pending: Option<super::history::DeleteTarget>,
    delete_error: Option<String>,
    deleting: bool,
    delete_focus: FocusHandle,
    _source_events: Option<Subscription>,
    _events: Vec<Subscription>,
}
impl EventEmitter<Configure> for Chat {}
impl Drop for Chat {
    fn drop(&mut self) {
        if let Some(session) = &self.session {
            session.release();
        }
    }
}
impl Chat {
    pub fn new(
        service: Result<Service, String>,
        source: Source,
        defaults: Preferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch_many(window, cx, |view: &Self| {
            [
                ("conversation", view.error.clone()),
                ("providers", view.provider_error.clone()),
                ("delete", view.delete_error.clone()),
                (
                    "session",
                    view.session.as_ref().and_then(|session| session.error()),
                ),
            ]
            .into_iter()
            .filter_map(|(source, error)| {
                error.map(|error| (source, crate::components::message::Message::error(error)))
            })
            .collect()
        });
        let scope = source.scope();
        let host_uuid = source.host_uuid();
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(source.placeholder())
                .submit_on_enter(true)
                .auto_grow(1, 6)
        });
        let model = cx.new(|cx| InputState::new(window, cx).placeholder("Model"));
        let history_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search conversations"));
        let mut events = vec![
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(
                    event,
                    InputEvent::PressEnter {
                        shift: false,
                        secondary: false
                    }
                ) {
                    this.send(window, cx);
                }
                cx.notify();
            }),
            cx.subscribe(&history_search, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(history) = &mut this.history
                    && history.set_query(&input.read(cx).value())
                {
                    this.history_scroll = UniformListScrollHandle::new();
                }
                cx.notify();
            }),
        ];
        let keyboard_owner = cx.weak_entity();
        let window_id = window.window_handle().window_id();
        events.push(cx.intercept_keystrokes(move |event, window, cx| {
            use gpui_kit::base::TextSelection;
            if window.window_handle().window_id() != window_id
                || !crate::shortcuts::accepts_event(event)
            {
                return;
            }
            let key = &event.keystroke;
            let primary = if cfg!(target_os = "macos") {
                key.modifiers.platform
            } else {
                key.modifiers.control
            };
            if !primary
                || key.modifiers.shift
                || key.modifiers.alt
                || !matches!(key.key.as_str(), "a" | "c")
            {
                return;
            }
            let Some(owner) = keyboard_owner.upgrade() else {
                return;
            };
            let (active, focused) = {
                let view = owner.read(cx);
                (view.active, view.message_text_focused(window, cx))
            };
            if !active || !focused {
                return;
            }
            if key.key == "a" {
                owner.update(cx, |view, cx| view.select_all_messages(window, cx));
            } else {
                let text = TextSelection::selected_text(window, cx);
                if text.is_empty() {
                    return;
                }
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            window.prevent_default();
            cx.stop_propagation();
        }));
        let (service, error) = match service {
            Ok(service) => (Some(service), None),
            Err(error) => (None, Some(error)),
        };
        let mut this = Self {
            source,
            scope,
            host_uuid: host_uuid.clone(),
            session: None,
            input,
            attachments: vec![],
            picking_attachments: false,
            model,
            history_search,
            providers: vec![],
            provider: None,
            provider_loading: false,
            provider_revision: None,
            provider_error: None,
            history_loading: false,
            history: None,
            service: service.clone(),
            error,
            defaults: defaults.clone(),
            context: defaults,
            active: false,
            scroll: ScrollHandle::new(),
            message_rows: windowed::Rows::default(),
            selecting_messages: false,
            selection_drag: false,
            selection_pending: false,
            history_scroll: UniformListScrollHandle::new(),
            follow: true,
            last_offset: px(0.),
            scroll_motion: scroll::Latest::default(),
            selection_focus: cx.focus_handle(),
            hovered_message: None,
            copied_text: None,
            copy_reset: None,
            generation: 0,
            updates: None,
            documents: Default::default(),
            command_scroll: Default::default(),
            command_clock: None,
            editor: None,
            delete_pending: None,
            delete_error: None,
            deleting: false,
            delete_focus: cx.focus_handle(),
            _source_events: None,
            _events: events,
        };
        if let Some(service) = service {
            this.install(Session::new(service, scope, host_uuid), window, cx);
        }
        this.observe_source(cx);
        this
    }
    pub fn set_theme(&mut self, colors: TerminalPalette, cx: &mut Context<Self>) {
        if self.source.set_terminal_theme(colors) {
            cx.notify();
        }
    }
    pub fn sync(
        &mut self,
        active: bool,
        defaults: &Preferences,
        source: Source,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.source.same(&source) {
            let changed_scope = self.scope != source.scope();
            self.scope = source.scope();
            self.host_uuid = source.host_uuid();
            self.source = source;
            self.observe_source(cx);
            if changed_scope && let Some(service) = self.service.clone() {
                self.install(
                    Session::new(service, self.scope, self.host_uuid.clone()),
                    window,
                    cx,
                );
            }
            cx.notify();
        }
        if &self.defaults != defaults {
            self.defaults = defaults.clone();
            self.context = defaults.clone();
            cx.notify();
        }
        // Dialogs, native choosers and window activation change interactivity,
        // not the provider catalog. Keep the loaded conversation UI mounted.
        if active && self.provider_revision != Some(super::provider_revision()) {
            self.reload_providers(window, cx);
        }
        self.active = active;
    }
    fn palette(&self) -> Palette {
        self.source.palette()
    }
    fn busy(&self) -> bool {
        self.session.as_ref().is_some_and(|session| session.busy())
    }
    fn dirty(&self) -> bool {
        self.session.as_ref().is_some_and(|session| session.dirty())
    }
    fn retry_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        self.history_loading = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = session.save_draft().await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.history_loading = false;
                this.error = result.err().map(|error| error.to_string());
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn discard_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading {
            return;
        }
        if let Some(service) = self.service.clone() {
            self.install(
                Session::new(service, self.scope, self.host_uuid.clone()),
                window,
                cx,
            );
        }
    }
    fn install(&mut self, session: Arc<Session>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(current) = &self.session {
            current.release();
        }
        self.editor = None;
        self.hovered_message = None;
        self.copied_text = None;
        self.copy_reset = None;
        self.attachments.clear();
        self.documents.clear();
        self.command_scroll.clear();
        self.command_clock = None;
        let entry = session.view().entry;
        if entry.provider_uuid.is_some() {
            self.provider = entry.provider_uuid;
        }
        let model = if entry.model.is_empty() && entry.id.is_some() {
            self.providers
                .iter()
                .find(|provider| provider.uuid == self.provider)
                .and_then(|provider| {
                    provider
                        .model
                        .split(',')
                        .map(str::trim)
                        .find(|model| !model.is_empty())
                })
                .unwrap_or_default()
                .to_owned()
        } else {
            entry.model
        };
        if !model.is_empty() || entry.id.is_some() {
            self.model
                .update(cx, |input, cx| input.set_value(model, window, cx));
        }
        self.history = None;
        self.history_loading = false;
        self.error = None;
        self.follow = true;
        self.last_offset = px(0.);
        self.scroll_motion.clear();
        self.scroll = ScrollHandle::new();
        self.message_rows = windowed::Rows::default();
        self.selecting_messages = false;
        self.selection_drag = false;
        self.selection_pending = false;
        let changes = session.subscribe();
        self.updates = Some(cx.spawn(async move |this, cx| {
            while changes.recv().await.is_ok() {
                if this
                    .update(cx, |this, cx| {
                        this.observe_scroll();
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        self.session = Some(session);
        cx.notify();
    }
    fn reload_providers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.provider_loading {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.provider_loading = true;
        self.provider_revision = Some(super::provider_revision());
        self.provider_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.provider_summaries().await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.provider_loading = false;
                match result {
                    Ok(providers) => {
                        if this.provider.is_none() {
                            this.provider = providers
                                .iter()
                                .find(|provider| provider.active)
                                .and_then(|provider| provider.uuid.clone());
                        }
                        this.providers = providers;
                        if this.model.read(cx).value().trim().is_empty()
                            && let Some(provider) = this
                                .providers
                                .iter()
                                .find(|provider| provider.uuid == this.provider)
                        {
                            let model = provider
                                .model
                                .split(',')
                                .map(str::trim)
                                .find(|model| !model.is_empty())
                                .unwrap_or_default()
                                .to_owned();
                            this.model
                                .update(cx, |input, cx| input.set_value(model, window, cx));
                        }
                    }
                    Err(error) => this.provider_error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading || self.history.is_some() || self.picking_attachments
        {
            return;
        }
        if self.editor.is_some() {
            self.error = Some("Save or cancel the message edit first.".into());
            cx.notify();
            return;
        }
        let Some(session) = &self.session else {
            return;
        };
        let prompt = self.input.read(cx).value().to_string();
        let context = self.source.capture(&self.context, cx);
        let model = self.model.read(cx).value().trim().to_owned();
        let target = self.command_owner(cx);
        match session.start(
            prompt,
            context,
            self.attachments
                .iter()
                .map(super::attachment::Attachment::value)
                .collect(),
            self.provider.clone().unwrap_or_default(),
            model,
            target,
        ) {
            Ok(()) => {
                self.attachments.clear();
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.error = None;
                self.follow = true;
                self.scroll_motion.clear();
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    fn observe_scroll(&mut self) {
        let offset = self.scroll.offset().y;
        let extent = self.scroll.max_offset().y.max(px(0.));
        let clamped = self.last_offset.clamp(-extent, px(0.));
        if self.scroll_motion.moved(offset.into(), extent.into())
            || (offset - clamped).abs() > px(0.5)
        {
            self.scroll_motion.clear();
            self.follow = self.at_bottom();
        }
        self.last_offset = offset;
        if !self.follow {
            self.scroll_motion.clear();
        }
    }
    fn at_bottom(&self) -> bool {
        self.scroll.max_offset().y + self.scroll.offset().y <= px(2.)
    }
    fn follow_latest(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.follow || self.selecting_messages {
            self.scroll_motion.clear();
            return;
        }
        let mut offset = self.scroll.offset();
        let (next, moving) = self.scroll_motion.step(
            offset.y.into(),
            self.scroll.max_offset().y.into(),
            std::time::Instant::now(),
            cx.reduce_motion(),
        );
        offset.y = px(next);
        self.scroll.set_offset(offset);
        self.last_offset = offset.y;
        if moving {
            window.request_animation_frame();
        }
    }
    fn jump_to_bottom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        gpui_kit::base::TextSelection::clear(window, cx);
        self.selecting_messages = false;
        self.selection_drag = false;
        self.selection_pending = false;
        self.follow = true;
        self.scroll_motion.clear();
        self.last_offset = self.scroll.offset().y;
        cx.notify();
    }
    fn new_conversation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading || self.picking_attachments {
            return;
        }
        if self.dirty() {
            self.error = Some("Save the current draft or explicitly discard it first.".into());
            cx.notify();
            return;
        }
        if let Some(service) = self.service.clone() {
            self.install(
                Session::new(service, self.scope, self.host_uuid.clone()),
                window,
                cx,
            );
            self.input
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
    }
    fn show_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading || self.picking_attachments {
            return;
        }
        if self.history.take().is_some() {
            cx.notify();
            return;
        }
        self.refresh_history(window, cx);
    }
    fn refresh_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading {
            return;
        }
        if self.dirty() {
            self.error = Some(
                "Save the current draft or explicitly discard it before opening history.".into(),
            );
            cx.notify();
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.history_loading = true;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let host = self.host_uuid.clone();
        let scope = self.scope;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.conversations(scope, host).await;
            let result = cx
                .background_spawn(async move { result.map(history_list::Catalog::new) })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if generation != this.generation {
                    return;
                }
                this.history_loading = false;
                match result {
                    Ok(mut rows) => {
                        rows.set_query(&this.history_search.read(cx).value());
                        this.history = Some(rows);
                        this.error = None;
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn open_history(&mut self, uuid: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.history_loading = true;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let scope = self.scope;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.conversation(uuid, scope).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if generation != this.generation {
                    return;
                }
                this.history_loading = false;
                match result {
                    Ok(session) => this.install(session, window, cx),
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn typing_dots(p: Palette, reduced: bool) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(2.))
        .children((0usize..3).map(|index| {
            div()
                .size(px(3.))
                .rounded_full()
                .bg(p.text)
                .relative()
                .map(|dot| {
                    if reduced {
                        dot.into_any_element()
                    } else {
                        dot.with_animation(
                            ("ai-typing", index),
                            Animation::new(Duration::from_millis(1000)).repeat_synced(),
                            move |dot, t| {
                                let phase = (t - index as f32 * 0.15).rem_euclid(1.);
                                dot.top(px(-((phase * std::f32::consts::TAU).sin().max(0.) * 2.)))
                            },
                        )
                        .into_any_element()
                    }
                })
        }))
        .into_any_element()
}
impl Render for Chat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observe_scroll();
        self.follow_latest(window, cx);
        let p = self.palette();
        let busy = self.busy();
        let history = self.history.is_some();
        let dirty = self.dirty();
        let delete_overlay = self.delete_overlay(cx);
        div()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(p.border)
            .text_color(p.text)
            .bg(p.background)
            .when(self.provider_loading || self.history_loading, |body| {
                body.child(div().px(px(12.)).child(Spinner::new()))
            })
            .when(dirty && !busy, |body| {
                body.child(
                    div()
                        .px(px(10.))
                        .flex()
                        .gap(px(6.))
                        .child(
                            Button::new("ai-save-draft")
                                .xsmall()
                                .ghost()
                                .custom(ButtonCustomVariant::for_surface(p))
                                .label("Retry save")
                                .disabled(self.history_loading)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.retry_save(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("ai-discard-draft")
                                .xsmall()
                                .ghost()
                                .custom(ButtonCustomVariant::for_surface(p))
                                .label("Discard draft")
                                .disabled(self.history_loading)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.discard_draft(window, cx)
                                })),
                        ),
                )
            })
            .child(if let Some(rows) = &self.history {
                self.history_body(rows, cx)
            } else {
                self.messages(window, cx)
            })
            .when(!history, |body| body.child(self.composer(window, cx)))
            .children(delete_overlay)
    }
}
