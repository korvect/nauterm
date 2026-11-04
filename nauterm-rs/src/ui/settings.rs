use super::*;
use crate::settings::{Event, Settings, Values};

pub(super) struct SettingsWindow {
    pub(super) handle: AnyWindowHandle,
    pub(super) _view: Entity<Settings>,
    _events: Subscription,
}
impl Nauterm {
    pub(super) fn apply_window_size(
        &mut self,
        size: crate::general_preferences::WindowSize,
        window: &mut Window,
    ) {
        if window.is_fullscreen() {
            self.pending_window_resize = Some(size);
            return;
        }
        self.pending_window_resize = None;
        if window.is_maximized() {
            window.zoom_window();
        }
        window.resize(gpui_kit::size(px(size.width), px(size.height)));
    }
    pub(super) fn sync_app_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = self.data.dark;
        crate::palette::apply_theme(dark, window, cx);
        if let Some(keychain) = &self.keychain {
            keychain.update(cx, |keychain, cx| {
                keychain.set_dark(dark, cx);
                keychain.set_resources(self.data.clone(), cx);
            });
        }
        for files in self.sftp.iter().chain(
            self.terminals
                .iter()
                .flat_map(|tab| tab.tree.sessions())
                .flat_map(|session| [&session.files, &session.tool_files])
                .flatten(),
        ) {
            files.update(cx, |files, cx| files.set_data(self.data.clone(), cx));
        }
        for session in self.terminals.iter().flat_map(|tab| tab.tree.sessions()) {
            if let TabContent::Connection(connection) = &session.content {
                connection.update(cx, |connection, cx| connection.set_dark(dark, cx));
            }
        }
        if let Some(settings) = &self.settings_window {
            settings
                ._view
                .update(cx, |settings, cx| settings.system_appearance(dark, cx));
        }
        cx.notify();
    }
    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings_at(None, window, cx);
    }
    pub(crate) fn check_native_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if cfg!(target_os = "macos") {
            #[cfg(target_os = "macos")]
            cx.spawn_in(window, async move |this, cx| {
                if let Err(error) = crate::sparkle::check_for_updates() {
                    let _ = this.update_in(cx, |this, _, cx| {
                        this.notice = Some(error.to_string());
                        cx.notify();
                    });
                }
            })
            .detach();
            return;
        }
        self.open_settings_at(Some(crate::settings::Page::About), window, cx);
        if let Some(settings) = &self.settings_window {
            let handle = settings.handle;
            let view = settings._view.clone();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    view.update(cx, |settings, cx| settings.check_for_updates(window, cx));
                });
            });
        }
    }
    pub(crate) fn open_settings_at(
        &mut self,
        page: Option<crate::settings::Page>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_theme_catalog(cx);
        if let Some(settings) = &self.settings_window
            && settings
                .handle
                .update(cx, |_, window, cx| {
                    if let Some(page) = page {
                        settings
                            ._view
                            .update(cx, |view, cx| view.select_page(page, window, cx));
                    }
                    window.activate_window();
                })
                .is_ok()
        {
            return;
        }
        self.settings_window = None;
        let values = Values::from_state(&self.data);
        let recording_directory = self.store.paths.captures();
        let provider_service = self.store.ai_providers().map_err(|error| error.to_string());
        let sync_service = self.store.sync_service().map_err(|error| error.to_string());
        let update_directory = self.store.state_directory.join("updates");
        let standard_titlebar = {
            #[cfg(feature = "window-diagnostics")]
            {
                std::env::var("NAUTERM_RESIZE_BENCH_DECOR").as_deref() != Ok("client")
            }
            #[cfg(not(feature = "window-diagnostics"))]
            {
                true
            }
        };
        let result = gpui_kit::open_window(
            WindowOptions {
                app_id: Some("com.korvect.nauterm.rs".into()),
                window_decorations: Some(if standard_titlebar {
                    WindowDecorations::Server
                } else {
                    WindowDecorations::Client
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(950.), px(620.)),
                    cx,
                ))),
                window_min_size: Some(size(px(950.), px(440.))),
                titlebar: Some(TitlebarOptions {
                    title: Some(
                        format!("Nauterm RS {}", crate::localization::text(cx, "Settings")).into(),
                    ),
                    appears_transparent: !standard_titlebar,
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| {
                crate::window_chrome::register_window(window);
                cx.new(|cx| {
                    let mut settings = Settings::new(
                        values,
                        recording_directory,
                        provider_service,
                        sync_service,
                        update_directory,
                        window,
                        cx,
                    );
                    if let Some(page) = page {
                        settings.select_page(page, window, cx);
                    }
                    settings
                })
            },
        );
        match result {
            Ok((handle, view)) => {
                let events = cx.subscribe_in(
                    &view,
                    window,
                    |this, settings, event: &Event, window, cx| match event {
                        Event::WorkspaceCommand(command) => {
                            this.workspace_shortcut(*command, window, cx);
                            window.activate_window();
                        }
                        Event::DataChanged(message) => {
                            if this.exiting.is_some() {
                                return;
                            }
                            crate::ai::invalidate_providers();
                            this.retire_editor();
                            this.saved_drawers.clear();
                            this.retire_history();
                            this.history_events = None;
                            this.card_navigation.reset();
                            this.selection.clear();
                            this.selection_kind = None;
                            this.delete_pending = false;
                            this.keychain = None;
                            this.proxies = None;
                            this.host_import = None;
                            this.snippet_state.ready = false;
                            this.group = None;
                            this.selected_tags.clear();
                            if let Err(error) = this.store.refresh_resources(&mut this.data) {
                                this.notice = Some(format!(
                                    "{message} Refreshing saved data failed: {error}"
                                ));
                            } else {
                                this.notice = None;
                                crate::components::message::show(
                                    window,
                                    crate::components::message::Message::info(message.clone()),
                                    cx,
                                );
                                this.sync_tag_catalog(cx);
                                this.sync_app_theme(window, cx);
                            }
                            cx.notify();
                        }
                        Event::CloseRequested { values, action } => {
                            if this.settings_window.as_ref().is_none_or(|current| {
                                current._view.entity_id() != settings.entity_id()
                            }) {
                                return;
                            }
                            if !this.apply_settings(values, settings, window, cx) {
                                this.release_settings_exit(cx);
                                return;
                            }
                            if *action == crate::settings::ExitAction::Settings {
                                if let Some(error) = settings.read(cx).error.clone() {
                                    this.notice = Some(error);
                                }
                                if let Some(settings) = this.settings_window.take() {
                                    let _ = settings
                                        .handle
                                        .update(cx, |_, window, _| window.remove_window());
                                }
                                return;
                            }
                            this.exit_ready(*action, window, cx);
                        }
                        Event::Changed(values) => {
                            if this.settings_window.as_ref().is_some_and(|current| {
                                current._view.entity_id() == settings.entity_id()
                            }) {
                                this.apply_settings(values, settings, window, cx);
                            }
                        }
                    },
                );
                self.settings_window = Some(SettingsWindow {
                    handle,
                    _view: view,
                    _events: events,
                });
                if self.automatic_sync.running {
                    self.mark_settings_sync_running(cx);
                }
            }
            Err(error) => {
                self.notice = Some(format!("Could not open settings: {error}"));
                cx.notify();
            }
        }
    }
    fn apply_settings(
        &mut self,
        values: &Values,
        settings: &Entity<Settings>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut next = self.data.clone();
        next.theme = Some(values.theme);
        next.host_icons = values.host_icons;
        next.shortcuts = values.shortcuts.clone();
        next.general = values.general.clone();
        let resize = self.data.general.window != next.general.window;
        if resize && let Some(display) = window.display(cx) {
            let bounds = display.bounds();
            next.general.window = next
                .general
                .window
                .constrained(f32::from(bounds.size.width), f32::from(bounds.size.height));
        }
        next.dark = values.theme.dark(window.appearance());
        next.restore_behavior = values.restore;
        next.terminal = values.terminal.clone();
        next.terminal_preferences = values.terminal_preferences.clone();
        next.sftp_editors = values.sftp_editors.clone();
        next.sftp = values.sftp.clone();
        next.recordings = values.recordings;
        next.ai = values.ai.clone();
        next.sync = values.sync;
        let saved = if self.storage_loading {
            Err(anyhow::anyhow!(
                "Saved resources are still loading. Please retry after unlocking Keychain."
            ))
        } else if let Some(error) = &self.storage_error {
            Err(anyhow::anyhow!(error.clone()))
        } else {
            next.validate().and_then(|_| self.store.save(&mut next))
        };
        if let Err(error) = saved {
            settings.update(cx, |settings, cx| {
                settings.settings_save_failed(
                    format!("Could not save settings: {error}. Running sessions were not changed."),
                    cx,
                );
            });
            return false;
        }
        let app_theme_changed = self.data.dark != next.dark;
        let recording_settings_changed = self.data.recordings != next.recordings;
        let host_icons_changed = self.data.host_icons != next.host_icons;
        let sync_changed = self.data.sync != next.sync;
        let shortcuts_changed = self.data.shortcuts != next.shortcuts;
        let language_changed = self.data.general.language != next.general.language;
        self.data = next;
        crate::localization::install(self.data.general.language, cx);
        if language_changed {
            for (page, input) in &self.searches {
                input.update(cx, |input, cx| {
                    input.set_placeholder(
                        super::search_hint(*page, self.data.general.language),
                        window,
                        cx,
                    )
                });
            }
        }
        if sync_changed {
            let _ = self.automatic_sync.wake.try_send(());
        }
        if !self.data.general.workspace_enabled && self.page == Page::Sessions && self.overview {
            self.overview = false;
            self.navigate(Page::Hosts, window, cx);
            self.focus.focus(window, cx);
        }
        if resize {
            let size = self.data.general.window;
            self.apply_window_size(size, window);
            if let Some(settings_window) = &self.settings_window {
                let settings = settings.clone();
                let _ = settings_window.handle.update(cx, |_, window, cx| {
                    settings.update(cx, |settings, cx| {
                        settings.window_size_applied(size, window, cx)
                    });
                });
            }
            self.stage_recovery(window, cx);
        }
        cx.set_global(self.data.shortcuts.clone());
        if shortcuts_changed {
            crate::app_menu::update_shortcuts(&self.data.shortcuts, cx);
        }
        crate::native::configure_terminal_defaults(
            &self.data.terminal,
            &self.data.terminal_preferences,
        );
        if host_icons_changed {
            cx.set_global(self.data.host_icons);
            cx.refresh_windows();
        }
        if recording_settings_changed
            && let Some(service) = self.ensure_recordings(cx)
            && let Err(error) = service.configure(self.data.recordings)
        {
            self.notice = Some(format!(
                "Recording settings were saved, but could not be applied: {error}"
            ));
        }
        if !self.data.sftp.show_tab && self.page == Page::Sftp {
            self.overview = true;
            self.navigate(Page::Sessions, window, cx);
            self.focus.focus(window, cx);
        }
        // Menus may already be open behind Settings. Update their retained
        // eligibility immediately after persistence, not only on the next paint.
        let file_views: Vec<_> = self
            .sftp
            .iter()
            .cloned()
            .chain(
                self.terminals
                    .iter()
                    .flat_map(|tab| tab.tree.sessions())
                    .flat_map(|session| [&session.files, &session.tool_files])
                    .flatten()
                    .cloned(),
            )
            .collect();
        for view in file_views {
            view.update(cx, |view, cx| view.set_data(self.data.clone(), cx));
        }
        if app_theme_changed {
            self.sync_app_theme(window, cx);
        }
        let mut apply_error = None;
        for session in self.terminals.iter().flat_map(|tab| tab.tree.sessions()) {
            let appearance = session.appearance(&self.data);
            let result = match &session.content {
                TabContent::Terminal(terminal) => {
                    terminal.update(cx, |terminal, cx| terminal.set_appearance(&appearance, cx))
                }
                TabContent::Connection(connection) => connection.update(cx, |connection, _| {
                    connection.set_terminal_palette(appearance.palette())
                }),
                TabContent::Preparing(_) => Ok(()),
            };
            if let Err(error) = result {
                apply_error = Some(format!(
                    "Settings were saved, but a running terminal could not apply them: {error}"
                ));
            }
        }
        settings.update(cx, |settings, cx| {
            settings.error = apply_error;
            settings.settings_saved(cx);
            cx.notify();
        });
        cx.notify();
        true
    }
    pub(super) fn close_settings_window(&mut self, cx: &mut Context<Self>) {
        if let Some(settings) = self.settings_window.take() {
            let _ = settings
                .handle
                .update(cx, |_, window, _| window.remove_window());
        }
    }
}
