use crate::{
    components::{Button, Field, Input, ScrollbarsExt, Spinner},
    connection::{Connection, ConnectionEvent},
    export_request::{self, DEFAULT_SCRIPT, ExportRequest, FormErrors},
    host_picker::{HostPicker, PickerEvent},
    layout,
    model::{AppState, Host, Id},
    native::{Core, SshOptions},
    palette::Palette,
    typography::FontWeights,
    vault::{KeySummary, Vault},
};
use gpui_kit::base::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{
    assets::IconName,
    component::{Disableable, Icon, Sizable},
    prelude::*,
    *,
};
use std::path::PathBuf;

pub struct ExportClosed;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Form,
    Selecting,
    Connecting,
    Running,
    Success,
    Failed,
}

/// This view owns only public-key material and an authenticated connection.
/// Connection credentials are fetched by the existing vault worker, not copied
/// into the export request or workspace metadata.
pub struct KeyExport {
    scroll: ScrollHandle,
    key: KeySummary,
    data: AppState,
    vault: Vault,
    keychain: Option<WeakEntity<crate::keychain::Keychain>>,
    known_hosts_path: PathBuf,
    picker: Entity<HostPicker>,
    focus: FocusHandle,
    stage: Stage,
    host: Option<Id>,
    target: Option<Host>,
    location: Entity<InputState>,
    filename: Entity<InputState>,
    script: Entity<TextareaState>,
    result_error: Entity<TextareaState>,
    advanced: bool,
    error: Option<String>,
    form_errors: FormErrors,
    host_error: Option<SharedString>,
    submitted: Option<ExportRequest>,
    connection: Option<Entity<Connection>>,
    epoch: u64,
    _connection_events: Option<Subscription>,
    _picker_events: Subscription,
    _inputs: Vec<Subscription>,
}
impl EventEmitter<ExportClosed> for KeyExport {}
impl KeyExport {
    pub(crate) fn with_keychain(mut self, keychain: WeakEntity<crate::keychain::Keychain>) -> Self {
        self.keychain = Some(keychain);
        self
    }
    pub fn new(
        key: KeySummary,
        data: AppState,
        vault: Vault,
        known_hosts_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        let picker = cx.new(|cx| HostPicker::new(data.clone(), window, cx));
        let events = cx.subscribe_in(
            &picker,
            window,
            |this, picker, event: &PickerEvent, window, cx| {
                if this.stage != Stage::Selecting {
                    return;
                }
                match event {
                    PickerEvent::UseLocal => return,
                    PickerEvent::Back => this.stage = Stage::Form,
                    PickerEvent::Selected(id) => {
                        this.host = Some(*id);
                        this.error = None;
                        this.host_error = None;
                        this.stage = Stage::Form;
                    }
                }
                if picker.read(cx).contains_focus(window, cx) {
                    this.focus.focus(window, cx);
                }
                cx.notify();
            },
        );
        let location = cx.new(|cx| InputState::new(window, cx).default_value(".ssh"));
        let filename = cx.new(|cx| InputState::new(window, cx).default_value("authorized_keys"));
        let script = cx.new(|cx| TextareaState::new(window, cx).default_value(DEFAULT_SCRIPT));
        let mut inputs: Vec<_> = [&location, &filename]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |this: &mut Self, input, event: &InputEvent, cx| {
                    if !matches!(event, InputEvent::Change) || this.stage != Stage::Form {
                        return;
                    }
                    if input.entity_id() == this.location.entity_id()
                        && this.form_errors.location.is_some()
                    {
                        this.form_errors.location =
                            export_request::location_error(input.read(cx).value().trim());
                    } else if input.entity_id() == this.filename.entity_id()
                        && this.form_errors.filename.is_some()
                    {
                        this.form_errors.filename =
                            export_request::filename_error(&input.read(cx).value());
                    }
                    cx.notify();
                })
            })
            .collect();
        inputs.push(
            cx.subscribe(&script, |this: &mut Self, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) && this.stage == Stage::Form {
                    if this.form_errors.script.is_some() {
                        this.form_errors.script =
                            export_request::script_error(&input.read(cx).value());
                    }
                    cx.notify();
                }
            }),
        );
        Self {
            scroll: ScrollHandle::new(),
            key,
            data,
            vault,
            keychain: None,
            known_hosts_path,
            picker,
            focus: cx.focus_handle(),
            stage: Stage::Form,
            host: None,
            target: None,
            location,
            filename,
            script,
            result_error: cx.new(|cx| TextareaState::new(window, cx)),
            advanced: false,
            error: None,
            form_errors: FormErrors::default(),
            host_error: None,
            submitted: None,
            connection: None,
            epoch: 0,
            _connection_events: None,
            _picker_events: events,
            _inputs: inputs,
        }
    }
    pub fn running(&self) -> bool {
        self.stage == Stage::Running
    }
    pub fn has_connection(&self) -> bool {
        matches!(self.stage, Stage::Connecting | Stage::Running)
    }
    pub fn set_data(&mut self, data: AppState, cx: &mut Context<Self>) {
        if let Some(connection) = &self.connection {
            connection.update(cx, |connection, cx| connection.set_dark(data.dark, cx));
        }
        self.picker
            .update(cx, |picker, cx| picker.set_data(data.clone(), cx));
        self.data = data;
        cx.notify();
    }
    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running() {
            return;
        }
        let owns_focus = self.focus.contains_focused(window, cx);
        self.epoch += 1;
        self.connection = None;
        self._connection_events = None;
        self.stage = Stage::Form;
        self.error = None;
        self.form_errors = FormErrors::default();
        self.host_error = None;
        if owns_focus {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stage != Stage::Form {
            return;
        }
        let host = self
            .data
            .hosts
            .iter()
            .find(|host| host.supports_ssh() && Some(host.id) == self.host)
            .cloned();
        let request = ExportRequest {
            public_key: self.key.public_key.clone().unwrap_or_default(),
            location: self.location.read(cx).value().trim().to_owned(),
            filename: self.filename.read(cx).value().trim().to_owned(),
            script: self.script.read(cx).value().to_string(),
        };
        self.host_error = host.is_none().then(|| "Select a host.".into());
        self.form_errors = request.form_errors();
        let host = match host {
            Some(host) if self.form_errors.is_empty() => host,
            _ => {
                self.advanced |= self.form_errors.script.is_some();
                cx.notify();
                return;
            }
        };
        if let Err(error) = request.validate() {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        let core = match Core::discover() {
            Ok(core) => core,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let options = SshOptions::new(
            host.address.clone(),
            host.port,
            host.username.clone(),
            self.known_hosts_path.clone(),
        );
        self.target = Some(host.clone());
        self.submitted = Some(request);
        if self.focus.contains_focused(window, cx) {
            self.focus.focus(window, cx);
        }
        self.stage = Stage::Connecting;
        self.error = None;
        self.epoch += 1;
        let epoch = self.epoch;
        let vault = self.vault.clone();
        let binding = crate::vault::authentication::Binding::from_host(&host);
        cx.spawn_in(window, async move |this, cx| {
            let result = vault
                .apply_connection(host.credential, host.proxy_id, host.ssh, options)
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.epoch != epoch || this.stage != Stage::Connecting {
                    return;
                }
                match result {
                    Ok(options) => {
                        let connection = cx.new(|cx| {
                            Connection::new(
                                core,
                                options,
                                host.name,
                                this.data.dark,
                                this.data.terminal.palette(),
                                window,
                                cx,
                            )
                            .with_host_platform(host.platform)
                            .with_authentication(
                                Some(this.vault.clone()),
                                this.keychain.clone(),
                                binding,
                            )
                            .in_drawer()
                        });
                        this._connection_events = Some(cx.subscribe_in(
                            &connection,
                            window,
                            |this, connection, event: &ConnectionEvent, window, cx| match event {
                                ConnectionEvent::Close => this.back(window, cx),
                                ConnectionEvent::Ready => this.run(connection, window, cx),
                            },
                        ));
                        this.connection = Some(connection);
                    }
                    Err(error) => {
                        this.error = Some(error.to_string());
                        this.stage = Stage::Form;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn run(
        &mut self,
        connection: &Entity<Connection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.stage != Stage::Connecting {
            return;
        }
        let Some((session, _)) = connection.update(cx, |connection, _| connection.take_session())
        else {
            return;
        };
        let Some(request) = self.submitted.clone() else {
            return;
        };
        if connection.read(cx).contains_focus(window, cx) {
            self.focus.focus(window, cx);
        }
        let epoch = self.epoch;
        self.stage = Stage::Running;
        self.connection = None;
        self._connection_events = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let result = session.export_public_key(&request);
                    drop(session);
                    result
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.epoch != epoch {
                    return;
                }
                match result {
                    Ok(()) => {
                        this.stage = Stage::Success;
                        this.error = None;
                    }
                    Err(error) => {
                        this.stage = Stage::Failed;
                        this.error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn section(&self, title: &'static str) -> Div {
        div()
            .p(px(16.))
            .rounded(px(10.))
            .bg(Palette::new(self.data.dark).surface)
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .font_weight(FontWeights::MEDIUM)
                    .child(crate::localization::view(title)),
            )
    }
    fn form(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.data.dark);
        let host = self
            .data
            .hosts
            .iter()
            .find(|host| Some(host.id) == self.host);
        div()
            .id("export-form-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .scrollbars_with(&self.scroll)
            .p(px(16.))
            .child(
                self.section("Key").child(
                    div()
                        .h(px(40.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(
                            div()
                                .size(px(40.))
                                .flex_shrink_0()
                                .rounded(px(10.))
                                .bg(rgb(0x075e92))
                                .text_color(rgb(0xffffff))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(IconName::KeyRound).with_size(px(20.))),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().truncate().child(self.key.name.clone()))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.muted)
                                        .truncate()
                                        .child(self.key.algorithm.clone()),
                                ),
                        ),
                ),
            )
            .child(
                self.section("Destination")
                    .mt(px(14.))
                    .child(
                        Field::trigger(
                            Button::new("export-select-host")
                                .label(
                                    host.map(|host| format!("{} — {}", host.name, host.address))
                                        .unwrap_or_default(),
                                )
                                .trailing_icon(
                                    Icon::new(IconName::ChevronRight).text_color(p.muted),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.stage = Stage::Selecting;
                                    cx.notify();
                                })),
                            "Host *",
                            host.is_some(),
                        )
                        .error(self.host_error.clone()),
                    )
                    .child(
                        Field::input(&self.location, "Location ($1) *")
                            .error(self.form_errors.location.map(SharedString::from)),
                    )
                    .child(
                        Field::input(&self.filename, "Filename ($2) *")
                            .error(self.form_errors.filename.map(SharedString::from)),
                    )
                    .child(
                        div()
                            .p(px(11.))
                            .rounded(px(9.))
                            .bg(rgba(0x168df212))
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .child(crate::localization::text(
                                cx,
                                "rs.ui.copy.exportPublicKeyInstructions",
                            )),
                    ),
            )
            .child(
                Button::new("export-advanced")
                    .toolbar()
                    .ghost()
                    .w_full()
                    .text_color(p.muted)
                    .mt(px(14.))
                    .icon(if self.advanced {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .label("Advanced")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.advanced = !this.advanced;
                        cx.notify();
                    })),
            )
            .when(self.advanced, |view| {
                view.child(
                    self.section("Script")
                        .mt(px(8.))
                        .child(div().text_size(px(12.)).text_color(p.muted).child(
                            crate::localization::text(cx, "rs.ui.copy.publicKeyExportPlaceholders"),
                        ))
                        .child(
                            Field::multiline(&self.script, "Script", 200.)
                                .error(self.form_errors.script.map(SharedString::from)),
                        ),
                )
            })
            .into_any_element()
    }
    fn result(&self, cx: &App) -> AnyElement {
        let p = Palette::new(self.data.dark);
        let success = self.stage == Stage::Success;
        let color = if success {
            rgb(0x34a853)
        } else {
            rgb(0xe5453d)
        };
        let request = self.submitted.as_ref();
        div()
            .id("export-result-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .scrollbars()
            .p(px(24.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .when(self.running(), |view| {
                view.child(Spinner::new().with_size(px(40.)))
            })
            .when(!self.running(), |view| {
                view.child(
                    div()
                        .size(px(64.))
                        .rounded_full()
                        .bg(if success {
                            rgba(0x34a85320)
                        } else {
                            rgba(0xe5453d20)
                        })
                        .text_color(color)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::new(if success {
                                IconName::Check
                            } else {
                                IconName::CircleAlert
                            })
                            .with_size(px(32.)),
                        ),
                )
            })
            .child(
                div()
                    .mt(px(8.))
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_center()
                    .child(crate::localization::view(if self.running() {
                        "Exporting key..."
                    } else if success {
                        "Key exported successfully."
                    } else {
                        "Key export failed"
                    })),
            )
            .child(div().text_center().text_color(p.muted).child(format!(
                    "{} → {}",
                    self.key.name,
                    self.target
                        .as_ref()
                        .map(|host| host.name.as_str())
                        .unwrap_or("Host")
                )))
            .child(
                div().text_center().text_color(p.muted).child(
                    request
                        .map(|request| format!("{}/{}", request.location, request.filename))
                        .unwrap_or_default(),
                ),
            )
            .when(self.stage == Stage::Failed, |view| {
                view.child(
                    Input::multiline(&self.result_error)
                        .readonly(true)
                        .h(px(150.))
                        .aria_label(crate::localization::text(cx, "Key export error")),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_center()
                        .text_color(p.muted)
                        .child(crate::localization::text(
                            cx,
                            "rs.ui.copy.checkRemoteFileBeforeRetry",
                        )),
                )
            })
            .into_any_element()
    }
}
impl Render for KeyExport {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.data.dark);
        if self.result_error.read(cx).value().as_ref() != self.error.as_deref().unwrap_or_default()
        {
            self.result_error.update(cx, |field, cx| {
                field.set_value(self.error.clone().unwrap_or_default(), window, cx)
            });
        }
        let body = match self.stage {
            Stage::Form => self.form(cx),
            Stage::Selecting => self.picker.clone().into_any_element(),
            Stage::Connecting => self
                .connection
                .as_ref()
                .map(|connection| connection.clone().into_any_element())
                .unwrap_or_else(|| {
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Spinner::new())
                        .into_any_element()
                }),
            _ => self.result(cx),
        };
        div()
            .w(px(layout::DRAWER_WIDTH))
            .track_focus(&self.focus)
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .bg(p.background)
            .text_color(p.text)
            .border_l_1()
            .border_color(p.border)
            .flex()
            .flex_col()
            .when(self.stage != Stage::Selecting, |view| {
                view.child(crate::components::management::drawer_header(
                    "Export Key",
                    self.data.dark,
                    vec![],
                    vec![
                        Button::new("close-key-export")
                            .toolbar()
                            .ghost()
                            .icon(IconName::ArrowRightToLine)
                            .tooltip("Close export")
                            .disabled(self.running())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.running() {
                                    cx.emit(ExportClosed);
                                }
                            }))
                            .into_any_element(),
                    ],
                ))
            })
            .child(body)
            .when(self.stage != Stage::Selecting, |view| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .border_t_1()
                        .border_color(p.border)
                        .p(px(16.))
                        .child(
                            Button::new("key-export-continue")
                                .large()
                                .w_full()
                                .accent()
                                .disabled(self.running())
                                .loading(self.running())
                                .label(match self.stage {
                                    Stage::Form => "Export to host",
                                    Stage::Running => "Exporting...",
                                    Stage::Success => "Done",
                                    _ => "Back",
                                })
                                .on_click(cx.listener(|this, _, window, cx| match this.stage {
                                    Stage::Form => this.start(window, cx),
                                    Stage::Success => cx.emit(ExportClosed),
                                    _ => this.back(window, cx),
                                })),
                        ),
                )
            })
    }
}
