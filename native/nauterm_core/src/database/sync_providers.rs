//! Owned Rust sync-provider edits. Secrets never leave a record read; the
//! private fingerprint covers their stored representation for optimistic edits.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct SyncProviderRecord {
    pub uuid: String,
    pub provider: String,
    pub name: String,
    pub config: Value,
    pub has_credentials: bool,
    pub active: bool,
    fingerprint: [u8; 32],
}
#[derive(Clone, Default)]
pub struct SyncProviderCredentials {
    pub values: BTreeMap<String, Zeroizing<String>>,
}
impl std::fmt::Debug for SyncProviderCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncProviderCredentials")
            .field("field_count", &self.values.len())
            .finish_non_exhaustive()
    }
}
pub struct SyncProviderUpdate {
    pub uuid: String,
    pub provider: String,
    pub name: String,
    pub config: Value,
    /// None retains the saved credential. Supplied values replace it explicitly.
    pub credentials: Option<SyncProviderCredentials>,
}
impl SyncProviderUpdate {
    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        if !self.config.is_object() || self.config.to_string().len() > 256 * 1024 {
            return Err("Sync configuration must be a bounded JSON object.".into());
        }
        let required = match self.uuid.as_str() {
            "github_repository" => {
                if self.provider != "github_repository" {
                    return Err("Sync provider type changed.".into());
                }
                let config: crate::github_sync::GithubRepoConfig =
                    serde_json::from_value(self.config.clone())?;
                crate::github_sync::validate_repository_url(&config.repository_url)?;
                vec!["token"]
            }
            "github_gist" => {
                if self.provider != "github_gist" {
                    return Err("Sync provider type changed.".into());
                }
                let config: crate::github_gist_sync::GithubGistConfig =
                    serde_json::from_value(self.config.clone())?;
                if config.filename.trim().is_empty() || config.filename.contains(['/', '\\']) {
                    return Err(
                        "GitHub Gist filename must be a nonempty filename without path separators."
                            .into(),
                    );
                }
                vec!["token"]
            }
            "s3" => {
                if self.provider != "s3_compatible" {
                    return Err("Sync provider type changed.".into());
                }
                crate::s3_sync::validate_config(&serde_json::from_value(self.config.clone())?)?;
                vec!["access_key_id", "secret_access_key"]
            }
            _ => {
                let cloud = crate::cloud_sync::CloudProviderConfig {
                    id: self.uuid.clone(),
                    vendor: self.provider.clone(),
                    name: self.name.clone(),
                    scheme: self.config["scheme"].as_str().unwrap_or_default().into(),
                    config: serde_json::from_value(
                        self.config
                            .get("config")
                            .cloned()
                            .unwrap_or_else(|| json!({})),
                    )?,
                };
                crate::cloud_sync::validate_provider(&cloud)?;
                match cloud.scheme.as_str() {
                    "s3" => vec!["access_key_id", "secret_access_key"],
                    "webdav" => vec!["username", "password"],
                    "azblob" => vec!["account_name", "account_key"],
                    // OAuth authorization supplies refresh/client/access tokens;
                    // metadata-only edits retain the complete saved credential.
                    "gcs" | "gdrive" | "onedrive" | "dropbox" => vec!["client_id", "refresh_token"],
                    _ => return Err("Unsupported sync provider.".into()),
                }
            }
        };
        if self.name.trim().is_empty() || self.name.chars().count() > 80 {
            return Err("Provider name must contain 1 to 80 characters.".into());
        }
        if let Some(credentials) = &self.credentials {
            if credentials.values.is_empty()
                || credentials.values.len() > 16
                || credentials.values.iter().any(|(key, value)| {
                    key.is_empty()
                        || key.len() > 64
                        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        || value.trim().is_empty()
                        || value.len() > 16_384
                })
                || required
                    .iter()
                    .any(|key| !credentials.values.contains_key(*key))
            {
                return Err("Enter all required credential fields, or leave all blank to keep the saved credentials.".into());
            }
        }
        Ok(())
    }
}

fn fingerprint(fields: &[&str], credentials: Option<&str>, created: i64, updated: i64) -> [u8; 32] {
    let mut hash = Sha256::new();
    for field in fields
        .iter()
        .copied()
        .chain(std::iter::once(credentials.unwrap_or_default()))
    {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    hash.update([u8::from(credentials.is_some())]);
    hash.update(created.to_le_bytes());
    hash.update(updated.to_le_bytes());
    hash.finalize().into()
}
fn record(row: &Row<'_>) -> rusqlite::Result<SyncProviderRecord> {
    let uuid: String = row.get(0)?;
    let provider: String = row.get(1)?;
    let name: String = row.get(2)?;
    let config: String = row.get(3)?;
    let credentials = row.get::<_, Option<String>>(4)?.map(Zeroizing::new);
    let created: i64 = row.get(6)?;
    let fingerprint = fingerprint(
        &[&uuid, &provider, &name, &config],
        credentials.as_deref().map(String::as_str),
        created,
        row.get(7)?,
    );
    Ok(SyncProviderRecord {
        uuid,
        provider,
        name,
        config: serde_json::from_str(&config).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(error))
        })?,
        has_credentials: credentials.is_some(),
        active: row.get(5)?,
        fingerprint,
    })
}
pub(super) fn verify(
    current: Option<&SyncProviderRecord>,
    expected: Option<&SyncProviderRecord>,
) -> Result<(), Box<dyn Error>> {
    match (current, expected) {
        (None, None) => Ok(()),
        (Some(current), Some(expected))
            if current.uuid == expected.uuid && current.fingerprint == expected.fingerprint =>
        {
            Ok(())
        }
        _ => Err(
            "This sync provider changed in another window. Reload it before saving or removing it."
                .into(),
        ),
    }
}
impl NautermDatabase {
    pub fn list_sync_provider_records(&self) -> rusqlite::Result<Vec<SyncProviderRecord>> {
        self.connection.prepare("SELECT uuid, provider, name, config_json, credentials_json, active, created_at, updated_at FROM sync_providers ORDER BY created_at, uuid")?
            .query_map([], record)?.collect()
    }
    pub fn get_sync_provider_record(
        &self,
        uuid: &str,
    ) -> rusqlite::Result<Option<SyncProviderRecord>> {
        self.connection.query_row("SELECT uuid, provider, name, config_json, credentials_json, active, created_at, updated_at FROM sync_providers WHERE uuid = ?", params![uuid], record).optional()
    }
    pub fn save_sync_provider_checked(
        &mut self,
        update: &SyncProviderUpdate,
        expected: Option<&SyncProviderRecord>,
    ) -> Result<SyncProviderRecord, Box<dyn Error>> {
        update.validate()?;
        self.with_sync_operation(|database| {
            database.save_sync_provider_checked_inner(update, expected)
        })
    }
    fn save_sync_provider_checked_inner(
        &mut self,
        update: &SyncProviderUpdate,
        expected: Option<&SyncProviderRecord>,
    ) -> Result<SyncProviderRecord, Box<dyn Error>> {
        self.atomic(|database| {
            let current = database.get_sync_provider_record(&update.uuid)?;
            verify(current.as_ref(), expected)?;
            if current.is_none() && !matches!(update.uuid.as_str(), "github_repository" | "github_gist" | "s3") {
                let count: i64 = database.connection.query_row("SELECT COUNT(*) FROM sync_providers WHERE uuid NOT IN ('github_repository','github_gist','s3')", [], |row| row.get(0))?;
                if count >= 32 { return Err("A maximum of 32 cloud providers can be configured.".into()); }
            }
            database.save_sync_provider_config(&update.uuid, &update.provider, &update.name, &update.config.to_string())?;
            if let Some(credentials) = &update.credentials {
                let values: BTreeMap<&str, &str> = credentials.values.iter().map(|(key, value)| (key.as_str(), value.as_str())).collect();
                #[derive(Serialize)]
                struct Wrapped<'a> { values: BTreeMap<&'a str, &'a str> }
                let encoded = Zeroizing::new(if matches!(update.uuid.as_str(), "github_repository" | "github_gist" | "s3") {
                    serde_json::to_string(&values)?
                } else { serde_json::to_string(&Wrapped { values })? });
                database.save_provider_credentials(&update.uuid, &update.provider, &update.name, &encoded)?;
            }
            database.get_sync_provider_record(&update.uuid)?.ok_or_else(|| "The saved sync provider is unavailable.".into())
        })
    }
    pub fn remove_sync_provider_checked(
        &mut self,
        expected: &SyncProviderRecord,
        disconnect_only: bool,
    ) -> Result<(), Box<dyn Error>> {
        self.with_sync_operation(|database| {
            database.remove_sync_provider_checked_inner(expected, disconnect_only)
        })
    }
    fn remove_sync_provider_checked_inner(
        &mut self,
        expected: &SyncProviderRecord,
        disconnect_only: bool,
    ) -> Result<(), Box<dyn Error>> {
        self.atomic(|database| {
            let current = database.get_sync_provider_record(&expected.uuid)?;
            verify(current.as_ref(), Some(expected))?;
            if current
                .as_ref()
                .is_some_and(|current| current.active != expected.active)
            {
                return Err("The active provider changed. Review this operation again.".into());
            }
            if disconnect_only {
                database.delete_provider_credentials(&expected.uuid)?;
                database.connection.execute(
                    "UPDATE sync_providers SET active = 0 WHERE uuid = ?",
                    params![expected.uuid],
                )?;
            } else {
                database.delete_sync_provider_config(&expected.uuid)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sync_provider_stamps_cover_secrets_and_public_config_without_exposing_them() {
        let a = fingerprint(
            &["id", "webdav", "Name", "{}"],
            Some("synthetic-old-password"),
            1,
            2,
        );
        assert_ne!(
            a,
            fingerprint(
                &["id", "webdav", "Name", "{}"],
                Some("synthetic-new-password"),
                1,
                2
            )
        );
        assert_ne!(
            a,
            fingerprint(
                &["id", "webdav", "Other", "{}"],
                Some("synthetic-old-password"),
                1,
                2
            )
        );
        assert_ne!(
            fingerprint(&["id"], None, 1, 2),
            fingerprint(&["id"], Some(""), 1, 2)
        );
        assert_ne!(
            a,
            fingerprint(
                &["id", "webdav", "Name", "{}"],
                Some("synthetic-old-password"),
                1,
                3
            )
        );
        let mut credentials = SyncProviderCredentials::default();
        credentials
            .values
            .insert("password".into(), Zeroizing::new("synthetic-secret".into()));
        assert!(!format!("{credentials:?}").contains("synthetic-secret"));
    }
    #[test]
    fn checked_provider_edits_reject_stale_deleted_and_colliding_records() {
        let original = SyncProviderRecord {
            uuid: "provider-test".into(),
            provider: "webdav".into(),
            name: "Test".into(),
            config: json!({}),
            has_credentials: true,
            active: false,
            fingerprint: fingerprint(&["provider-test"], Some("synthetic-secret"), 1, 2),
        };
        assert!(verify(None, None).is_ok());
        assert!(verify(Some(&original), Some(&original)).is_ok());
        assert!(verify(None, Some(&original)).is_err());
        assert!(verify(Some(&original), None).is_err());
        let mut changed = original.clone();
        changed.fingerprint = fingerprint(&["provider-test"], Some("synthetic-secret"), 1, 3);
        assert!(verify(Some(&changed), Some(&original)).is_err());
        changed = original.clone();
        changed.uuid = "other-provider".into();
        assert!(verify(Some(&changed), Some(&original)).is_err());
    }
    #[test]
    fn provider_validation_retains_unknown_config_and_requires_complete_credential_pairs(
    ) -> Result<(), Box<dyn Error>> {
        let mut update = SyncProviderUpdate {
            uuid: "provider-test".into(),
            provider: "webdav".into(),
            name: "Test".into(),
            config: json!({"scheme":"webdav","config":{"endpoint":"https://example.invalid/dav","filename":"nauterm-sync.enc","future":"retained"},"future_outer":true}),
            credentials: None,
        };
        update.validate()?;
        let before = update.config.clone();
        let mut credentials = SyncProviderCredentials::default();
        credentials
            .values
            .insert("username".into(), Zeroizing::new("user".into()));
        update.credentials = Some(credentials);
        assert!(update.validate().is_err());
        update
            .credentials
            .as_mut()
            .unwrap()
            .values
            .insert("password".into(), Zeroizing::new(" exact secret ".into()));
        update.validate()?;
        assert_eq!(update.config, before);
        Ok(())
    }
}
