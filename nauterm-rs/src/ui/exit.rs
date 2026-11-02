//! The final quit hook has a short GPUI deadline. Normal user exits drain work
//! here first, after preference publication and session consent have succeeded.
use super::*;
use crate::settings::ExitAction;
impl Nauterm {
    pub(crate) fn request_exit(
        &mut self,
        action: ExitAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exiting.is_some() || self.close_pending.is_some() {
            return;
        }
        if let Some(settings) = &self.settings_window {
            let view = settings._view.clone();
            if settings
                .handle
                .update(cx, |_, window, cx| {
                    view.update(cx, |settings, cx| settings.request_exit(action, window, cx))
                })
                .is_ok()
            {
                return;
            }
        }
        self.exit_ready(action, window, cx);
    }
    pub(super) fn exit_ready(
        &mut self,
        action: ExitAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exiting.is_some() {
            return;
        }
        let target = match action {
            ExitAction::Quit => CloseTarget::Quit,
            ExitAction::Window => CloseTarget::Window,
            ExitAction::Restart => CloseTarget::Restart,
            ExitAction::Settings => return,
        };
        self.notice = None;
        if self.has_open_sessions(cx) {
            self.close_pending = Some(target);
            window.activate_window();
            self.focus.focus(window, cx);
            cx.notify();
        } else {
            self.begin_exit(target, window, cx);
        }
    }
    pub(super) fn release_settings_exit(&mut self, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings_window {
            let view = settings._view.clone();
            let _ = settings.handle.update(cx, |_, window, cx| {
                view.update(cx, |settings, cx| settings.cancel_exit(window, cx))
            });
        }
    }
    pub(super) fn cancel_close(&mut self, cx: &mut Context<Self>) {
        if self.exiting.is_some() {
            return;
        }
        if self.close_pending.take().is_some_and(|target| {
            matches!(
                target,
                CloseTarget::Quit | CloseTarget::Window | CloseTarget::Restart
            )
        }) {
            self.release_settings_exit(cx);
        }
        cx.notify();
    }
    pub(super) fn begin_exit(
        &mut self,
        target: CloseTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exiting.is_some() {
            return;
        }
        let action = match target {
            CloseTarget::Quit => ExitAction::Quit,
            CloseTarget::Window => ExitAction::Window,
            CloseTarget::Restart => ExitAction::Restart,
            _ => return,
        };
        self.notice = None;
        if let Err(error) = self.save_exit_checkpoint(window, cx) {
            if action == ExitAction::Restart
                || (cfg!(target_os = "macos") && action == ExitAction::Window)
            {
                self.notice = Some(format!(
                    "Could not save the workspace before closing: {error}"
                ));
                self.close_pending = Some(target);
                cx.notify();
                return;
            }
            eprintln!("Could not save workspace recovery on exit: {error}");
        }
        #[cfg(target_os = "macos")]
        if action == ExitAction::Window {
            if let Some(journal) = &self.journal
                && let Err(error) = journal.suspend_window()
            {
                self.notice = Some(format!("Could not save workspace recovery: {error}"));
                self.close_pending = Some(target);
                cx.notify();
                return;
            }
            self.exiting = Some(action);
            self.startup_task = None;
            self.restore_task = None;
            self.checkpoint_task = None;
            self.stop_automatic_sync();
            self.close_pending = None;
            self.close_settings_window(cx);
            crate::app_menu::close_main_window(window, cx);
            return;
        }
        self.exiting = Some(action);
        self.startup_task = None;
        self.restore_task = None;
        self.stop_automatic_sync();
        self.close_pending = None;
        self.close_settings_window(cx);
        self.exit_task = Some(cx.spawn_in(window, async move |this, cx| {
            crate::lifecycle::prepare_shutdown().await;
            let _ = this.update_in(cx, |_, window, cx| match action {
                ExitAction::Restart => cx.restart(),
                ExitAction::Window => window.remove_window(),
                ExitAction::Quit => cx.quit(),
                ExitAction::Settings => {}
            });
            if action == ExitAction::Restart {
                // Native restart reports helper-spawn failures only through its
                // logger. Teardown is already committed, so finish the accepted
                // exit if the platform leaves this old window alive.
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                let _ = this.update_in(cx, |_, _, cx| cx.quit());
            }
        }));
        cx.notify();
    }
}
