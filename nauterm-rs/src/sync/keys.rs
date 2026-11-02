//! Keys exist only in transient inputs and owned zeroizing operation plans.
use super::*;
pub use nauterm_core::{
    SyncKeyManagement as Management, SyncKeyPhase as Phase, SyncKeyProgress as Progress,
    SyncKeyRotationResult as Rotation, SyncKeyTarget as Target,
};
use std::sync::Arc;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Change {
    current: Zeroizing<String>,
    new: Zeroizing<String>,
}
impl Change {
    pub fn new(
        current: Zeroizing<String>,
        new: Zeroizing<String>,
        confirmation: Zeroizing<String>,
    ) -> Result<Self> {
        validate_master_key(&current).context("Current Master Key")?;
        validate_master_key(&new).context("New Master Key")?;
        ensure!(
            current.as_str() != new.as_str(),
            "New Master Key must be different from the current key."
        );
        ensure!(
            new.as_str() == confirmation.as_str(),
            "New Master Key confirmation does not match."
        );
        Ok(Self { current, new })
    }
}
#[derive(Clone)]
pub struct Plan {
    pub reviewed: Arc<Management>,
    pub change: Change,
    pub backup_count: u16,
}
pub struct Attempt {
    pub outcome: Result<Rotation, String>,
    pub overview: Result<Overview, String>,
    pub management: Result<Arc<Management>, String>,
}
pub struct Forgotten {
    pub overview: Result<Overview, String>,
}
impl Service {
    pub async fn key_management(&self) -> Result<Arc<Management>> {
        self.vault
            .dispatch(|db| {
                db.inner
                    .sync_key_management()
                    .map(Arc::new)
                    .map_err(|error| anyhow::anyhow!(error.to_string()))
            })
            .await
    }
    pub async fn forget_key(&self, expected: Arc<Management>) -> Result<Forgotten> {
        self.vault
            .dispatch(move |db| {
                db.inner
                    .forget_cached_sync_key(Some(&expected.state))
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                Ok(Forgotten {
                    overview: overview(&mut db.inner).map_err(|error| error.to_string()),
                })
            })
            .await
    }
    pub async fn rotate_key(
        &self,
        plan: Plan,
        progress: async_channel::Sender<Progress>,
    ) -> Result<Attempt> {
        ensure!(
            plan.backup_count <= 100,
            "Keep between 0 and 100 local backups."
        );
        self.vault
            .dispatch(move |db| {
                let outcome = db
                    .inner
                    .rotate_sync_master_key(
                        &plan.reviewed,
                        &plan.change.current,
                        &plan.change.new,
                        usize::from(plan.backup_count),
                        |event| {
                            let _ = progress.try_send(event);
                        },
                    )
                    .map_err(|error| error.to_string());
                Ok(Attempt {
                    outcome,
                    overview: overview(&mut db.inner).map_err(|error| error.to_string()),
                    management: db
                        .inner
                        .sync_key_management()
                        .map(Arc::new)
                        .map_err(|error| error.to_string()),
                })
            })
            .await
    }
}
pub fn completed_message(completed: &[Target]) -> String {
    if completed.is_empty() {
        "No provider has confirmed an update yet.".into()
    } else {
        format!(
            "Updated: {}.",
            completed
                .iter()
                .map(|target| target.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(value: &str) -> Zeroizing<String> {
        Zeroizing::new(value.into())
    }
    #[test]
    fn a_rotation_requires_distinct_valid_keys_and_a_matching_confirmation() {
        assert!(
            Change::new(
                key("Current-Key-12"),
                key("New-Secret-123"),
                key("New-Secret-123")
            )
            .is_ok()
        );
        for (old, new, confirm) in [
            ("too-short", "New-Secret-123", "New-Secret-123"),
            ("Current-Key-12", "New-Secret-123", "different"),
            ("Current-Key-12", "Current-Key-12", "Current-Key-12"),
            (
                "Current-Key-12",
                "unsupported-汉字-123",
                "unsupported-汉字-123",
            ),
        ] {
            assert!(Change::new(key(old), key(new), key(confirm)).is_err());
        }
    }
    #[test]
    fn partial_result_names_every_confirmed_provider() {
        assert!(completed_message(&[]).contains("No provider"));
        let message = completed_message(&[
            Target {
                scope: "first".into(),
                name: "Repository".into(),
            },
            Target {
                scope: "cloud:second".into(),
                name: "Archive".into(),
            },
        ]);
        assert!(message.contains("Repository, Archive"));
    }
}
