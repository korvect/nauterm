//! Native Mosh events own reachability. Output, latency and UI activity do not.
use std::time::{Duration, Instant};

pub const RESTORED_DURATION: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Stable,
    Switching,
    Degraded,
    Restored,
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stable => "",
            Self::Switching => "Switching network…",
            Self::Degraded => "Connection interrupted · input queued",
            Self::Restored => "Connection restored",
        }
    }
}

#[derive(Default)]
pub struct Network {
    state: State,
    restored_until: Option<Instant>,
}
impl Network {
    pub fn state(&self) -> State {
        self.state
    }
    pub fn restored_until(&self) -> Option<Instant> {
        self.restored_until
    }
    pub fn apply(&mut self, kind: &str, now: Instant) -> bool {
        let next = match kind {
            "mosh_network_switching" => State::Switching,
            "mosh_network_degraded" => State::Degraded,
            "mosh_network_restored" => State::Restored,
            "connect_start" | "retry" | "exit_status" | "session_closed" | "connection_lost"
            | "error" => State::Stable,
            _ => return false,
        };
        let deadline = (next == State::Restored).then(|| now + RESTORED_DURATION);
        let changed = self.state != next || self.restored_until != deadline;
        self.state = next;
        self.restored_until = deadline;
        changed
    }
    pub fn expire(&mut self, now: Instant) -> bool {
        if self.restored_until.is_some_and(|deadline| now >= deadline) {
            self.state = State::Stable;
            self.restored_until = None;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reachability_changes_only_on_network_events_and_restored_expires() {
        let now = Instant::now();
        let mut network = Network::default();
        assert!(!network.apply("mosh_latency_updated", now));
        assert!(network.apply("mosh_network_switching", now));
        assert_eq!(network.state(), State::Switching);
        assert!(network.apply("mosh_network_degraded", now));
        for kind in [
            "connected",
            "mosh_udp_peer_confirmed",
            "mosh_screen_committed",
        ] {
            assert!(!network.apply(kind, now));
            assert_eq!(network.state(), State::Degraded);
        }
        assert!(network.apply("mosh_network_restored", now));
        assert_eq!(network.state(), State::Restored);
        assert!(!network.expire(now + RESTORED_DURATION - Duration::from_millis(1)));
        assert!(network.expire(now + RESTORED_DURATION));
        assert_eq!(network.state(), State::Stable);
        assert_eq!(network.restored_until(), None);
    }

    #[test]
    fn another_network_change_or_exit_invalidates_an_older_restored_deadline() {
        let now = Instant::now();
        for kind in [
            "mosh_network_switching",
            "mosh_network_degraded",
            "session_closed",
            "retry",
        ] {
            let mut network = Network::default();
            network.apply("mosh_network_restored", now);
            network.apply(kind, now + Duration::from_millis(500));
            let state = network.state();
            assert_eq!(network.restored_until(), None);
            assert!(!network.expire(now + RESTORED_DURATION));
            assert_eq!(network.state(), state);
        }
        let mut network = Network::default();
        network.apply("mosh_network_restored", now);
        network.apply("mosh_network_restored", now + Duration::from_secs(1));
        assert!(!network.expire(now + RESTORED_DURATION));
        assert_eq!(network.state(), State::Restored);
        assert!(network.expire(now + RESTORED_DURATION + Duration::from_secs(1)));
    }
}
