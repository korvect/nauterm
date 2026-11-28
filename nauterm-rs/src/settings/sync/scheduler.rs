use super::*;
use crate::sync::scheduler::Attempt;
impl Panel {
    pub fn finish_preferences(&mut self, cx: &App) -> anyhow::Result<Preferences> {
        let mut preferences = self.preferences;
        preferences.set_minutes(&self.interval.read(cx).value())?;
        preferences.validate()?;
        self.preferences = preferences;
        Ok(preferences)
    }
    pub fn ready_for_automatic_sync(&self) -> bool {
        !self.busy()
            && self.editor.is_none()
            && !self.catalog
            && self.management.is_none()
            && self.history.is_none()
            && !self.backups_open
            && self.confirm.is_none()
    }
    pub fn automatic_sync_started(&mut self, cx: &mut Context<Self>) {
        self.background_running = true;
        self.error = None;
        self.validation_error = None;
        self.notice = Some("settings.sync.status.syncing".into());
        cx.notify();
    }
    pub fn automatic_sync_finished(
        &mut self,
        result: &anyhow::Result<Attempt>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.background_running = false;
        self.notice = None;
        match result {
            Ok(attempt) => {
                match &attempt.overview {
                    Ok(overview) => {
                        if overview.has_key {
                            self.key.clear(window, cx);
                        }
                        self.overview = Some(overview.clone());
                    }
                    Err(error) => {
                        self.error = Some(
                            crate::localization::format(
                                cx,
                                "settings.sync.error.refreshStatus",
                                &[("error", &error.to_string())],
                            )
                            .to_string(),
                        )
                    }
                }
                match &attempt.outcome {
                    Ok(Some(summary)) => {
                        self.notice = Some(summary.message(
                            crate::localization::language(cx),
                            "settings.sync.result.automaticComplete",
                        ))
                    }
                    Ok(None) => {}
                    Err(error) => {
                        self.error = Some(
                            crate::localization::format(
                                cx,
                                "settings.sync.error.automatic",
                                &[("error", &error.to_string())],
                            )
                            .to_string(),
                        )
                    }
                }
                if attempt.needs_refresh() {
                    self.backups = None;
                }
            }
            Err(error) => {
                self.error = Some(
                    crate::localization::format(
                        cx,
                        "settings.sync.error.automatic",
                        &[("error", &error.to_string())],
                    )
                    .to_string(),
                )
            }
        }
        // Closing waited for storage to settle, not for a successful network.
        cx.emit(Event::Settled(true));
        cx.notify();
    }
}
impl Settings {
    pub fn ready_for_automatic_sync(&self, cx: &App) -> bool {
        self.pending_sync_operation.is_none()
            && !self.exit_pending
            && self.close_after_sync.is_none()
            && self.close_after_providers.is_none()
            && self.sync.read(cx).ready_for_automatic_sync()
            && self.providers.read(cx).ready_for_automatic_sync(cx)
    }
    pub fn automatic_sync_started(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.providers.update(cx, |providers, cx| {
            providers.pause_for_sync(true, window, cx)
        });
        self.sync
            .update(cx, |panel, cx| panel.automatic_sync_started(cx));
        cx.notify();
    }
    pub fn automatic_sync_finished(
        &mut self,
        result: &anyhow::Result<Attempt>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // No AI provider edit could begin while the shared merge was running.
        self.providers.update(cx, |providers, cx| {
            if result.as_ref().is_ok_and(Attempt::needs_refresh) {
                providers.restored(window, cx);
            } else {
                providers.pause_for_sync(false, window, cx);
            }
            if self.page == Page::Ai {
                providers.open(window, cx);
            }
        });
        self.sync.update(cx, |panel, cx| {
            panel.automatic_sync_finished(result, window, cx)
        });
        cx.notify();
    }
}
