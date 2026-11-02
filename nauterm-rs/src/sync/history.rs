//! Remote descriptors remain bound to their native provider stamp. They never
//! become config/recovery data or a provider-independent version identifier.
use super::*;
pub use nauterm_core::{SyncHistoryVersion as Version, SyncProviderHistory as Snapshot};
use std::sync::Arc;

#[derive(Clone)]
pub struct RestorePlan {
    pub snapshot: Arc<Snapshot>,
    pub version: Version,
    pub backup_count: u16,
}
impl RestorePlan {
    pub fn new(snapshot: Arc<Snapshot>, version: Version, backup_count: u16) -> Result<Self> {
        ensure!(backup_count <= 100, "Keep between 0 and 100 local backups.");
        ensure!(
            snapshot.can_restore,
            "Complete a sync on this device before restoring cloud history."
        );
        ensure!(
            !version.current && snapshot.versions.contains(&version),
            "Refresh the history before choosing this version."
        );
        Ok(Self {
            snapshot,
            version,
            backup_count,
        })
    }
}
pub fn short_id(version: &Version) -> String {
    version.version_id.chars().take(12).collect()
}
pub fn date(version: &Version) -> String {
    let Some(timestamp) = version
        .timestamp
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return "No timestamp".into();
    };
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|_| timestamp.into())
}
impl Service {
    pub async fn history(&self, scope: String) -> Result<Arc<Snapshot>> {
        self.vault
            .dispatch(move |db| {
                db.inner
                    .sync_provider_history(&scope)
                    .map(Arc::new)
                    .map_err(|error| anyhow::anyhow!(error.to_string()))
            })
            .await
    }
    pub async fn restore_version(&self, plan: RestorePlan) -> Result<SyncAttempt> {
        ensure!(
            plan.backup_count <= 100,
            "Keep between 0 and 100 local backups."
        );
        self.vault
            .dispatch(move |db| {
                let outcome = db
                    .inner
                    .restore_sync_history_version(
                        &plan.snapshot,
                        &plan.version,
                        usize::from(plan.backup_count),
                    )
                    .map_err(|error| error.to_string())
                    .and_then(|value| {
                        serde_json::from_value(value).map_err(|error| error.to_string())
                    });
                let overview = overview(&mut db.inner).map_err(|error| error.to_string());
                Ok(SyncAttempt { outcome, overview })
            })
            .await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_labels_keep_unknown_dates_and_shorten_unicode_ids_at_character_boundaries() {
        let mut version = Version {
            version_id: "opaque/版本标识+long-identifier".into(),
            timestamp: None,
            message: String::new(),
            author: String::new(),
            etag: String::new(),
            bytes: None,
            current: false,
        };
        assert_eq!(short_id(&version).chars().count(), 12);
        assert!(version.version_id.starts_with(&short_id(&version)));
        assert_eq!(date(&version), "No timestamp");
        version.timestamp = Some("provider-specific date".into());
        assert_eq!(date(&version), "provider-specific date");
        version.timestamp = Some("2026-10-27T01:02:03Z".into());
        assert!(date(&version).starts_with("2026-10-"));
        assert_eq!(date(&version).len(), 19);
        assert!(!date(&version).contains('T'));
    }
}
