use crate::components::Button;
use crate::connection_state::{ConnectionState, LogEntry, Phase};
use crate::native::{Frame, Session};
use crate::terminal_appearance::{Appearance, Palette as TerminalPalette, cell_colors};
use crate::terminal_search::Selection;
use gpui_kit::base::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{assets::IconName, component::Sizable};
use gpui_kit::{prelude::*, *};
use std::{
    ops::Range,
    sync::Arc,
    time::{Duration, Instant},
};

mod background;
pub(crate) mod bell;
pub(crate) mod blocks;
mod completion;
mod context_menu;
mod decorations;
mod focus;
mod graphics;
pub use context_menu::Command as TerminalCommand;
mod composer;
mod history;
mod keyboard;
mod links;
mod local_paths;
mod network;
mod paths;
mod pointer;
mod powerline;
mod prediction;
pub(crate) mod prompt_click;
pub use paths::CompletionError as TerminalCompletionError;
pub(crate) mod reconnect;
mod renderer;
mod scene;
mod scrollbar;
mod search;
mod text;

gpui_kit::actions!(terminal, [Find]);

pub struct Terminal {
    session: Session,
    connection: ConnectionState,
    network: network::Status,
    reconnect: reconnect::Reconnect,
    bell: bell::State,
    prediction: prediction::State,
    frame: Arc<Frame>,
    scene: scene::Cache,
    graphics: graphics::Cache,
    refresh_scheduled: bool,
    refresh_fallback: Option<Task<()>>,
    focus: FocusHandle,
    focus_reporting: focus::Reporting,
    bounds: Bounds<Pixels>,
    viewport_bounds: Bounds<Pixels>,
    cell_width: Pixels,
    cell_height: Pixels,
    appearance: Appearance,
    applied_palette: Option<TerminalPalette>,
    blink_visible: bool,
    blink_reset: Instant,
    _blink: Task<()>,
    _focus_events: Subscription,
    _blur_events: Subscription,
    _window_activation: Subscription,
    _keyboard: Subscription,
    marked: String,
    selection: Option<Selection>,
    command_block: Option<blocks::Block>,
    block_revision: u64,
    block_refresh: Option<Task<()>>,
    selecting: Option<pointer::Drag>,
    mouse: pointer::Reporting,
    prompt_click: Option<prompt_click::Gesture>,
    hovered_link: Option<links::Target>,
    pressed_link: Option<links::Target>,
    link_cache: Option<links::CachedTarget>,
    selection_drag: Option<Task<()>>,
    wheel_remainder: f32,
    scroll_drag: Option<Pixels>,
    error: Option<String>,
    _output: Task<()>,
    composer: Entity<TextareaState>,
    secret_composer: Entity<InputState>,
    completion: Entity<completion::Suggestions>,
    history: history::History,
    paths: paths::Paths,
    _path_events: Vec<Subscription>,
    _composer_events: Subscription,
    _secret_composer_events: Subscription,
    composer_visible: bool,
    composer_ui: composer::State,
    working_directory: crate::working_directory::WorkingDirectory,
    search: search::Search,
}

pub struct TerminalMetadataChanged;
impl EventEmitter<TerminalMetadataChanged> for Terminal {}
pub struct TerminalConnected;
impl EventEmitter<TerminalConnected> for Terminal {}
pub struct TerminalDirectoryChanged(pub Option<String>);
impl EventEmitter<TerminalDirectoryChanged> for Terminal {}

impl Terminal {
    pub fn ai_context(&self, preferences: &crate::ai::Preferences) -> String {
        if !self.ai_context_available() {
            return String::new();
        }
        let selection = if preferences.include_selection {
            self.effective_selection()
                .and_then(|range| self.session.selected_text(range.start, range.end))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let recent = if preferences.include_recent_output {
            let start = (i64::from(self.frame.history_lines)
                - i64::from(self.frame.display_offset))
                * i64::from(self.frame.columns);
            self.session
                .selected_text(
                    start,
                    start + i64::from(self.frame.rows) * i64::from(self.frame.columns),
                )
                .unwrap_or_default()
        } else {
            String::new()
        };
        let directory = self
            .command_block
            .as_ref()
            .filter(|_| preferences.include_selection && self.selection.is_none())
            .and_then(|block| block.directory.clone())
            .or_else(|| self.working_directory());
        crate::ai::context::build(
            self.title(),
            &selection,
            &recent,
            preferences,
            self.command_block
                .as_ref()
                .filter(|_| self.selection.is_none())
                .map(|block| block.prompt.as_str()),
            directory.as_deref(),
        )
    }
    pub fn ai_context_available(&self) -> bool {
        self.phase() == Phase::Completing
            && self.frame.input_echo_enabled
            && !self.session.is_replay()
    }
    /// Context chip metadata; capture text only when the user sends a message.
    pub fn ai_context_kinds(&self) -> (Option<&'static str>, bool) {
        if !self.ai_context_available() {
            return (None, false);
        }
        let selection = self.effective_selection().map(|_| {
            if self.selection.is_none() && self.command_block.is_some() {
                "Command block"
            } else {
                "Selection"
            }
        });
        (selection, true)
    }
    pub(crate) fn ai_owner(&self) -> Option<crate::native::CommandOwner> {
        (self.ai_context_available() && !self.frame.alternate_screen)
            .then(|| self.session.command_owner())
    }
    pub(crate) fn prepare_ai_command(
        &mut self,
        owner: &crate::native::CommandOwner,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<crate::native::CommandTarget> {
        anyhow::ensure!(
            self.ai_owner().is_some(),
            "The terminal is not available for a shell command."
        );
        let target = self.session.capture_command(owner)?;
        self.suspend_prediction(cx);
        self.clear_selection();
        self.session.bottom();
        self.refresh(cx);
        Ok(target)
    }
    pub fn local_shell(&self) -> Option<String> {
        self.session.local_shell()
    }
    pub fn serial_config(&self) -> Option<&crate::serial::Config> {
        self.session.serial_config()
    }
    pub fn telnet_options(&self) -> Option<&crate::telnet::Options> {
        self.session.telnet_options()
    }
    pub fn new(
        mut session: Session,
        logs: Vec<LogEntry>,
        appearance: Appearance,
        ssh: Option<crate::native::SshOptions>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        let reconnect = reconnect::Reconnect::new(ssh.clone().filter(|_| !session.is_replay()));
        let paths = paths::Paths::new(session.path_source(ssh));
        let palette = appearance.palette();
        let palette_error = session
            .apply_palette(palette)
            .err()
            .map(|error| error.to_string());
        let applied_palette = palette_error.is_none().then_some(palette);
        let settings_error = session
            .apply_runtime_options(appearance.runtime_options())
            .err()
            .map(|error| error.to_string());
        let (frame, error) = match session.refresh() {
            Ok(frame) => (frame, palette_error.or(settings_error)),
            Err(error) => (Frame::default(), Some(error.to_string())),
        };
        session.recording_frame(&frame);
        session.recording_connection_history(logs.iter().map(|log| &log.event));
        let frame = Arc::new(frame);
        let mut working_directory = crate::working_directory::WorkingDirectory::default();
        working_directory.update(&frame);
        let notifications = session.notifications();
        let output = cx.spawn(async move |this, cx| {
            while notifications.recv().await.is_ok() {
                if this
                    .update(cx, |this, cx| this.schedule_refresh(cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let focus = cx.focus_handle();
        // GPUI resolves bindings (including Root's Tab traversal) before raw
        // key-down listeners. The focused terminal owns its protocol keys;
        // app shortcuts and text/IME input continue through normal dispatch.
        let keyboard_owner = cx.weak_entity();
        let keyboard = cx.intercept_keystrokes(move |event, window, cx| {
            if !crate::shortcuts::accepts_event(event) {
                return;
            }
            let Some(owner) = keyboard_owner.upgrade() else {
                return;
            };
            if !owner.read(cx).focus.is_focused(window) {
                return;
            }
            if owner.update(cx, |this, cx| this.key_down(&event.keystroke, window, cx)) {
                window.prevent_default();
                cx.stop_propagation();
            }
        });
        let focus_events = cx.on_focus(&focus, window, |this, window, cx| {
            this.blink_reset = Instant::now();
            this.blink_visible = true;
            this.report_focus(window);
            cx.notify();
        });
        let blur_events = cx.on_blur(&focus, window, |this, window, cx| {
            this.cancel_mouse(cx);
            this.hovered_link = None;
            this.pressed_link = None;
            this.selecting = None;
            this.selection_drag = None;
            this.report_focus(window);
            cx.notify();
        });
        let window_activation = cx.observe_window_activation(window, |this, window, cx| {
            this.report_focus(window);
            if this.owns_keyboard(window) {
                this.blink_reset = Instant::now();
                this.blink_visible = true;
                cx.notify();
            }
            if !window.is_window_active() {
                this.cancel_mouse(cx);
                this.hovered_link = None;
                this.pressed_link = None;
                this.selecting = None;
                this.selection_drag = None;
                this.scroll_drag = None;
                this.wheel_remainder = 0.;
                cx.notify();
            }
        });
        let blink = cx.spawn_in(window, async move |this, cx| {
            loop {
                let delay = this.update_in(cx, |this, window, cx| {
                    let elapsed = this.blink_reset.elapsed().as_millis() as u64;
                    let active = this.appearance.cursor_blink
                        && this.frame.cursor_blinking
                        && this.frame.cursor_visible
                        && this.frame.display_offset == 0
                        && this.focus.is_focused(window)
                        && window.is_window_active();
                    let visible = !active || (elapsed / 530).is_multiple_of(2);
                    if this.blink_visible != visible {
                        this.blink_visible = visible;
                        cx.notify();
                    }
                    Duration::from_millis(530 - elapsed % 530)
                });
                let Ok(delay) = delay else {
                    break;
                };
                cx.background_executor().timer(delay).await;
            }
        });
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Command")
                .auto_grow(1, 3)
                .submit_on_enter(true)
        });
        let composer_events = cx.subscribe_in(
            &composer,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { shift: false, .. })
                    && !this.composer_sensitive()
                {
                    this.submit_composer(window, cx);
                } else if matches!(event, InputEvent::Focus) {
                    this.refresh_history(cx);
                }
                if matches!(
                    event,
                    InputEvent::Change | InputEvent::Focus | InputEvent::Blur
                ) {
                    cx.notify();
                }
            },
        );
        let secret_composer = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Password")
                .masked(true)
        });
        let secret_composer_events = cx.subscribe_in(
            &secret_composer,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) && this.composer_sensitive() {
                    this.submit_composer(window, cx);
                }
                cx.notify();
            },
        );
        let completion = cx.new(|cx| completion::Suggestions::new(composer.clone(), window, cx));
        let path_events = vec![
            cx.subscribe_in(
                &completion,
                window,
                |this, _, _: &completion::Changed, window, cx| this.refresh_paths(window, cx),
            ),
            cx.subscribe_in(
                &completion,
                window,
                |this, _, _: &completion::Dismissed, _, cx| this.dismiss_paths(cx),
            ),
        ];
        let search = search::Search::new(window, cx);
        let replay = session.is_replay();
        let composer_visible = !replay && appearance.composer_enabled;
        let network = network::Status::new(session.is_mosh() && !replay, &logs);
        let bell = bell::State::new(frame.bell_count);
        let prediction = prediction::State::new(&logs);
        let mut terminal = Self {
            session,
            connection: ConnectionState {
                phase: Phase::Completing,
                logs,
                ..Default::default()
            },
            network,
            reconnect,
            bell,
            prediction,
            frame,
            scene: Default::default(),
            graphics: Default::default(),
            refresh_scheduled: false,
            refresh_fallback: None,
            focus,
            focus_reporting: focus::Reporting::default(),
            bounds: Bounds::default(),
            viewport_bounds: Bounds::default(),
            cell_width: px(8.),
            cell_height: px(appearance.cell_height()),
            appearance,
            applied_palette,
            blink_visible: true,
            blink_reset: Instant::now(),
            _blink: blink,
            _focus_events: focus_events,
            _blur_events: blur_events,
            _window_activation: window_activation,
            _keyboard: keyboard,
            marked: String::new(),
            selection: None,
            command_block: None,
            block_revision: 0,
            block_refresh: None,
            selecting: None,
            mouse: pointer::Reporting::default(),
            prompt_click: None,
            hovered_link: None,
            pressed_link: None,
            link_cache: None,
            selection_drag: None,
            wheel_remainder: 0.,
            scroll_drag: None,
            error,
            _output: output,
            composer,
            secret_composer,
            completion,
            history: history::History::default(),
            paths,
            _path_events: path_events,
            _composer_events: composer_events,
            _secret_composer_events: secret_composer_events,
            composer_visible,
            composer_ui: composer::State::default(),
            working_directory,
            search,
        };
        cx.on_release(|this, cx| this.graphics.release(cx)).detach();
        terminal.sync_graphics(cx);
        terminal.refresh_history(cx);
        terminal.sync_network_expiry(cx);
        terminal
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if self.reconnect_shows_login()
            && let Some(login) = &self.reconnect.login
        {
            login.update(cx, |login, cx| login.focus(window, cx));
            return;
        }
        self.focus.focus(window, cx);
    }

    pub fn title(&self) -> &str {
        &self.frame.title
    }
    pub(crate) fn system_info_request(
        &self,
        options: &crate::native::SshOptions,
    ) -> anyhow::Result<crate::native::SystemInfoRequest> {
        anyhow::ensure!(
            self.phase() == Phase::Completing,
            "The SSH session is not connected"
        );
        self.session.system_info_request(options)
    }
    pub(crate) fn host_os_request(
        &self,
        options: &crate::native::SshOptions,
    ) -> anyhow::Result<crate::native::HostOsRequest> {
        anyhow::ensure!(
            self.phase() == Phase::Completing,
            "The SSH session is not connected"
        );
        self.session.host_os_request(options)
    }
    pub(crate) fn ssh_latency_ms(&self) -> Option<f64> {
        self.connection.latency_ms.map(f64::from)
    }
    pub fn set_appearance(
        &mut self,
        appearance: &Appearance,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.session
            .apply_runtime_options(appearance.runtime_options())?;
        let palette = appearance.palette();
        let palette_changed = self.applied_palette != Some(palette);
        if palette_changed {
            self.session.apply_palette(palette)?;
            self.applied_palette = Some(palette);
        }
        if self.appearance != *appearance {
            if !self.session.is_mosh()
                && self.appearance.ssh_prediction != appearance.ssh_prediction
            {
                self.reset_prediction(cx);
            }
            if !appearance.bell.visual {
                self.clear_visual_bell();
            }
            let scrollback_changed =
                self.appearance.scrollback_lines != appearance.scrollback_lines;
            let padding_changed = self.appearance.padding != appearance.padding;
            if !appearance.report_mouse_events {
                self.cancel_mouse(cx);
            }
            if !appearance.command_click_opens_filename_or_url {
                self.hovered_link = None;
                self.pressed_link = None;
            }
            if !appearance.option_click_moves_cursor {
                self.prompt_click = None;
            }
            if !appearance.scrollbar {
                self.scroll_drag = None;
            }
            self.appearance = appearance.clone();
            self.clear_selection();
            self.blink_visible = true;
            self.blink_reset = Instant::now();
            self.refresh(cx);
            if (scrollback_changed || padding_changed) && self.search.visible {
                self.run_search(crate::terminal_search::Direction::Next, false, cx);
            }
            cx.notify();
        } else if palette_changed {
            // A user theme may change while its stable ID stays the same.
            // Apply its new native colors without discarding the selection.
            self.refresh(cx);
            cx.notify();
        }
        Ok(())
    }

    pub fn phase(&self) -> Phase {
        self.connection.phase.with_pty_exit(self.frame.exited)
    }
    pub fn working_directory(&self) -> Option<String> {
        self.working_directory.value()
    }
    pub fn set_composer_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        let visible = visible && !self.session.is_replay();
        if self.composer_visible != visible {
            self.composer_visible = visible;
            self.sync_completion(cx);
            self.refresh_history(cx);
            cx.notify();
        }
    }
    pub fn set_composer_sources(&mut self, sources: Vec<String>, cx: &mut Context<Self>) {
        self.completion
            .update(cx, |completion, cx| completion.set_sources(sources, cx));
    }
    fn composer_sensitive(&self) -> bool {
        let frame = &self.frame;
        let columns = frame.columns as usize;
        let row = frame.cursor.1.min(frame.rows.saturating_sub(1)) as usize;
        let line = |row: usize| {
            frame
                .cells
                .get(row * columns..(row + 1) * columns)
                .unwrap_or_default()
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<String>()
        };
        let current = line(row);
        let previous = (row > 0).then(|| line(row - 1));
        crate::composer_completion::sensitive(
            frame.input_echo_enabled,
            &current,
            previous.as_deref(),
        )
    }
    fn sync_completion(&mut self, cx: &mut Context<Self>) {
        let sensitive = self.composer_sensitive();
        let palette = self.appearance.palette();
        self.completion.update(cx, |completion, cx| {
            completion.policy(
                self.appearance.composer_autocomplete_enabled(),
                sensitive,
                self.composer_visible,
                self.appearance.composer_max_suggestions,
                (
                    palette.base.background,
                    palette.base.foreground,
                    palette.accent,
                ),
                cx,
            )
        });
    }

    pub fn apply_snippet(
        &mut self,
        text: &str,
        run: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.apply_tool_command(text, run, false, window, cx)
    }

    fn apply_tool_command(
        &mut self,
        text: &str,
        run: bool,
        replace_prompt: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.session.is_replay() && self.phase() == Phase::Completing,
            "The target terminal is not connected."
        );
        let sensitive = self.composer_sensitive();
        let input = if run {
            if replace_prompt {
                crate::snippets::replace_prompt_execution(text)?
            } else {
                crate::snippets::execution(text)?
            }
        } else {
            // Ask the live native encoder for multiline paste, rather than
            // relying on a possibly stale frame's keyboard flags.
            let (text, bracketed) = crate::snippets::prompt(text, 1 << 2)?;
            if bracketed {
                crate::snippets::encoded_prompt(&text, self.session.encode_paste(&text)?)?
            } else {
                text
            }
        };
        let result = self.session.send(input.as_bytes());
        self.prediction_input(input.as_bytes(), result.is_ok(), sensitive, cx);
        result?;
        self.note_reconnect_input(if run { &input } else { text });
        self.clear_selection();
        self.session.bottom();
        self.error = None;
        self.focus.focus(window, cx);
        self.refresh(cx);
        Ok(())
    }

    pub fn open_ssh_editor(
        &mut self,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.composer_sensitive(),
            "The terminal is waiting for sensitive input. Return to the shell before opening an editor."
        );
        self.apply_snippet(command, true, window, cx)
    }

    fn submit_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sensitive = self.composer_sensitive();
        let command = if sensitive {
            self.secret_composer.read(cx).value().to_string()
        } else {
            self.composer.read(cx).value().to_string()
        };
        if !crate::composer_completion::should_submit(&command, sensitive) {
            return;
        }
        self.suspend_prediction(cx);
        let result = if command.is_empty() {
            self.session.send(b"\r")
        } else if sensitive {
            self.session
                .send(command.as_bytes())
                .and_then(|()| self.session.send(b"\r"))
        } else {
            self.session
                .paste(&command)
                .and_then(|()| self.session.send(b"\r"))
        };
        match result {
            Ok(()) => {
                if !sensitive {
                    self.note_reconnect_input(&command);
                    self.note_reconnect_input("\r");
                }
                self.record_composer_command(&command, cx);
                if sensitive {
                    self.secret_composer.update(cx, |input, cx| {
                        input.set_value("", window, cx);
                        input.focus(window, cx);
                    });
                } else {
                    self.composer.update(cx, |input, cx| {
                        input.set_value("", window, cx);
                        input.focus(window, cx);
                    });
                }
                self.completion
                    .update(cx, |completion, cx| completion.sync_input(cx));
                self.clear_selection();
                self.session.bottom();
                self.error = None;
                self.refresh(cx);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    fn schedule_refresh(&mut self, cx: &mut Context<Self>) {
        if self.refresh_scheduled {
            return;
        }
        self.refresh_scheduled = true;
        cx.notify();
        // Minimized/occluded windows may stop producing display frames. A
        // one-shot fallback still drains captures/history; this is not polling.
        self.refresh_fallback = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.refresh_scheduled {
                    this.refresh(cx);
                }
            });
        }));
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.refresh_scheduled = false;
        self.refresh_fallback = None;
        let previous_phase = self.phase();
        let previous_title = self.frame.title.clone();
        let mut screen_changed = false;
        let mut connection_changed = false;
        let mut network_changed = false;
        match self.session.refresh() {
            Ok(frame) => {
                self.session.recording_frame(&frame);
                if let Some(error) = self.session.recording_error() {
                    self.error = Some(error);
                }
                if *self.frame != frame {
                    screen_changed = true;
                    if self.frame.cursor != frame.cursor
                        || self.frame.cursor_shape != frame.cursor_shape
                    {
                        self.blink_reset = Instant::now();
                        self.blink_visible = true;
                    }
                    self.frame = Arc::new(frame);
                    self.refresh_command_block(cx);
                    self.hovered_link = None;
                    cx.notify();
                }
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
        // Poll first: Mosh screen-commit events must reconcile against the
        // snapshot produced by that poll, including commits with equal cells.
        match self.session.drain_events() {
            Ok(events) => {
                connection_changed = !events.is_empty();
                for event in events {
                    network_changed |= self.network.apply(&event.kind);
                    if self.session.is_mosh() && self.prediction.event(&event) {
                        cx.notify();
                    }
                    self.connection.apply(event);
                }
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
        if network_changed {
            self.sync_network_expiry(cx);
        }
        self.sync_graphics(cx);
        self.observe_bell(cx);
        self.reconcile_prediction(screen_changed, cx);
        self.take_shell_commands(cx);
        self.sync_reconnect(cx);
        if connection_changed || previous_phase != self.phase() {
            cx.emit(reconnect::Changed);
        }
        if previous_phase != Phase::Completing && self.phase() == Phase::Completing {
            cx.emit(TerminalConnected);
        }
        let directory_changed =
            self.phase() == Phase::Completing && self.working_directory.update(&self.frame);
        self.sync_completion(cx);
        self.refresh_history(cx);
        if directory_changed {
            self.completion
                .update(cx, |completion, cx| completion.sync_input(cx));
            cx.emit(TerminalDirectoryChanged(self.working_directory.value()));
        }
        if previous_phase != self.phase() || previous_title != self.frame.title || directory_changed
        {
            cx.emit(TerminalMetadataChanged);
        }
    }

    fn send(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if self.session.is_replay() {
            return;
        }
        self.prompt_click = None;
        self.clear_selection();
        self.session.bottom();
        let sensitive = self.composer_sensitive();
        let result = self.session.send(bytes);
        if result.is_ok()
            && let Ok(text) = std::str::from_utf8(bytes)
        {
            self.note_reconnect_input(text);
        }
        self.prediction_input(bytes, result.is_ok(), sensitive, cx);
        if let Err(error) = result {
            self.error = Some(error.to_string());
        }
        self.refresh(cx);
    }

    fn paste_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.session.is_replay() {
            return;
        }
        self.prompt_click = None;
        self.clear_selection();
        self.session.bottom();
        let sensitive = self.composer_sensitive();
        let result = self.session.encode_paste(text).and_then(|encoded| {
            let result = self.session.send(encoded.as_bytes());
            self.prediction_input(encoded.as_bytes(), result.is_ok(), sensitive, cx);
            result
        });
        if result.is_ok() {
            self.note_reconnect_input(text);
        }
        if let Err(error) = result {
            self.suspend_prediction(cx);
            self.error = Some(error.to_string());
        }
        self.refresh(cx);
    }

    fn key_down(
        &mut self,
        stroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.owns_keyboard(window) {
            return false;
        }
        let shortcuts = crate::shortcuts::current(cx);
        if let Some(command) = shortcuts.matching(crate::shortcuts::Scope::Terminal, stroke) {
            match command {
                crate::shortcuts::Command::Search => self.show_search(window, cx),
                crate::shortcuts::Command::Copy => self.copy_selection(cx),
                crate::shortcuts::Command::Paste => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.paste_text(&text, cx);
                    }
                }
                crate::shortcuts::Command::SelectAll => self.select_all(cx),
                _ => unreachable!(),
            }
            return true;
        }
        if shortcuts
            .matching(crate::shortcuts::Scope::Workspace, stroke)
            .is_some()
        {
            return false;
        }
        if !self.marked.is_empty() {
            return false;
        }
        if stroke.key == "escape"
            && self
                .effective_selection()
                .is_some_and(|selection| selection.start < selection.end)
        {
            self.clear_selection();
            cx.notify();
            return true;
        }
        if let Some(action) = keyboard::scroll_key(
            stroke,
            self.appearance.navigation_keys_scroll,
            self.frame.alternate_screen,
        ) {
            match action {
                keyboard::ScrollKey::PageUp => self.session.page_up(),
                keyboard::ScrollKey::PageDown => self.session.page_down(),
                keyboard::ScrollKey::Top => self.session.scroll(i32::MAX),
                keyboard::ScrollKey::Bottom => self.session.bottom(),
            }
            self.refresh(cx);
            return true;
        }
        if self.session.is_replay() {
            return false;
        }
        if let Some(bytes) = keyboard::encode_event(
            stroke,
            self.frame.keyboard_mode,
            self.appearance.use_option_as_meta_key,
            window,
        ) {
            self.send(&bytes, cx);
            return true;
        }
        // Printable characters are committed through EntityInputHandler so IME,
        // dead keys and keyboard layouts are handled once by the platform.
        false
    }

    fn offset(&self, position: Point<Pixels>) -> i64 {
        self.offset_for_cell(self.viewport_cell(position))
    }
    fn offset_for_cell(&self, (column, row): (u32, u32)) -> i64 {
        let history_row =
            i64::from(self.frame.history_lines) - i64::from(self.frame.display_offset);
        (history_row + i64::from(row.min(self.frame.rows.saturating_sub(1))))
            * i64::from(self.frame.columns)
            + i64::from(column.min(self.frame.columns.saturating_sub(1)))
    }

    fn extend_selection(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(mut drag) = self.selecting {
            let head = self.offset(position);
            drag.moved |= head != drag.anchor;
            self.selecting = Some(drag);
            let selection = drag.extend(head);
            if drag.moved {
                self.command_block = None;
            }
            if self.selection != selection {
                self.selection = selection;
                cx.notify();
            }
        }
    }

    fn start_selection_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // AppKit continues delivering mouseDragged outside the window. A window
        // listener (below), rather than a hovered element, owns this gesture.
        // Keep scrolling while the pointer is stationary beyond the viewport.
        self.selection_drag = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(32))
                    .await;
                let running = this.update_in(cx, |this, window, cx| {
                    if this.selecting.is_none()
                        || !window.is_window_active()
                        || !this.focus.is_focused(window)
                    {
                        this.selecting = None;
                        return false;
                    }
                    let position = window.mouse_position();
                    let distance = if position.y < this.bounds.top() {
                        this.bounds.top() - position.y
                    } else if position.y > this.bounds.bottom() {
                        this.bounds.bottom() - position.y
                    } else {
                        px(0.)
                    };
                    if distance != px(0.) {
                        let lines =
                            (f32::from(distance) / f32::from(this.cell_height)).clamp(-16., 16.);
                        let lines = if lines > 0. {
                            lines.ceil()
                        } else {
                            lines.floor()
                        } as i32;
                        this.session.scroll(lines);
                        this.refresh(cx);
                        this.extend_selection(position, cx);
                    }
                    true
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        }));
    }
}

impl Terminal {
    fn surface(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let paint_view = view.clone();
        let default_bg = self.appearance.palette().base.background;
        let surface = div()
            .id("native-terminal")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(rgb(default_bg))
            .track_focus(&self.focus)
            .key_context("Terminal")
            .cursor(CursorStyle::IBeam)
            .when(self.hovered_link.is_some(), |view| view.cursor_pointer())
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if !hovered && this.hovered_link.take().is_some() {
                    cx.notify();
                }
            }))
            .on_modifiers_changed(
                cx.listener(|this, event: &ModifiersChangedEvent, window, cx| {
                    this.update_link_hover(window.mouse_position(), event.modifiers, cx);
                }),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.update_link_hover(event.position, event.modifiers, cx);
                if this.mouse.pressed.is_none() {
                    this.report_motion(event, cx);
                }
            }))
            .on_scroll(cx.listener(Self::mouse_wheel))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let appearance = view.read(cx).appearance.clone();
                        let base_font = renderer::terminal_font(&appearance);
                        let height = px(appearance.cell_height());
                        let text: SharedString = "W".into();
                        let run = TextRun {
                            len: 1,
                            font: base_font.clone(),
                            color: rgb(0xffffff).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let width = window
                            .text_system()
                            .shape_line(text, px(appearance.size), &[run], None)
                            .width();
                        let width = px((f32::from(width) + appearance.letter_spacing)
                            .ceil()
                            .max(1.));
                        view.update(cx, |this, cx| {
                            this.viewport_bounds = bounds;
                            let bounds = renderer::padded_bounds(bounds, appearance.padding);
                            let (columns, rows) = crate::native::terminal_grid_size(
                                (bounds.size.width / width).floor().max(2.) as u32,
                                (bounds.size.height / height).floor().max(1.) as u32,
                            );
                            this.bounds = bounds;
                            this.cell_width = width;
                            this.cell_height = height;
                            if (columns, rows) != (this.frame.columns, this.frame.rows) {
                                this.session.resize(
                                    columns,
                                    rows,
                                    f32::from(width).ceil() as u32,
                                    f32::from(height) as u32,
                                );
                                this.refresh(cx);
                                // A reflow changes the meaning of grid offsets.
                                // Re-resolve an open query instead of painting
                                // its old selection over unrelated text.
                                this.clear_selection();
                                this.hovered_link = None;
                                this.pressed_link = None;
                                this.prompt_click = None;
                                if this.search.visible {
                                    this.run_search(
                                        crate::terminal_search::Direction::Next,
                                        false,
                                        cx,
                                    );
                                }
                            } else if this.refresh_scheduled {
                                // Visible panes consume the batch before paint;
                                // the one-shot fallback drains hidden panes.
                                this.refresh(cx);
                            }
                            let focused =
                                this.focus.is_focused(window) && window.is_window_active();
                            let predicted = this.prediction_display();
                            let text_cursor = (focused
                                && predicted.text.is_empty()
                                && this.frame.cursor_visible
                                && this.frame.display_offset == 0
                                && !this.frame.exited
                                && this.blink_visible
                                && this.frame.cursor_shape == 0)
                                .then(|| {
                                    (renderer::cursor_column(&this.frame), this.frame.cursor.1)
                                });
                            let scene = this.scene.prepare(
                                scene::Request {
                                    frame: &this.frame,
                                    appearance: &appearance,
                                    font: &base_font,
                                    cell: size(width, height),
                                    selection: this.selection,
                                    cursor: text_cursor,
                                },
                                window,
                            );
                            let graphics = this.graphics.prepare(&this.frame, size(width, height));
                            (
                                this.frame.clone(),
                                scene,
                                graphics,
                                this.marked.clone(),
                                predicted,
                                this.command_block.as_ref().map(|block| block.selection),
                                this.hovered_link.as_ref().map(|link| link.selection),
                                width,
                                this.focus.clone(),
                                appearance,
                                base_font,
                                height,
                                this.blink_visible,
                                bounds,
                            )
                        })
                    },
                    move |viewport_bounds,
                          (
                        frame,
                        scene,
                        graphics,
                        marked,
                        predicted,
                        command_block,
                        hovered_link,
                        width,
                        focus,
                        appearance,
                        base_font,
                        height,
                        blink,
                        bounds,
                    ),
                          window,
                          cx| {
                        let dragged = paint_view.downgrade();
                        let released = dragged.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                let _ = released.update(cx, |this, cx| this.mouse_up(event, cx));
                            }
                        });
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                let _ = dragged.update(cx, |this, cx| {
                                    if this.drag_prompt_click(event) {
                                        return;
                                    }
                                    if this.mouse.pressed.is_some() {
                                        this.report_motion(event, cx);
                                    }
                                    if this.selecting.is_some() {
                                        if event.pressed_button == Some(MouseButton::Left) {
                                            this.extend_selection(event.position, cx);
                                        } else {
                                            this.selecting = None;
                                            this.selection_drag = None;
                                        }
                                    }
                                });
                            }
                        });
                        if !paint_view.read(cx).session.is_replay() {
                            window.handle_input(
                                &focus,
                                ElementInputHandler::new(bounds, paint_view.clone()),
                                cx,
                            );
                        }
                        let palette = appearance.palette();
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            let focused = focus.is_focused(window) && window.is_window_active();
                            let show_cursor = frame.cursor_visible
                                && predicted.text.is_empty()
                                && frame.display_offset == 0
                                && !frame.exited
                                && (!focused || blink);
                            let cursor = renderer::cursor_rect(bounds, &frame, width, height)
                                .filter(|_| show_cursor);
                            let layered = !graphics.is_empty();
                            if layered {
                                use crate::terminal_graphics::Layer;
                                graphics::paint(&graphics, Layer::BelowBackground, bounds, window);
                                window.paint_layer(bounds, |window| {
                                    scene.paint_background(bounds, window)
                                });
                                graphics::paint(&graphics, Layer::BelowText, bounds, window);
                            } else {
                                scene.paint_background(bounds, window);
                            }
                            let mut paint_text = |window: &mut Window| {
                                scene.paint_selection(bounds, window);
                                if let Some(rect) = cursor
                                    && focused
                                    && frame.cursor_shape == 0
                                {
                                    window.paint_quad(fill(rect, rgb(frame.cursor_color)));
                                }
                                scene.paint_text(bounds, window, cx);
                            };
                            if layered {
                                window.paint_layer(bounds, paint_text);
                            } else {
                                paint_text(window);
                            }
                            graphics::paint(
                                &graphics,
                                crate::terminal_graphics::Layer::AboveText,
                                bounds,
                                window,
                            );
                            if !predicted.text.is_empty() {
                                window.paint_layer(bounds, |window| {
                                    prediction::Paint {
                                        text: &predicted.text,
                                        start: predicted.start,
                                        frame: &frame,
                                        appearance: &appearance,
                                        font: &base_font,
                                        cell: size(width, height),
                                        focused,
                                    }
                                    .paint(bounds, window, cx);
                                });
                            }
                            if !marked.is_empty() && frame.display_offset == 0 {
                                let position = point(
                                    bounds.left() + width * frame.cursor.0 as f32,
                                    bounds.top() + height * frame.cursor.1 as f32,
                                );
                                let text: SharedString = marked.into();
                                let run = TextRun {
                                    len: text.len(),
                                    font: base_font.clone(),
                                    color: rgb(palette.base.foreground).into(),
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                };
                                let line = window.text_system().shape_line(
                                    text,
                                    px(appearance.size),
                                    &[run],
                                    None,
                                );
                                let _ =
                                    line.paint(position, height, TextAlign::Left, None, window, cx);
                            }
                            if let Some(rect) = cursor {
                                window.paint_layer(rect, |window| {
                                    renderer::paint_cursor(
                                        rect,
                                        frame.cursor_shape,
                                        focused,
                                        frame.cursor_color,
                                        window,
                                    );
                                });
                            }
                        });
                        if let Some(selection) = command_block {
                            // Keep row geometry aligned with the text, but let
                            // the focus border and dimming span both gutters.
                            let focus_bounds = Bounds::new(
                                point(viewport_bounds.left(), bounds.top()),
                                size(viewport_bounds.size.width, bounds.size.height),
                            );
                            window.with_content_mask(
                                Some(ContentMask {
                                    bounds: viewport_bounds,
                                }),
                                |window| {
                                    blocks::paint_focus(
                                        selection,
                                        &frame,
                                        focus_bounds,
                                        height,
                                        palette.base.background,
                                        palette.accent,
                                        window,
                                    );
                                },
                            );
                        }
                        if let Some(selection) = hovered_link {
                            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                                links::paint_underline(
                                    selection,
                                    &frame,
                                    bounds,
                                    width,
                                    height,
                                    palette.accent,
                                    window,
                                );
                            });
                        }
                    },
                )
                .size_full(),
            )
            .child(self.scrollbar(cx));
        self.context_menu(surface, cx)
    }
}

impl Render for Terminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.reconnect_shows_login()
            && let Some(login) = self.reconnect.login.clone()
        {
            let allowed = self.reconnect.focus_allowed
                && (self.focus.is_focused(window) || login.read(cx).contains_focus(window, cx));
            login.update(cx, |login, cx| login.allow_auto_focus(allowed, cx));
            if self.focus.is_focused(window) {
                login.update(cx, |login, cx| login.focus(window, cx));
            }
            return div().size_full().child(login).into_any_element();
        }
        self.sync_completion(cx);
        let palette = self.appearance.palette();
        let background = palette.base.background;
        let foreground = palette.base.foreground;
        let reserved = self.composer_reserved_height();
        let reconnect = self.reconnect_overlay(reserved, cx);
        let composer = self.composer_visible.then(|| self.composer_overlay(cx));
        let network = self.network_overlay(window, cx);
        let bell = self.bell_overlay(window, cx);
        div()
            .size_full()
            .relative()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(background))
            .text_color(rgb(foreground))
            .key_context("TerminalView")
            .on_action(cx.listener(Self::find_from_shortcut))
            .on_action(cx.listener(|this, _: &crate::app_menu::Find, window, cx| {
                this.show_search(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &gpui_kit::base::input::Copy, window, cx| {
                    if this.focus.is_focused(window) {
                        this.copy_selection(cx);
                    } else {
                        cx.propagate();
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &gpui_kit::base::input::Paste, window, cx| {
                    if this.focus.is_focused(window) {
                        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                            this.paste_text(&text, cx);
                        }
                    } else {
                        cx.propagate();
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &gpui_kit::base::input::SelectAll, window, cx| {
                    if this.focus.is_focused(window) {
                        this.select_all(cx);
                    } else {
                        cx.propagate();
                    }
                }),
            )
            .on_action(cx.listener(Self::close_search_action))
            .child(
                div().size_full().pb(reserved).min_h_0().relative().child(
                    div()
                        .relative()
                        .size_full()
                        .child(self.surface(cx))
                        .children(network)
                        .children(bell)
                        .when(self.search.visible, |view| {
                            view.child(self.search_overlay(cx))
                        }),
                ),
            )
            .children(composer)
            .children(reconnect)
            .into_any_element()
    }
}

fn wheel_scroll_lines(delta: ScrollDelta, cell_height: Pixels, remainder: &mut f32) -> i32 {
    let pixels = match delta {
        ScrollDelta::Lines(delta) => {
            if delta.y == 0. || !delta.y.is_finite() {
                return 0;
            }
            // macOS can report a single discrete wheel notch as a fractional
            // line. Unlike a precise trackpad gesture, it must act immediately.
            *remainder = 0.;
            let lines = delta.y * 3.;
            return if lines.abs() < 1. {
                lines.signum() as i32
            } else {
                lines.trunc() as i32
            };
        }
        ScrollDelta::Pixels(delta) => delta.y,
    };
    let height = f32::from(cell_height);
    let lines = f32::from(pixels) / height;
    if height <= 0. || !lines.is_finite() || lines == 0. {
        return 0;
    }
    // Zero/horizontal events do not discard partial vertical motion. Reversing
    // direction starts immediately without first consuming the old residue.
    if lines.signum() != remainder.signum() {
        *remainder = 0.;
    }
    *remainder += lines;
    let whole = remainder.trunc() as i32;
    *remainder -= whole as f32;
    whole
}

// Only non-text keys are handled here. Text goes through the OS input handler.
fn encode_key(key: &Keystroke, mode: u32) -> Option<Vec<u8>> {
    keyboard::encode(key, mode, true)
}

impl EntityInputHandler for Terminal {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text: Vec<u16> = self.marked.encode_utf16().collect();
        let range = range.start.min(text.len())..range.end.min(text.len());
        *adjusted = Some(range.clone());
        Some(String::from_utf16_lossy(&text[range]))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self.marked.encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.marked.is_empty()).then(|| 0..self.marked.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked.clear();
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_keyboard(window) {
            return;
        }
        self.marked.clear();
        self.send(text.as_bytes(), cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_keyboard(window) {
            return;
        }
        self.suspend_prediction(cx);
        self.marked = text.to_owned();
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(Bounds::new(
            point(
                self.bounds.left() + self.cell_width * self.frame.cursor.0 as f32,
                self.bounds.top() + self.cell_height * self.frame.cursor.1 as f32,
            ),
            size(self.cell_width, self.cell_height),
        ))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::{encode_key, wheel_scroll_lines};
    use core::prelude::v1::test;
    use gpui_kit::{Keystroke, ScrollDelta, point, px};
    #[test]
    fn discrete_notches_scroll_immediately_and_precise_motion_accumulates() {
        let mut remainder = 0.;
        let line = |y| ScrollDelta::Lines(point(0., y));
        let pixel = |x, y| ScrollDelta::Pixels(point(px(x), px(y)));
        for (delta, expected) in [(0.1, 1), (-0.1, -1), (1., 3), (-2., -6), (0., 0)] {
            assert_eq!(
                wheel_scroll_lines(line(delta), px(20.), &mut remainder),
                expected
            );
        }
        assert_eq!(
            wheel_scroll_lines(pixel(0., 8.), px(20.), &mut remainder),
            0
        );
        assert_eq!(
            wheel_scroll_lines(pixel(10., 0.), px(20.), &mut remainder),
            0
        );
        assert_eq!(
            wheel_scroll_lines(pixel(0., 12.), px(20.), &mut remainder),
            1
        );
        assert_eq!(
            wheel_scroll_lines(pixel(0., 50.), px(20.), &mut remainder),
            2
        );
        assert_eq!(remainder, 0.5);
        assert_eq!(
            wheel_scroll_lines(pixel(0., -20.), px(20.), &mut remainder),
            -1
        );
        assert_eq!(remainder, 0.);
    }
    #[test]
    fn cursor_modes_control_keys_and_modifiers() {
        let key = |s| Keystroke::parse(s).unwrap();
        assert_eq!(encode_key(&key("up"), 0), Some(b"\x1b[A".to_vec()));
        assert_eq!(encode_key(&key("up"), 1), Some(b"\x1bOA".to_vec()));
        assert_eq!(encode_key(&key("ctrl-c"), 0), Some(vec![3]));
        assert_eq!(encode_key(&key("tab"), 0), Some(b"\t".to_vec()));
        assert_eq!(encode_key(&key("shift-tab"), 0), Some(b"\x1b[Z".to_vec()));
        assert_eq!(encode_key(&key("cmd-w"), 0), None);
        assert_eq!(
            encode_key(&key("shift-left"), 0),
            Some(b"\x1b[1;2D".to_vec())
        );
        assert_eq!(encode_key(&key("a"), 0), None);
    }
}
