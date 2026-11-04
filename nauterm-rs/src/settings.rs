//! Dedicated settings content. Shared controls emit public preferences only;
//! the owning workspace persists them and updates its live terminal entities.
use crate::{
    components::{
        RadioGroup, RgbChange, RgbPicker, RgbState, ScrollbarsExt, SelectEvent, SelectModel,
        SelectOption, Switch, TrackedText,
        settings::{Button, Input},
    },
    model::{AppState, AppTheme, RestoreBehavior},
    palette::Palette,
    terminal_appearance::{Appearance, CursorShape, ThemeId},
    typography::{FontWeights, SettingsText},
};
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::{
    assets::IconName,
    component::{Icon, Sizable},
    prelude::*,
    *,
};
use std::path::PathBuf;
mod about;
mod ai;
mod application_theme;
mod bell;
mod editors;
mod exit;
mod fonts;
mod general;
mod padding;
mod recording;
mod reset;
mod search;
mod sections;
mod session;
mod sftp;
mod shortcuts;
mod sync;
mod themes;
use search::Section;

gpui_kit::actions!(settings, [Dismiss]);

#[derive(Clone, PartialEq)]
pub struct Values {
    pub dark: bool,
    pub theme: AppTheme,
    pub host_icons: crate::avatar::Mode,
    pub shortcuts: crate::shortcuts::Config,
    pub general: crate::general_preferences::Preferences,
    pub restore: RestoreBehavior,
    pub terminal: Appearance,
    pub terminal_preferences: crate::terminal_preferences::Preferences,
    pub sftp_editors: crate::editor_config::Editors,
    pub sftp: crate::sftp_preferences::Preferences,
    pub recordings: crate::recordings::Preferences,
    pub ai: crate::ai::Preferences,
    pub sync: crate::sync::Preferences,
}
impl Values {
    pub fn from_state(data: &AppState) -> Self {
        Self {
            dark: data.dark,
            theme: data.app_theme(),
            host_icons: data.host_icons,
            shortcuts: data.shortcuts.clone(),
            general: data.general.clone(),
            restore: data.restore_behavior,
            terminal: data.terminal.clone(),
            terminal_preferences: data.terminal_preferences.clone(),
            sftp_editors: data.sftp_editors.clone(),
            sftp: data.sftp.clone(),
            recordings: data.recordings,
            ai: data.ai.clone(),
            sync: data.sync,
        }
    }
}
pub enum Event {
    Changed(Box<Values>),
    WorkspaceCommand(crate::shortcuts::Command),
    CloseRequested {
        values: Box<Values>,
        action: ExitAction,
    },
    DataChanged(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitAction {
    Settings,
    Window,
    Quit,
    Restart,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Page {
    General,
    Terminal,
    Sftp,
    Ai,
    Shortcuts,
    Sync,
    About,
}
impl Page {
    const ALL: [Self; 7] = [
        Self::General,
        Self::Terminal,
        Self::Sftp,
        Self::Ai,
        Self::Shortcuts,
        Self::Sync,
        Self::About,
    ];
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Family,
    CjkFamily,
    Size,
    LineHeight,
    LetterSpacing,
    Weight,
    BoldWeight,
}
impl Field {
    fn value(self, appearance: &Appearance) -> String {
        match self {
            Self::Family => appearance.family.clone(),
            Self::CjkFamily => appearance.cjk_family.clone().unwrap_or_default(),
            Self::Size => appearance.size.to_string(),
            Self::LineHeight => format!("{:.2}", appearance.line_height),
            Self::LetterSpacing => format!("{:.1}", appearance.letter_spacing),
            Self::Weight => appearance.weight.to_string(),
            Self::BoldWeight => appearance.bold_weight.to_string(),
        }
    }
    fn apply(self, value: &str, appearance: &mut Appearance) -> anyhow::Result<()> {
        match self {
            Self::Family => appearance.family = value.into(),
            Self::CjkFamily => {
                appearance.cjk_family = (!value.trim().is_empty()).then(|| value.trim().to_owned())
            }
            Self::Size => appearance.size = value.parse()?,
            Self::LineHeight => appearance.line_height = value.parse()?,
            Self::LetterSpacing => appearance.letter_spacing = value.parse()?,
            Self::Weight => appearance.weight = value.parse()?,
            Self::BoldWeight => appearance.bold_weight = value.parse()?,
        }
        appearance.validate()
    }
}
struct Control {
    field: Field,
    select: SelectModel<String>,
    _events: Subscription,
}
struct ColorControl {
    state: Entity<RgbState>,
    _events: Subscription,
}

pub struct Settings {
    values: Values,
    language: general::LanguageControl,
    host_icons: SelectModel<crate::avatar::Mode>,
    _host_icon_events: Subscription,
    page: Page,
    page_generation: u64,
    search: search::State,
    reset_pending: Option<reset::Scope>,
    reset_notice: Option<&'static str>,
    controls: Vec<Control>,
    font_options: Option<fonts::Options>,
    font_loading: Option<Task<()>>,
    session_controls: Option<session::Controls>,
    padding_control: Option<padding::Control>,
    window_controls: general::WindowControls,
    recording_controls: Vec<recording::Control>,
    recording_directory: PathBuf,
    recording_location_busy: bool,
    colors: Vec<ColorControl>,
    theme_search: Entity<InputState>,
    theme_matches: Vec<ThemeId>,
    theme_gallery_ready: bool,
    theme_scroll: UniformListScrollHandle,
    _theme_search_events: Subscription,
    word_boundaries: Entity<InputState>,
    _word_boundaries_events: Subscription,
    cursor_shape: SelectModel<CursorShape>,
    _cursor_shape_events: Subscription,
    scrollback_lines: Entity<InputState>,
    _scrollback_events: Subscription,
    focus: FocusHandle,
    pub error: Option<String>,
    validation_error: Option<String>,
    editors: Entity<editors::EditorSettings>,
    editor_detection: Option<Task<()>>,
    editor_extensions: Option<Vec<String>>,
    editor_generation: u64,
    _editor_events: Subscription,
    sftp: Entity<sftp::SftpPreferences>,
    _sftp_events: Subscription,
    providers: Entity<ai::Providers>,
    sync: Entity<sync::Panel>,
    about: Entity<about::Panel>,
    _sync_events: Subscription,
    pending_sync_operation: Option<sync::Operation>,
    close_after_sync: Option<ExitAction>,
    _provider_events: Subscription,
    close_after_providers: Option<ExitAction>,
    exit_pending: bool,
    exit_focus: FocusHandle,
    _about_events: Subscription,
    provider_error: Option<String>,
    shortcut_recording: Option<crate::shortcuts::Command>,
    shortcut_focus: FocusHandle,
    _shortcut_keys: Subscription,
    _shortcut_blur: Subscription,
}
impl EventEmitter<Event> for Settings {}
impl Settings {
    pub fn refresh_themes(&mut self, cx: &mut Context<Self>) {
        if !self.theme_gallery_ready {
            return;
        }
        self.theme_matches = crate::theme_catalog::filter(&self.theme_search.read(cx).value());
        cx.notify();
    }
    pub fn system_appearance(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.values.theme == AppTheme::System {
            self.values.dark = dark;
            cx.notify();
        }
    }
    pub fn new(
        values: Values,
        recording_directory: PathBuf,
        provider_service: Result<crate::ai::Service, String>,
        sync_service: Result<crate::sync::Service, String>,
        update_directory: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::settings_window(window, cx);
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        crate::components::message::watch(window, cx, |view: &Self| {
            view.reset_notice
                .map(crate::components::message::Message::info)
        });
        let focus = cx.focus_handle();
        let language = Self::language_control(values.general.language, window, cx);
        let host_icons = SelectModel::new(
            crate::avatar::Mode::ALL
                .into_iter()
                .map(|mode| SelectOption::localized(mode, mode.label()))
                .collect(),
            Some(values.host_icons),
            window,
            cx,
        );
        let host_icon_events = cx.subscribe_in(
            &host_icons.state,
            window,
            |this, _, event: &SelectEvent<crate::avatar::Mode>, window, cx| {
                if let SelectEvent::Confirm(Some(Some(mode))) = event {
                    this.values.host_icons = *mode;
                    this.changed(window, cx);
                }
            },
        );
        focus.focus(window, cx);
        let owner = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            owner
                .update(cx, |this, cx| {
                    this.request_close(false, window, cx);
                    false
                })
                .unwrap_or(true)
        });
        let theme_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search themes..."));
        let word_boundaries = cx.new(|cx| {
            InputState::new(window, cx).default_value(
                crate::terminal_appearance::format_word_boundaries(
                    &values.terminal.word_boundaries,
                ),
            )
        });
        let word_boundaries_events = cx.subscribe_in(
            &word_boundaries,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let mut next = this.values.terminal.clone();
                    next.word_boundaries =
                        crate::terminal_appearance::parse_word_boundaries(&input.read(cx).value());
                    match next.validate() {
                        Ok(()) => {
                            let changed = this.values.terminal != next;
                            this.values.terminal = next;
                            this.validation_error = None;
                            if changed {
                                this.changed(window, cx);
                            } else {
                                cx.notify();
                            }
                        }
                        Err(error) => {
                            this.validation_error = Some(error.to_string());
                            cx.notify();
                        }
                    }
                }
            },
        );
        let theme_events = cx.subscribe(&theme_search, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) && this.theme_gallery_ready {
                this.theme_matches = crate::theme_catalog::filter(&input.read(cx).value());
                if !this.theme_matches.is_empty() {
                    this.theme_scroll.scroll_to_item(0, ScrollStrategy::Nearest);
                }
                cx.notify();
            }
        });
        let editors = cx.new(|cx| {
            editors::EditorSettings::new(values.sftp_editors.clone(), values.dark, window, cx)
        });
        let home_hint = directories::UserDirs::new()
            .map(|dirs| dirs.home_dir().to_string_lossy().into_owned())
            .unwrap_or_else(|| "Home directory".into());
        let sftp = cx.new(|cx| {
            sftp::SftpPreferences::new(values.sftp.clone(), values.dark, home_hint, window, cx)
        });
        let sftp_events = cx.subscribe_in(
            &sftp,
            window,
            |this, _, event: &sftp::Changed, window, cx| {
                this.values.sftp = event.0.clone();
                this.changed(window, cx);
            },
        );
        let editor_events = cx.subscribe_in(
            &editors,
            window,
            |this, _, event: &editors::Changed, window, cx| {
                let detect = this.values.sftp_editors.text_extensions != event.0.text_extensions;
                this.values.sftp_editors = event.0.clone();
                this.changed(window, cx);
                if detect {
                    this.detect_editors(window, cx);
                }
            },
        );
        let (shortcut_focus, shortcut_keys, shortcut_blur) = Self::shortcut_recorder(window, cx);
        let window_controls = Self::window_controls(values.general.window, window, cx);
        let cursor_shape = SelectModel::new(
            CursorShape::ALL
                .into_iter()
                .map(|shape| SelectOption::localized(shape, shape.label()))
                .collect(),
            Some(values.terminal.cursor_shape),
            window,
            cx,
        );
        let cursor_shape_events = cx.subscribe_in(
            &cursor_shape.state,
            window,
            |this, _, event: &SelectEvent<CursorShape>, window, cx| {
                if let SelectEvent::Confirm(Some(Some(shape))) = event {
                    this.values.terminal.cursor_shape = *shape;
                    this.changed(window, cx);
                }
            },
        );
        let scrollback_lines = cx.new(|cx| {
            InputState::new(window, cx).default_value(values.terminal.scrollback_lines.to_string())
        });
        let scrollback_events = cx.subscribe_in(
            &scrollback_lines,
            window,
            |this, input, event: &InputEvent, window, cx| {
                // Applying each keystroke could irreversibly trim history
                // while the user is still replacing a multi-digit number.
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    match parse_scrollback(&input.read(cx).value()) {
                        Ok(lines) => {
                            this.validation_error = None;
                            if this.values.terminal.scrollback_lines == lines {
                                cx.notify();
                                return;
                            }
                            this.values.terminal.scrollback_lines = lines;
                            this.changed(window, cx);
                        }
                        Err(error) => {
                            this.validation_error = Some(error.to_string());
                            cx.notify();
                        }
                    }
                }
            },
        );
        let providers = cx.new(|cx| {
            ai::Providers::new(
                provider_service,
                values.dark,
                values.ai.presets_url.clone(),
                window,
                cx,
            )
        });
        let provider_events = cx.subscribe_in(
            &providers,
            window,
            |this, _, event: &ai::Event, window, cx| {
                match event {
                    ai::Event::Flushed => {
                        if this.pending_sync_operation.is_some() {
                            this.finish_sync_operation(window, cx);
                        }
                        if let Some(action) = this.close_after_providers.take() {
                            this.request_exit(action, window, cx);
                        }
                    }
                    ai::Event::Failed(error) => {
                        if this.pending_sync_operation.take().is_some() {
                            this.sync
                                .update(cx, |panel, cx| panel.operation_failed(error.clone(), cx));
                            return;
                        }
                        this.close_after_providers = None;
                        this.error = Some(error.clone());
                        this.provider_error = Some(error.clone());
                        this.select_page(Page::Ai, window, cx);
                    }
                    ai::Event::Saved => {
                        if this
                            .provider_error
                            .take()
                            .is_some_and(|error| this.error.as_ref() == Some(&error))
                        {
                            this.error = None;
                        }
                    }
                }
                cx.notify();
            },
        );
        let sync =
            cx.new(|cx| sync::Panel::new(sync_service, values.sync, values.dark, window, cx));
        let about = cx.new(|cx| about::Panel::new(values.dark, update_directory, window, cx));
        let about_events =
            cx.subscribe_in(&about, window, |this, _, _: &about::Event, window, cx| {
                this.request_exit(ExitAction::Restart, window, cx);
            });
        let sync_events = cx.subscribe_in(
            &sync,
            window,
            |this, _, event: &sync::Event, window, cx| match event {
                sync::Event::Preferences(preferences) => {
                    this.values.sync = *preferences;
                    this.changed(window, cx);
                }
                sync::Event::MutationRequested(operation) => {
                    this.pending_sync_operation = Some(operation.clone());
                    this.finish_sync_operation(window, cx);
                }
                sync::Event::DataChanged(message) => {
                    this.providers
                        .update(cx, |providers, cx| providers.restored(window, cx));
                    cx.emit(Event::DataChanged(message.clone()));
                }
                sync::Event::Settled(success) => {
                    if !success {
                        this.close_after_sync = None;
                    } else if !this.sync.read(cx).busy()
                        && let Some(action) = this.close_after_sync.take()
                    {
                        this.request_exit(action, window, cx);
                    }
                    cx.notify();
                }
                sync::Event::Unlocked => cx.notify(),
            },
        );
        let search = Self::search_state(values.general.language, window, cx);
        // Start discovery while the first settings page is usable, before the
        // user reaches SFTP. The saved selection is already in its control.
        cx.defer_in(window, |this, window, cx| this.detect_editors(window, cx));
        Self {
            values,
            language,
            host_icons,
            _host_icon_events: host_icon_events,
            page: Page::General,
            page_generation: 0,
            search,
            reset_pending: None,
            reset_notice: None,
            controls: vec![],
            font_options: None,
            font_loading: None,
            session_controls: None,
            padding_control: None,
            window_controls,
            recording_controls: vec![],
            recording_directory,
            recording_location_busy: false,
            colors: vec![],
            theme_search,
            theme_matches: vec![],
            theme_gallery_ready: false,
            theme_scroll: UniformListScrollHandle::default(),
            _theme_search_events: theme_events,
            word_boundaries,
            _word_boundaries_events: word_boundaries_events,
            cursor_shape,
            _cursor_shape_events: cursor_shape_events,
            scrollback_lines,
            _scrollback_events: scrollback_events,
            focus,
            error: None,
            validation_error: None,
            editors,
            editor_detection: None,
            editor_extensions: None,
            editor_generation: 0,
            _editor_events: editor_events,
            sftp,
            _sftp_events: sftp_events,
            providers,
            sync,
            about,
            _sync_events: sync_events,
            pending_sync_operation: None,
            close_after_sync: None,
            _provider_events: provider_events,
            close_after_providers: None,
            exit_pending: false,
            exit_focus: cx.focus_handle(),
            _about_events: about_events,
            provider_error: None,
            shortcut_recording: None,
            shortcut_focus,
            _shortcut_keys: shortcut_keys,
            _shortcut_blur: shortcut_blur,
        }
    }
    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_language_control(window, cx);
        self.reset_notice = None;
        // The owner applies the app theme only after persistence succeeds.
        cx.emit(Event::Changed(Box::new(self.values.clone())));
        cx.notify();
    }
    fn detect_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Windows editor discovery always uses recommended .txt handlers;
        // changing the file-action extensions does not invalidate that catalog.
        let extensions = if cfg!(target_os = "windows") {
            vec!["txt".to_owned()]
        } else {
            self.values.sftp_editors.text_extensions.clone()
        };
        if self.editor_extensions.as_ref() == Some(&extensions)
            && (self.editor_detection.is_some() || self.editors.read(cx).catalog_ready())
        {
            return;
        }
        self.editor_generation = self.editor_generation.wrapping_add(1);
        let generation = self.editor_generation;
        self.editor_extensions = Some(extensions.clone());
        let configured = self.values.sftp_editors.external_editor.clone();
        self.editors.update(cx, |editors, cx| editors.detecting(cx));
        self.editor_detection = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::external_application::editor_catalog(&extensions)
                        .map_err(|error| error.to_string())
                })
                .await;
            let catalog = result.as_ref().ok().cloned().unwrap_or_default();
            let _ = this.update_in(cx, |this, window, cx| {
                if this.editor_generation != generation {
                    return;
                }
                this.editors
                    .update(cx, |editors, cx| editors.detected(result, window, cx));
            });
            cx.background_spawn(async move {
                if let Some(editor) = &configured {
                    crate::external_application::preload_icon(editor);
                }
                for editor in &catalog {
                    crate::external_application::preload_icon(editor);
                }
            })
            .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.editor_generation != generation {
                    return;
                }
                this.editor_detection = None;
                this.editors
                    .update(cx, |editors, cx| editors.refresh_icons(window, cx));
            });
        }));
    }
    fn finish_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.validation_error = None;
        self.finish_padding_editing(window, cx);
        match self
            .sync
            .update(cx, |panel, cx| panel.finish_preferences(cx))
        {
            Ok(preferences) => self.values.sync = preferences,
            Err(error) => {
                self.validation_error = Some(error.to_string());
                cx.notify();
                return false;
            }
        }
        if let Err(error) = self.finish_window_editing(cx) {
            self.validation_error = Some(error.to_string());
            cx.notify();
            return false;
        }
        if let Err(error) = self.finish_session_preferences(cx) {
            self.validation_error = Some(error.to_string());
            cx.notify();
            return false;
        }
        let mut terminal = self.values.terminal.clone();
        match parse_scrollback(&self.scrollback_lines.read(cx).value()) {
            Ok(lines) => terminal.scrollback_lines = lines,
            Err(error) => {
                self.validation_error = Some(error.to_string());
                cx.notify();
                return false;
            }
        }
        terminal.word_boundaries = crate::terminal_appearance::parse_word_boundaries(
            &self.word_boundaries.read(cx).value(),
        );
        if let Err(error) = terminal.validate() {
            self.validation_error = Some(error.to_string());
            cx.notify();
            return false;
        }
        self.values.terminal = terminal;
        match self
            .editors
            .update(cx, |editors, cx| editors.finish_editing(window, cx))
        {
            Ok(values) => {
                let sftp = match self.sftp.read(cx).finish_editing(cx) {
                    Ok(sftp) => sftp,
                    Err(error) => {
                        self.validation_error = Some(error.to_string());
                        cx.notify();
                        return false;
                    }
                };
                self.values.sftp = sftp;
                self.values.sftp_editors = values;
                true
            }
            Err(error) => {
                self.validation_error = Some(error.to_string());
                cx.notify();
                false
            }
        }
    }
    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_close(false, window, cx);
    }
    fn request_close(&mut self, quit: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.request_exit(
            if quit {
                ExitAction::Quit
            } else {
                ExitAction::Settings
            },
            window,
            cx,
        );
    }
    pub fn request_exit(
        &mut self,
        action: ExitAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exit_pending {
            return;
        }
        self.sync.update(cx, |panel, cx| {
            panel.cancel_authorization(cx);
        });
        if self.sync.read(cx).busy() {
            self.close_after_sync = Some(action);
            return;
        }
        self.close_after_providers = Some(action);
        match self
            .providers
            .update(cx, |view, cx| view.finish(window, cx))
        {
            Ok(true) => self.close_after_providers = None,
            Ok(false) => return,
            Err(error) => {
                self.close_after_providers = None;
                self.error = Some(error);
                self.select_page(Page::Ai, window, cx);
                return;
            }
        }
        if self.finish_editing(window, cx) {
            // The workspace must acknowledge persistence before removing this
            // window. A failed save keeps the controls, draft and error visible.
            self.exit_pending = true;
            cx.emit(Event::CloseRequested {
                values: Box::new(self.values.clone()),
                action,
            });
        }
    }
    pub fn cancel_exit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.exit_pending = false;
        self.close_after_sync = None;
        self.close_after_providers = None;
        crate::components::Modal::complete(&self.exit_focus, window, cx);
        cx.notify();
    }
    fn terminal_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.controls.is_empty() {
            return;
        }
        // Controls are usable immediately with their saved values. The font
        // catalog fills their existing models without gating the settings page.
        let fonts = self.font_options.clone().unwrap_or_default();
        if self.font_options.is_none() && self.font_loading.is_none() {
            let text = cx.text_system().clone();
            self.font_loading = Some(cx.spawn_in(window, async move |this, cx| {
                let options = cx
                    .background_spawn(async move { fonts::options(&text) })
                    .await;
                let _ = this.update_in(cx, |this, window, cx| {
                    this.font_loading = None;
                    this.font_options = Some(options);
                    this.refresh_font_options(window, cx);
                    cx.notify();
                });
            }));
        }
        self.init_recording_controls(window, cx);
        self.init_session_controls(window, cx);
        self.init_padding_control(window, cx);
        self.init_theme_colors(window, cx);
        let numeric = |values: &[&str]| {
            values
                .iter()
                .map(|value| SelectOption::new((*value).to_owned(), *value))
                .collect::<Vec<_>>()
        };
        let options = [
            (Field::Family, fonts.primary),
            (Field::CjkFamily, fonts.cjk),
            (
                Field::Size,
                crate::fonts::size_options(self.values.terminal.size),
            ),
            (
                Field::LineHeight,
                numeric(&[
                    "1.00", "1.10", "1.18", "1.20", "1.30", "1.40", "1.50", "1.60",
                ]),
            ),
            (
                Field::LetterSpacing,
                numeric(&["0.0", "0.5", "1.0", "1.5", "2.0"]),
            ),
            (
                Field::Weight,
                crate::fonts::weight_options(&[300, 400, 500]),
            ),
            (
                Field::BoldWeight,
                crate::fonts::weight_options(&[600, 700, 800, 900]),
            ),
        ];
        for (field, mut options) in options {
            let selected = field.value(&self.values.terminal);
            if !options
                .iter()
                .any(|option| option.value() == &Some(selected.clone()))
            {
                options.push(SelectOption::new(selected.clone(), selected.clone()));
            }
            let select = SelectModel::new(options, Some(selected), window, cx);
            let select = if field == Field::Family {
                select.allow_custom_values(str::to_owned, cx)
            } else {
                select
            };
            let events = cx.subscribe_in(
                &select.state,
                window,
                move |this, _, event: &SelectEvent<String>, window, cx| {
                    let SelectEvent::Confirm(value) = event;
                    if let Some(Some(value)) = value {
                        let mut appearance = this.values.terminal.clone();
                        if let Err(error) = field.apply(value, &mut appearance) {
                            this.validation_error = Some(error.to_string());
                            cx.notify();
                            return;
                        }
                        this.validation_error = None;
                        this.values.terminal = appearance;
                        this.changed(window, cx);
                    }
                },
            );
            self.controls.push(Control {
                field,
                select,
                _events: events,
            });
        }
    }
    fn refresh_font_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(fonts) = &self.font_options else {
            return;
        };
        for control in &mut self.controls {
            let mut options = match control.field {
                Field::Family => fonts.primary.clone(),
                Field::CjkFamily => fonts.cjk.clone(),
                _ => continue,
            };
            let selected = control.field.value(&self.values.terminal);
            if !options
                .iter()
                .any(|option| option.value().as_ref() == Some(&selected))
            {
                options.push(SelectOption::new(selected.clone(), selected.clone()));
            }
            control
                .select
                .reconcile(options, Some(selected), window, cx);
        }
    }
    fn control(&self, field: Field) -> &SelectModel<String> {
        &self
            .controls
            .iter()
            .find(|control| control.field == field)
            .expect("terminal controls initialized")
            .select
    }
    fn row_caption(&self, title: &'static str, description: &'static str) -> Div {
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(SettingsText::Label.apply(div()).child(self.text(title)))
            .child(
                SettingsText::Description
                    .apply(div())
                    .text_color(Palette::settings_faint(self.values.dark))
                    .child(self.text(description)),
            )
    }
    fn row(
        &self,
        title: &'static str,
        description: &'static str,
        trailing: impl IntoElement,
    ) -> AnyElement {
        div()
            .w_full()
            .flex()
            .items_center()
            .gap(px(16.))
            .child(self.row_caption(title, description).flex_1())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(trailing),
            )
            .into_any_element()
    }
    fn section(&self, section: Section, icon: IconName, content: impl IntoElement) -> AnyElement {
        self.section_with_action(section, icon, content, None)
    }
    fn section_with_action(
        &self,
        section: Section,
        icon: IconName,
        content: impl IntoElement,
        action: Option<AnyElement>,
    ) -> AnyElement {
        let content = sections::frame(
            section.title(),
            self.text(section.title()),
            icon,
            Palette::settings(self.values.dark),
            content,
            action,
        );
        self.search.section(section, content)
    }
    fn general(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let restore_behavior = RadioGroup::new(
            "restore-behavior",
            [
                (
                    RestoreBehavior::Ask,
                    self.text("settings.general.workspaceRestore.ask"),
                ),
                (
                    RestoreBehavior::Always,
                    self.text("settings.general.workspaceRestore.always"),
                ),
                (
                    RestoreBehavior::Never,
                    self.text("settings.general.workspaceRestore.never"),
                ),
            ],
            self.values.restore,
        )
        .on_change(cx.listener(|this, value, window, cx| {
            this.values.restore = *value;
            this.changed(window, cx);
        }));
        sections::stack()
            .child(
                self.section(
                    Section::Appearance,
                    IconName::Palette,
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            SettingsText::Label
                                .apply(div())
                                .child(self.text("settings.general.applicationTheme.title")),
                        )
                        .child(
                            SettingsText::Description
                                .apply(div())
                                .mt(px(2.))
                                .text_color(Palette::settings_faint(self.values.dark))
                                .child(self.text("settings.general.applicationTheme.description")),
                        )
                        .child(div().mt(px(14.)).child(self.theme_choices(cx)))
                        .child(div().mt(px(18.)).child(self.row(
                            "settings.general.language.label",
                            "settings.general.language.description",
                            self.language.select.control("Language", cx),
                        )))
                        .child(div().mt(px(18.)).child(self.row(
                            "settings.general.hostIcon.label",
                            "settings.general.hostIcon.description",
                            self.host_icons.control("Host Icon", cx),
                        ))),
                ),
            )
            .child(self.window_settings(window, cx))
            .child(
                self.section(
                    Section::Behavior,
                    IconName::SlidersHorizontal,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(
                            self.row(
                                "settings.general.workspacePage.label",
                                "settings.general.workspacePage.description",
                                Switch::new(
                                    "workspace-page",
                                    "Workspace Page",
                                    self.values.general.workspace_enabled,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.general.workspace_enabled = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        )
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap(px(10.))
                                .child(self.row_caption(
                                    "settings.general.workspaceRestore.label",
                                    "settings.general.workspaceRestore.description",
                                ))
                                .child(div().w_full().max_w(px(560.)).child(restore_behavior)),
                        )
                        .child(
                            self.row(
                                "settings.general.confirmOnClose.label",
                                "settings.general.confirmOnClose.description",
                                Switch::new(
                                    "confirm-on-close",
                                    "Confirm on Close",
                                    self.values.general.confirm_on_close,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.general.confirm_on_close = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        ),
                ),
            )
            .into_any_element()
    }
    fn sftp(&self, cx: &mut Context<Self>) -> AnyElement {
        let workspace = self.sftp.update(cx, |view, cx| view.workspace(cx));
        let transfers = self.sftp.update(cx, |view, cx| view.transfers(cx));
        let files = self.sftp.update(cx, |view, cx| view.files(cx));
        sections::stack()
            .child(self.section(Section::SftpWorkspace, IconName::Folder, workspace))
            .child(self.section(Section::Editors, IconName::Folder, self.editors.clone()))
            .child(self.section(Section::Transfers, IconName::ArrowDownUp, transfers))
            .child(self.section(Section::Files, IconName::Folder, files))
            .into_any_element()
    }
    fn terminal(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let appearance = &self.values.terminal;
        sections::stack()
            .child(self.section(
                Section::Theme,
                IconName::Palette,
                self.theme_section(window, cx),
            ))
            .child(
                self.section(
                    Section::Font,
                    IconName::Type,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(
                            div()
                                .flex()
                                .gap(px(16.))
                                .when(window.viewport_size().width < px(720.), |view| {
                                    view.flex_col()
                                })
                                .child(
                                    div().flex_1().min_w_0().child(
                                        self.control(Field::Family).settings_field("Font", cx),
                                    ),
                                )
                                .child(div().flex_1().min_w_0().child(
                                    self.control(Field::Size).settings_field("Font Size", cx),
                                )),
                        )
                        .child(self.row(
                            "settings.terminal.cjkFont.label",
                            "rs.settings.terminal.cjkFont.description",
                            self.control(Field::CjkFamily).control("CJK Font", cx),
                        ))
                        .child(self.row(
                            "settings.terminal.lineHeight.label",
                            "settings.terminal.lineHeight.description",
                            self.control(Field::LineHeight).control("Line Height", cx),
                        ))
                        .child(
                            self.row(
                                "settings.terminal.letterSpacing.label",
                                "settings.terminal.letterSpacing.description",
                                self.control(Field::LetterSpacing)
                                    .control("Letter Spacing", cx),
                            ),
                        )
                        .child(self.row(
                            "settings.terminal.weight.label",
                            "settings.terminal.weight.description",
                            self.control(Field::Weight).control("Weight", cx),
                        ))
                        .child(self.row(
                            "settings.terminal.boldWeight.label",
                            "settings.terminal.boldWeight.description",
                            self.control(Field::BoldWeight).control("Bold Weight", cx),
                        ))
                        .child(
                            self.row(
                                "settings.terminal.ligatures.label",
                                "settings.terminal.ligatures.description",
                                Switch::new(
                                    "font-ligatures",
                                    "Ligatures",
                                    appearance.enable_ligatures,
                                )
                                .on_change(cx.listener(
                                    |this, enabled, window, cx| {
                                        this.values.terminal.enable_ligatures = *enabled;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        ),
                ),
            )
            .child(
                self.section(
                    Section::Cursor,
                    IconName::TextCursor,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(self.row(
                            "settings.terminal.cursorShape.label",
                            "settings.terminal.cursorShape.description",
                            self.cursor_shape.control("Cursor Shape", cx),
                        ))
                        .child(
                            self.row(
                                "settings.terminal.cursorBlink.label",
                                "settings.terminal.cursorBlink.description",
                                Switch::new("cursor-blink", "Blink", appearance.cursor_blink)
                                    .on_change(cx.listener(|this, value, window, cx| {
                                        this.values.terminal.cursor_blink = *value;
                                        this.changed(window, cx);
                                    })),
                            ),
                        ),
                ),
            )
            .child(
                self.section(
                    Section::Layout,
                    IconName::PanelsTopLeft,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(self.padding_row(cx))
                        .child(
                            self.row(
                                "settings.terminal.scrollbar.label",
                                "settings.terminal.scrollbar.description",
                                Switch::new(
                                    "terminal-scrollbar",
                                    "Scrollbar",
                                    appearance.scrollbar,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.terminal.scrollbar = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        ),
                ),
            )
            .child(
                self.section(
                    Section::Input,
                    IconName::Keyboard,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(
                            self.row(
                                "settings.terminal.input.reportMouseEvents.label",
                                "settings.terminal.input.reportMouseEvents.description",
                                Switch::new(
                                    "report-mouse-events",
                                    "Report Mouse Events",
                                    appearance.report_mouse_events,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.terminal.report_mouse_events = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        )
                        .child(
                            self.row(
                                if cfg!(target_os = "macos") {
                                    "rs.settings.terminal.openTarget.label.macos"
                                } else {
                                    "rs.settings.terminal.openTarget.label.other"
                                },
                                "rs.settings.terminal.openTarget.description",
                                Switch::new(
                                    "command-click-opens-filename-or-url",
                                    "Open Filename/URL",
                                    appearance.command_click_opens_filename_or_url,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.terminal.command_click_opens_filename_or_url =
                                            *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        )
                        .child(
                            self.row(
                                if cfg!(target_os = "macos") {
                                    "rs.settings.terminal.clickMovesCursor.label.macos"
                                } else {
                                    "rs.settings.terminal.clickMovesCursor.label.other"
                                },
                                "rs.settings.terminal.clickMovesCursor.description",
                                Switch::new(
                                    "option-click-moves-cursor",
                                    "Click Moves Cursor",
                                    appearance.option_click_moves_cursor,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.terminal.option_click_moves_cursor = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        ),
                ),
            )
            .child(
                self.section(
                    Section::Selection,
                    IconName::Type,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(
                            self.row(
                                "settings.terminal.copyOnSelect.label",
                                "settings.terminal.copyOnSelect.description",
                                Switch::new(
                                    "copy-on-select",
                                    "Copy on Select",
                                    appearance.copy_on_select,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.terminal.copy_on_select = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        )
                        .child(
                            self.row(
                                "settings.terminal.selectCommandBlockOnClick.label",
                                "settings.terminal.selectCommandBlockOnClick.description",
                                Switch::new(
                                    "select-command-block-on-click",
                                    "Select Command Block on Click",
                                    appearance.select_command_block_on_click,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.terminal.select_command_block_on_click = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        )
                        .child(self.row(
                            "settings.terminal.wordBoundaries.label",
                            "rs.settings.terminal.wordBoundaries.description",
                            Input::new(&self.word_boundaries).w_full().aria_label(
                                crate::localization::text(cx, "Word Selection Boundaries"),
                            ),
                        )),
                ),
            )
            .child(self.bell_settings(cx))
            .child(self.session_settings(cx))
            .child(
                self.section(
                    Section::Features,
                    IconName::PanelsTopLeft,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(
                            self.row(
                                "settings.terminal.composer.label",
                                "rs.settings.terminal.composer.description",
                                Switch::new(
                                    "composer-enabled",
                                    "Composer",
                                    appearance.composer_enabled,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.terminal.composer_enabled = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        )
                        .child(
                            self.row(
                                "settings.terminal.multipleTabs.label",
                                "settings.terminal.multipleTabs.description",
                                Switch::new(
                                    "multiple-tabs",
                                    "Multiple Tabs",
                                    appearance.multiple_tabs,
                                )
                                .on_change(cx.listener(
                                    |this, value, window, cx| {
                                        this.values.terminal.multiple_tabs = *value;
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                        ),
                ),
            )
            .child(self.recording_settings(cx))
            .into_any_element()
    }
}
impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::settings(self.values.dark);
        let compact = window.viewport_size().width < px(720.);
        self.editors
            .update(cx, |editors, cx| editors.set_dark(self.values.dark, cx));
        self.sftp
            .update(cx, |view, cx| view.set_dark(self.values.dark, cx));
        self.providers
            .update(cx, |view, cx| view.set_dark(self.values.dark, cx));
        let (title, description) = match self.page {
            Page::General => (
                "settings.pages.general.title",
                "settings.pages.general.description",
            ),
            Page::Terminal => (
                "settings.pages.terminal.title",
                "settings.pages.terminal.description",
            ),
            Page::Sftp => (
                "settings.pages.sftp.title",
                "settings.pages.sftp.description",
            ),
            Page::Ai => ("settings.pages.ai.title", "settings.pages.ai.description"),
            Page::Shortcuts => (
                "settings.pages.shortcuts.title",
                "rs.settings.pages.shortcuts.description",
            ),
            Page::About => (
                "rs.settings.pages.about.title",
                "settings.pages.about.description",
            ),
            Page::Sync => {
                if self.sync.read(cx).requires_master_key() {
                    ("Master Key", "rs.settings.pages.sync.masterKey.description")
                } else {
                    (
                        "settings.pages.sync.title",
                        "rs.settings.pages.sync.description",
                    )
                }
            }
        };
        SettingsText::Body
            .apply(div())
            .size_full()
            .flex()
            .bg(p.surface)
            .text_color(p.text)
            .track_focus(&self.focus)
            .key_context("Settings")
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| this.dismiss(window, cx)))
            .on_action(cx.listener(|this, _: &crate::app_menu::About, window, cx| {
                this.select_page(Page::About, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &crate::app_menu::CheckForUpdates, window, cx| {
                    this.check_for_updates(window, cx)
                }),
            )
            .on_action(
                cx.listener(|_, _: &crate::app_menu::Settings, window, _| window.activate_window()),
            )
            .on_action(
                cx.listener(|this, _: &crate::app_menu::CloseWindow, window, cx| {
                    this.request_close(false, window, cx);
                }),
            )
            .on_action(cx.listener(|_, _: &crate::app_menu::NewConnection, _, cx| {
                cx.emit(Event::WorkspaceCommand(
                    crate::shortcuts::Command::QuickConnect,
                ))
            }))
            .on_action(
                cx.listener(|_, _: &crate::app_menu::NewLocalTerminal, _, cx| {
                    cx.emit(Event::WorkspaceCommand(
                        crate::shortcuts::Command::NewLocalTerminal,
                    ))
                }),
            )
            .on_action(cx.listener(|this, _: &crate::ui::Quit, window, cx| {
                this.request_close(true, window, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .w(px(200.))
                    .h_full()
                    .flex_shrink_0()
                    .p(px(8.))
                    .text_size(px(13.))
                    .bg(p.sidebar)
                    .border_r_1()
                    .border_color(p.border)
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .h(px(52.))
                            .flex()
                            .items_center()
                            .px(px(10.))
                            .gap(px(10.))
                            .child(Icon::new(IconName::Terminal))
                            .child(
                                div()
                                    .font_weight(FontWeights::SEMIBOLD)
                                    .child(crate::localization::text(cx, "Nauterm RS")),
                            ),
                    )
                    .children(
                        [
                            (Page::General, "General", IconName::SlidersHorizontal),
                            (Page::Terminal, "Terminal", IconName::Terminal),
                            (Page::Sftp, "SFTP", IconName::Folder),
                            (Page::Ai, "AI Assistant", IconName::Sparkles),
                            (Page::Sync, "Sync & Backup", IconName::Cloud),
                            (Page::Shortcuts, "Shortcuts", IconName::Keyboard),
                            (Page::About, "About", IconName::Info),
                        ]
                        .into_iter()
                        .map(|(page, label, icon)| {
                            Button::new(label)
                                .w_full()
                                .h(px(32.))
                                .line_height(px(18.))
                                .ghost()
                                .content_start()
                                .icon(icon)
                                .label(self.text(label))
                                .when(self.page == page, |view| {
                                    view.bg(p.hover).text_color(p.accent)
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_page(page, window, cx)
                                }))
                        }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(56.))
                            .flex_shrink_0()
                            .px(px(24.))
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .border_b_1()
                            .border_color(p.border)
                            .child(
                                SettingsText::Header
                                    .apply(div())
                                    .child(self.text("Settings")),
                            )
                            .child(div().flex_1())
                            .child(
                                Input::new(&self.search.input)
                                    .w(px(192.))
                                    .small()
                                    .h(px(31.))
                                    .text_size(px(12.))
                                    .line_height(px(18.))
                                    .font_weight(FontWeights::REGULAR)
                                    .text_color(p.muted)
                                    .bg(p.control)
                                    .cleanable(true)
                                    .prefix(
                                        Icon::new(IconName::Search)
                                            .size(px(16.))
                                            .text_color(p.muted),
                                    )
                                    .aria_label(crate::localization::text(cx, "Search settings")),
                            )
                            .child(self.reset_button(cx))
                            .child(
                                Button::new("settings-done")
                                    .accent()
                                    .label(self.text("Done"))
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.dismiss(window, cx)),
                                    ),
                            ),
                    )
                    .child(if !self.search.query.is_empty() {
                        self.search_results(compact, cx)
                    } else {
                        div()
                            .id("settings-content")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .scrollbars_with(&self.search.content[self.page as usize])
                            .pl(px(if compact { 20. } else { 26. }))
                            .pr(px(if compact { 22. } else { 30. }))
                            .pt(px(if compact { 20. } else { 24. }))
                            .pb(px(30.))
                            .child(SettingsText::PageTitle.apply(div()).child(self.text(title)))
                            .children(crate::components::validation_message(
                                self.validation_error.as_deref(),
                                cx,
                            ))
                            .child(
                                SettingsText::Intro
                                    .apply(div())
                                    .mt(px(3.))
                                    .text_color(p.muted)
                                    .child(self.text(description)),
                            )
                            .child(div().mt(px(30.)).max_w(px(1000.)).w_full().child(
                                match self.page {
                                    Page::General => self.general(window, cx),
                                    Page::Terminal => self.terminal(window, cx),
                                    Page::Sftp => self.sftp(cx),
                                    Page::Ai => self.ai_settings(cx),
                                    Page::Shortcuts => self.shortcut_settings(cx),
                                    Page::Sync => self.sync_settings(window, cx),
                                    Page::About => self.about_settings(cx),
                                },
                            ))
                            .into_any_element()
                    }),
            )
            .children(self.exit_guard(cx))
    }
}

fn parse_scrollback(value: &str) -> anyhow::Result<u32> {
    value.parse().map_err(|_| {
        anyhow::anyhow!(
            "Scrollback lines must be a whole number between 0 and {}.",
            u32::MAX
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn scrollback_accepts_zero_and_custom_counts_without_truncation() -> anyhow::Result<()> {
        assert_eq!(parse_scrollback("0")?, 0);
        assert_eq!(parse_scrollback("23456")?, 23456);
        assert_eq!(parse_scrollback("4294967295")?, u32::MAX);
        for invalid in ["", "-1", "1.5", "NaN", "4294967296"] {
            assert!(parse_scrollback(invalid).is_err());
        }
        Ok(())
    }
    #[test]
    fn setting_values_round_trip_without_losing_precision() -> anyhow::Result<()> {
        let mut appearance = Appearance::default();
        for (field, value) in [
            (Field::CjkFamily, "Example CJK"),
            (Field::CjkFamily, ""),
            (Field::Size, "16"),
            (Field::LineHeight, "1.18"),
            (Field::LetterSpacing, "0.5"),
            (Field::Weight, "500"),
            (Field::BoldWeight, "800"),
        ] {
            field.apply(value, &mut appearance)?;
            assert_eq!(field.value(&appearance), value);
        }
        assert!(Field::LineHeight.apply("NaN", &mut appearance).is_err());
        Ok(())
    }
    #[test]
    fn old_metadata_keeps_the_reference_terminal_defaults() -> anyhow::Result<()> {
        let mut state: AppState = serde_json::from_str(r#"{"schema":1,"dark":true}"#)?;
        state.validate()?;
        assert_eq!(state.terminal, Appearance::default());
        state.terminal.theme = ThemeId::NysaDark;
        state.terminal.size = 24.;
        let restored: AppState = serde_json::from_slice(&serde_json::to_vec(&state)?)?;
        assert_eq!(restored.terminal, state.terminal);
        restored.validate()
    }
}
