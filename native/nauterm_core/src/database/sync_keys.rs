//! Opaque cache/provider snapshots for local key management. Master Keys stay
//! in caller-owned memory and are used only by the existing envelope rewrap.
use super::*;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct SyncKeyState {
    pub has_key: bool,
    stamp: [u8; 32],
}
impl SyncKeyState {
    pub(super) fn verify(&self, expected: &Self) -> Result<(), Box<dyn Error>> {
        if self.stamp != expected.stamp || self.has_key != expected.has_key {
            return Err(
                "The local sync key state changed. Reload its details before trying again.".into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncKeyTarget {
    pub scope: String,
    pub name: String,
}
#[derive(Clone)]
pub struct SyncKeyManagement {
    pub state: SyncKeyState,
    providers: Vec<SyncProviderRecord>,
}
impl SyncKeyManagement {
    pub fn targets(&self) -> Vec<SyncKeyTarget> {
        self.providers.iter().map(target).collect()
    }
    pub fn can_rotate(&self) -> bool {
        !self.providers.is_empty()
    }
    fn same_targets(&self, other: &Self) -> bool {
        self.providers.len() == other.providers.len()
            && self.providers.iter().all(|old| {
                other.providers.iter().any(|new| {
                    old.uuid == new.uuid
                        && old.provider == new.provider
                        && old.name == new.name
                        && old.config == new.config
                })
            })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncKeyPhase {
    Updating,
    Updated,
    Failed,
}
#[derive(Clone, Debug)]
pub struct SyncKeyProgress {
    pub target: SyncKeyTarget,
    pub phase: SyncKeyPhase,
}
pub struct SyncKeyRotationResult {
    pub completed: Vec<SyncKeyTarget>,
    pub failed: Option<(SyncKeyTarget, String)>,
    pub retry: Option<SyncKeyManagement>,
}
fn target(record: &SyncProviderRecord) -> SyncKeyTarget {
    SyncKeyTarget {
        scope: if builtin(&record.uuid) {
            record.uuid.clone()
        } else {
            format!("cloud:{}", record.uuid)
        },
        name: record.name.clone(),
    }
}
fn builtin(uuid: &str) -> bool {
    matches!(uuid, "github_repository" | "github_gist" | "s3")
}
fn connected(uuid: &str, has_credentials: bool, has_config: bool) -> bool {
    has_credentials && (!builtin(uuid) || has_config)
}
fn selected(records: Vec<SyncProviderRecord>) -> Vec<SyncProviderRecord> {
    let mut records: Vec<_> = records
        .into_iter()
        .filter(|record| {
            connected(
                &record.uuid,
                record.has_credentials,
                record
                    .config
                    .as_object()
                    .is_some_and(|config| !config.is_empty()),
            )
        })
        .collect();
    records.sort_by_key(|record| match record.uuid.as_str() {
        "github_repository" => 0,
        "github_gist" => 1,
        "s3" => 2,
        _ => 3,
    });
    records
}
fn fingerprint(rows: &[(String, Zeroizing<String>, i64)]) -> [u8; 32] {
    let mut sorted: Vec<_> = rows.iter().collect();
    sorted.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hash = Sha256::new();
    hash.update(b"nauterm-local-sync-key-state-v1");
    for (key, value, updated) in sorted {
        for field in [key.as_str(), value.as_str()] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field.as_bytes());
        }
        hash.update(updated.to_le_bytes());
    }
    hash.finalize().into()
}
impl NautermDatabase {
    pub fn sync_key_state(&self) -> Result<SyncKeyState, Box<dyn Error>> {
        let rows = self.connection.prepare(
            "SELECT key, value, updated_at FROM app_metadata
             WHERE key IN ('sync_dek', 'sync_vault_id', 'sync_envelope_header', 'sync_preferences_v1')
                OR key LIKE 'sync_revision:%' ORDER BY key"
        )?.query_map([], |row| Ok((row.get::<_, String>(0)?, Zeroizing::new(row.get::<_, String>(1)?), row.get::<_, i64>(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let present = |key: &str| rows.iter().any(|row| row.0 == key);
        Ok(SyncKeyState {
            has_key: present("sync_dek") && present("sync_envelope_header"),
            stamp: fingerprint(&rows),
        })
    }
    pub fn sync_key_management(&mut self) -> Result<SyncKeyManagement, Box<dyn Error>> {
        self.atomic(|database| {
            Ok(SyncKeyManagement {
                state: database.sync_key_state()?,
                providers: selected(database.list_sync_provider_records()?),
            })
        })
    }
    pub fn rotate_sync_master_key(
        &mut self,
        reviewed: &SyncKeyManagement,
        current: &str,
        new: &str,
        retention_count: usize,
        mut progress: impl FnMut(SyncKeyProgress),
    ) -> Result<SyncKeyRotationResult, Box<dyn Error>> {
        crate::crypto::validate_master_key(current)?;
        crate::crypto::validate_master_key(new)?;
        if current == new {
            return Err("New Master Key must be different from the current key.".into());
        }
        if retention_count > 100 {
            return Err("Keep between 0 and 100 local backups.".into());
        }
        if !reviewed.can_rotate() {
            return Err("Connect a sync provider before changing the Master Key.".into());
        }
        self.with_sync_operation(|database| {
            database.atomic(|database| -> Result<(), Box<dyn Error>> {
                let fresh = database.sync_key_management()?;
                fresh.state.verify(&reviewed.state)?;
                if !fresh.same_targets(reviewed) { return Err("The connected sync providers changed. Reload them before changing the Master Key.".into()); }
                for expected in &reviewed.providers {
                    let actual = fresh.providers.iter().find(|record| record.uuid == expected.uuid);
                    super::sync_providers::verify(actual, Some(expected))?;
                }
                Ok(())
            })?;
            database.create_local_sync_backup(retention_count)?;
            let mut completed = Vec::new();
            for provider in &reviewed.providers {
                let target = target(provider);
                progress(SyncKeyProgress { target: target.clone(), phase: SyncKeyPhase::Updating });
                match database.rotate_sync_provider_checked(provider, current, new) {
                    Ok(_) => {
                        progress(SyncKeyProgress { target: target.clone(), phase: SyncKeyPhase::Updated });
                        completed.push(target);
                    }
                    Err(error) => {
                        progress(SyncKeyProgress { target: target.clone(), phase: SyncKeyPhase::Failed });
                        let retry = database.sync_key_management().ok().filter(|fresh| fresh.same_targets(reviewed));
                        return Ok(SyncKeyRotationResult { completed, failed: Some((target, error.to_string())), retry });
                    }
                }
            }
            Ok(SyncKeyRotationResult { completed, failed: None, retry: None })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_state_stamps_detect_key_header_and_checkpoint_changes_without_exposing_values() {
        let rows = vec![
            (
                "sync_dek".into(),
                Zeroizing::new("synthetic-sealed-key".into()),
                1,
            ),
            (
                "sync_envelope_header".into(),
                Zeroizing::new("synthetic-header".into()),
                2,
            ),
        ];
        let state = SyncKeyState {
            has_key: true,
            stamp: fingerprint(&rows),
        };
        let reversed: Vec<_> = rows.iter().rev().cloned().collect();
        assert!(state
            .verify(&SyncKeyState {
                has_key: true,
                stamp: fingerprint(&reversed)
            })
            .is_ok());
        let mut changed = rows.clone();
        changed[1].1 = Zeroizing::new("changed-header".into());
        assert!(state
            .verify(&SyncKeyState {
                has_key: true,
                stamp: fingerprint(&changed)
            })
            .is_err());
        changed = rows.clone();
        changed[0].2 += 1;
        assert!(state
            .verify(&SyncKeyState {
                has_key: true,
                stamp: fingerprint(&changed)
            })
            .is_err());
        changed = rows.clone();
        changed.push((
            "sync_revision:synthetic".into(),
            Zeroizing::new("2".into()),
            3,
        ));
        assert!(state
            .verify(&SyncKeyState {
                has_key: true,
                stamp: fingerprint(&changed)
            })
            .is_err());
    }
    #[test]
    fn rotation_includes_every_connected_provider_independent_of_activation() {
        for uuid in ["github_repository", "github_gist", "s3"] {
            assert!(!connected(uuid, true, false));
            assert!(connected(uuid, true, true));
            assert!(!connected(uuid, false, true));
        }
        assert!(connected("inactive-cloud-provider", true, false));
        assert!(!connected("inactive-cloud-provider", false, true));
    }
}
