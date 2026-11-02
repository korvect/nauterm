//! Retained SFTP preference controls over the product's settings components.
use crate::{
    components::{SelectEvent, SelectModel, SelectOption, Switch, settings::Input},
    palette::Palette,
    sftp_preferences::Preferences,
    typography::SettingsText,
};
use gpui_kit::{
    base::input::{InputEvent, InputState},
    prelude::*,
    *,
};

pub struct Changed(pub Preferences);
pub struct SftpPreferences {
    config: Preferences,
    dark: bool,
    pub directory: Entity<InputState>,
    pub tasks: SelectModel<usize>,
    pub threads: SelectModel<usize>,
    error: Option<String>,
    _events: Vec<Subscription>,
}
impl EventEmitter<Changed> for SftpPreferences {}
impl SftpPreferences {
    pub fn new(
        config: Preferences,
        dark: bool,
        home_hint: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let home_hint = home_hint.into();
        let directory = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(&config.initial_local_directory)
                .placeholder(home_hint)
        });
        let tasks = SelectModel::new(
            (1..=8)
                .map(|value| SelectOption::new(value, value.to_string()))
                .collect(),
            Some(config.limits.concurrent_tasks),
            window,
            cx,
        );
        let mut thread_options: Vec<_> = [1, 2, 4, 8, 16, 32]
            .into_iter()
            .map(|value| SelectOption::new(value, value.to_string()))
            .collect();
        if ![1, 2, 4, 8, 16, 32].contains(&config.limits.transfer_threads) {
            thread_options.push(SelectOption::new(
                config.limits.transfer_threads,
                config.limits.transfer_threads.to_string(),
            ));
        }
        let threads = SelectModel::new(
            thread_options,
            Some(config.limits.transfer_threads),
            window,
            cx,
        );
        let events = vec![
            cx.subscribe(&directory, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let mut next = this.config.clone();
                    next.initial_local_directory = input.read(cx).value().trim().into();
                    this.commit(next, cx);
                }
            }),
            cx.subscribe(&tasks.state, |this, _, event: &SelectEvent<usize>, cx| {
                if let SelectEvent::Confirm(Some(Some(value))) = event {
                    let mut next = this.config.clone();
                    next.limits.concurrent_tasks = *value;
                    this.commit(next, cx);
                }
            }),
            cx.subscribe(&threads.state, |this, _, event: &SelectEvent<usize>, cx| {
                if let SelectEvent::Confirm(Some(Some(value))) = event {
                    let mut next = this.config.clone();
                    next.limits.transfer_threads = *value;
                    this.commit(next, cx);
                }
            }),
        ];
        Self {
            config,
            dark,
            directory,
            tasks,
            threads,
            error: None,
            _events: events,
        }
    }
    fn commit(&mut self, next: Preferences, cx: &mut Context<Self>) {
        if let Err(error) = next.validate() {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        self.error = self.finish_editing(cx).err().map(|error| error.to_string());
        if self.config != next {
            self.config = next;
            cx.emit(Changed(self.config.clone()));
        }
        cx.notify();
    }
    pub fn finish_editing(&self, cx: &App) -> anyhow::Result<Preferences> {
        let mut next = self.config.clone();
        next.initial_local_directory = self.directory.read(cx).value().trim().into();
        next.validate()?;
        Ok(next)
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.dark != dark {
            self.dark = dark;
            cx.notify();
        }
    }
    pub fn set_config(&mut self, config: Preferences, window: &mut Window, cx: &mut Context<Self>) {
        self.config = config;
        self.error = None;
        self.directory.update(cx, |input, cx| {
            input.set_value(&self.config.initial_local_directory, window, cx)
        });
        self.tasks
            .set_selected(Some(self.config.limits.concurrent_tasks), window, cx);
        self.threads
            .set_selected(Some(self.config.limits.transfer_threads), window, cx);
        cx.notify();
    }
    fn row(
        &self,
        title: &'static str,
        description: &'static str,
        control: impl IntoElement,
        cx: &App,
    ) -> AnyElement {
        div()
            .w_full()
            .flex()
            .items_center()
            .gap(px(16.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        SettingsText::Label
                            .apply(div())
                            .child(crate::localization::text(cx, title)),
                    )
                    .child(
                        SettingsText::Description
                            .apply(div())
                            .text_color(Palette::settings_faint(self.dark))
                            .child(crate::localization::text(cx, description)),
                    ),
            )
            .child(div().flex_1().min_w_0().flex().justify_end().child(control))
            .into_any_element()
    }
    pub fn workspace(&self, cx: &mut Context<Self>) -> AnyElement {
        self.row(
            "settings.sftp.workspaceTab.label",
            "settings.sftp.workspaceTab.description",
            Switch::new("show-sftp-tab", "SFTP Tab", self.config.show_tab).on_change(cx.listener(
                |this, value, _, cx| {
                    let mut next = this.config.clone();
                    next.show_tab = *value;
                    this.commit(next, cx);
                },
            )),
            cx,
        )
    }
    pub fn transfers(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(18.))
            .child(self.row(
                "settings.sftp.concurrentTasks.label",
                "settings.sftp.concurrentTasks.description",
                self.tasks.control("Concurrent Tasks", cx),
                cx,
            ))
            .child(self.row(
                "settings.sftp.transferConcurrency.label",
                "settings.sftp.transferConcurrency.description",
                self.threads.control("Transfer Concurrency", cx),
                cx,
            ))
            .into_any_element()
    }
    pub fn files(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(18.))
            .child(
                self.row(
                    "settings.sftp.defaultDirectory.label",
                    "settings.sftp.defaultDirectory.description",
                    Input::new(&self.directory)
                        .aria_label(crate::localization::text(cx, "Initial Local Directory")),
                    cx,
                ),
            )
            .children(crate::components::validation_message(
                self.error.as_deref(),
                cx,
            ))
            .child(
                self.row(
                    "settings.sftp.showHiddenFiles.label",
                    "settings.sftp.showHiddenFiles.description",
                    Switch::new(
                        "sftp-hidden-default",
                        "Show Hidden Files",
                        self.config.show_hidden_files,
                    )
                    .on_change(cx.listener(|this, value, _, cx| {
                        let mut next = this.config.clone();
                        next.show_hidden_files = *value;
                        this.commit(next, cx);
                    })),
                    cx,
                ),
            )
            .into_any_element()
    }
}
impl Render for SftpPreferences {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(30.))
            .child(self.workspace(cx))
            .child(self.transfers(cx))
            .child(self.files(cx))
    }
}
