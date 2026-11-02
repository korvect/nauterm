//! Sync settings own transient views; configuration stays in RS config and
//! provider/key/backup operations use the original encrypted shared database.
use super::*;
use crate::typography::{FontWeights, SettingsText};
use crate::{
    components::{Dialog, DialogColors, DialogFrame, Modal, Spinner},
    sync::{Backup, Overview, Preferences, Service, Strategy, SyncPlan},
};
use anyhow::Context as _;
use gpui_kit::component::Disableable;
mod authorization;
mod history;
mod key;
mod key_management;
mod providers;
mod scheduler;

pub enum Event {
    Preferences(Preferences),
    MutationRequested(Operation),
    DataChanged(String),
    Settled(bool),
    Unlocked,
}
#[derive(Clone)]
pub enum Operation {
    Restore(Backup),
    Sync(SyncPlan),
    RestoreVersion(crate::sync::history::RestorePlan),
    RotateKey(crate::sync::keys::Plan),
}
struct SyncCompletion {
    applied: bool,
    clean: bool,
}
pub struct Panel {
    service: Option<Service>,
    preferences: Preferences,
    dark: bool,
    opened: bool,
    overview: Option<Overview>,
    backups: Option<Vec<Backup>>,
    loading: bool,
    mutating: bool,
    syncing: bool,
    background_running: bool,
    error: Option<String>,
    validation_error: Option<String>,
    notice: Option<String>,
    history: Option<history::History>,
    strategy: SelectModel<Strategy>,
    retention: Entity<InputState>,
    interval: Entity<InputState>,
    key: key::Key,
    management: Option<key_management::Management>,
    editor: Option<providers::Editor>,
    catalog: bool,
    remove_provider: Option<bool>,
    provider_focus: FocusHandle,
    catalog_focus: FocusHandle,
    remove_focus: FocusHandle,
    backups_open: bool,
    confirm: Option<Backup>,
    focus: FocusHandle,
    backup_focus: FocusHandle,
    confirm_focus: FocusHandle,
    scroll: ScrollHandle,
    _events: Vec<Subscription>,
}
impl EventEmitter<Event> for Panel {}
impl Panel {
    pub fn new(
        service: Result<Service, String>,
        preferences: Preferences,
        dark: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .as_ref()
                .filter(|_| view.opened || view.service.is_some())
                .cloned()
                .map(crate::components::message::Message::error)
        });
        let (service, error) = match service {
            Ok(service) => (Some(service), None),
            Err(error) => (None, Some(error)),
        };
        let strategy = SelectModel::new(
            Strategy::ALL
                .into_iter()
                .map(|value| SelectOption::localized(value, value.label()))
                .collect(),
            Some(preferences.merge_strategy),
            window,
            cx,
        );
        let retention = cx.new(|cx| {
            InputState::new(window, cx).default_value(preferences.backup_count.to_string())
        });
        let interval = cx
            .new(|cx| InputState::new(window, cx).default_value(preferences.minutes().to_string()));
        let events = vec![
            cx.subscribe(&interval, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    match this.finish_preferences(cx) {
                        Ok(preferences) => {
                            this.validation_error = None;
                            cx.emit(Event::Preferences(preferences));
                        }
                        Err(error) => this.validation_error = Some(error.to_string()),
                    }
                    cx.notify();
                }
            }),
            cx.subscribe(
                &strategy.state,
                |this, _, event: &SelectEvent<Strategy>, cx| {
                    if let SelectEvent::Confirm(Some(Some(value))) = event {
                        this.preferences.merge_strategy = *value;
                        cx.emit(Event::Preferences(this.preferences));
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(
                &retention,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        let value = input
                            .read(cx)
                            .value()
                            .trim()
                            .parse::<u32>()
                            .map_or(this.preferences.backup_count, |value| value.min(100) as u16);
                        input.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx)
                        });
                        if value != this.preferences.backup_count {
                            this.preferences.backup_count = value;
                            cx.emit(Event::Preferences(this.preferences));
                        }
                        cx.notify();
                    }
                },
            ),
        ];
        let key = key::Key::new(window, cx);
        Self {
            service,
            preferences,
            dark,
            opened: false,
            overview: None,
            backups: None,
            loading: false,
            mutating: false,
            syncing: false,
            background_running: false,
            error,
            validation_error: None,
            notice: None,
            history: None,
            strategy,
            retention,
            interval,
            key,
            management: None,
            editor: None,
            catalog: false,
            remove_provider: None,
            provider_focus: cx.focus_handle(),
            catalog_focus: cx.focus_handle(),
            remove_focus: cx.focus_handle(),
            backups_open: false,
            confirm: None,
            focus: cx.focus_handle(),
            backup_focus: cx.focus_handle(),
            confirm_focus: cx.focus_handle(),
            scroll: ScrollHandle::default(),
            _events: events,
        }
    }
    pub fn busy(&self) -> bool {
        self.storage_busy() || self.authorization_running()
    }
    fn storage_busy(&self) -> bool {
        self.loading || self.mutating || self.background_running
    }
    pub fn configure(
        &mut self,
        preferences: Preferences,
        dark: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dark = dark;
        if self.preferences != preferences {
            if self.preferences.interval != preferences.interval {
                self.interval.update(cx, |input, cx| {
                    input.set_value(preferences.minutes().to_string(), window, cx)
                });
            }
            self.preferences = preferences;
            self.strategy
                .set_selected(Some(preferences.merge_strategy), window, cx);
            self.retention.update(cx, |input, cx| {
                input.set_value(preferences.backup_count.to_string(), window, cx)
            });
        }
    }
    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.opened = true;
        cx.notify();
        self.refresh(cx);
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.loading = true;
        self.error = None;
        self.validation_error = None;
        let backups = self.backups_open;
        cx.spawn(async move |this, cx| {
            let result = async {
                let overview = service.overview().await?;
                let backups = if backups {
                    Some(service.backups().await?)
                } else {
                    None
                };
                Ok::<_, anyhow::Error>((overview, backups))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                let success = result.is_ok();
                match result {
                    Ok((overview, backups)) => {
                        if overview.has_key {
                            this.key.pending = None;
                        }
                        this.overview = Some(overview);
                        if backups.is_some() {
                            this.backups = backups;
                        }
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.emit(Event::Settled(success));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn activate(&mut self, id: String, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        let expected = self
            .overview
            .as_ref()
            .and_then(|overview| overview.status.active_provider_id.clone());
        self.loading = true;
        self.error = None;
        self.validation_error = None;
        cx.spawn(async move |this, cx| {
            let result = service.activate(expected, Some(id)).await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                let success = result.is_ok();
                match result {
                    Ok(overview) => this.overview = Some(overview),
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.emit(Event::Settled(success));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn show_backups(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.backups_open = true;
        self.refresh(cx);
    }
    fn request_sync(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let plan = self
            .overview
            .as_ref()
            .context("Refresh sync status before syncing.")
            .and_then(|overview| overview.sync_plan(self.preferences));
        match plan {
            Ok(plan) => {
                self.mutating = true;
                self.syncing = true;
                self.error = None;
                self.validation_error = None;
                self.notice = None;
                cx.emit(Event::MutationRequested(Operation::Sync(plan)));
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    fn run_sync(&mut self, plan: SyncPlan, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.operation_failed("Sync storage is unavailable.".into(), cx);
            return;
        };
        let key = self.key.pending.clone();
        self.mutating = true;
        self.syncing = true;
        self.error = None;
        self.validation_error = None;
        self.notice = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.sync_now(plan, key).await;
            let _ = this.update_in(cx, |this, window, cx| {
                let completion = this.finish_sync_attempt(result, false, window, cx);
                cx.emit(Event::Settled(completion.clean));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn finish_sync_attempt(
        &mut self,
        result: anyhow::Result<crate::sync::SyncAttempt>,
        restoring_version: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> SyncCompletion {
        self.mutating = false;
        self.syncing = false;
        let mut applied = false;
        match result {
            Ok(attempt) => {
                let message = match attempt.outcome {
                    Ok(summary) => {
                        applied = true;
                        let message = if restoring_version {
                            summary.message_for("Cloud version restored")
                        } else {
                            summary.message()
                        };
                        self.notice = Some(message.clone());
                        message
                    }
                    Err(error) => {
                        self.error = Some(error);
                        if restoring_version {
                            "Version restoration did not complete. Saved data was refreshed."
                        } else {
                            "Sync did not complete. Saved data was refreshed."
                        }
                        .into()
                    }
                };
                match attempt.overview {
                    Ok(overview) => {
                        if overview.has_key {
                            self.key.clear(window, cx);
                        }
                        self.overview = Some(overview);
                    }
                    Err(error) => {
                        self.error = Some(match self.error.take() {
                            Some(previous) => {
                                format!("{previous} Could not refresh sync status: {error}")
                            }
                            None => format!("Could not refresh sync status: {error}"),
                        })
                    }
                }
                self.backups = None;
                cx.emit(Event::DataChanged(message));
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        SyncCompletion {
            applied,
            clean: self.error.is_none(),
        }
    }
    pub fn operation_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.mutating = false;
        self.syncing = false;
        self.error = Some(error);
        cx.emit(Event::Settled(false));
        cx.notify();
    }
    pub fn restore(&mut self, backup: Backup, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.operation_failed("Sync storage is unavailable.".into(), cx);
            return;
        };
        let count = self.preferences.backup_count;
        self.mutating = true;
        self.error = None;
        self.validation_error = None;
        self.notice = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.restore(backup, count).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.mutating = false;
                match result {
                    Ok(()) => {
                        this.key.clear(window, cx);
                        Modal::complete(&this.confirm_focus, window, cx);
                        this.confirm = None;
                        this.overview = None;
                        this.backups = None;
                        cx.emit(Event::DataChanged("Local backup restored.".into()));
                        this.refresh(cx);
                    }
                    Err(error) => {
                        this.error = Some(error.to_string());
                        cx.emit(Event::Settled(false));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn colors(&self) -> DialogColors {
        let p = Palette::settings(self.dark);
        DialogColors {
            surface: p.surface,
            header: p.sidebar,
            border: p.border,
            foreground: p.text,
            dark: self.dark,
        }
    }
    fn row(
        &self,
        title: &'static str,
        subtitle: &'static str,
        control: impl IntoElement,
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
                            .child(crate::localization::view(title)),
                    )
                    .child(
                        SettingsText::Description
                            .apply(div())
                            .text_color(Palette::settings_faint(self.dark))
                            .child(crate::localization::view(subtitle)),
                    ),
            )
            .child(div().flex_1().min_w_0().child(control))
            .into_any_element()
    }
    fn field(
        &self,
        title: &'static str,
        hint: &'static str,
        control: impl IntoElement,
    ) -> AnyElement {
        let p = Palette::settings(self.dark);
        div()
            .w_full()
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
    fn backups_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.backups_open {
            return None;
        }
        let p = Palette::settings(self.dark);
        let mut body = div()
            .id("sync-backup-list")
            .min_h_0()
            .max_h(px(350.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .scrollbars_with(&self.scroll)
            .flex()
            .flex_col()
            .border_1()
            .border_color(p.border)
            .rounded(px(8.));
        if self.loading {
            body = body.child(div().p(px(12.)).child(Spinner::new()));
        }
        if let Some(backups) = &self.backups {
            if backups.is_empty() {
                body = body.child(
                    div()
                        .text_color(p.muted)
                        .text_size(px(12.))
                        .px(px(12.))
                        .py(px(10.))
                        .child(crate::localization::text(
                            cx,
                            "settings.sync.localBackups.empty",
                        )),
                );
            }
            for (index, backup) in backups.iter().enumerate() {
                let selected = backup.clone();
                body = body.child(
                    div()
                        .flex_shrink_0()
                        .h(px(40.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .when(index > 0, |row| row.border_t_1().border_color(p.border))
                        .child(
                            Icon::new(IconName::Clock)
                                .with_size(px(14.))
                                .text_color(p.muted),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_baseline()
                                .gap(px(8.))
                                .child(backup.date())
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.muted)
                                        .child(format!("{:.1} KB", backup.bytes as f64 / 1024.)),
                                ),
                        )
                        .child(
                            Button::new(("restore-sync-backup", index))
                                .small()
                                .ghost()
                                .label("Restore")
                                .disabled(self.busy())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.confirm = Some(selected.clone());
                                    this.error = None;
                                    this.validation_error = None;
                                    cx.notify();
                                })),
                        ),
                );
            }
        }
        Some(
            Modal::new("local-sync-backups", "Local Backups")
                .focus_scope(&self.backup_focus)
                .return_focus(&self.focus)
                .dismissible(!self.busy())
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.backups_open = false;
                    this.confirm = None;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("local-backups-frame", "Local Backups")
                        .colors(self.colors())
                        .width(480.)
                        .content(body)
                        .action(
                            Button::new("refresh-backups")
                                .outline()
                                .label("Refresh")
                                .disabled(self.busy())
                                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                        )
                        .action(
                            Button::new("close-backups")
                                .label("Done")
                                .disabled(self.busy())
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        ),
                )
                .into_any_element(),
        )
    }
    fn restore_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let backup = self.confirm.as_ref()?;
        let content = div().flex().flex_col().gap(px(10.))
            .child(crate::localization::view(format!("Restore the local backup from {}? Saved data will be replaced and open resource editors will close. A backup of the current data is kept before restoring.", backup.date())));
        Some(
            Dialog::new("restore-sync-backup-confirm", "Restore Local Backup")
                .focus_scope(&self.confirm_focus)
                .return_focus(&self.backup_focus)
                .dismissible(!self.mutating)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.confirm = None;
                    this.error = None;
                    this.validation_error = None;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("restore-sync-backup-frame", "Restore Local Backup")
                        .colors(self.colors())
                        .content(content)
                        .action(
                            Button::new("cancel-backup-restore")
                                .outline()
                                .label("Cancel")
                                .disabled(self.mutating)
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        )
                        .action(
                            Button::new("confirm-backup-restore")
                                .primary()
                                .label(if self.mutating {
                                    "Restoring…"
                                } else {
                                    "Restore"
                                })
                                .disabled(self.mutating)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if this.mutating {
                                        return;
                                    }
                                    if let Some(backup) = this.confirm.clone() {
                                        this.mutating = true;
                                        cx.emit(Event::MutationRequested(Operation::Restore(
                                            backup,
                                        )));
                                        cx.notify();
                                    }
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}
impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.requires_master_key() && self.management.is_none() {
            return self.master_key_gate(cx);
        }
        let p = Palette::settings(self.dark);
        let mut providers = div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(p.border)
            .rounded(px(8.));
        if let Some(overview) = &self.overview {
            if overview.providers.is_empty() {
                providers = providers.child(
                    div()
                        .px(px(12.))
                        .py(px(10.))
                        .text_size(px(12.))
                        .text_color(p.muted)
                        .child(crate::localization::text(
                            cx,
                            "rs.ui.copy.noConnectedProvidersYet",
                        )),
                );
            }
            for (index, provider) in overview.providers.iter().enumerate() {
                let id = provider.id.clone();
                let edit_id = id.clone();
                let history_provider = provider.clone();
                let active = overview.status.active_provider_id.as_deref() == Some(id.as_str());
                let dot = if !provider.connected {
                    rgb(0xf59e0b)
                } else if active {
                    rgb(0x22c55e)
                } else {
                    p.muted
                };
                providers = providers.child(
                    div()
                        .h(px(40.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .when(index > 0, |row| row.border_t_1().border_color(p.border))
                        .child(div().size(px(7.)).rounded_full().bg(dot))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_baseline()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .truncate()
                                        .font_weight(FontWeights::MEDIUM)
                                        .child(provider.name.clone()),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(px(11.))
                                        .text_color(p.muted)
                                        .child(crate::localization::view(if provider.connected {
                                            provider.kind.to_string()
                                        } else {
                                            format!("{} · Credentials required", provider.kind)
                                        })),
                                ),
                        )
                        .child(if active {
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
                            Button::new(("activate-sync-provider", index))
                                .small()
                                .ghost()
                                .label("Use")
                                .disabled(!provider.connected || self.busy())
                                .on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.activate(id.clone(), cx)
                                    }),
                                )
                                .into_any_element()
                        })
                        .when(provider.supports_history, |row| {
                            row.child(
                                Button::new(("sync-provider-history", index))
                                    .small()
                                    .ghost()
                                    .icon(IconName::Clock)
                                    .tooltip("Version History")
                                    .disabled(!provider.connected || self.busy())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_history(history_provider.clone(), window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new(("configure-sync-provider", index))
                                .small()
                                .ghost()
                                .icon(IconName::Settings)
                                .tooltip("Configure")
                                .disabled(self.busy())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_provider_editor(edit_id.clone(), window, cx)
                                })),
                        ),
                );
            }
        }
        let overview = self.overview.as_ref();
        let local = overview.and_then(|value| value.status.sync_snapshot.as_ref());
        let remote = overview.and_then(|value| value.status.remote_revision);
        let has_key = overview.is_some_and(|value| value.has_key);
        let active_name = overview.and_then(|value| {
            let id = value.status.active_provider_id.as_deref()?;
            value
                .providers
                .iter()
                .find(|provider| provider.id == id)
                .map(|provider| provider.name.clone())
        });
        let in_sync = match (local.map(|value| value.revision), remote) {
            (Some(local), Some(remote)) => local == remote,
            _ => false,
        };
        let (status_color, status_label): (Rgba, &str) = if self.syncing {
            (p.accent, "Syncing…")
        } else if self.error.is_some() {
            (rgb(0xef4444), "Attention needed")
        } else if active_name.is_none() {
            (p.muted, "No active provider")
        } else if in_sync {
            (rgb(0x22c55e), "Up to date")
        } else {
            (rgb(0xf59e0b), "Changes pending")
        };
        let group = |icon: IconName, title: &'static str, trailing: Option<AnyElement>| {
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .pb(px(6.))
                .border_b_1()
                .border_color(p.border)
                .child(Icon::new(icon).with_size(px(18.)).text_color(p.accent))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.))
                        .font_weight(FontWeights::SEMIBOLD)
                        .text_color(p.muted)
                        .child(crate::localization::view(title)),
                )
                .children(trailing)
        };
        let short = |id: Option<&str>| -> String {
            id.filter(|id| !id.is_empty())
                .map_or_else(|| "—".to_owned(), |id| id.chars().take(7).collect())
        };
        let version = |label: &'static str, value: String, id: String, color: Rgba| {
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(crate::localization::view(label))
                .child(
                    div()
                        .font_weight(FontWeights::SEMIBOLD)
                        .text_color(color)
                        .child(value),
                )
                .child(div().opacity(0.7).child(id))
        };
        let status_card = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .size(px(36.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(10.))
                            .bg(status_color.opacity(0.14))
                            .text_color(status_color)
                            .child(Icon::new(IconName::Cloud).with_size(px(18.))),
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
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .font_weight(FontWeights::SEMIBOLD)
                                            .child(crate::localization::view(status_label)),
                                    )
                                    .child(div().size(px(6.)).rounded_full().bg(status_color))
                                    .when(self.loading, |row| row.child(Spinner::new())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .child(div().truncate().child(match &active_name {
                                        Some(name) => format!("via {name}"),
                                        None => "Add or activate a provider below".into(),
                                    }))
                                    .child("·")
                                    .child(version(
                                        "Local",
                                        local.map_or_else(
                                            || "—".to_owned(),
                                            |value| format!("r{}", value.revision),
                                        ),
                                        short(local.map(|value| value.snapshot_id.as_str())),
                                        p.text,
                                    ))
                                    .child(Icon::new(IconName::ArrowLeftRight).with_size(px(11.)))
                                    .child(version(
                                        "Remote",
                                        remote.map_or_else(
                                            || "—".to_owned(),
                                            |value| format!("r{value}"),
                                        ),
                                        short(overview.and_then(|value| {
                                            value.status.remote_snapshot_id.as_deref()
                                        })),
                                        if in_sync || remote.is_none() {
                                            p.text
                                        } else {
                                            rgb(0xf59e0b)
                                        },
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .child(
                                Button::new("sync-key-details")
                                    .ghost()
                                    .icon(IconName::KeyRound)
                                    .label("Key")
                                    .tooltip(if has_key {
                                        "Sync key saved on this device"
                                    } else {
                                        "Sync key not saved on this device"
                                    })
                                    .disabled(self.busy())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_key_management(window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(px(4.))
                                    .left(px(18.))
                                    .size(px(6.))
                                    .rounded_full()
                                    .bg(if has_key {
                                        rgb(0x22c55e)
                                    } else {
                                        rgb(0xf59e0b)
                                    }),
                            ),
                    )
                    .child(
                        Button::new("refresh-sync-status")
                            .ghost()
                            .icon(IconName::RotateCcw)
                            .tooltip("Refresh Status")
                            .disabled(self.busy())
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        Button::new("sync-now")
                            .primary()
                            .icon(IconName::RefreshCw)
                            .label(if self.syncing {
                                "Syncing…"
                            } else {
                                "Sync Now"
                            })
                            .disabled(
                                self.busy()
                                    || overview.is_none_or(|overview| {
                                        overview.sync_plan(self.preferences).is_err()
                                    }),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.request_sync(cx))),
                    ),
            )
            .child(
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border)
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(self.row(
                        "Merge Strategy",
                        "How to reconcile local and cloud changes.",
                        self.strategy.control("Merge Strategy", cx),
                    ))
                    .child(
                        self.row(
                            "Automatic Sync",
                            "Sync at this interval, after pending edits finish.",
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div().flex_1().min_w_0().child(
                                        Input::new(&self.interval)
                                            .digits_only()
                                            .aria_label(crate::localization::text(
                                                cx,
                                                "Sync interval in minutes",
                                            ))
                                            .disabled(self.busy() || !self.preferences.automatic),
                                    ),
                                )
                                .child(div().text_size(px(12.)).text_color(p.muted).child(
                                    crate::localization::text(cx, "settings.sync.automatic.unit"),
                                ))
                                .child(
                                    Switch::new(
                                        "automatic-sync",
                                        "Automatic Sync",
                                        self.preferences.automatic,
                                    )
                                    .disabled(self.busy())
                                    .on_change(cx.listener(
                                        |this, enabled, _, cx| {
                                            match this.finish_preferences(cx) {
                                                Ok(mut preferences) => {
                                                    preferences.automatic = *enabled;
                                                    this.preferences = preferences;
                                                    this.error = None;
                                                    this.validation_error = None;
                                                    cx.emit(Event::Preferences(preferences));
                                                }
                                                Err(error) => this.error = Some(error.to_string()),
                                            }
                                            cx.notify();
                                        },
                                    )),
                                ),
                        ),
                    )
                    .child(
                        self.row(
                            "Local Backups",
                            "Keep an encrypted local backup before each sync.",
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div().flex_1().min_w_0().child(
                                        Input::new(&self.retention)
                                            .digits_only()
                                            .aria_label(crate::localization::text(
                                                cx,
                                                "Local backups to keep",
                                            ))
                                            .disabled(self.busy()),
                                    ),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.muted)
                                        .child(crate::localization::text(cx, "rs.ui.copy.copies")),
                                )
                                .child(
                                    Button::new("view-sync-backups")
                                        .outline()
                                        .icon(IconName::Clock)
                                        .label("Browse")
                                        .disabled(self.busy())
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.show_backups(cx)),
                                        ),
                                ),
                        ),
                    ),
            )
            .when_some(self.notice.clone(), |view, notice| {
                view.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.accent)
                        .child(crate::localization::message(cx, &notice)),
                )
            })
            .children(crate::components::validation_message(
                self.validation_error.as_deref(),
                cx,
            ));
        div()
            .w_full()
            .relative()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(status_card)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(group(
                        IconName::Cloud,
                        "PROVIDERS",
                        Some(
                            Button::new("add-sync-provider")
                                .outline()
                                .icon(crate::assets::icon(IconName::Plus))
                                .label("Add Provider")
                                .disabled(self.busy())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.catalog = true;
                                    this.error = None;
                                    this.validation_error = None;
                                    cx.notify();
                                }))
                                .into_any_element(),
                        ),
                    ))
                    .child(providers),
            )
            .children(self.backups_modal(cx))
            .children(self.restore_dialog(cx))
            .children(self.provider_catalog(cx))
            .children(self.provider_editor(cx))
            .children(self.remove_provider_dialog(cx))
            .children(self.history_modal(cx))
            .children(self.version_restore_dialog(cx))
            .children(self.key_management_modal(cx))
            .children(self.forget_key_dialog(cx))
            .into_any_element()
    }
}
impl Settings {
    pub(super) fn sync_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.sync.update(cx, |panel, cx| {
            panel.configure(self.values.sync, self.values.dark, window, cx)
        });
        self.search.section(Section::Sync, self.sync.clone())
    }
    pub(super) fn finish_sync_operation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_sync_operation.is_none() {
            return;
        }
        match self
            .providers
            .update(cx, |providers, cx| providers.finish(window, cx))
        {
            Ok(false) => return,
            Err(error) => {
                self.pending_sync_operation = None;
                self.sync
                    .update(cx, |panel, cx| panel.operation_failed(error, cx));
                return;
            }
            Ok(true) => {}
        }
        if let Some(operation) = self.pending_sync_operation.take() {
            self.sync.update(cx, |panel, cx| match operation {
                Operation::Restore(backup) => panel.restore(backup, window, cx),
                Operation::Sync(plan) => panel.run_sync(plan, window, cx),
                Operation::RestoreVersion(plan) => panel.run_version_restore(plan, window, cx),
                Operation::RotateKey(plan) => panel.run_key_rotation(plan, window, cx),
            });
        }
    }
}
