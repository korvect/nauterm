//! Page-owned preference reset; resource catalogs and unknown config stay intact.
use super::*;
use crate::components::menu::{DropdownMenu, MenuItem};

#[derive(Clone, Copy)]
pub(super) enum Scope {
    Page(Page),
    All,
}
impl Scope {
    fn includes(self, page: Page) -> bool {
        matches!(self, Self::All) || matches!(self, Self::Page(current) if current == page)
    }
    fn notice(self) -> &'static str {
        match self {
            Self::Page(_) => "Page reset to defaults.",
            Self::All => "All settings reset to defaults.",
        }
    }
}

impl Values {
    fn reset(&mut self, scope: Scope, defaults: &Self) {
        if scope.includes(Page::General) {
            self.dark = defaults.dark;
            self.theme = defaults.theme;
            self.host_icons = defaults.host_icons;
            self.general = defaults.general.clone();
            self.restore = defaults.restore;
        }
        if scope.includes(Page::Terminal) {
            // Meta belongs to Shortcuts, even though the native input model
            // stores it with the other terminal presentation preferences.
            let meta = self.terminal.use_option_as_meta_key;
            self.terminal = defaults.terminal.clone();
            self.terminal.use_option_as_meta_key = meta;
            self.terminal_preferences = defaults.terminal_preferences.clone();
            self.recordings = defaults.recordings;
        }
        if scope.includes(Page::Sftp) {
            self.sftp = defaults.sftp.clone();
            self.sftp_editors = defaults.sftp_editors.clone();
        }
        if scope.includes(Page::Shortcuts) {
            self.terminal.use_option_as_meta_key = defaults.terminal.use_option_as_meta_key;
            // Reset known bindings without dropping actions introduced by a
            // newer client and retained in the portable shortcuts object.
            for command in crate::shortcuts::Command::ALL {
                self.shortcuts
                    .set(command, &defaults.shortcuts.raw(command));
            }
        }
        if scope.includes(Page::Ai) {
            self.ai.include_selection = defaults.ai.include_selection;
            self.ai.include_recent_output = defaults.ai.include_recent_output;
        }
        if scope.includes(Page::Sync) {
            self.sync = defaults.sync;
        }
    }
}

impl Settings {
    pub(super) fn reset_button(&self, cx: &Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let page = self.page;
        Button::new("reset-settings")
            .ghost()
            .text_color(Palette::settings(self.values.dark).muted)
            .label(self.text("Reset"))
            .icon(IconName::RotateCcw)
            .tooltip("Reset settings")
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.width(210.);
                for (label, scope, icon) in [
                    ("Reset Current Page", Scope::Page(page), IconName::RotateCcw),
                    ("Reset All Settings", Scope::All, IconName::RefreshCw),
                ] {
                    if matches!(scope, Scope::Page(Page::About)) {
                        continue;
                    }
                    let owner = owner.clone();
                    menu = menu.item(MenuItem::localized(label).icon(icon).on_click(
                        move |_, window, cx| {
                            // The menu dismisses and restores its old focus before
                            // any reset replaces controls that owned that focus.
                            let owner = owner.clone();
                            window.defer(cx, move |window, cx| {
                                let _ = owner
                                    .update(cx, |this, cx| this.reset_settings(scope, window, cx));
                            });
                        },
                    ));
                }
                menu
            })
            .into_any_element()
    }

    fn reset_settings(&mut self, scope: Scope, window: &mut Window, cx: &mut Context<Self>) {
        let defaults = match crate::store::Store::default_preferences() {
            Ok(state) => Values::from_state(&state),
            Err(error) => {
                self.error = Some(format!("Could not load default settings: {error}"));
                cx.notify();
                return;
            }
        };
        self.shortcut_recording = None;
        self.values.reset(scope, &defaults);
        self.error = None;
        self.reset_pending = Some(scope);
        if scope.includes(Page::General) {
            self.values.dark = self.values.theme.dark(window.appearance());
            self.host_icons
                .set_selected(Some(self.values.host_icons), window, cx);
            self.reset_window_controls(window, cx);
        }
        if scope.includes(Page::Terminal) {
            self.controls.clear();
            self.session_controls = None;
            self.padding_control = None;
            self.recording_controls.clear();
            self.colors.clear();
            self.cursor_shape
                .set_selected(Some(self.values.terminal.cursor_shape), window, cx);
            self.scrollback_lines.update(cx, |input, cx| {
                input.set_value(
                    self.values.terminal.scrollback_lines.to_string(),
                    window,
                    cx,
                )
            });
            self.word_boundaries.update(cx, |input, cx| {
                input.set_value(
                    crate::terminal_appearance::format_word_boundaries(
                        &self.values.terminal.word_boundaries,
                    ),
                    window,
                    cx,
                )
            });
            if self.page == Page::Terminal {
                self.terminal_controls(window, cx);
            }
        }
        if scope.includes(Page::Sftp) {
            // A completion for the old detection request must not republish a
            // saved external editor after reset clears that selection.
            self.editor_generation = self.editor_generation.wrapping_add(1);
            self.editor_detection = None;
            self.editors.update(cx, |view, cx| {
                view.set_config(self.values.sftp_editors.clone(), window, cx)
            });
            self.sftp.update(cx, |view, cx| {
                view.set_config(self.values.sftp.clone(), window, cx)
            });
        }
        self.focus.focus(window, cx);
        self.changed(window, cx);
    }

    pub fn settings_saved(&mut self, cx: &mut Context<Self>) {
        if let Some(scope) = self.reset_pending.take() {
            self.reset_notice = Some(scope.notice());
            cx.notify();
        }
    }
    pub fn settings_save_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.reset_pending = None;
        self.reset_notice = None;
        self.error = Some(error);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    fn defaults() -> anyhow::Result<Values> {
        Ok(Values::from_state(
            &crate::store::Store::default_preferences()?,
        ))
    }
    fn customized() -> anyhow::Result<Values> {
        let mut values = defaults()?;
        values.theme = AppTheme::Dark;
        values.dark = true;
        values.sync.backup_count = 34;
        values.sync.automatic = true;
        values.sync.interval = 300000;
        values.sync.merge_strategy = crate::sync::Strategy::RemoteWins;
        values.host_icons = crate::avatar::Mode::ALL
            .into_iter()
            .find(|mode| *mode != values.host_icons)
            .expect("multiple host icon modes");
        values.general.window.width = 1200.;
        values.general.confirm_on_close = false;
        values.general.workspace_enabled = false;
        values.restore = RestoreBehavior::Never;
        values.terminal.size = 18.;
        values.terminal.cjk_family = Some("Example CJK".into());
        values.terminal.enable_ligatures = true;
        values.terminal.use_option_as_meta_key = false;
        values.terminal_preferences.ssh_keepalive_seconds = 90;
        values.recordings.capture = !values.recordings.capture;
        values.ai.include_selection = false;
        values.ai.include_recent_output = false;
        values.ai.presets_url = "https://example.invalid/custom-presets.json".into();
        values.sftp.show_hidden_files = true;
        values.sftp.initial_local_directory = if cfg!(windows) {
            "C:\\example"
        } else {
            "/example"
        }
        .into();
        values.sftp_editors.ssh_editor = "nano".into();
        values.shortcuts = serde_json::from_value(serde_json::json!({
            "quickConnect":"cmd+alt+k", "futureAction":{"key":"future"},
            "tabSwitches":["cmd+1","cmd+2","cmd+3","cmd+4","cmd+5","cmd+6","cmd+7","cmd+8","cmd+9","cmd+0",{"future":true}]
        }))?;
        Ok(values)
    }

    #[test]
    fn page_reset_preserves_preferences_owned_by_other_pages() -> anyhow::Result<()> {
        let defaults = defaults()?;
        let original = customized()?;
        for page in [
            Page::General,
            Page::Terminal,
            Page::Sftp,
            Page::Ai,
            Page::Sync,
        ] {
            let mut actual = original.clone();
            actual.reset(Scope::Page(page), &defaults);
            let mut expected = original.clone();
            match page {
                Page::General => {
                    expected.dark = defaults.dark;
                    expected.theme = defaults.theme;
                    expected.host_icons = defaults.host_icons;
                    expected.general = defaults.general.clone();
                    expected.restore = defaults.restore;
                }
                Page::Terminal => {
                    expected.terminal = defaults.terminal.clone();
                    expected.terminal.use_option_as_meta_key =
                        original.terminal.use_option_as_meta_key;
                    expected.terminal_preferences = defaults.terminal_preferences.clone();
                    expected.recordings = defaults.recordings;
                }
                Page::Sftp => {
                    expected.sftp = defaults.sftp.clone();
                    expected.sftp_editors = defaults.sftp_editors.clone();
                }
                Page::Ai => {
                    expected.ai.include_selection = defaults.ai.include_selection;
                    expected.ai.include_recent_output = defaults.ai.include_recent_output;
                }
                Page::Shortcuts => unreachable!(),
                Page::Sync => expected.sync = defaults.sync,
                Page::About => {}
            }
            assert!(actual == expected);
        }
        Ok(())
    }

    #[test]
    fn shortcut_page_reset_preserves_terminal_settings_and_future_bindings() -> anyhow::Result<()> {
        let defaults = defaults()?;
        let original = customized()?;
        let mut actual = original.clone();
        actual.reset(Scope::Page(Page::Shortcuts), &defaults);
        assert_eq!(
            actual.terminal.use_option_as_meta_key,
            defaults.terminal.use_option_as_meta_key
        );
        for command in crate::shortcuts::Command::ALL {
            assert_eq!(
                actual.shortcuts.raw(command),
                defaults.shortcuts.raw(command)
            );
        }
        assert_eq!(
            serde_json::to_value(&actual.shortcuts)?["futureAction"]["key"],
            "future"
        );
        assert_eq!(
            serde_json::to_value(&actual.shortcuts)?["tabSwitches"][9],
            "cmd+0"
        );
        assert_eq!(
            serde_json::to_value(&actual.shortcuts)?["tabSwitches"][10]["future"],
            true
        );
        actual.terminal.use_option_as_meta_key = original.terminal.use_option_as_meta_key;
        actual.shortcuts = original.shortcuts.clone();
        assert!(actual == original);
        Ok(())
    }

    #[test]
    fn reset_all_uses_compiled_platform_defaults_and_retains_unknown_shortcuts()
    -> anyhow::Result<()> {
        let defaults = defaults()?;
        let mut actual = customized()?;
        actual.reset(Scope::All, &defaults);
        assert_eq!(
            serde_json::to_value(&actual.shortcuts)?["futureAction"]["key"],
            "future"
        );
        for command in crate::shortcuts::Command::ALL {
            assert_eq!(
                actual.shortcuts.raw(command),
                defaults.shortcuts.raw(command)
            );
        }
        actual.shortcuts = defaults.shortcuts.clone();
        let mut expected = defaults.clone();
        expected.ai.presets_url = "https://example.invalid/custom-presets.json".into();
        assert!(actual == expected);
        Ok(())
    }
}
