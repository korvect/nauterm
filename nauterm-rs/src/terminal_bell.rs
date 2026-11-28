//! Portable bell preferences and counter coalescing. No native window or I/O.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub sound: bool,
    pub visual: bool,
    pub tab_indicator: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            sound: true,
            visual: false,
            tab_indicator: true,
        }
    }
}

pub struct Counter {
    last: u64,
    pending: bool,
}
impl Counter {
    pub fn new(last: u64) -> Self {
        Self {
            last,
            pending: false,
        }
    }
    /// A counter reset is a new baseline, not a wrapped/lost burst of bells.
    /// Multiple observations before delivery share one notification.
    pub fn observe(&mut self, count: u64, enabled: bool) -> bool {
        let advanced = count > self.last;
        let reset = count < self.last;
        self.last = count;
        if !enabled || reset {
            self.pending = false;
            return false;
        }
        advanced && !std::mem::replace(&mut self.pending, true)
    }
    pub fn take(&mut self) -> bool {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::{Counter, Preferences};
    #[test]
    fn reference_defaults_and_partial_settings_round_trip() {
        let defaults = Preferences::default();
        assert!(defaults.sound && !defaults.visual && defaults.tab_indicator);
        let settings: Preferences =
            serde_json::from_value(serde_json::json!({"visual":true,"tabIndicator":false}))
                .unwrap();
        assert!(settings.sound && settings.visual && !settings.tab_indicator);
        assert_eq!(
            serde_json::to_value(settings).unwrap()["tabIndicator"],
            false
        );
    }
    #[test]
    fn native_bell_counters_coalesce_and_never_replay_old_or_recorded_bells() {
        let mut count = Counter::new(9);
        assert!(!count.observe(9, true));
        assert!(!count.take());
        assert!(count.observe(10, true));
        assert!(!count.observe(15, true));
        assert!(count.take());
        assert!(!count.take());
        assert!(!count.observe(15, true));
        assert!(!count.take());
        assert!(count.observe(16, true));
        assert!(!count.observe(0, true));
        assert!(!count.take());
        assert!(!count.observe(5, false));
        assert!(!count.take());
        assert!(!count.observe(5, true));
        assert!(count.observe(6, true));
        assert!(count.take());
    }
}
