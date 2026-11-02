//! The clock is a pure state machine; only the UI driver dispatches work.
use super::*;
use std::time::{Duration, Instant};

pub struct Clock {
    preferences: Preferences,
    deadline: Option<Instant>,
}
impl Clock {
    pub fn new(preferences: Preferences, now: Instant) -> Self {
        Self {
            preferences,
            deadline: preferences
                .automatic
                .then(|| now + Duration::from_millis(preferences.interval)),
        }
    }
    pub fn configure(&mut self, preferences: Preferences, now: Instant) {
        if self.preferences.automatic != preferences.automatic
            || self.preferences.interval != preferences.interval
        {
            *self = Self::new(preferences, now);
        } else {
            self.preferences = preferences;
        }
    }
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }
    pub fn due(&self, now: Instant) -> bool {
        self.remaining(now).is_some_and(|delay| delay.is_zero())
    }
    /// Success, failure and an unconfigured provider all wait a full interval.
    pub fn finished(&mut self, now: Instant) {
        *self = Self::new(self.preferences, now);
    }
}

pub struct Attempt {
    pub outcome: Result<Option<SyncSummary>, String>,
    pub overview: Result<Overview, String>,
}
impl Attempt {
    pub fn needs_refresh(&self) -> bool {
        !matches!(self.outcome, Ok(None))
    }
}
impl Service {
    pub async fn sync_automatically(&self, preferences: Preferences) -> Result<Attempt> {
        preferences.validate()?;
        ensure!(preferences.automatic, "Automatic sync is disabled.");
        self.vault
            .dispatch(move |db| {
                let outcome = db
                    .inner
                    .sync_automatically(
                        preferences.merge_strategy.into(),
                        usize::from(preferences.backup_count),
                    )
                    .map_err(|error| error.to_string())
                    .and_then(|value| {
                        value
                            .map(serde_json::from_value)
                            .transpose()
                            .map_err(|error| error.to_string())
                    });
                Ok(Attempt {
                    outcome,
                    overview: overview(&mut db.inner).map_err(|error| error.to_string()),
                })
            })
            .await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enabling_startup_and_interval_changes_wait_without_starting_immediately() {
        let now = Instant::now();
        let mut preferences = Preferences {
            automatic: true,
            interval: 300000,
            ..Default::default()
        };
        let mut clock = Clock::new(preferences, now);
        assert!(!clock.due(now));
        assert!(clock.due(now + Duration::from_millis(300000)));
        preferences.interval = 600000;
        clock.configure(preferences, now + Duration::from_millis(300000));
        assert!(!clock.due(now + Duration::from_millis(300000)));
        assert!(clock.due(now + Duration::from_millis(900000)));
        preferences.automatic = false;
        clock.configure(preferences, now);
        assert_eq!(clock.remaining(now), None);
        preferences.automatic = true;
        clock.configure(preferences, now);
        assert!(!clock.due(now));
    }
    #[test]
    fn non_cadence_edits_keep_the_deadline_and_completion_restarts_from_the_end() {
        let now = Instant::now();
        let mut preferences = Preferences {
            automatic: true,
            interval: 300000,
            ..Default::default()
        };
        let mut clock = Clock::new(preferences, now);
        preferences.backup_count = 31;
        preferences.merge_strategy = Strategy::LocalWins;
        clock.configure(preferences, now + Duration::from_millis(200000));
        assert!(clock.due(now + Duration::from_millis(300000)));
        clock.finished(now + Duration::from_millis(400000));
        assert!(!clock.due(now + Duration::from_millis(699999)));
        assert!(clock.due(now + Duration::from_millis(700000)));
    }
    #[test]
    fn minutes_and_original_milliseconds_have_one_bounded_policy() -> Result<()> {
        let mut preferences = Preferences::default();
        for value in ["0", "4", "10081", "1.5", "-5", "not-a-number"] {
            assert!(preferences.set_minutes(value).is_err());
        }
        preferences.set_minutes(" 5 ")?;
        assert_eq!(preferences.interval, 300000);
        assert_eq!(preferences.minutes(), 5);
        preferences.set_minutes("10080")?;
        assert_eq!(preferences.interval, Preferences::MAX_INTERVAL);
        preferences.validate()?;
        Ok(())
    }
}
