//! Keep the two action groups together, wrapping groups in narrow panes like
//! Flutter's OverflowBar. This is independent of single-row management toolbars.
use super::*;

#[derive(Clone, Copy)]
pub(crate) enum HostAction {
    Edit,
    Retry,
}
impl EventEmitter<HostAction> for Connection {}

pub(crate) struct PreviousAttempt {
    logs: Vec<crate::connection_state::LogEntry>,
    interventions: Vec<Intervention>,
    pipeline: progress::Pipeline,
}
impl PreviousAttempt {
    fn apply(mut self, state: &mut ConnectionState) -> progress::Pipeline {
        self.logs.append(&mut state.logs);
        if self.logs.len() > 2000 {
            self.logs.drain(..self.logs.len() - 2000);
        }
        for (sequence, log) in self.logs.iter_mut().enumerate() {
            log.sequence = sequence as u64;
        }
        state.logs = self.logs;
        for intervention in state.interventions.drain(..) {
            if !self.interventions.contains(&intervention) {
                self.interventions.push(intervention);
            }
        }
        state.interventions = self.interventions;
        self.pipeline
    }
}
impl Connection {
    pub(crate) fn previous_attempt(&self) -> PreviousAttempt {
        PreviousAttempt {
            logs: self.state.logs.clone(),
            interventions: self.state.interventions.clone(),
            pipeline: self.pipeline.clone(),
        }
    }
    pub(crate) fn inherit_previous_attempt(&mut self, previous: PreviousAttempt) {
        self.pipeline = previous.apply(&mut self.state);
    }
    pub(crate) fn begin_saved_reload(&mut self, cx: &mut Context<Self>) {
        self._output = None;
        self._reveal = None;
        self.session = None;
        self.auth.invalidate();
        self.reloading = true;
        self.error = None;
        self.show_logs = false;
        self.state.begin_attempt();
        cx.notify();
    }
    pub(crate) fn saved_reload_failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.reloading = false;
        self.state.fail(message);
        cx.notify();
    }
    pub(crate) fn allow_host_actions(&mut self, allowed: bool, cx: &mut Context<Self>) {
        if self.host_actions != allowed {
            self.host_actions = allowed;
            cx.notify();
        }
    }
    fn can_start_over(&self) -> bool {
        !self.reloading
            && (self.state.phase == Phase::Failed
                || self.show_logs
                    && self.state.phase == Phase::Connecting
                    && self.target.ssh().is_some())
    }
    fn start_over(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_start_over() || self.auth.busy {
            return;
        }
        if self.host_actions {
            // The owning saved-Host session reloads current catalog settings,
            // rather than reconnecting an options snapshot from before editing.
            cx.emit(HostAction::Retry);
        } else {
            self.start(window, cx);
        }
    }
    pub(super) fn footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let failed = self.state.phase == Phase::Failed;
        let choosing = matches!(self.target, Target::Choice(_));
        let mut secondary = vec![
            self.action("cancel-connection", "Close", false)
                .disabled(self.auth.busy)
                .on_click(cx.listener(|this, _, _, cx| {
                    if !this.auth.busy {
                        cx.emit(ConnectionEvent::Close);
                    }
                }))
                .into_any_element(),
        ];
        if self.state.phase == Phase::Authentication
            && self.mode == AuthMode::PublicKey
            && !self.requires_fido2_pin()
        {
            secondary.push(
                self.action("add-authentication-key", "Add key", false)
                    .disabled(
                        self.auth.busy || self.auth.vault.is_none() || self.auth.keychain.is_none(),
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.add_authentication_key(window, cx)),
                    )
                    .into_any_element(),
            );
        }
        if failed && self.host_actions {
            secondary.push(
                self.action("edit-connection-host", "Edit Host", false)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.state.phase == Phase::Failed {
                            cx.emit(HostAction::Edit);
                        }
                    }))
                    .into_any_element(),
            );
        }
        let mut primary = vec![];
        if choosing {
            primary.push(
                self.action("connect-selected-protocol", "Connect", true)
                    .disabled(self.confirming_protocol())
                    .on_click(cx.listener(|this, _, _, cx| this.confirm_protocol(cx)))
                    .into_any_element(),
            );
        } else if self.state.phase == Phase::HostKey {
            primary.push(
                self.action("trust-once", "Continue", false)
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(options) = this.target.ssh_mut() {
                            options.trust = HostTrust::AcceptOnce;
                        }
                        this.start(window, cx);
                    }))
                    .into_any_element(),
            );
            primary.push(
                self.action("trust-save", "Add and continue", true)
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(options) = this.target.ssh_mut() {
                            options.trust = HostTrust::AcceptAndSave;
                        }
                        this.start(window, cx);
                    }))
                    .into_any_element(),
            );
        } else if self.state.phase == Phase::Authentication {
            primary.push(self.authentication_footer(cx));
        } else if self.can_start_over() {
            // Retrying a changed key still goes through native verification;
            // the user may have corrected known_hosts since the failure.
            primary.push(
                self.action("retry-connection", "Start over", true)
                    .on_click(cx.listener(|this, _, window, cx| this.start_over(window, cx)))
                    .into_any_element(),
            );
        }
        let has_primary = !primary.is_empty();
        div()
            .id("connection-footer")
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(10.))
                    .children(secondary),
            )
            .when(has_primary, |view| {
                view.child(
                    div()
                        .ml_auto()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_end()
                        .gap(px(10.))
                        .children(primary),
                )
            })
            .on_prepaint({
                let owner = cx.weak_entity();
                move |bounds, _, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if bounds.size.height > px(0.)
                            && (this.footer_height - bounds.size.height).abs() > px(0.5)
                        {
                            this.footer_height = bounds.size.height;
                            cx.notify();
                        }
                    });
                }
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::PreviousAttempt;
    use crate::{
        connection_state::{ConnectionState, Intervention, Phase},
        native::SessionEvent,
    };
    #[test]
    fn retry_keeps_history_and_intervention_order_without_restoring_old_failure_state() {
        let mut old = ConnectionState::default();
        old.apply(SessionEvent {
            kind: "host_key_unknown".into(),
            message: "old trust prompt".into(),
            ..Default::default()
        });
        old.apply(SessionEvent {
            kind: "host_key_changed".into(),
            message: "old failure".into(),
            fingerprint: Some("old fingerprint".into()),
            ..Default::default()
        });
        let previous = PreviousAttempt {
            logs: old.logs,
            interventions: old.interventions,
            pipeline: Default::default(),
        };
        let mut next = ConnectionState::default();
        next.apply(SessionEvent {
            kind: "auth_failed".into(),
            message: "new auth prompt".into(),
            ..Default::default()
        });
        let _ = previous.apply(&mut next);
        assert_eq!(next.phase, Phase::Authentication);
        assert!(!next.changed_key);
        assert!(next.fingerprint.is_none());
        assert_eq!(next.message, "new auth prompt");
        assert_eq!(
            next.interventions,
            [Intervention::HostKey, Intervention::Authentication]
        );
        assert_eq!(
            next.logs
                .iter()
                .map(|log| log.event.message.as_str())
                .collect::<Vec<_>>(),
            ["old trust prompt", "old failure", "new auth prompt"]
        );
        assert_eq!(
            next.logs.iter().map(|log| log.sequence).collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }
    #[test]
    fn retry_history_remains_bounded_and_keeps_the_latest_native_event() {
        let mut old = ConnectionState::default();
        for index in 0..2000 {
            old.apply(SessionEvent {
                kind: "connect_start".into(),
                message: index.to_string(),
                ..Default::default()
            });
        }
        let previous = PreviousAttempt {
            logs: old.logs,
            interventions: old.interventions,
            pipeline: Default::default(),
        };
        let mut next = ConnectionState::default();
        next.apply(SessionEvent {
            kind: "connected".into(),
            message: "latest".into(),
            ..Default::default()
        });
        let _ = previous.apply(&mut next);
        assert_eq!(next.logs.len(), 2000);
        assert_eq!(next.logs.first().unwrap().event.message, "1");
        assert_eq!(next.logs.last().unwrap().event.message, "latest");
        assert_eq!(next.logs.last().unwrap().sequence, 1999);
        assert_eq!(next.phase, Phase::Completing);
    }
}
