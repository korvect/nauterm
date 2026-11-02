//! Shared sync metadata and encrypted local backups. Background IO is opt-in.
use crate::vault::Vault;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub mod history;
pub mod keys;
pub mod oauth;
pub mod provider;
pub mod scheduler;

pub fn validate_master_key(value: &str) -> Result<()> {
    ensure!(
        value.chars().count() >= 12,
        "Master Key must contain at least 12 characters."
    );
    ensure!(
        value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)),
        "Master Key contains unsupported characters."
    );
    ensure!(
        master_key_classes(value)
            .into_iter()
            .filter(|present| *present)
            .count()
            >= 3,
        "Master Key must include at least 3 character types: uppercase letters, lowercase letters, numbers, and symbols."
    );
    Ok(())
}
pub fn master_key_classes(value: &str) -> [bool; 4] {
    [
        value.bytes().any(|byte| byte.is_ascii_uppercase()),
        value.bytes().any(|byte| byte.is_ascii_lowercase()),
        value.bytes().any(|byte| byte.is_ascii_digit()),
        value
            .bytes()
            .any(|byte| (0x20..=0x7e).contains(&byte) && !byte.is_ascii_alphanumeric()),
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    #[default]
    SmartMerge,
    LocalWins,
    RemoteWins,
}
impl Strategy {
    pub const ALL: [Self; 3] = [Self::SmartMerge, Self::LocalWins, Self::RemoteWins];
    pub fn label(self) -> &'static str {
        match self {
            Self::SmartMerge => "Smart Merge",
            Self::LocalWins => "Local Overwrite Cloud",
            Self::RemoteWins => "Cloud Overwrite Local",
        }
    }
}
impl From<Strategy> for nauterm_core::SyncStrategy {
    fn from(value: Strategy) -> Self {
        match value {
            Strategy::SmartMerge => Self::SmartMerge,
            Strategy::LocalWins => Self::LocalWins,
            Strategy::RemoteWins => Self::RemoteWins,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub merge_strategy: Strategy,
    pub backup_count: u16,
    pub automatic: bool,
    /// Original config unit: milliseconds, independent of the minutes control.
    pub interval: u64,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            merge_strategy: Strategy::SmartMerge,
            backup_count: 10,
            automatic: false,
            interval: 3 * 24 * 60 * 60 * 1000,
        }
    }
}
impl Preferences {
    pub const MIN_INTERVAL: u64 = 5 * 60 * 1000;
    pub const MAX_INTERVAL: u64 = 7 * 24 * 60 * 60 * 1000;
    pub fn minutes(self) -> u64 {
        self.interval / 60_000
    }
    pub fn set_minutes(&mut self, value: &str) -> Result<()> {
        let minutes: u64 = value
            .trim()
            .parse()
            .context("Enter a whole number of minutes.")?;
        ensure!(
            (5..=10080).contains(&minutes),
            "Sync interval must be between 5 and 10080 minutes."
        );
        self.interval = minutes * 60_000;
        Ok(())
    }
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.backup_count <= 100,
            "Keep between 0 and 100 local backups."
        );
        ensure!(
            (Self::MIN_INTERVAL..=Self::MAX_INTERVAL).contains(&self.interval),
            "Sync interval must be between 5 and 10080 minutes."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Backup {
    pub id: String,
    pub created_at: i64,
    pub bytes: u64,
}
impl Backup {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.id.starts_with("nauterm-local-backup-")
                && self.id.ends_with(".sqlite")
                && !self.id.contains(['/', '\\', '\0']),
            "Invalid local backup identifier."
        );
        Ok(())
    }
    pub fn date(&self) -> String {
        chrono::DateTime::from_timestamp_millis(self.created_at)
            .map(|date| {
                date.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_else(|| "Unknown date".into())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub connected: bool,
    pub supports_history: bool,
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Status {
    pub active_provider_id: Option<String>,
    pub sync_snapshot: Option<Version>,
    pub remote_revision: Option<u64>,
    pub remote_snapshot_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Version {
    pub revision: u64,
    pub snapshot_id: String,
}
#[derive(Clone, Default)]
pub struct Overview {
    pub providers: Vec<Provider>,
    pub status: Status,
    pub has_key: bool,
}
#[derive(Clone)]
pub struct SyncPlan {
    pub provider: Provider,
    pub preferences: Preferences,
}
impl Overview {
    pub fn sync_plan(&self, preferences: Preferences) -> Result<SyncPlan> {
        preferences.validate()?;
        let provider = self
            .providers
            .iter()
            .find(|provider| {
                Some(provider.id.as_str()) == self.status.active_provider_id.as_deref()
                    && provider.connected
            })
            .context("Select a configured provider before syncing.")?;
        Ok(SyncPlan {
            provider: provider.clone(),
            preferences,
        })
    }
}
#[derive(Deserialize)]
pub struct SyncSummary {
    pub revision: u64,
    pub imported_records: usize,
    pub total_records: usize,
}
impl SyncSummary {
    pub fn message(&self) -> String {
        self.message_for("Sync complete")
    }
    pub fn message_for(&self, action: &str) -> String {
        format!(
            "{action} · revision {} · imported {} · {} records.",
            self.revision, self.imported_records, self.total_records
        )
    }
}
pub struct SyncAttempt {
    pub outcome: Result<SyncSummary, String>,
    pub overview: Result<Overview, String>,
}
fn request<T: serde::de::DeserializeOwned>(
    database: &mut nauterm_core::NautermDatabase,
    value: Value,
) -> Result<T> {
    let value = database
        .handle_json_request(&value.to_string())
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(serde_json::from_value(value)?)
}
fn overview(database: &mut nauterm_core::NautermDatabase) -> Result<Overview> {
    database.atomic(|database| {
        let status = request(database, json!({"op":"sync_preferences"}))?;
        let key: Value = request(database, json!({"op":"sync_key_status"}))?;
        let mut providers = vec![];
        for (id, name, config_op, credential_op, flag) in [
            (
                "github_repository",
                "GitHub Repository",
                "github_load_config",
                "github_load_token",
                "has_token",
            ),
            (
                "github_gist",
                "GitHub Gist",
                "github_gist_load_config",
                "github_gist_load_token",
                "has_token",
            ),
            (
                "s3",
                "S3",
                "s3_load_config",
                "s3_load_credentials",
                "has_credentials",
            ),
        ] {
            let config: Value = request(database, json!({"op":config_op}))?;
            {
                let credentials: Value = request(database, json!({"op":credential_op}))?;
                providers.push(Provider {
                    id: id.into(),
                    name: name.into(),
                    kind: name.into(),
                    connected: !config.is_null()
                        && config.as_object().is_some_and(|value| !value.is_empty())
                        && credentials[flag] == true,
                    supports_history: true,
                });
            }
        }
        let cloud: Vec<Value> = request(database, json!({"op":"cloud_list_providers"}))?;
        providers.extend(cloud.iter().map(provider).collect::<Result<Vec<_>>>()?);
        Ok(Overview {
            providers,
            status,
            has_key: key["has_local_sync_key"] == true,
        })
    })
}
fn provider(value: &Value) -> Result<Provider> {
    let id = value["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .context("A sync provider has no identifier.")?;
    Ok(Provider {
        id: format!("cloud:{id}"),
        name: value["name"]
            .as_str()
            .unwrap_or("Cloud Provider")
            .to_owned(),
        kind: value["vendor"]
            .as_str()
            .or_else(|| value["scheme"].as_str())
            .unwrap_or("Cloud")
            .to_owned(),
        connected: value["has_credentials"] == true,
        supports_history: matches!(
            value["scheme"].as_str(),
            Some("s3" | "azblob" | "gcs" | "gdrive" | "onedrive" | "dropbox")
        ),
    })
}
#[derive(Clone)]
pub struct Service {
    vault: Vault,
}
impl Service {
    pub fn new(vault: Vault) -> Self {
        Self { vault }
    }
    pub async fn overview(&self) -> Result<Overview> {
        self.vault.dispatch(|db| overview(&mut db.inner)).await
    }
    pub async fn sync_now(
        &self,
        plan: SyncPlan,
        master_key: Option<zeroize::Zeroizing<String>>,
    ) -> Result<SyncAttempt> {
        plan.preferences.validate()?;
        if let Some(key) = master_key.as_ref() {
            validate_master_key(key)?;
        }
        self.vault
            .dispatch(move |db| {
                let outcome = db
                    .inner
                    .sync_active_provider(
                        &plan.provider.id,
                        master_key.as_ref().map(|key| key.as_str()),
                        plan.preferences.merge_strategy.into(),
                        usize::from(plan.preferences.backup_count),
                    )
                    .map_err(|error| error.to_string())
                    .and_then(|value| {
                        serde_json::from_value(value).map_err(|error| error.to_string())
                    });
                // The core can import records before a later upload fails. Always
                // return fresh local state, independently of the network outcome.
                let overview = overview(&mut db.inner).map_err(|error| error.to_string());
                Ok(SyncAttempt { outcome, overview })
            })
            .await
    }
    pub async fn activate(
        &self,
        expected: Option<String>,
        selected: Option<String>,
    ) -> Result<Overview> {
        self.vault
            .dispatch(move |db| {
                db.inner
                    .activate_sync_provider_checked(expected.as_deref(), selected.as_deref())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                overview(&mut db.inner)
            })
            .await
    }
    pub async fn backups(&self) -> Result<Vec<Backup>> {
        self.vault
            .dispatch(|db| {
                let backups = db
                    .inner
                    .local_sync_backups()
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                let backups: Vec<Backup> = serde_json::from_value(backups)?;
                for backup in &backups {
                    backup.validate()?;
                }
                Ok(backups)
            })
            .await
    }
    pub async fn restore(&self, selected: Backup, count: u16) -> Result<()> {
        selected.validate()?;
        ensure!(count <= 100, "Keep between 0 and 100 local backups.");
        self.vault
            .dispatch(move |db| {
                let current: Vec<Backup> = serde_json::from_value(
                    db.inner
                        .local_sync_backups()
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?,
                )?;
                ensure!(
                    current.contains(&selected),
                    "This backup changed or was removed. Refresh before restoring."
                );
                db.inner
                    .restore_local_sync_backup(&selected.id, usize::from(count))
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                Ok(())
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sync_plans_require_the_exact_configured_active_provider_and_keep_the_reviewed_preferences()
    -> Result<()> {
        let mut overview = Overview {
            providers: vec![Provider {
                id: "cloud:provider-test".into(),
                name: "Test".into(),
                kind: "webdav".into(),
                connected: true,
                supports_history: false,
            }],
            ..Default::default()
        };
        assert!(overview.sync_plan(Preferences::default()).is_err());
        overview.status.active_provider_id = Some("provider-test".into());
        assert!(overview.sync_plan(Preferences::default()).is_err());
        overview.status.active_provider_id = Some("cloud:provider-test".into());
        let preferences = Preferences {
            merge_strategy: Strategy::RemoteWins,
            backup_count: 23,
            ..Default::default()
        };
        let plan = overview.sync_plan(preferences)?;
        assert_eq!(plan.provider.id, "cloud:provider-test");
        assert_eq!(plan.preferences, preferences);
        overview.providers[0].connected = false;
        assert!(overview.sync_plan(preferences).is_err());
        overview.providers.clear();
        assert!(overview.sync_plan(preferences).is_err());
        Ok(())
    }
    #[test]
    fn native_sync_strategies_and_result_counts_keep_the_reference_meaning() -> Result<()> {
        for strategy in Strategy::ALL {
            assert_eq!(
                serde_json::to_value(strategy)?,
                serde_json::to_value(nauterm_core::SyncStrategy::from(strategy))?
            );
        }
        let summary: SyncSummary = serde_json::from_value(json!({
            "revision": 42, "imported_records": 7, "total_records": 18,
            "commit_sha":"ignored-transport-detail", "snapshot_id":"synthetic",
        }))?;
        assert_eq!(
            summary.message(),
            "Sync complete · revision 42 · imported 7 · 18 records."
        );
        assert!(serde_json::from_value::<SyncSummary>(json!({"revision":42})).is_err());
        Ok(())
    }
    #[test]
    fn master_key_validation_matches_the_reference_ascii_policy() {
        for key in [
            "shortAa1!",
            "abcdefghijkl",
            "ABCDEFGHIJKL",
            "abcdefghijkl1",
            "强".repeat(12).as_str(),
            "Abcdefghi1!\n",
        ] {
            assert!(validate_master_key(key).is_err());
        }
        assert!(validate_master_key("Abcdefghijkl1").is_ok());
        assert!(validate_master_key("abcdefghijkl ").is_err());
        assert!(
            validate_master_key("Abcdefghijkl ").is_ok(),
            "ASCII space is a symbol in the reference policy"
        );
        assert_eq!(master_key_classes("Aa1!"), [true; 4]);
    }
    #[test]
    fn backup_identifiers_and_limits_are_checked_without_opening_a_database() -> Result<()> {
        for id in [
            "../nauterm-local-backup-test.sqlite",
            "nauterm-local-backup-x/../x.sqlite",
            "nauterm-local-backup-x\\y.sqlite",
            "unrelated.sqlite",
        ] {
            assert!(
                Backup {
                    id: id.into(),
                    created_at: 0,
                    bytes: 1
                }
                .validate()
                .is_err()
            );
        }
        Backup {
            id: "nauterm-local-backup-sync-1.sqlite".into(),
            created_at: 0,
            bytes: 1,
        }
        .validate()?;
        assert!(
            Preferences {
                backup_count: 101,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Preferences {
                backup_count: 0,
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
        Ok(())
    }
    #[test]
    fn summaries_only_project_public_labels_and_credential_presence() -> Result<()> {
        let summary = provider(
            &json!({"id":"id","name":"Lab","vendor":"minio","has_credentials":true,
            "config":{"endpoint":"https://example.invalid","access_token":"synthetic-secret"}}),
        )?;
        assert_eq!(summary.id, "cloud:id");
        assert!(summary.connected);
        assert!(!format!("{summary:?}").contains("synthetic-secret"));
        assert_eq!(
            serde_json::to_value(Preferences::default())?,
            json!({"mergeStrategy":"smart_merge","backupCount":10,"automatic":false,"interval":259200000})
        );
        Ok(())
    }
    #[test]
    fn history_capability_follows_the_transport_scheme_instead_of_the_vendor_label() -> Result<()> {
        for scheme in [
            "s3", "azblob", "gcs", "gdrive", "onedrive", "dropbox", "webdav", "unknown",
        ] {
            let summary = provider(
                &json!({"id":"synthetic", "vendor":"custom-label", "scheme":scheme, "has_credentials":true}),
            )?;
            assert_eq!(
                summary.supports_history,
                !matches!(scheme, "webdav" | "unknown")
            );
        }
        Ok(())
    }
}
