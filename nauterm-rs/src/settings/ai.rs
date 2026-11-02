//! Provider drafts are saved explicitly, separate from public preferences.
use super::*;
use crate::typography::FontWeights;
use crate::{
    ai::{self, Draft, Protocol, Record, Service, Snapshot, Summary, presets::Preset},
    components::{DialogColors, DialogFrame, Modal, Spinner},
};
use gpui_kit::component::Disableable;
use zeroize::Zeroizing;

pub enum Event {
    Flushed,
    Saved,
    Failed(String),
}
enum Intent {
    Select(i64),
    Activate(i64),
    Delete(i64),
}
struct Editor {
    record: Option<Record>,
    protocol: Protocol,
    dirty: bool,
}

pub struct Providers {
    service: Option<Service>,
    dark: bool,
    preset_url: String,
    rows: Vec<Summary>,
    editor: Option<Editor>,
    fields: Vec<Entity<InputState>>,
    protocol: SelectModel<Protocol>,
    focus: FocusHandle,
    started: bool,
    busy: bool,
    saving: bool,
    sync_paused: bool,
    wait_close: bool,
    operation: Option<Task<()>>,
    presets_task: Option<Task<()>>,
    presets: Vec<Preset>,
    preset_search: Entity<InputState>,
    adding: bool,
    deleting: Option<i64>,
    editing: bool,
    advanced: bool,
    error: Option<String>,
    validation_error: Option<String>,
    preset_status: Option<String>,
    _events: Vec<Subscription>,
}
impl EventEmitter<Event> for Providers {}
impl Providers {
    pub fn ready_for_automatic_sync(&self, cx: &App) -> bool {
        !self.busy
            && !self.saving
            && !self.adding
            && !self.editing
            && self.deleting.is_none()
            && self.editor.as_ref().is_none_or(|editor| !editor.dirty)
            && !self.protocol.state.read(cx).is_open()
    }
    pub fn pause_for_sync(&mut self, paused: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_paused = paused;
        let focused = self
            .fields
            .iter()
            .any(|field| field.read(cx).focus_handle(cx).is_focused(window));
        for field in &self.fields {
            field.update(cx, |field, cx| field.set_disabled(paused, cx));
        }
        if paused && focused {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
    pub fn restored(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pause_for_sync(false, window, cx);
        self.operation = None;
        self.editor = None;
        self.rows.clear();
        self.started = false;
        self.error = None;
        self.wait_close = false;
        for field in &self.fields {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        cx.notify();
    }
    pub fn new(
        service: Result<Service, String>,
        dark: bool,
        preset_url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .as_ref()
                .filter(|_| view.started || view.service.is_some())
                .cloned()
                .map(crate::components::message::Message::error)
        });
        let (service, error) = match service {
            Ok(service) => (Some(service), None),
            Err(error) => (None, Some(error)),
        };
        let fields: Vec<_> = (0..6)
            .map(|index| cx.new(|cx| InputState::new(window, cx).masked(index == 3)))
            .collect();
        let mut events: Vec<_> = fields
            .iter()
            .map(|input| {
                cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.edited(window, cx);
                    }
                })
            })
            .collect();
        let protocol = SelectModel::new(
            Protocol::ALL
                .into_iter()
                .map(|value| SelectOption::localized(value, value.label()))
                .collect(),
            Some(Protocol::OpenAi),
            window,
            cx,
        );
        events.push(cx.subscribe_in(
            &protocol.state,
            window,
            |this, _, event: &SelectEvent<Protocol>, window, cx| {
                if let SelectEvent::Confirm(Some(Some(value))) = event
                    && let Some(editor) = &mut this.editor
                {
                    if this.fields[1].read(cx).value().trim().trim_end_matches('/')
                        == editor.protocol.base_url().trim_end_matches('/')
                    {
                        this.fields[1].update(cx, |input, cx| {
                            input.set_value(value.base_url(), window, cx)
                        });
                    }
                    editor.protocol = *value;
                    this.edited(window, cx);
                }
            },
        ));
        let preset_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search providers"));
        events.push(cx.subscribe(&preset_search, |_, _, _: &InputEvent, cx| cx.notify()));
        Self {
            service,
            dark,
            preset_url,
            rows: vec![],
            editor: None,
            fields,
            protocol,
            focus: cx.focus_handle(),
            started: false,
            busy: false,
            saving: false,
            sync_paused: false,
            wait_close: false,
            operation: None,
            presets_task: None,
            presets: ai::presets::bundled().unwrap_or_default(),
            preset_search,
            adding: false,
            deleting: None,
            editing: false,
            advanced: false,
            error,
            validation_error: None,
            preset_status: None,
            _events: events,
        }
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.dark != dark {
            self.dark = dark;
            cx.notify();
        }
    }
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.started || self.sync_paused {
            return;
        }
        self.started = true;
        cx.notify();
        self.reload(window, cx);
    }
    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.sync_paused {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        let selected = self
            .editor
            .as_ref()
            .and_then(|editor| editor.record.as_ref())
            .and_then(Record::id);
        self.editor = None;
        self.busy = true;
        self.error = None;
        self.focus.focus(window, cx);
        self.operation = Some(cx.spawn_in(window, async move |this, cx| {
            let path = service.presets_path.clone();
            let presets = smol::unblock(move || ai::presets::load(&path)).await;
            let result = service.load(selected).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Ok(presets) = presets {
                    this.presets = presets;
                }
                this.completed(result, window, cx);
            });
        }));
        cx.notify();
    }
    fn install(
        &mut self,
        record: Record,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let draft = record.draft().map_err(|error| error.to_string())?;
        self.install_draft(Some(record), draft, window, cx);
        Ok(())
    }
    fn install_draft(
        &mut self,
        record: Option<Record>,
        draft: Draft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let values = [
            draft.name,
            draft.base_url,
            draft.model,
            draft.api_key.to_string(),
            draft.max_tokens.map(|v| v.to_string()).unwrap_or_default(),
            draft.temperature.map(|v| v.to_string()).unwrap_or_default(),
        ];
        // The masked Input owns its editing buffer; temporary key copies are
        // zeroized immediately after installing the new editor contents.
        for (field, value) in self.fields.iter().zip(values) {
            let value = Zeroizing::new(value);
            field.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
        self.protocol.set_selected(Some(draft.protocol), window, cx);
        self.editor = Some(Editor {
            dirty: record.is_none(),
            record,
            protocol: draft.protocol,
        });
    }
    fn draft(&self, cx: &App) -> Result<Draft, String> {
        let value = |index: usize| self.fields[index].read(cx).value().to_string();
        let tokens = value(4);
        let temperature = value(5);
        let max_tokens = if tokens.trim().is_empty() {
            None
        } else {
            Some(
                tokens
                    .trim()
                    .parse()
                    .map_err(|_| "Max Tokens must be a positive whole number.")?,
            )
        };
        let temperature = if temperature.trim().is_empty() {
            None
        } else {
            Some(
                temperature
                    .trim()
                    .parse()
                    .map_err(|_| "Temperature must be between 0 and 1.")?,
            )
        };
        let draft = Draft {
            name: value(0),
            base_url: value(1),
            model: value(2),
            api_key: Zeroizing::new(value(3)),
            protocol: self
                .editor
                .as_ref()
                .ok_or("Select an AI provider.")?
                .protocol,
            max_tokens,
            temperature,
        };
        draft.validate().map_err(|error| error.to_string())?;
        Ok(draft)
    }
    fn edited(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.sync_paused || self.busy {
            return;
        }
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.dirty = true;
        self.error = None;
        self.validation_error = None;
        cx.notify();
    }
    fn flush(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        if self.busy || self.sync_paused || self.editor.as_ref().is_none_or(|editor| !editor.dirty)
        {
            return Ok(());
        }
        let draft = match self.draft(cx) {
            Ok(draft) => draft,
            Err(error) => {
                self.validation_error = Some(error.clone());
                cx.notify();
                return Err(error);
            }
        };
        let Some(service) = self.service.clone() else {
            return Err("The AI provider store is unavailable.".into());
        };
        let editor = self.editor.as_ref().unwrap();
        let record = editor.record.clone();
        self.validation_error = None;
        self.busy = true;
        self.saving = true;
        self.operation = Some(cx.spawn_in(window, async move |this, cx| {
            let result = match record {
                Some(record) => service.save(record, draft).await,
                None => service.create(draft).await,
            }
            .map_err(|error| error.to_string());
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                this.saving = false;
                this.operation = None;
                match result {
                    Ok(snapshot) => {
                        this.rows = snapshot.rows;
                        this.close_editor(window, cx);
                        this.error = None;
                        cx.emit(Event::Saved);
                        this.advance(window, cx);
                    }
                    Err(error) => this.failed(error, cx),
                }
                cx.notify();
            });
        }));
        cx.notify();
        Ok(())
    }
    fn failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.error = Some(error.clone());
        if self.wait_close {
            self.wait_close = false;
            cx.emit(Event::Failed(error));
        }
        cx.notify();
    }
    fn completed(
        &mut self,
        result: anyhow::Result<Snapshot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        self.operation = None;
        match result {
            Ok(snapshot) => {
                self.rows = snapshot.rows;
                self.error = None;
                if let Some(record) = snapshot.selected
                    && let Err(error) = self.install(record, window, cx)
                {
                    self.failed(error, cx);
                }
                if self.error.is_none() {
                    cx.emit(Event::Saved);
                }
                self.advance(window, cx);
            }
            Err(error) => self.failed(error.to_string(), cx),
        }
        cx.notify();
    }
    fn advance(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.wait_close {
            self.wait_close = false;
            cx.emit(Event::Flushed);
        }
    }
    fn request(&mut self, intent: Intent, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.sync_paused {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        let selected = self
            .editor
            .as_ref()
            .and_then(|editor| editor.record.as_ref())
            .and_then(Record::id);
        let row = match &intent {
            Intent::Select(id) | Intent::Activate(id) | Intent::Delete(id) => {
                self.rows.iter().find(|row| row.id == *id).cloned()
            }
        };
        if row.is_none() {
            return;
        }
        let rows = self.rows.clone();
        self.editor = None;
        self.busy = true;
        self.error = None;
        self.focus.focus(window, cx);
        self.operation = Some(cx.spawn_in(window, async move |this, cx| {
            let result = match intent {
                Intent::Select(_) => service.select(row.unwrap()).await.map(|selected| Snapshot {
                    rows,
                    selected: Some(selected),
                }),
                Intent::Activate(_) => service.activate(row.unwrap()).await,
                Intent::Delete(_) => service.delete(row.unwrap(), selected).await,
            };
            let _ = this.update_in(cx, |this, window, cx| this.completed(result, window, cx));
        }));
        cx.notify();
    }
    pub fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<bool, String> {
        if !self.saving {
            self.adding = false;
            self.close_editor(window, cx);
        }
        if !self.busy {
            return Ok(true);
        }
        self.wait_close = true;
        Ok(false)
    }
    fn refresh_presets(&mut self, cx: &mut Context<Self>) {
        if self.presets_task.is_some() {
            return;
        }
        let Some(service) = &self.service else {
            return;
        };
        let path = service.presets_path.clone();
        let url = self.preset_url.clone();
        self.preset_status = None;
        self.presets_task = Some(cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || ai::presets::refresh(&url, &path)).await;
            let _ = this.update(cx, |this, cx| {
                this.presets_task = None;
                match result {
                    Ok(presets) => {
                        this.presets = presets;
                        this.preset_status = Some("Presets updated.".into());
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn edit(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = true;
        if self
            .editor
            .as_ref()
            .and_then(|editor| editor.record.as_ref())
            .and_then(Record::id)
            != Some(id)
        {
            self.request(Intent::Select(id), window, cx);
        }
        cx.notify();
    }
    fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.validation_error = None;
        if self.saving {
            return;
        }
        self.editing = false;
        self.editor = None;
        self.error = None;
        for field in &self.fields {
            field.update(cx, |input, cx| input.set_value("", window, cx));
        }
        cx.notify();
    }
    fn create(&mut self, draft: Draft, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.sync_paused {
            return;
        }
        self.adding = false;
        self.editing = true;
        self.advanced = false;
        self.error = None;
        self.install_draft(None, draft, window, cx);
        cx.notify();
    }
    fn save_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.sync_paused {
            return;
        }
        if self.editor.as_ref().is_some_and(|editor| editor.dirty) {
            let _ = self.flush(window, cx);
        } else {
            self.close_editor(window, cx);
        }
    }
    fn field(
        &self,
        title: &'static str,
        hint: &'static str,
        control: impl IntoElement,
    ) -> AnyElement {
        let p = Palette::settings(self.dark);
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeights::MEDIUM)
                    .child(crate::localization::view(title)),
            )
            .child(control)
            .when(!hint.is_empty(), |view| {
                view.child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .child(crate::localization::view(hint)),
                )
            })
            .into_any_element()
    }
    fn editor_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.editing {
            return None;
        }
        let p = Palette::settings(self.dark);
        let pair =
            |a: AnyElement, b: AnyElement| div().w_full().flex().gap(px(10.)).child(a).child(b);
        let id = self
            .editor
            .as_ref()
            .and_then(|editor| editor.record.as_ref())
            .and_then(Record::id);
        let title = if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.record.is_none())
        {
            "Add AI Provider"
        } else {
            "Edit Provider"
        };
        let body = if self.editor.is_none() {
            div()
                .h(px(120.))
                .flex()
                .items_center()
                .justify_center()
                .child(Spinner::new())
        } else {
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(pair(
                    self.field(
                        "Name",
                        "",
                        Input::new(&self.fields[0])
                            .disabled(self.busy)
                            .aria_label(crate::localization::text(cx, "Provider name")),
                    ),
                    self.field(
                        "Protocol",
                        "",
                        self.protocol.control_disabled("Protocol", self.busy),
                    ),
                ))
                .child(
                    self.field(
                        "Base URL",
                        "",
                        Input::new(&self.fields[1])
                            .disabled(self.busy)
                            .aria_label(crate::localization::text(cx, "Base URL")),
                    ),
                )
                .child(
                    self.field(
                        "API Key",
                        "",
                        Input::new(&self.fields[3])
                            .disabled(self.busy)
                            .aria_label(crate::localization::text(cx, "API Key")),
                    ),
                )
                .child(
                    self.field(
                        "Models",
                        "Comma-separated. The first model is the default.",
                        Input::new(&self.fields[2])
                            .disabled(self.busy)
                            .aria_label(crate::localization::text(cx, "Models")),
                    ),
                )
                .child(
                    Button::new("ai-advanced")
                        .small()
                        .ghost()
                        .content_start()
                        .label("Advanced")
                        .icon(if self.advanced {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.advanced = !this.advanced;
                            cx.notify();
                        })),
                )
                .when(self.advanced, |view| {
                    view.child(pair(
                        self.field(
                            "Max Tokens",
                            "Blank uses the provider default.",
                            Input::new(&self.fields[4])
                                .disabled(self.busy)
                                .digits_only()
                                .aria_label(crate::localization::text(cx, "Max Tokens")),
                        ),
                        self.field(
                            "Temperature",
                            "Between 0 and 1.",
                            Input::new(&self.fields[5])
                                .disabled(self.busy)
                                .aria_label(crate::localization::text(cx, "Temperature")),
                        ),
                    ))
                })
                .children(crate::components::validation_message(
                    self.validation_error.as_deref(),
                    cx,
                ))
        };
        let status = if self.saving {
            "Saving…"
        } else if self.busy {
            "Loading…"
        } else if self.editor.as_ref().is_some_and(|editor| editor.dirty) {
            "Unsaved changes"
        } else {
            ""
        };
        Some(
            Modal::new("edit-ai-provider", title)
                .return_focus(&self.focus)
                .dismissible(!self.saving)
                .dismiss_on_backdrop(false)
                .on_dismiss(cx.listener(|this, _, window, cx| this.close_editor(window, cx)))
                .content(
                    DialogFrame::with_header(
                        "edit-ai-provider-frame",
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .child(div().flex_1().child(crate::localization::text(cx, title)))
                            .child(
                                Button::new("ai-editor-close")
                                    .ghost()
                                    .size(px(24.))
                                    .p_0()
                                    .icon_size(px(14.))
                                    .text_color(p.muted)
                                    .icon(crate::assets::icon(IconName::X))
                                    .tooltip("Close")
                                    .disabled(self.saving)
                                    .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                            ),
                    )
                    .width(520.)
                    .colors(DialogColors {
                        surface: p.surface,
                        header: p.sidebar,
                        border: p.border,
                        foreground: p.text,
                        dark: self.dark,
                    })
                    .content(
                        div()
                            .id("edit-ai-provider-body")
                            .max_h(px(440.))
                            .overflow_y_scroll()
                            .child(body),
                    )
                    .action(
                        Button::new("ai-editor-delete")
                            .ghost()
                            .size(px(24.))
                            .p_0()
                            .icon_size(px(14.))
                            .text_color(rgb(0xef4444))
                            .icon(IconName::Trash)
                            .tooltip("Delete provider")
                            .when(id.is_none(), |button| button.invisible())
                            .disabled(self.busy || id.is_none())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.deleting = id;
                                cx.notify();
                            })),
                    )
                    .action(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child(crate::localization::text(cx, status)),
                    )
                    .action(
                        Button::new("ai-editor-cancel")
                            .ghost()
                            .label("Cancel")
                            .disabled(self.saving)
                            .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                    )
                    .action(
                        Button::new("ai-editor-save")
                            .primary()
                            .label("Save")
                            .disabled(self.busy || self.editor.is_none())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.save_editor(window, cx)),
                            ),
                    ),
                )
                .into_any_element(),
        )
    }
    fn delete_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.deleting?;
        let row = self.rows.iter().find(|row| row.id == id)?;
        let p = Palette::settings(self.dark);
        let last = self.rows.len() == 1;
        let body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .text_size(px(13.))
            .child(crate::localization::view(format!("Delete \"{}\" and its saved API key?", row.name)))
            .when(row.active || last, |view| {
                view.child(div().text_size(px(12.)).text_color(p.muted).child(crate::localization::view(if last {
                    "This is your only provider. The assistant stays unavailable until you add another."
                } else {
                    "This provider is active. The next provider in the list becomes active."
                })))
            });
        Some(
            Modal::new("delete-ai-provider", "Delete Provider")
                .return_focus(&self.focus)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.deleting = None;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("delete-ai-provider-frame", "Delete Provider")
                        .colors(DialogColors {
                            surface: p.surface,
                            header: p.sidebar,
                            border: p.border,
                            foreground: p.text,
                            dark: self.dark,
                        })
                        .width(400.)
                        .content(body)
                        .action(
                            Button::new("ai-delete-cancel")
                                .outline()
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.deleting = None;
                                    cx.notify();
                                })),
                        )
                        .action(
                            Button::new("ai-delete-confirm")
                                .danger()
                                .label("Delete")
                                .disabled(self.busy)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.deleting = None;
                                    this.close_editor(window, cx);
                                    this.request(Intent::Delete(id), window, cx);
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
    fn picker(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.adding {
            return None;
        }
        let p = Palette::settings(self.dark);
        let query = self.preset_search.read(cx).value().to_lowercase();
        Some(
            Modal::new("add-ai-provider", "Add AI Provider")
                .return_focus(&self.focus)
                .initial_focus(&self.preset_search.read(cx).focus_handle(cx))
                .dismiss_on_backdrop(false)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.adding = false;
                    cx.notify();
                }))
                .content(
                    DialogFrame::with_header(
                        "add-ai-provider-frame",
                        div()
                            .flex()
                            .items_start()
                            .gap(px(9.))
                            .child(
                                div()
                                    .size(px(28.))
                                    .rounded(px(7.))
                                    .bg(p.accent.opacity(0.09))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconName::Sparkles)
                                            .size(px(14.))
                                            .text_color(p.accent),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.))
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeights::SEMIBOLD)
                                            .child(crate::localization::text(
                                                cx,
                                                "settings.label.addAiProvider",
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_weight(FontWeight::NORMAL)
                                            .text_color(p.muted)
                                            .child(crate::localization::text(
                                                cx,
                                                "Select a provider preset or create a blank one.",
                                            )),
                                    ),
                            )
                            .child(
                                Button::new("ai-provider-picker-close")
                                    .ghost()
                                    .icon(crate::assets::icon(IconName::X))
                                    .tooltip("Close")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.adding = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .width(520.)
                    .colors(DialogColors {
                        surface: p.surface,
                        header: p.sidebar,
                        border: p.border,
                        foreground: p.text,
                        dark: self.dark,
                    })
                    .content(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(
                                Input::new(&self.preset_search)
                                    .cleanable(true)
                                    .aria_label(crate::localization::text(cx, "Search providers")),
                            )
                            .child(
                                div()
                                    .id("ai-preset-list")
                                    .min_h(px(120.))
                                    .max_h(px(300.))
                                    .overflow_y_scroll()
                                    .scrollbars()
                                    .when(
                                        !self.presets.iter().any(|preset| {
                                            format!("{} {}", preset.name, preset.protocol.label())
                                                .to_lowercase()
                                                .contains(&query)
                                        }),
                                        |view| {
                                            view.child(div().p(px(14.)).text_color(p.muted).child(
                                                crate::localization::text(
                                                    cx,
                                                    "rs.ui.copy.noMatchingProviders",
                                                ),
                                            ))
                                        },
                                    )
                                    .children(
                                        self.presets
                                            .iter()
                                            .enumerate()
                                            .filter(|(_, preset)| {
                                                format!(
                                                    "{} {}",
                                                    preset.name,
                                                    preset.protocol.label()
                                                )
                                                .to_lowercase()
                                                .contains(&query)
                                            })
                                            .map(|(index, preset)| {
                                                let draft = preset.draft();
                                                Button::new(("ai-preset", index))
                                                    .ghost()
                                                    .w_full()
                                                    .content_start()
                                                    .label(preset.name.clone())
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.create(draft.clone(), window, cx);
                                                        },
                                                    ))
                                            }),
                                    ),
                            ),
                    )
                    .action(
                        Button::new("ai-provider-blank")
                            .ghost()
                            .label("Create Blank Provider")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create(ai::presets::blank(), window, cx);
                            })),
                    )
                    .action(
                        Button::new("ai-provider-cancel")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.adding = false;
                                cx.notify();
                            })),
                    ),
                )
                .into_any_element(),
        )
    }
}
impl Render for Providers {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.sync_paused {
            return div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(Spinner::new())
                .child(crate::localization::text(
                    cx,
                    "rs.ui.copy.automaticSyncRunning",
                ))
                .into_any_element();
        }
        let p = Palette::settings(self.dark);
        let unavailable = self.service.is_none();
        let mut list = div()
            .w_full()
            .rounded(px(9.))
            .border_1()
            .border_color(p.border)
            .overflow_hidden();
        for (index, row) in self.rows.iter().enumerate() {
            let id = row.id;
            let protocol = Protocol::parse(&row.protocol)
                .map(|protocol| protocol.label())
                .unwrap_or("Unsupported");
            let mut detail = vec![protocol.to_string()];
            if !row.model.trim().is_empty() {
                detail.push(row.model.clone());
            }
            if let Some(host) = host(&row.base_url) {
                detail.push(host);
            }
            let tint = if row.active { p.accent } else { p.muted };
            list = list.child(
                div()
                    .id(("ai-provider-row", id as u64))
                    .w_full()
                    .h(px(48.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .when(index > 0, |view| view.border_t_1().border_color(p.border))
                    .hover(|view| view.bg(p.hover))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| this.edit(id, window, cx)))
                    .child(
                        div()
                            .size(px(28.))
                            .flex_none()
                            .rounded(px(7.))
                            .bg(tint.opacity(0.1))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(tint)
                            .child(crate::assets::icon(IconName::Sparkles).size(px(14.))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeights::MEDIUM)
                                    .child(row.name.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .child(detail.join(" · ")),
                            ),
                    )
                    .child(if row.active {
                        div()
                            .px(px(6.))
                            .py(px(1.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(p.accent)
                            .text_size(px(10.))
                            .text_color(p.accent)
                            .child(crate::localization::text(cx, "rs.ui.copy.active"))
                            .into_any_element()
                    } else {
                        Button::new(("ai-active", id as u64))
                            .small()
                            .ghost()
                            .label("Use")
                            .disabled(self.busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.request(Intent::Activate(id), window, cx)
                            }))
                            .into_any_element()
                    })
                    .child(
                        Button::new(("ai-edit", id as u64))
                            .size(px(24.))
                            .p_0()
                            .icon_size(px(14.))
                            .ghost()
                            .text_color(p.accent)
                            .icon(IconName::Pencil)
                            .tooltip("Edit provider")
                            .disabled(self.busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.edit(id, window, cx)
                            })),
                    )
                    .child(
                        Button::new(("ai-delete", id as u64))
                            .size(px(24.))
                            .p_0()
                            .icon_size(px(14.))
                            .ghost()
                            .text_color(rgb(0xef4444))
                            .icon(IconName::Trash)
                            .tooltip("Delete provider")
                            .disabled(self.busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.deleting = Some(id);
                                cx.notify();
                            })),
                    ),
            );
        }
        if self.rows.is_empty() {
            list = list.child(
                div()
                    .w_full()
                    .py(px(22.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.))
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(crate::localization::view(if self.busy {
                        "Loading providers…"
                    } else {
                        "No AI providers yet. Add one to use the assistant."
                    }))
                    .when(!self.busy, |view| {
                        view.child(
                            Button::new("ai-add-empty")
                                .primary()
                                .small()
                                .icon(crate::assets::icon(IconName::Plus))
                                .label("Add Provider")
                                .disabled(unavailable)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.adding = true;
                                    cx.notify();
                                })),
                        )
                    }),
            );
        }
        list = list.child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(4.))
                .px(px(6.))
                .py(px(4.))
                .border_t_1()
                .border_color(p.border)
                .bg(p.sidebar)
                .child(
                    Button::new("ai-add")
                        .small()
                        .ghost()
                        .icon(crate::assets::icon(IconName::Plus))
                        .label(crate::localization::text(cx, "settings.label.addProvider"))
                        .disabled(unavailable || self.busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.adding = true;
                            cx.notify();
                        })),
                )
                .child(div().flex_1())
                .when(self.busy, |view| view.child(Spinner::new()))
                .child(
                    Button::new("ai-presets-refresh")
                        .size(px(24.))
                        .p_0()
                        .icon_size(px(14.))
                        .ghost()
                        .text_color(rgb(0x8b5cf6))
                        .icon(IconName::Download)
                        .tooltip(crate::localization::text(cx, "rs.ai.action.updatePresets"))
                        .disabled(unavailable || self.presets_task.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_presets(cx))),
                ),
        );
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .track_focus(&self.focus)
            .child(list)
            .when_some(self.preset_status.clone(), |view, status| {
                view.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.muted)
                        .child(crate::localization::message(cx, &status)),
                )
            })
            .when_some(self.editor_modal(cx), |view, modal| view.child(modal))
            .when_some(self.picker(cx), |view, picker| view.child(picker))
            .when_some(self.delete_dialog(cx), |view, dialog| view.child(dialog))
            .into_any_element()
    }
}
fn host(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', '?']).next()?.trim();
    (!host.is_empty()).then(|| host.to_owned())
}

impl Settings {
    pub(super) fn ai_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        sections::stack()
            .child(self.section(
                Section::AiProviders,
                IconName::Sparkles,
                self.providers.clone(),
            ))
            .child(
                self.section(
                    Section::AiContext,
                    IconName::Terminal,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(
                            self.row(
                                "settings.ai.terminalSelection.label",
                                "settings.ai.terminalSelection.description",
                                Switch::new(
                                    "ai-terminal-selection",
                                    "Terminal Selection",
                                    self.values.ai.include_selection,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.ai.include_selection = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        )
                        .child(
                            self.row(
                                "settings.ai.recentOutput.label",
                                "settings.ai.recentOutput.description",
                                Switch::new(
                                    "ai-recent-output",
                                    "Recent Output",
                                    self.values.ai.include_recent_output,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.ai.include_recent_output = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        ),
                ),
            )
            .into_any_element()
    }
}
