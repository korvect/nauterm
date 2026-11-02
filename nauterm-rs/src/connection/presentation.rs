//! Native outcome and the user's log toggle select different body/header modes.
//! In particular, an ordinary exit is a message until the user asks for logs.
use crate::connection_state::Phase;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Body {
    Protocol,
    Mode,
    Logs,
    Failure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HeaderAction {
    None,
    ToggleLogs,
    CopyLogs,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PageState {
    pub body: Body,
    pub header: HeaderAction,
    pub scroll_body: bool,
}
impl PageState {
    pub fn new(phase: Phase, choosing: bool, show_logs: bool) -> Self {
        let body = if choosing {
            Body::Protocol
        } else if phase == Phase::Failed {
            Body::Failure
        } else if show_logs {
            Body::Logs
        } else {
            Body::Mode
        };
        Self {
            body,
            header: match body {
                Body::Protocol => HeaderAction::None,
                Body::Failure => HeaderAction::CopyLogs,
                Body::Logs | Body::Mode => HeaderAction::ToggleLogs,
            },
            scroll_body: body == Body::Mode
                && matches!(
                    phase,
                    Phase::HostKey | Phase::Authentication | Phase::Exited
                ),
        }
    }
    pub fn shows_diagnostics(self) -> bool {
        matches!(self.body, Body::Logs | Body::Failure)
    }
    pub fn header_label(self, show_logs: bool) -> &'static str {
        match self.header {
            HeaderAction::CopyLogs => "Copy logs",
            HeaderAction::ToggleLogs if show_logs => "Hide logs",
            HeaderAction::ToggleLogs => "Show logs",
            HeaderAction::None => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Body, HeaderAction, PageState};
    use crate::connection_state::Phase;
    #[test]
    fn ordinary_exit_has_a_message_and_an_explicit_log_toggle() {
        let page = PageState::new(Phase::Exited, false, false);
        assert_eq!(page.body, Body::Mode);
        assert!(page.scroll_body);
        assert!(!page.shows_diagnostics());
        assert_eq!(page.header, HeaderAction::ToggleLogs);
        assert_eq!(page.header_label(false), "Show logs");
        let logs = PageState::new(Phase::Exited, false, true);
        assert_eq!(logs.body, Body::Logs);
        assert!(logs.shows_diagnostics());
        assert!(!logs.scroll_body);
        assert_eq!(logs.header_label(true), "Hide logs");
    }
    #[test]
    fn failure_always_keeps_its_diagnostics_and_copy_action() {
        for show_logs in [false, true] {
            let page = PageState::new(Phase::Failed, false, show_logs);
            assert_eq!(page.body, Body::Failure);
            assert!(page.shows_diagnostics());
            assert!(!page.scroll_body);
            assert_eq!(page.header_label(show_logs), "Copy logs");
        }
    }
    #[test]
    fn protocol_choice_does_not_show_transport_logs_or_an_action() {
        let page = PageState::new(Phase::Connecting, true, true);
        assert_eq!(page.body, Body::Protocol);
        assert!(!page.shows_diagnostics());
        assert_eq!(page.header, HeaderAction::None);
    }
}
