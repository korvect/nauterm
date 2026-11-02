//! A single retained driver follows saved RS preferences. Wakes reschedule the
//! timer without cancelling any database job that has already been accepted.
use super::*;
use crate::sync::scheduler::{Attempt, Clock};
use std::time::{Duration, Instant};

pub(super) struct Automatic {
    pub(super) running: bool,
    refresh_pending: bool,
    pub(super) wake: async_channel::Sender<()>,
    receiver: Option<async_channel::Receiver<()>>,
    task: Option<Task<()>>,
}
impl Default for Automatic {
    fn default() -> Self {
        let (wake, receiver) = async_channel::bounded(1);
        Self {
            running: false,
            refresh_pending: false,
            wake,
            receiver: Some(receiver),
            task: None,
        }
    }
}
impl Nauterm {
    pub(super) fn stop_automatic_sync(&mut self) {
        // Accepted database jobs are still drained by Vault's shutdown. Drop
        // only the UI driver so it cannot schedule or publish another run.
        self.automatic_sync.wake.close();
        self.automatic_sync.task = None;
        self.secure_preload = None;
    }
    pub(super) fn start_automatic_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.exiting.is_some() || self.automatic_sync.task.is_some() || !self.resources_ready() {
            return;
        }
        let Ok(service) = self.store.sync_service() else {
            return;
        };
        let Some(wake) = self.automatic_sync.receiver.take() else {
            return;
        };
        let mut clock = Clock::new(self.data.sync, Instant::now());
        self.automatic_sync.task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                let Ok((preferences, refreshing)) = this.update_in(cx, |this, window, cx| {
                    this.refresh_after_automatic_sync(window, cx);
                    (this.data.sync, this.automatic_sync.refresh_pending)
                }) else {
                    return;
                };
                let now = Instant::now();
                clock.configure(preferences, now);
                if clock.due(now) {
                    let started = this
                        .update_in(cx, |this, _, cx| this.begin_automatic_sync(cx))
                        .unwrap_or(false);
                    if started {
                        let result = service.sync_automatically(preferences).await;
                        let _ = this.update_in(cx, |this, window, cx| {
                            this.automatic_sync_finished(result, window, cx)
                        });
                        clock.finished(Instant::now());
                        continue;
                    }
                }
                let delay = if clock.due(now) || refreshing {
                    Some(Duration::from_secs(10))
                } else {
                    clock.remaining(now)
                };
                let Some(delay) = delay else {
                    if wake.recv().await.is_err() {
                        return;
                    }
                    continue;
                };
                let timer = cx.background_executor().timer(delay);
                let event = wake.recv();
                futures_util::pin_mut!(timer, event);
                if matches!(
                    futures_util::future::select(timer, event).await,
                    futures_util::future::Either::Right((Err(_), _))
                ) {
                    return;
                }
            }
        }));
    }
    fn resource_editing(&self, cx: &App) -> bool {
        self.editor.is_some()
            || self
                .saved_drawers
                .values()
                .any(|saved| saved.editor.is_some())
            || self.host_import.is_some()
            || self.rename_tag.is_some()
            || self.rename_workspace.is_some()
            || self.delete_pending
            || self.host_exporting
            || !self.forward_mutations.is_empty()
            || self.forward_delete_busy
            || self.quick_connect.is_some()
            || self.serial_connect.is_some()
            || self
                .keychain
                .as_ref()
                .is_some_and(|view| view.read(cx).has_resource_edit(cx))
            || self
                .proxies
                .as_ref()
                .is_some_and(|view| view.read(cx).has_resource_edit())
    }
    fn begin_automatic_sync(&mut self, cx: &mut Context<Self>) -> bool {
        if self.automatic_sync.running
            || self.exiting.is_some()
            || self.automatic_sync.refresh_pending
            || !self.resources_ready()
            || self.recovery_pending.is_some()
            || self.restore_task.is_some()
            || self.close_pending.is_some()
            || self.resource_editing(cx)
            || self
                .settings_window
                .as_ref()
                .is_some_and(|settings| !settings._view.read(cx).ready_for_automatic_sync(cx))
        {
            return false;
        }
        self.automatic_sync.running = true;
        self.mark_settings_sync_running(cx);
        cx.notify();
        true
    }
    pub(super) fn mark_settings_sync_running(&mut self, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings_window {
            let settings_view = settings._view.clone();
            let _ = settings.handle.update(cx, |_, window, cx| {
                settings_view.update(cx, |settings, cx| {
                    settings.automatic_sync_started(window, cx)
                })
            });
        }
    }
    fn automatic_sync_finished(
        &mut self,
        result: anyhow::Result<Attempt>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.automatic_sync.running = false;
        if self.exiting.is_some() {
            return;
        }
        self.automatic_sync.refresh_pending |= result.as_ref().is_ok_and(Attempt::needs_refresh);
        self.notice = match &result {
            Ok(attempt) => match &attempt.outcome {
                Ok(Some(summary)) => Some(summary.message_for("Automatic sync complete")),
                Ok(None) => self.notice.take(),
                Err(error) => Some(format!("Automatic sync failed: {error}")),
            },
            Err(error) => Some(format!("Automatic sync failed: {error}")),
        };
        if let Some(settings) = &self.settings_window {
            let view = settings._view.clone();
            let _ = settings.handle.update(cx, |_, window, cx| {
                view.update(cx, |settings, cx| {
                    settings.automatic_sync_finished(&result, window, cx)
                })
            });
        }
        self.refresh_after_automatic_sync(window, cx);
        cx.notify();
    }
    fn refresh_after_automatic_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.automatic_sync.refresh_pending
            || self.exiting.is_some()
            || self.automatic_sync.running
            || self.resource_editing(cx)
            || self
                .settings_window
                .as_ref()
                .is_some_and(|settings| !settings._view.read(cx).ready_for_automatic_sync(cx))
        {
            return;
        }
        match self.store.refresh_resources(&mut self.data) {
            Ok(()) => {
                self.automatic_sync.refresh_pending = false;
                self.keychain = None;
                self.proxies = None;
                if self
                    .group
                    .is_some_and(|id| !self.data.groups.iter().any(|group| group.id == id))
                {
                    self.group = None;
                }
                self.sync_tag_catalog(cx);
                self.sync_app_theme(window, cx);
            }
            Err(error) => {
                self.notice = Some(format!(
                    "Could not refresh data after automatic sync: {error}"
                ))
            }
        }
        cx.notify();
    }
}
