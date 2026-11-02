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
    HostKey,
    Authentication,
}

#[derive(Clone, Default)]
pub struct ConnectionState {
    pub phase: Phase,
    pub logs: Vec<LogEntry>,
    pub fingerprint: Option<String>,
    pub message: String,
    pub changed_key: bool,
    pub interventions: Vec<Intervention>,
    pub latency_ms: Option<u32>,
}

impl ConnectionState {
    pub fn remember_intervention(&mut self, intervention: Intervention) {
        if !self.interventions.contains(&intervention) {
            self.interventions.push(intervention);
        }
    }
    pub fn requires_fido2_pin(&self) -> bool {
        if self.phase != Phase::Authentication {
            return false;
        }
        for log in self.logs.iter().rev() {
            match log.event.kind.as_str() {
                "auth_passphrase_required" => return log.event.method.as_deref() == Some("fido2"),
                "connect_start" | "auth_success" | "connected" => return false,
                _ => {}
            }
        }
        false
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
        match event.kind.as_str() {
            "ssh_latency_updated" | "mosh_latency_updated" => self.latency_ms = event.latency_ms,
            "host_key_unknown" => {
                self.phase = Phase::HostKey;
                self.remember_intervention(Intervention::HostKey);
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
            "auth_failed"
            | "auth_passphrase_required"
            | "auth_password_rejected"
            | "auth_password_failed"
            | "auth_key_rejected"
            | "auth_certificate_rejected"
            | "auth_agent_unavailable"
            | "auth_agent_failed"
            | "auth_key_failed"
            | "auth_certificate_failed" => {
                let preserve_rejection = event.kind.starts_with("auth_agent")
                    && self.phase == Phase::Authentication
                    && !self.message.is_empty();
                self.phase = Phase::Authentication;
                self.remember_intervention(Intervention::Authentication);
                if !preserve_rejection {
                    self.message = event.message.clone();
                }
            }
            "auth_success" => self.phase = Phase::Connecting,
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
            "error"
                if !matches!(self.phase, Phase::HostKey | Phase::Authentication)
                    && !self.changed_key =>
            {
                // The worker reports a transport error after rejecting a host key
                // or exhausted auth. Preserve the actionable intervention UI.
                self.phase = Phase::Failed;
                self.message = event.message.clone();
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
