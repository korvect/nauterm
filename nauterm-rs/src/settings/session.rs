//! New-session preferences use the Settings controls and portable config keys.
use super::*;
use crate::terminal_preferences::{Emulation, Engine, Preferences};

pub(super) struct Controls {
    shell: SelectModel<String>,
    engine: SelectModel<Engine>,
    emulation: SelectModel<Emulation>,
    keepalive: Entity<InputState>,
    prediction: SelectModel<crate::terminal_prediction::Mode>,
    _events: Vec<Subscription>,
}
impl Settings {
    pub(super) fn init_session_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.session_controls.is_some() {
            return;
        }
        let preferences = &self.values.terminal_preferences;
        let shell_options =
            std::iter::once(SelectOption::localized(String::new(), "System Default"))
                .chain(crate::shells::discover().into_iter().map(|shell| {
                    SelectOption::new(
                        shell.path.clone(),
                        format!("{} ({})", shell.label, shell.path),
                    )
                }))
                .collect();
        let shell = SelectModel::new(
            shell_options,
            Some(preferences.shell_path.clone().unwrap_or_default()),
            window,
            cx,
        );
        let engine = SelectModel::new(
            Engine::ALL
                .into_iter()
                .map(|value| SelectOption::localized(value, value.label()))
                .collect(),
            Some(preferences.engine),
            window,
            cx,
        );
        let emulation = SelectModel::new(
            Emulation::ALL
                .into_iter()
                .map(|value| SelectOption::localized(value, value.label()))
                .collect(),
            Some(preferences.emulation),
            window,
            cx,
        );
        let keepalive = cx.new(|cx| {
            InputState::new(window, cx).default_value(preferences.ssh_keepalive_seconds.to_string())
        });
        let prediction = SelectModel::new(
            crate::terminal_prediction::Mode::ALL
                .into_iter()
                .map(|mode| SelectOption::localized(mode, mode.label()))
                .collect(),
            Some(self.values.terminal.ssh_prediction),
            window,
            cx,
        );
        let prediction_events = cx.subscribe_in(
            &prediction.state,
            window,
            |this, _, event: &SelectEvent<crate::terminal_prediction::Mode>, window, cx| {
                if let SelectEvent::Confirm(Some(Some(mode))) = event {
                    this.values.terminal.ssh_prediction = *mode;
                    this.changed(window, cx);
                }
            },
        );
        let shell_events = cx.subscribe_in(
            &shell.state,
            window,
            |this, _, event: &SelectEvent<String>, window, cx| {
                if let SelectEvent::Confirm(Some(Some(value))) = event {
                    this.values.terminal_preferences.shell_path =
                        (!value.is_empty()).then(|| value.clone());
                    this.changed(window, cx);
                }
            },
        );
        let engine_events = cx.subscribe_in(
            &engine.state,
            window,
            |this, _, event: &SelectEvent<Engine>, window, cx| {
                if let SelectEvent::Confirm(Some(Some(value))) = event {
                    this.values.terminal_preferences.engine = *value;
                    this.changed(window, cx);
                }
            },
        );
        let emulation_events = cx.subscribe_in(
            &emulation.state,
            window,
            |this, _, event: &SelectEvent<Emulation>, window, cx| {
                if let SelectEvent::Confirm(Some(Some(value))) = event {
                    this.values.terminal_preferences.emulation = *value;
                    this.changed(window, cx);
                }
            },
        );
        let keepalive_events = cx.subscribe_in(
            &keepalive,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    match Preferences::parse_keepalive(&input.read(cx).value()) {
                        Ok(value) => {
                            this.validation_error = None;
                            if this.values.terminal_preferences.ssh_keepalive_seconds != value {
                                this.values.terminal_preferences.ssh_keepalive_seconds = value;
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
        self.session_controls = Some(Controls {
            shell,
            engine,
            emulation,
            keepalive,
            prediction,
            _events: vec![
                shell_events,
                engine_events,
                emulation_events,
                keepalive_events,
                prediction_events,
            ],
        });
    }
    pub(super) fn finish_session_preferences(&mut self, cx: &App) -> anyhow::Result<()> {
        if let Some(controls) = &self.session_controls {
            self.values.terminal_preferences.ssh_keepalive_seconds =
                Preferences::parse_keepalive(&controls.keepalive.read(cx).value())?;
        }
        self.values.terminal_preferences.validate()
    }
    pub(super) fn session_settings(&self, cx: &App) -> AnyElement {
        let controls = self
            .session_controls
            .as_ref()
            .expect("session controls initialized");
        self.section(
            Section::Session,
            IconName::Terminal,
            div()
                .flex()
                .flex_col()
                .gap(px(18.))
                .child(self.row(
                    "settings.terminal.shell.label",
                    "settings.terminal.shell.description",
                    controls.shell.control("Shell", cx),
                ))
                .child(self.row(
                    "settings.terminal.emulator.label",
                    "settings.terminal.emulator.description",
                    controls.engine.control("Emulation Engine", cx),
                ))
                .child(self.row(
                    "settings.terminal.emulation.label",
                    "rs.settings.terminal.emulation.description",
                    controls.emulation.control("Emulation Type", cx),
                ))
                .child(
                    self.row(
                        "settings.terminal.scrollback.label",
                        "rs.settings.terminal.scrollback.description",
                        Input::new(&self.scrollback_lines)
                            .digits_only()
                            .aria_label(crate::localization::text(cx, "Scrollback Lines")),
                    ),
                )
                .child(self.row(
                    "settings.terminal.sshPrediction.label",
                    "settings.terminal.sshPrediction.description",
                    controls.prediction.control("SSH Local Predictive Echo", cx),
                ))
                .child(
                    self.row(
                        "settings.terminal.sshKeepalive.label",
                        "settings.terminal.sshKeepalive.description",
                        Input::new(&controls.keepalive)
                            .digits_only()
                            .aria_label(crate::localization::text(cx, "SSH Keepalive Interval")),
                    ),
                ),
        )
    }
}
