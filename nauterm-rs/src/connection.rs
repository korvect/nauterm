mod authentication;
mod choice;
mod footer;
mod logs;
mod presentation;
mod progress;
use crate::components::ScrollbarsExt;
use crate::{
    components::{Button, ButtonCustomVariant, Input},
    connection_state::{ConnectionState, Intervention, Phase},
    layout::ConnectionMetrics,
    native::{Core, HostTrust, Session, SshOptions},
    palette::Palette,
    terminal_appearance::Palette as TerminalPalette,
    typography::FontWeights,
};
pub use choice::{Protocol, ProtocolSelected};
pub(crate) use footer::HostAction;
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::{
    assets::IconName,
    base::ElementExt,
    component::{Disableable, Sizable},
    prelude::*,
    *,
};
use presentation::{Body, HeaderAction, PageState};
use std::{path::PathBuf, sync::Arc};
use zeroize::{Zeroize, Zeroizing};

pub enum ConnectionEvent {
    Ready,
    Close,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthMode {
    Password,
    PublicKey,
}
enum Target {
    Choice(Box<choice::Choice>),
    Ssh(Box<SshOptions>),
    Mosh(Box<SshOptions>, crate::mosh::Config),
    Serial(crate::serial::Config),
    Telnet(crate::telnet::Options),
}
impl Target {
    fn ssh(&self) -> Option<&SshOptions> {
        match self {
            Self::Ssh(options) | Self::Mosh(options, _) => Some(options),
            Self::Serial(_) | Self::Telnet(_) | Self::Choice(_) => None,
        }
    }
    fn ssh_mut(&mut self) -> Option<&mut SshOptions> {
        match self {
            Self::Ssh(options) | Self::Mosh(options, _) => Some(options),
            Self::Serial(_) | Self::Telnet(_) | Self::Choice(_) => None,
        }
    }
    fn endpoint(&self) -> String {
        match self {
            Self::Choice(choice) => choice.address.clone(),
            Self::Ssh(options) | Self::Mosh(options, _) => {
                format!("{}:{}", options.host, options.port)
            }
            Self::Serial(config) => format!("{} @ {}", config.path, config.summary()),
            Self::Telnet(options) => options.endpoint(),
        }
    }
    fn open(
        &self,
        core: &Arc<Core>,
        palette: TerminalPalette,
        recording: Option<crate::recordings::Start>,
    ) -> anyhow::Result<Session> {
        let mut session = match self {
            Self::Choice(_) => anyhow::bail!("Choose a protocol before connecting."),
            Self::Mosh(options, config) => {
                return core.mosh_recorded(options, config, palette, recording);
            }
            Self::Ssh(options) => return core.ssh_recorded(options, palette, recording),
            Self::Serial(config) => core.serial(config, palette)?,
            Self::Telnet(options) => core.telnet(options, palette)?,
        };
        session.defer_recording(recording);
        Ok(session)
    }
}

pub struct Connection {
    core: Arc<Core>,
    target: Target,
    session: Option<Session>,
    attached: Option<WeakEntity<crate::terminal::Terminal>>,
    _attached_events: Option<Subscription>,
    state: ConnectionState,
    pipeline: progress::Pipeline,
    title: String,
    platform: crate::avatar::Platform,
    dark: bool,
    terminal_palette: TerminalPalette,
    recording: Option<crate::recordings::Start>,
    in_drawer: bool,
    sftp_directory: Option<String>,
    sftp_listing: Option<crate::sftp_client::Listing>,
    sftp_cancel: Option<crate::sftp_client::Cancellation>,
    sftp_epoch: u64,
    focus: FocusHandle,
    username: Entity<InputState>,
    password: Entity<InputState>,
    passphrase: Entity<InputState>,
    fido2_pin: Entity<InputState>,
    logs: Entity<logs::View>,
    mode: AuthMode,
    auth: authentication::State,
    show_logs: bool,
    error: Option<String>,
    needs_auth_focus: bool,
    auto_focus: bool,
    viewport: Size<Pixels>,
    footer_height: Pixels,
    host_actions: bool,
    reloading: bool,
    _output: Option<Task<()>>,
    _reveal: Option<Task<()>>,
    _inputs: Vec<Subscription>,
}
struct Presentation {
    directory: Option<String>,
    auto_focus: bool,
    terminal_palette: TerminalPalette,
    recording: Option<crate::recordings::Start>,
    attached: Option<WeakEntity<crate::terminal::Terminal>>,
}
pub(crate) struct TerminalPresentation {
    pub title: String,
    pub dark: bool,
    pub palette: TerminalPalette,
    pub recording: Option<crate::recordings::Start>,
}
impl EventEmitter<ConnectionEvent> for Connection {}

fn action(id: &str, label: &str, primary: bool, compact: bool) -> Button {
    Button::new(SharedString::from(id.to_owned()))
        .with_size(if compact {
            gpui_kit::component::Size::XSmall
        } else {
            gpui_kit::component::Size::Small
        })
        .h(px(if compact { 24. } else { 36. }))
        .flex_shrink_0()
        .px(px(if compact { 8. } else { 16. }))
        .rounded(px(8.))
        .label(label.to_owned())
        .when(primary, |button| button.primary())
        .when(!primary, |button| button.secondary())
}

fn auth_input(state: &Entity<InputState>, label: &'static str, secret: bool) -> Input {
    Input::new(state)
        .large()
        .aria_label(label)
        .when(secret, |input| input.mask_toggle())
}

impl Connection {
    pub fn with_host_platform(mut self, platform: crate::avatar::Platform) -> Self {
        self.platform = platform;
        self
    }
    pub fn set_host_platform(&mut self, platform: crate::avatar::Platform, cx: &mut Context<Self>) {
        if self.platform != platform {
            self.platform = platform;
            cx.notify();
        }
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.dark != dark {
            self.dark = dark;
            if let Some((editor, _)) = &self.auth.related {
                editor.update(cx, |editor, cx| editor.set_dark(dark, cx));
            }
            cx.notify();
        }
    }
    pub fn new(
        core: Arc<Core>,
        options: SshOptions,
        title: String,
        dark: bool,
        terminal_palette: TerminalPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create(
            core,
            options,
            title,
            dark,
            Presentation {
                attached: None,
                directory: None,
                auto_focus: true,
                terminal_palette,
                recording: None,
            },
            window,
            cx,
        )
    }
    pub(crate) fn new_reconnect(
        core: Arc<Core>,
        options: SshOptions,
        terminal: WeakEntity<crate::terminal::Terminal>,
        mosh: Option<crate::mosh::Config>,
        presentation: TerminalPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let target = match mosh {
            Some(config) => Target::Mosh(Box::new(options), config),
            None => Target::Ssh(Box::new(options)),
        };
        Self::create_target(
            core,
            target,
            presentation.title,
            presentation.dark,
            Presentation {
                directory: None,
                auto_focus: false,
                terminal_palette: presentation.palette,
                recording: None,
                attached: Some(terminal),
            },
            window,
            cx,
        )
    }
    pub fn new_unfocused(
        core: Arc<Core>,
        options: SshOptions,
        terminal: TerminalPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create(
            core,
            options,
            terminal.title,
            terminal.dark,
            Presentation {
                attached: None,
                directory: None,
                auto_focus: false,
                terminal_palette: terminal.palette,
                recording: terminal.recording,
            },
            window,
            cx,
        )
    }
    pub fn allow_auto_focus(&mut self, allowed: bool, cx: &mut Context<Self>) {
        if self.auto_focus != allowed {
            self.auto_focus = allowed;
            cx.notify();
        }
    }
    pub fn phase(&self) -> Phase {
        self.state.phase
    }
    pub(crate) fn retry_attached(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.attached.is_some() && !self.auth.busy {
            self.start(window, cx);
        }
    }
    pub fn new_sftp_unfocused(
        core: Arc<Core>,
        options: SshOptions,
        title: String,
        dark: bool,
        directory: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create(
            core,
            options,
            title,
            dark,
            Presentation {
                attached: None,
                directory: Some(directory),
                auto_focus: false,
                terminal_palette: TerminalPalette::default(),
                recording: None,
            },
            window,
            cx,
        )
    }
    fn create(
        core: Arc<Core>,
        options: SshOptions,
        title: String,
        dark: bool,
        presentation: Presentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create_target(
            core,
            Target::Ssh(Box::new(options)),
            title,
            dark,
            presentation,
            window,
            cx,
        )
    }
    pub fn new_mosh_unfocused(
        core: Arc<Core>,
        options: SshOptions,
        config: crate::mosh::Config,
        terminal: TerminalPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create_target(
            core,
            Target::Mosh(Box::new(options), config),
            terminal.title,
            terminal.dark,
            Presentation {
                attached: None,
                directory: None,
                auto_focus: false,
                terminal_palette: terminal.palette,
                recording: terminal.recording,
            },
            window,
            cx,
        )
    }
    pub fn new_serial_unfocused(
        core: Arc<Core>,
        config: crate::serial::Config,
        terminal: TerminalPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create_target(
            core,
            Target::Serial(config),
            terminal.title,
            terminal.dark,
            Presentation {
                attached: None,
                directory: None,
                auto_focus: false,
                terminal_palette: terminal.palette,
                recording: terminal.recording,
            },
            window,
            cx,
        )
    }
    pub fn new_telnet_unfocused(
        core: Arc<Core>,
        options: crate::telnet::Options,
        terminal: TerminalPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create_target(
            core,
            Target::Telnet(options),
            terminal.title,
            terminal.dark,
            Presentation {
                attached: None,
                directory: None,
                auto_focus: false,
                terminal_palette: terminal.palette,
                recording: terminal.recording,
            },
            window,
            cx,
        )
    }
    fn create_target(
        core: Arc<Core>,
        target: Target,
        title: String,
        dark: bool,
        presentation: Presentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let username = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Username")
                .default_value(
                    target
                        .ssh()
                        .map(|options| options.username.clone())
                        .unwrap_or_default(),
                )
        });
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Password")
                .masked(true)
        });
        let passphrase = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Key passphrase (optional)")
                .masked(true)
        });
        let logs = cx.new(logs::View::new);
        let fido2_pin = cx.new(|cx| InputState::new(window, cx).placeholder("PIN").masked(true));
        let mut inputs = vec![];
        for input in [&username, &password, &passphrase, &fido2_pin] {
            inputs.push(cx.subscribe_in(
                input,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.error = None;
                        cx.notify();
                    }
                    if matches!(event, InputEvent::PressEnter { .. })
                        && this.state.phase == Phase::Authentication
                    {
                        this.authenticate(window, cx);
                    }
                },
            ));
        }
        let focus = cx.focus_handle();
        if presentation.auto_focus {
            focus.focus(window, cx);
        }
        let has_saved_key = target
            .ssh()
            .is_some_and(|options| !options.private_key.is_empty());
        let mut this = Self {
            core,
            target,
            session: None,
            attached: presentation.attached,
            _attached_events: None,
            state: ConnectionState::default(),
            pipeline: progress::Pipeline::default(),
            title,
            dark,
            in_drawer: false,
            platform: Default::default(),
            terminal_palette: presentation.terminal_palette,
            recording: presentation.recording,
            sftp_directory: presentation.directory,
            sftp_listing: None,
            sftp_cancel: None,
            sftp_epoch: 0,
            focus,
            username,
            password,
            passphrase,
            fido2_pin,
            logs,
            mode: if has_saved_key {
                AuthMode::PublicKey
            } else {
                AuthMode::Password
            },
            auth: authentication::State::default(),
            show_logs: false,
            error: None,
            needs_auth_focus: false,
            auto_focus: presentation.auto_focus,
            viewport: Size::default(),
            footer_height: px(0.),
            host_actions: false,
            reloading: false,
            _output: None,
            _reveal: None,
            _inputs: inputs,
        };
        if matches!(this.target, Target::Choice(_)) {
            return this;
        }
        if this
            .target
            .ssh()
            .is_some_and(|options| options.username.trim().is_empty())
        {
            this.state.phase = Phase::Authentication;
            this.state
                .remember_intervention(Intervention::Authentication);
            this.state.message = "Enter a username to connect.".into();
            this.needs_auth_focus = true;
            if this.auto_focus {
                this.username
                    .update(cx, |input, cx| input.focus(window, cx));
            }
        } else {
            if let Some(terminal) = this.attached.as_ref().and_then(WeakEntity::upgrade) {
                this._attached_events = Some(cx.subscribe(
                    &terminal,
                    |this, _, _: &crate::terminal::reconnect::Changed, cx| this.refresh(cx),
                ));
            }
            this.start(window, cx);
        }
        this
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }

    pub fn contains_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
    }
    pub fn in_drawer(mut self) -> Self {
        self.in_drawer = true;
        self
    }
    fn action(&self, id: &str, label: &str, primary: bool) -> Button {
        action(id, label, primary, self.compact_controls())
    }
    fn compact_controls(&self) -> bool {
        let height = if self.viewport.height > px(0.) {
            f32::from(self.viewport.height)
        } else {
            480.
        };
        ConnectionMetrics::new(height, self.in_drawer).compact
    }

    pub fn take_session(&mut self) -> Option<(Session, Vec<crate::connection_state::LogEntry>)> {
        self._output = None;
        self._reveal = None;
        self.session
            .take()
            .map(|session| (session, std::mem::take(&mut self.state.logs)))
    }
    pub fn duplicate_options(&self) -> Option<SshOptions> {
        // A new network connection must perform its own strict host-key check.
        // A one-time trust decision for the original connection is not a bypass
        // for later split/inner-tab connections.
        self.target.ssh().map(SshOptions::for_new_connection)
    }
    pub fn set_terminal_palette(&mut self, palette: TerminalPalette) -> anyhow::Result<()> {
        if let Some(session) = &mut self.session {
            session.apply_palette(palette)?;
        }
        self.terminal_palette = palette;
        Ok(())
    }
    pub fn take_sftp(&mut self) -> Option<(SshOptions, crate::sftp_client::Listing)> {
        let listing = self.sftp_listing.take()?;
        self._output = None;
        self._reveal = None;
        Some((self.target.ssh()?.clone(), listing))
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.auth.invalidate();
        self.reloading = false;
        self.error = None;
        self.show_logs = false;
        self._reveal = None;
        self.state.begin_attempt();
        if self.target.ssh().is_none() {
            // Release the previous device before retrying. No SSH credentials,
            // trust files or shell setup are involved in this transport.
            self._output = None;
            self.session = None;
            match self
                .target
                .open(&self.core, self.terminal_palette, self.recording.clone())
            {
                Ok(session) => {
                    self.watch_session(session, cx);
                }
                Err(error) => self.state.fail(error.to_string()),
            }
            if self.auto_focus {
                self.focus.focus(window, cx);
            }
            self.refresh(cx);
            return;
        }
        if self.sftp_directory.is_some() {
            self.start_sftp(cx);
            if self.auto_focus {
                self.focus.focus(window, cx);
            }
            cx.notify();
            return;
        }
        if let Some(terminal) = self.attached.as_ref().and_then(WeakEntity::upgrade) {
            let result = self
                .target
                .ssh()
                .ok_or_else(|| anyhow::anyhow!("SSH options unavailable"))
                .and_then(|options| {
                    terminal.update(cx, |terminal, cx| terminal.reconnect_in_place(options, cx))
                });
            if let Err(error) = result {
                terminal.update(cx, |terminal, cx| {
                    terminal.reconnect_failed(error.to_string(), cx)
                });
            }
            self.refresh(cx);
            return;
        }
        let result = (|| {
            let options = self
                .target
                .ssh()
                .ok_or_else(|| anyhow::anyhow!("SSH options unavailable"))?;
            if let Some(parent) = options.known_hosts.parent() {
                std::fs::create_dir_all(parent)?;
            }
            match &mut self.session {
                Some(session) => session.reconnect_remote(options),
                None => {
                    let session = self.target.open(
                        &self.core,
                        self.terminal_palette,
                        self.recording.clone(),
                    )?;
                    self.watch_session(session, cx);
                    Ok(())
                }
            }
        })();
        if let Err(error) = result {
            self.state.fail(format!("{error:#}"));
        }
        if self.auto_focus {
            self.focus.focus(window, cx);
        }
        self.refresh(cx);
    }
    fn watch_session(&mut self, session: Session, cx: &mut Context<Self>) {
        let notifications = session.notifications();
        self.session = Some(session);
        self._output = Some(cx.spawn(async move |this, cx| {
            while notifications.recv().await.is_ok() {
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn start_sftp(&mut self, cx: &mut Context<Self>) {
        self.sftp_cancel = None;
        self._output = None;
        self.sftp_listing = None;
        self.sftp_epoch += 1;
        let Some(options) = self.target.ssh_mut() else {
            return;
        };
        if let Err(error) = options.prepare_temporary_sftp_trust() {
            self.state.fail(error.to_string());
            return;
        }
        let epoch = self.sftp_epoch;
        let result = crate::sftp_client::Request::new(self.core.clone());
        let (request, cancel) = match result {
            Ok(request) => request,
            Err(error) => {
                self.state.fail(error.to_string());
                return;
            }
        };
        self.sftp_cancel = Some(cancel);
        let options = options.clone();
        let directory = self.sftp_directory.clone().unwrap();
        self._output = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { request.list(&options, &directory) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.sftp_epoch != epoch {
                    return;
                }
                this.sftp_cancel = None;
                if let Some(options) = this.target.ssh_mut()
                    && options.temporary_trust.is_some()
                    && options.known_hosts.is_file()
                {
                    options.trust = HostTrust::Strict;
                }
                match result {
                    Ok(listing) => {
                        for event in &listing.events {
                            this.state.apply(event.clone());
                        }
                        if let Some(error) = &listing.error {
                            this.state.fail(error.clone());
                        } else {
                            this.state.apply(crate::native::SessionEvent {
                                kind: "connected".into(),
                                message: "SFTP connected".into(),
                                ..Default::default()
                            });
                            this.sftp_listing = Some(listing);
                        }
                    }
                    Err(error) => this.state.fail(error.to_string()),
                }
                this.needs_auth_focus = this.state.phase == Phase::Authentication;
                this.refresh(cx);
            });
        }));
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let previous_phase = self.state.phase;
        if let Some(terminal) = self.attached.as_ref().and_then(WeakEntity::upgrade) {
            self.state = terminal.read(cx).reconnect_snapshot();
        }
        let result = self
            .session
            .as_mut()
            .map(|session| session.refresh().and_then(|_| session.drain_events()));
        match result {
            Some(Ok(events)) => {
                for event in events {
                    self.state.apply(event);
                }
            }
            Some(Err(error)) => self.state.fail(error.to_string()),
            None => {}
        }
        if self.state.phase == Phase::Authentication && previous_phase != Phase::Authentication {
            self.needs_auth_focus = true;
        }
        if self.state.phase == Phase::Completing && self._reveal.is_none() {
            // Match the reference's 300ms completion animation. PTY/event
            // delivery itself is callback-driven and has no polling timer.
            self._reveal = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(progress::TRAVEL).await;
                let _ = this.update(cx, |this, cx| {
                    if this.state.phase == Phase::Completing {
                        cx.emit(ConnectionEvent::Ready);
                    }
                });
            }));
        }
        cx.notify();
    }

    fn requires_fido2_pin(&self) -> bool {
        self.state.requires_fido2_pin()
            || (self.state.latest_fido2_failure()
                && self
                    .target
                    .ssh()
                    .is_some_and(|options| !options.fido2_pin.is_empty()))
    }
    fn copy_logs(&self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.state.diagnostics()));
    }

    fn progress(&mut self, window: &mut Window, cx: &App) -> AnyElement {
        let protocol = if self.sftp_directory.is_some() {
            IconName::Cloud
        } else {
            match self.target {
                Target::Serial(_) => IconName::Usb,
                Target::Telnet(_) => IconName::Network,
                Target::Mosh(..) => IconName::Zap,
                _ => IconName::Power,
            }
        };
        let destination = if self.sftp_directory.is_some() {
            IconName::FolderOpen
        } else {
            IconName::SquareTerminal
        };
        self.pipeline.render(
            &self.state,
            (protocol, destination),
            matches!(self.target, Target::Choice(_)),
            self.dark,
            window,
            cx,
        )
    }

    fn log_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.dark);
        let compact = self.compact_controls();
        div()
            .h(px(280.))
            .min_h_0()
            .max_h(px(280.))
            .overflow_hidden()
            .bg(p
                .background
                .blend(p.surface.alpha(if self.dark { 0.86 } else { 0.92 })))
            .rounded(px(8.))
            .px(px(if compact { 12. } else { 18. }))
            .py(px(if compact { 12. } else { 16. }))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .child(crate::localization::text(
                                cx,
                                "workspace.label.connectionDiagnostics",
                            )),
                    )
                    .child(
                        Button::new("copy-inner-logs")
                            .with_size(if compact {
                                gpui_kit::component::Size::XSmall
                            } else {
                                gpui_kit::component::Size::Small
                            })
                            .size(px(if compact { 24. } else { 34. }))
                            .ghost()
                            .icon(IconName::Copy)
                            .icon_size(px(if compact { 16. } else { 18. }))
                            .text_color(p.accent)
                            .tooltip("Copy diagnostics")
                            .on_click(cx.listener(|this, _, _, cx| this.copy_logs(cx))),
                    ),
            )
            .child(div().flex_1().min_h_0().child(self.logs.clone()))
            .into_any_element()
    }

    fn authentication(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(options) = self.target.ssh() else {
            return div().into_any_element();
        };
        let p = Palette::new(self.dark);
        if self.requires_fido2_pin() {
            return div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(
                    div()
                        .text_size(px(18.))
                        .font_weight(FontWeights::BOLD)
                        .child(crate::localization::text(cx, "rs.ui.copy.unlockFidoKey")),
                )
                .child(div().text_color(p.muted).child(crate::localization::text(
                    cx,
                    "rs.ui.copy.enterDevicePinSsh",
                )))
                .child(auth_input(&self.fido2_pin, "PIN", true).disabled(self.auth.busy))
                .when_some(self.error.clone(), |view, error| {
                    view.child(
                        div()
                            .text_color(rgb(0xd24135))
                            .child(crate::localization::message(cx, &error)),
                    )
                })
                .into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                crate::components::Segmented::new(
                    "authentication-method",
                    self.mode,
                    [
                        (AuthMode::Password, "Password".into()),
                        (AuthMode::PublicKey, "Public Key".into()),
                    ],
                )
                .sliding()
                .height(if self.compact_controls() { 28. } else { 36. })
                .icon(AuthMode::Password, IconName::Lock)
                .icon(AuthMode::PublicKey, IconName::KeyRound)
                .disabled(self.auth.busy)
                .on_change({
                    let owner = cx.weak_entity();
                    move |mode, _, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if this.mode != *mode {
                                this.mode = *mode;
                                this.error = None;
                                this.needs_auth_focus = true;
                                cx.notify();
                            }
                        });
                    }
                }),
            )
            .when(options.username.is_empty(), |view| {
                view.child(auth_input(&self.username, "Username", false).disabled(self.auth.busy))
            })
            .when(self.mode == AuthMode::Password, |view| {
                view.child(auth_input(&self.password, "Password", true).disabled(self.auth.busy))
            })
            .when(self.mode == AuthMode::PublicKey, |view| {
                view.child(self.authentication_keys(cx))
                    .when(self.needs_key_passphrase(), |view| {
                        view.child(
                            auth_input(&self.passphrase, "Key passphrase", true)
                                .disabled(self.auth.busy),
                        )
                    })
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .text_color(rgb(0xe05545))
                        .child(crate::localization::message(cx, &error)),
                )
                .child(
                    self.action("reload-authentication", "Reload options", false)
                        .disabled(self.auth.busy || self.auth.loading)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.auth.invalidate();
                            this.error = None;
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }
}

impl Render for Connection {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.dark);
        if let Some((editor, _)) = &self.auth.related {
            return div()
                .size_full()
                .bg(p.background)
                .track_focus(&self.focus)
                .flex()
                .justify_center()
                .child(editor.clone())
                .into_any_element();
        }
        self.ensure_authentication(cx);
        let height = if self.viewport.height > px(0.) {
            self.viewport.height
        } else {
            window.viewport_size().height - px(44.)
        };
        let metrics = ConnectionMetrics::new(f32::from(height), self.in_drawer)
            .with_footer_height(f32::from(self.footer_height));
        if self.auto_focus
            && self.needs_auth_focus
            && !self.show_logs
            && self.state.phase == Phase::Authentication
        {
            self.needs_auth_focus = false;
            let input = if self.requires_fido2_pin() {
                &self.fido2_pin
            } else if self
                .target
                .ssh()
                .is_some_and(|options| options.username.is_empty())
            {
                &self.username
            } else if self.mode == AuthMode::Password {
                &self.password
            } else if self.needs_key_passphrase() {
                &self.passphrase
            } else {
                self.focus.focus(window, cx);
                &self.username
            };
            if self.mode != AuthMode::PublicKey
                || self.requires_fido2_pin()
                || self.needs_key_passphrase()
                || self
                    .target
                    .ssh()
                    .is_some_and(|options| options.username.is_empty())
            {
                input.update(cx, |input, cx| input.focus(window, cx));
            }
        }
        let choosing = matches!(self.target, Target::Choice(_));
        let page = PageState::new(self.state.phase, choosing, self.show_logs);
        self.logs.update(cx, |logs, cx| {
            logs.configure(
                &self.state.logs,
                self.dark,
                page.shows_diagnostics(),
                window,
                cx,
            );
        });
        let body = match page.body {
            Body::Protocol => self.protocol_body(window, cx),
            Body::Logs => self.log_panel(cx),
            Body::Failure => div().flex_1().min_h_0().flex().flex_col().gap(px(16.))
                .child(div().text_color(rgb(0xef4444)).font_weight(FontWeight::BOLD).child(crate::localization::text(cx, "workspace.label.connectionFailedWithConnectionLog")))
                .child(self.log_panel(cx)).into_any_element(),
            Body::Mode => match self.state.phase {
                Phase::HostKey => div().flex().flex_col().text_size(px(14.)).line_height(px(21.))
                    .child(div().font_weight(FontWeight::BOLD).text_size(px(18.)).mb(px(28.)).child(crate::localization::text(cx, "common.label.checkingHostAuthenticity")))
                    .child(crate::localization::message(cx, &format!("The authenticity of {} is not in Nauterm known_hosts yet. Trust it for this connection only, or save the fingerprint and reconnect.",self.target.endpoint())))
                    .when_some(self.state.fingerprint.clone(), |view, fingerprint| {
                        view.child(div().mt(px(18.)).child(crate::localization::text(cx, "workspace.label.serverFingerprint")))
                            .child(div().mt(px(18.)).child(fingerprint))
                    }).into_any_element(),
                Phase::Authentication => self.authentication(cx),
                Phase::Exited => {
                    use gpui_kit::component::Icon;
                    let message = if self.state.message.is_empty() {
                        crate::localization::message(cx, match self.target {
                            Target::Serial(_) => "Serial session closed.",
                            Target::Telnet(_) => "Telnet session exited.",
                            _ => "SSH session exited.",
                        })
                    } else {
                        crate::localization::message(cx, &self.state.message)
                    };
                    div().flex().items_start().gap(px(10.))
                        .child(Icon::new(IconName::LogOut).with_size(px(18.)).text_color(p.muted))
                        .child(div().flex_1().min_w_0().text_size(px(14.)).line_height(relative(1.4))
                            .font_weight(FontWeights::MEDIUM).child(message)).into_any_element()
                }
                _ => div().h(px(1.)).into_any_element(),
            },
        };
        div()
            .size_full()
            .relative()
            .on_prepaint({
                let view = cx.entity().downgrade();
                move |bounds, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        if this.viewport != bounds.size {
                            this.viewport = bounds.size;
                            cx.notify();
                        }
                    });
                }
            })
            .bg(p.background)
            .text_color(p.text)
            .track_focus(&self.focus)
            .flex()
            .items_center()
            .justify_center()
            .px(px(metrics.horizontal))
            .py(px(metrics.outer))
            .child(
                div()
                    .w_full()
                    .max_w(px(510.))
                    .max_h_full()
                    .px(px(metrics.inner))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .child(crate::avatar::host(&self.title).platform(self.platform.clone()))
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
                                            .font_weight(FontWeights::SEMIBOLD)
                                            .text_size(px(13.))
                                            .child(self.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_weight(FontWeights::MEDIUM)
                                            .text_color(p.muted)
                                            .truncate()
                                            .child(if choosing {
                                                self.target.endpoint()
                                            } else {
                                                format!(
                                                    "{} {}",
                                                    if self.sftp_directory.is_some() {
                                                        "SFTP"
                                                    } else if matches!(
                                                        self.target,
                                                        Target::Mosh(..)
                                                    ) {
                                                        "Mosh"
                                                    } else if matches!(
                                                        self.target,
                                                        Target::Serial(_)
                                                    ) {
                                                        "Serial"
                                                    } else if matches!(
                                                        self.target,
                                                        Target::Telnet(_)
                                                    ) {
                                                        "Telnet"
                                                    } else {
                                                        "SSH"
                                                    },
                                                    self.target.endpoint()
                                                )
                                            }),
                                    ),
                            )
                            .when(page.header != HeaderAction::None, |view| {
                                view.child(
                                    self.action(
                                        "connection-header-logs",
                                        page.header_label(self.show_logs),
                                        false,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            match PageState::new(
                                                this.state.phase,
                                                matches!(this.target, Target::Choice(_)),
                                                this.show_logs,
                                            )
                                            .header
                                            {
                                                HeaderAction::CopyLogs => this.copy_logs(cx),
                                                HeaderAction::ToggleLogs => {
                                                    this.show_logs = !this.show_logs;
                                                    if !this.show_logs {
                                                        this.needs_auth_focus = this.state.phase
                                                            == Phase::Authentication;
                                                        this.focus.focus(window, cx);
                                                    }
                                                    cx.notify();
                                                }
                                                HeaderAction::None => {}
                                            }
                                        },
                                    )),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .mt(px(metrics.progress_top))
                            .mb(px(metrics.progress_bottom))
                            .child(self.progress(window, cx)),
                    )
                    .child(
                        div()
                            .id("connection-intervention-scroll")
                            .min_h_0()
                            .max_h(px(metrics.body_max))
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .when(page.scroll_body, |view| view.overflow_y_scroll())
                            .child(body)
                            .scrollbars(),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .mt(px(metrics.footer_gap))
                            .child(self.footer(cx)),
                    ),
            )
            .into_any_element()
    }
}

pub(crate) fn read_key_file(path: PathBuf) -> anyhow::Result<Zeroizing<String>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    anyhow::ensure!(
        metadata.is_file() && metadata.len() <= 4 * 1024 * 1024,
        "Choose a key/certificate file smaller than 4 MB"
    );
    let mut text = Zeroizing::new(String::new());
    file.take(4 * 1024 * 1024 + 1).read_to_string(&mut text)?;
    anyhow::ensure!(text.len() <= 4 * 1024 * 1024, "Key file exceeds 4 MB");
    anyhow::ensure!(
        !text.as_bytes().contains(&0),
        "The key file contains binary data"
    );
    Ok(text)
}
