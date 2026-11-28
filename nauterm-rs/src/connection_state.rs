//! Connection presentation follows native event order, never guesses success
//! from terminal output or treats an authentication challenge as a fatal error.
use crate::native::SessionEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Connecting,
    HostKey,
    Authentication,
    Completing,
    Failed,
    Exited,
}

impl Phase {
    /// Overview status uses real transport/PTY state, not a successful render.
    pub fn monitor_label(self) -> &'static str {
        match self {
            Self::Connecting | Self::HostKey | Self::Authentication => "Connecting",
            Self::Completing => "Live",
            Self::Failed => "Failed",
            Self::Exited => "Exited",
        }
    }

    pub fn with_pty_exit(self, exited: bool) -> Self {
        if exited && !matches!(self, Self::Failed | Self::HostKey | Self::Authentication) {
            Self::Exited
        } else {
            self
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub sequence: u64,
    pub time: String,
    pub category: &'static str,
    pub event: SessionEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}
impl LogEntry {
    pub fn severity(&self) -> Severity {
        match self.event.kind.as_str() {
            "auth_interactive_request"
                if self
                    .event
                    .challenge
                    .as_ref()
                    .is_some_and(|challenge| challenge.retry) =>
            {
                Severity::Warning
            }
            "known_host_verified"
            | "host_key_accepted"
            | "host_key_accepted_for_session"
            | "auth_success"
            | "connected"
            | "mosh_network_restored" => Severity::Success,
            "host_key_changed"
            | "host_key_rejected"
            | "host_key_save_failed"
            | "connection_lost"
            | "error"
            | "auth_failed" => Severity::Error,
            "host_key_unknown"
            | "known_host_store_missing"
            | "auth_passphrase_required"
            | "auth_agent_unavailable"
            | "retry"
            | "mosh_network_switching"
            | "mosh_network_degraded" => Severity::Warning,
            kind if kind.starts_with("auth_") && kind.ends_with("_rejected") => Severity::Warning,
            kind if kind.starts_with("auth_") && kind.ends_with("_failed") => Severity::Error,
            _ => Severity::Info,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intervention {
    Username,
    HostKey,
    Passphrase,
    SecurityKeyPin,
    Password,
    PublicKey,
    VerificationCode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct InterventionPage {
    step: Intervention,
    challenge_id: Option<u64>,
    request_id: Option<u64>,
}

#[derive(Clone, Default)]
pub struct ProgressHistory {
    pub steps: Vec<Intervention>,
    page: Option<InterventionPage>,
}

impl ProgressHistory {
    fn observe(&mut self, page: InterventionPage, segmented: bool, retry: bool) {
        if let Some(previous) = self.page {
            if previous.step == page.step
                && (previous.challenge_id == page.challenge_id
                    || retry && previous.challenge_id.is_some())
            {
                self.page = Some(page);
                return;
            }
            // Password and Public Key are two choices on one credential form.
            // A different request (or any interactive challenge) is another page.
            if segmented
                && page.challenge_id.is_none()
                && previous.challenge_id.is_none()
                && previous.request_id == page.request_id
                && matches!(
                    previous.step,
                    Intervention::Password | Intervention::PublicKey
                )
                && let Some(last) = self.steps.last_mut()
            {
                *last = page.step;
                self.page = Some(page);
                return;
            }
        }
        self.steps.push(page.step);
        self.page = Some(page);
    }
}

#[derive(Clone, Default)]
pub struct ConnectionState {
    pub phase: Phase,
    pub logs: Vec<LogEntry>,
    pub fingerprint: Option<String>,
    pub message: String,
    pub changed_key: bool,
    pub progress: ProgressHistory,
    pub latency_ms: Option<u32>,
    pub challenge: Option<nauterm_core::ssh::KeyboardInteractiveChallenge>,
    pub credentials: Option<(u64, Vec<String>)>,
}

impl ConnectionState {
    pub fn observe_intervention(&mut self, step: Intervention, segmented: bool) {
        self.progress.observe(
            InterventionPage {
                step,
                challenge_id: self.challenge.as_ref().map(|challenge| challenge.id),
                request_id: self.credentials.as_ref().map(|(id, _)| *id),
            },
            segmented,
            self.challenge
                .as_ref()
                .is_some_and(|challenge| challenge.retry),
        );
    }

    fn key_unlock_request(&self) -> Option<Intervention> {
        if self.phase != Phase::Authentication {
            return None;
        }
        for log in self.logs.iter().rev() {
            match log.event.kind.as_str() {
                "auth_passphrase_required" => {
                    return Some(if log.event.method.as_deref() == Some("fido2") {
                        Intervention::SecurityKeyPin
                    } else {
                        Intervention::Passphrase
                    });
                }
                "auth_credentials_required"
                | "auth_interactive_request"
                | "connect_start"
                | "auth_success"
                | "connected" => return None,
                _ => {}
            }
        }
        None
    }
    pub fn requires_key_passphrase(&self) -> bool {
        self.key_unlock_request() == Some(Intervention::Passphrase)
    }
    pub fn requires_fido2_pin(&self) -> bool {
        self.key_unlock_request() == Some(Intervention::SecurityKeyPin)
    }
    pub fn latest_fido2_failure(&self) -> bool {
        self.phase == Phase::Authentication
            && self
                .logs
                .iter()
                .rev()
                .find(|log| log.event.kind.starts_with("auth_"))
                .is_some_and(|log| log.event.method.as_deref() == Some("fido2"))
    }
    pub fn begin_attempt(&mut self) {
        self.phase = Phase::Connecting;
        self.changed_key = false;
        self.message.clear();
        self.fingerprint = None;
        self.latency_ms = None;
        self.challenge = None;
        self.credentials = None;
    }

    pub fn apply(&mut self, event: SessionEvent) {
        let category = if event.kind.starts_with("auth_agent_") {
            "agent"
        } else if event.kind.starts_with("auth_") {
            "auth"
        } else if event.kind.contains("host_key") || event.kind.starts_with("known_host") {
            "host-key"
        } else if matches!(
            event.kind.as_str(),
            "connected" | "ssh_latency_updated" | "exit_status" | "session_closed"
        ) {
            "session"
        } else {
            "transport"
        };
        if let Some(id) = event.authentication_request_id {
            self.credentials = Some((id, event.authentication_methods.clone().unwrap_or_default()));
            self.challenge = None;
        }
        match event.kind.as_str() {
            "auth_interactive_request" if event.challenge.is_some() => {
                self.credentials = None;
                self.challenge = event.challenge.clone();
                self.phase = Phase::Authentication;
                self.message = event.message.clone();
            }
            "auth_interactive_submitted" | "auth_credentials_submitted" => {
                self.challenge = None;
                self.credentials = None;
                self.phase = Phase::Connecting;
            }
            "ssh_latency_updated" | "mosh_latency_updated" => self.latency_ms = event.latency_ms,
            "host_key_unknown" => {
                self.phase = Phase::HostKey;
                self.fingerprint = event.fingerprint.clone();
                self.message = event.message.clone();
            }
            "host_key_changed"
            | "host_key_rejected"
            | "host_key_save_failed"
            | "known_host_store_missing" => {
                self.phase = Phase::Failed;
                self.changed_key = true;
                self.fingerprint = event.fingerprint.clone();
                self.message = event.message.clone();
            }
            "auth_credentials_required" | "auth_failed" => {
                self.phase = Phase::Authentication;
                self.message = event.message.clone();
            }
            "auth_passphrase_required" if event.authentication_request_id.is_some() => {
                self.phase = Phase::Authentication;
                self.message = event.message.clone();
            }
            "auth_success" => {
                self.challenge = None;
                self.credentials = None;
                self.phase = Phase::Connecting;
            }
            "connected" => {
                self.phase = Phase::Completing;
                self.message = "Connected".into();
            }
            "session_closed" | "exit_status" => {
                if !matches!(
                    self.phase,
                    Phase::HostKey | Phase::Authentication | Phase::Failed
                ) {
                    self.phase = Phase::Exited;
                    self.message = event.message.clone();
                }
            }
            "error" if self.challenge.is_some() || self.credentials.is_some() => {
                self.challenge = None;
                self.credentials = None;
                self.phase = Phase::Failed;
                self.message = event.message.clone();
            }
            "error"
                if !matches!(self.phase, Phase::HostKey | Phase::Authentication)
                    && !self.changed_key =>
            {
                // A transport without live authentication requests (such as
                // an initial SFTP listing) can still retry an actionable error.
                if self.phase == Phase::Connecting
                    && let Some(message) = self.auth_error_for_retry()
                {
                    self.phase = Phase::Authentication;
                    self.message = message;
                } else {
                    self.phase = Phase::Failed;
                    self.message = event.message.clone();
                }
            }
            _ => {}
        }
        self.logs.push(LogEntry {
            sequence: self
                .logs
                .last()
                .map_or(0, |last| last.sequence.saturating_add(1)),
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            category,
            event,
        });
        // Bound retained diagnostics without silently growing over long sessions.
        if self.logs.len() > 2000 {
            self.logs.drain(..self.logs.len() - 2000);
        }
    }

    fn auth_error_for_retry(&self) -> Option<String> {
        let mut agent = None;
        for log in self.logs.iter().rev() {
            match log.event.kind.as_str() {
                "auth_interactive_start"
                | "auth_interactive_failed"
                | "auth_success"
                | "auth_partial_success"
                | "connect_start" => break,
                "auth_agent_unavailable" | "auth_agent_failed" => {
                    agent.get_or_insert(&log.event.message);
                }
                "auth_passphrase_required"
                | "auth_password_rejected"
                | "auth_password_failed"
                | "auth_key_rejected"
                | "auth_key_failed"
                | "auth_certificate_rejected"
                | "auth_certificate_failed" => return Some(log.event.message.clone()),
                _ => {}
            }
        }
        agent.cloned()
    }

    pub fn diagnostics(&self) -> String {
        self.logs
            .iter()
            .map(|entry| {
                format!(
                    "{}  {}  {}",
                    entry.time, entry.category, entry.event.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn fail(&mut self, message: String) {
        self.apply(SessionEvent {
            kind: "error".into(),
            message,
            ..Default::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_pin_prompt_is_distinct_from_file_passphrase_and_resets_on_new_attempt() {
        let mut state = ConnectionState::default();
        let mut required = event("auth_passphrase_required");
        required.method = Some("fido2".into());
        required.authentication_request_id = Some(1);
        state.apply(required.clone());
        assert!(state.requires_fido2_pin());
        state.begin_attempt();
        assert!(!state.requires_fido2_pin());
        state.apply(event("connect_start"));
        let mut file_passphrase = required.clone();
        file_passphrase.method = Some("publickey".into());
        state.apply(file_passphrase);
        assert!(!state.requires_fido2_pin());
        state.apply(required);
        state.apply(event("connected"));
        assert!(!state.requires_fido2_pin());
    }
    fn event(kind: &str) -> SessionEvent {
        SessionEvent {
            kind: kind.into(),
            message: format!("{kind} message"),
            fingerprint: Some("SHA256:test".into()),
            ..Default::default()
        }
    }

    #[test]
    fn remote_latency_is_transient_and_resets_with_a_new_attempt() {
        for kind in ["ssh_latency_updated", "mosh_latency_updated"] {
            let mut state = ConnectionState::default();
            let sample: SessionEvent = serde_json::from_value(serde_json::json!({
                "kind":kind, "latency_ms":23
            }))
            .unwrap();
            state.apply(sample);
            assert_eq!(state.latency_ms, Some(23));
            state.begin_attempt();
            assert_eq!(state.latency_ms, None);
        }
    }

    #[test]
    fn overview_status_waits_for_transport_success_and_preserves_failure_on_exit() {
        for phase in [Phase::Connecting, Phase::HostKey, Phase::Authentication] {
            assert_eq!(phase.monitor_label(), "Connecting");
        }
        assert_eq!(Phase::Completing.monitor_label(), "Live");
        assert_eq!(
            Phase::Completing.with_pty_exit(true).monitor_label(),
            "Exited"
        );
        assert_eq!(Phase::Failed.with_pty_exit(true).monitor_label(), "Failed");
        assert_eq!(Phase::Exited.with_pty_exit(false).monitor_label(), "Exited");
        assert_eq!(Phase::HostKey.with_pty_exit(true), Phase::HostKey);
        assert_eq!(
            Phase::Authentication.with_pty_exit(true),
            Phase::Authentication
        );
    }

    #[test]
    fn host_key_intervention_survives_generic_worker_failure() {
        let mut state = ConnectionState::default();
        state.apply(event("host_key_unknown"));
        state.apply(event("error"));
        assert_eq!(state.phase, Phase::HostKey);
        assert_eq!(state.fingerprint.as_deref(), Some("SHA256:test"));
    }
    #[test]
    fn transport_and_host_key_failures_survive_following_close_notifications() {
        for failure in [
            "error",
            "host_key_changed",
            "host_key_rejected",
            "host_key_save_failed",
        ] {
            let mut state = ConnectionState::default();
            state.apply(event(failure));
            let message = state.message.clone();
            state.apply(event("exit_status"));
            state.apply(event("session_closed"));
            assert_eq!(state.phase, Phase::Failed, "{failure}");
            assert_eq!(state.message, message, "{failure}");
            assert_eq!(state.logs.len(), 3);
        }
    }
    #[test]
    fn connected_session_close_is_an_exit_with_its_native_message() {
        let mut state = ConnectionState::default();
        state.apply(event("connected"));
        state.apply(event("exit_status"));
        state.apply(event("session_closed"));
        assert_eq!(state.phase, Phase::Exited);
        assert_eq!(state.message, "session_closed message");
        assert!(!state.changed_key);
    }
    #[test]
    fn changed_key_is_not_an_accept_once_challenge() {
        let mut state = ConnectionState::default();
        state.apply(event("host_key_changed"));
        state.apply(event("error"));
        assert_eq!(state.phase, Phase::Failed);
        assert!(state.changed_key);
        assert!(state.message.starts_with("host_key_changed"));
    }
    fn credentials(id: u64, methods: &[&str]) -> SessionEvent {
        let mut request = event("auth_credentials_required");
        request.authentication_request_id = Some(id);
        request.authentication_methods =
            Some(methods.iter().map(|method| method.to_string()).collect());
        request
    }

    fn challenge(id: u64, prompt: &str) -> SessionEvent {
        let mut request = event("auth_interactive_request");
        request.challenge = Some(
            serde_json::from_value(serde_json::json!({
                "id": id, "name": "", "instructions": "",
                "prompts": [{"prompt": prompt, "echo": false}]
            }))
            .unwrap(),
        );
        request
    }

    #[test]
    fn progress_follows_visible_forms_and_keeps_every_challenge_round() {
        let mut state = ConnectionState::default();
        state.observe_intervention(Intervention::Username, false);
        state.begin_attempt();
        state.apply(event("host_key_unknown"));
        state.observe_intervention(Intervention::HostKey, false);
        state.begin_attempt();
        state.apply(event("auth_agent_unavailable"));
        assert_eq!(state.phase, Phase::Connecting);

        state.apply(credentials(1, &["password"]));
        state.observe_intervention(Intervention::Password, false);
        state.apply(event("auth_credentials_submitted"));
        state.apply(credentials(2, &["publickey"]));
        state.observe_intervention(Intervention::PublicKey, false);
        state.apply(event("auth_credentials_submitted"));

        for id in [3, 4] {
            state.apply(challenge(id, "Verification code:"));
            assert_eq!(state.challenge.as_ref().unwrap().id, id);
            assert!(state.credentials.is_none());
            state.observe_intervention(Intervention::VerificationCode, false);
            // Renders, logs toggles and duplicate delivery keep the same node.
            state.apply(challenge(id, "Verification code:"));
            state.observe_intervention(Intervention::VerificationCode, false);
            state.apply(event("auth_interactive_submitted"));
        }
        assert_eq!(
            state.progress.steps,
            [
                Intervention::Username,
                Intervention::HostKey,
                Intervention::Password,
                Intervention::PublicKey,
                Intervention::VerificationCode,
                Intervention::VerificationCode,
            ]
        );
        state.apply(event("auth_success"));
        assert!(state.challenge.is_none());
    }

    #[test]
    fn switching_auth_segments_and_retrying_credentials_keep_the_current_node() {
        let mut state = ConnectionState::default();
        state.apply(credentials(1, &["password", "publickey"]));
        state.observe_intervention(Intervention::Password, true);
        state.observe_intervention(Intervention::PublicKey, true);
        assert_eq!(state.progress.steps, [Intervention::PublicKey]);
        state.apply(credentials(2, &["publickey"]));
        state.observe_intervention(Intervention::PublicKey, false);
        assert_eq!(state.progress.steps, [Intervention::PublicKey]);
        state.apply(challenge(3, "Password:"));
        state.observe_intervention(Intervention::Password, false);
        assert_eq!(
            state.progress.steps,
            [Intervention::PublicKey, Intervention::Password]
        );
    }

    #[test]
    fn key_probe_logs_do_not_add_a_step_before_an_unlock_request() {
        let mut state = ConnectionState::default();
        let mut request = event("auth_passphrase_required");
        request.method = Some("public_key".into());
        state.apply(request.clone());
        assert_eq!(state.phase, Phase::Connecting);
        assert!(state.progress.steps.is_empty());
        request.authentication_request_id = Some(1);
        request.authentication_methods = Some(vec!["publickey".into()]);
        state.apply(request);
        assert!(state.requires_key_passphrase());
        state.observe_intervention(Intervention::Passphrase, false);
        assert_eq!(state.progress.steps, [Intervention::Passphrase]);
    }

    #[test]
    fn retrying_a_visible_interactive_step_reuses_its_node() {
        let mut state = ConnectionState::default();
        state.apply(challenge(1, "OTP:"));
        state.observe_intervention(Intervention::VerificationCode, false);
        state.apply(event("auth_interactive_submitted"));

        let mut retry = challenge(2, "OTP:");
        retry.challenge.as_mut().unwrap().retry = true;
        state.apply(retry);
        state.observe_intervention(Intervention::VerificationCode, false);
        assert_eq!(state.progress.steps, [Intervention::VerificationCode]);
        assert_eq!(state.logs.last().unwrap().severity(), Severity::Warning);

        state.apply(challenge(3, "OTP:"));
        state.observe_intervention(Intervention::VerificationCode, false);
        assert_eq!(
            state.progress.steps,
            [
                Intervention::VerificationCode,
                Intervention::VerificationCode
            ]
        );
    }

    #[test]
    fn rejected_otp_does_not_fall_back_to_an_older_password_prompt() {
        let mut state = ConnectionState::default();
        state.apply(event("auth_password_rejected"));
        state.apply(event("auth_interactive_start"));
        state.apply(challenge(1, "OTP:"));
        state.apply(event("auth_interactive_submitted"));
        state.apply(event("auth_interactive_failed"));
        state.apply(event("error"));
        assert_eq!(state.phase, Phase::Failed);
        assert!(state.challenge.is_none());
    }

    #[test]
    fn auth_rejection_remains_actionable_and_success_requires_connected_event() {
        let mut state = ConnectionState::default();
        state.apply(event("auth_failed"));
        state.apply(event("error"));
        assert_eq!(state.phase, Phase::Authentication);
        state.begin_attempt();
        state.apply(event("auth_success"));
        assert_eq!(state.phase, Phase::Connecting);
        state.apply(event("connected"));
        assert_eq!(state.phase, Phase::Completing);
    }

    #[test]
    fn agent_unavailable_is_actionable_without_auth_failed_event() {
        let mut state = ConnectionState::default();
        state.apply(event("auth_password_rejected"));
        state.apply(event("auth_agent_unavailable"));
        state.apply(event("error"));
        assert_eq!(state.phase, Phase::Authentication);
        assert!(state.message.starts_with("auth_password_rejected"));
        state.apply(event("auth_success"));
        assert_eq!(state.phase, Phase::Connecting);
    }
    #[test]
    fn copied_logs_have_explicit_line_and_field_separators() {
        let mut state = ConnectionState::default();
        state.apply(event("connect_start"));
        state.apply(event("error"));
        let text = state.diagnostics();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("  transport  connect_start message\n"));
    }
    #[test]
    fn log_categories_and_severity_match_the_connection_reference() {
        let mut state = ConnectionState::default();
        for (kind, category, severity) in [
            ("connect_start", "transport", Severity::Info),
            ("host_key_unknown", "host-key", Severity::Warning),
            ("known_host_verified", "host-key", Severity::Success),
            ("host_key_changed", "host-key", Severity::Error),
            ("auth_agent_unavailable", "agent", Severity::Warning),
            ("auth_agent_failed", "agent", Severity::Error),
            ("auth_certificate_rejected", "auth", Severity::Warning),
            ("auth_success", "auth", Severity::Success),
            ("connected", "session", Severity::Success),
            ("ssh_latency_updated", "session", Severity::Info),
        ] {
            state.apply(event(kind));
            let entry = state.logs.last().unwrap();
            assert_eq!(entry.category, category, "{kind}");
            assert_eq!(entry.severity(), severity, "{kind}");
        }
    }
    #[test]
    fn bounded_log_revisions_keep_advancing_across_retries_and_identical_events() {
        let mut state = ConnectionState::default();
        for _ in 0..2002 {
            state.apply(event("connect_start"));
        }
        assert_eq!(state.logs.len(), 2000);
        assert_eq!(state.logs.first().unwrap().sequence, 2);
        assert_eq!(state.logs.last().unwrap().sequence, 2001);
        state.begin_attempt();
        state.apply(event("connect_start"));
        assert_eq!(state.logs.first().unwrap().sequence, 3);
        assert_eq!(state.logs.last().unwrap().sequence, 2002);
    }
}
