//! History clients retain the reviewed destination through listing, promotion
//! and the following pull. No operation retargets itself after a config edit.
use super::*;
use crate::{cloud_sync, github_gist_sync, github_sync, object_sync};

const HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncHistoryVersion {
    pub version_id: String,
    pub timestamp: Option<String>,
    pub message: String,
    pub author: String,
    pub etag: String,
    pub bytes: Option<u64>,
    pub current: bool,
}
pub struct SyncProviderHistory {
    scope: String,
    pub provider: SyncProviderRecord,
    pub versions: Vec<SyncHistoryVersion>,
    pub can_restore: bool,
}
impl SyncProviderHistory {
    pub fn scope(&self) -> &str {
        &self.scope
    }
}

fn scope_uuid(scope: &str) -> Result<&str, Box<dyn Error>> {
    if matches!(scope, "github_repository" | "github_gist" | "s3") {
        return Ok(scope);
    }
    let id = scope
        .strip_prefix("cloud:")
        .ok_or("Unsupported sync history provider.")?;
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || matches!(id, "github_repository" | "github_gist" | "s3")
    {
        return Err("Invalid sync history provider.".into());
    }
    Ok(id)
}
fn github_versions(rows: Vec<github_sync::GithubRevision>) -> Vec<SyncHistoryVersion> {
    rows.into_iter()
        .take(HISTORY_LIMIT)
        .enumerate()
        .map(|(index, row)| SyncHistoryVersion {
            version_id: row.sha,
            timestamp: (!row.committed_at.is_empty()).then_some(row.committed_at),
            message: row.message.chars().take(1024).collect(),
            author: row.author.chars().take(256).collect(),
            etag: String::new(),
            bytes: None,
            current: index == 0,
        })
        .collect()
}
fn object_versions(rows: Vec<object_sync::ObjectVersionSummary>) -> Vec<SyncHistoryVersion> {
    rows.into_iter()
        .take(HISTORY_LIMIT)
        .map(|row| SyncHistoryVersion {
            version_id: row.version_id,
            timestamp: row.last_modified,
            message: String::new(),
            author: String::new(),
            etag: row.etag,
            bytes: Some(row.content_length),
            current: row.is_current,
        })
        .collect()
}
fn validate_versions(versions: &[SyncHistoryVersion]) -> Result<(), Box<dyn Error>> {
    let mut ids = BTreeSet::new();
    for version in versions {
        if version.version_id.trim().is_empty()
            || version.version_id.len() > 4096
            || version.version_id.chars().any(char::is_control)
            || version.etag.len() > 4096
            || version
                .timestamp
                .as_ref()
                .is_some_and(|value| value.len() > 256)
            || !ids.insert(&version.version_id)
        {
            return Err("The provider returned invalid or ambiguous version history.".into());
        }
    }
    Ok(())
}
fn verify_version(
    versions: &[SyncHistoryVersion],
    selected: &SyncHistoryVersion,
) -> Result<(), Box<dyn Error>> {
    if selected.current
        || !versions
            .iter()
            .any(|version| version == selected && !version.current)
    {
        return Err(
            "This version changed or is no longer available. Refresh the history before restoring."
                .into(),
        );
    }
    Ok(())
}

enum HistoryClient {
    Github(Box<github_sync::GithubClient>),
    Gist(Box<github_gist_sync::GithubGistClient>),
    Object(Box<object_sync::OpenDalSyncTransport>),
    Drive(
        Box<cloud_sync::GoogleDriveHistoryClient>,
        Box<object_sync::OpenDalSyncTransport>,
    ),
    Onedrive(
        Box<cloud_sync::OneDriveHistoryClient>,
        Box<object_sync::OpenDalSyncTransport>,
    ),
    Dropbox(
        Box<cloud_sync::DropboxHistoryClient>,
        Box<object_sync::OpenDalSyncTransport>,
    ),
}
impl HistoryClient {
    fn rotate(
        self,
        database: &mut NautermDatabase,
        scope: &str,
        current: &str,
        new: &str,
    ) -> Result<Value, Box<dyn Error>> {
        match self {
            Self::Github(client) => {
                database.github_sync_with_client(*client, Some(current), Some(new), None)
            }
            Self::Gist(client) => {
                database.github_gist_sync_with_client(*client, Some(current), Some(new), None)
            }
            Self::Object(client)
            | Self::Drive(_, client)
            | Self::Onedrive(_, client)
            | Self::Dropbox(_, client) => database.object_provider_sync_with_master_key_rotation(
                *client,
                scope,
                "key-rotation",
                Some(current),
                Some(new),
                None,
            ),
        }
    }
    fn list(&self) -> Result<Vec<SyncHistoryVersion>, Box<dyn Error>> {
        let versions = match self {
            Self::Github(client) => github_versions(client.list_history(HISTORY_LIMIT)?),
            Self::Gist(client) => github_versions(client.list_history(HISTORY_LIMIT)?),
            Self::Object(client) => object_versions(client.list_versions(HISTORY_LIMIT)?),
            Self::Drive(client, _) => object_versions(client.list_versions(HISTORY_LIMIT)?),
            Self::Onedrive(client, _) => object_versions(client.list_versions(HISTORY_LIMIT)?),
            Self::Dropbox(client, _) => object_versions(client.list_versions(HISTORY_LIMIT)?),
        };
        validate_versions(&versions)?;
        Ok(versions)
    }
    fn restore(
        self,
        database: &mut NautermDatabase,
        scope: &str,
        version_id: &str,
    ) -> Result<Value, Box<dyn Error>> {
        let remote_wins = Some(crate::sync::SyncStrategy::RemoteWins);
        let mut outcome = match self {
            Self::Github(client) => {
                client.restore_revision(version_id)?;
                database.clear_provider_revision_checkpoint(scope)?;
                database.github_sync_with_client(*client, None, None, remote_wins)?
            }
            Self::Gist(client) => {
                client.restore_version(version_id)?;
                database.clear_provider_revision_checkpoint(scope)?;
                database.github_gist_sync_with_client(*client, None, None, remote_wins)?
            }
            Self::Object(client) => {
                client.restore_version(version_id)?;
                database.finish_object_history_restore(*client, scope)?
            }
            Self::Drive(history, client) => {
                let bytes = history.read_version(version_id)?;
                let current = client.fetch_current()?;
                client.write(&bytes, current.as_ref().map(|value| value.etag.as_str()))?;
                database.finish_object_history_restore(*client, scope)?
            }
            Self::Onedrive(history, client) => {
                history.restore_version(version_id)?;
                database.finish_object_history_restore(*client, scope)?
            }
            Self::Dropbox(history, client) => {
                history.restore_version(version_id)?;
                database.finish_object_history_restore(*client, scope)?
            }
        };
        if let Some(id) = scope.strip_prefix("cloud:") {
            outcome["provider_id"] = json!(id);
        }
        Ok(outcome)
    }
}
impl NautermDatabase {
    pub(in crate::database) fn has_cached_sync_key(&self) -> rusqlite::Result<bool> {
        self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM app_metadata WHERE key = 'sync_dek')
             AND EXISTS(SELECT 1 FROM app_metadata WHERE key = 'sync_envelope_header')",
            [],
            |row| row.get(0),
        )
    }
    fn pin_sync_history(
        &mut self,
        scope: &str,
        expected: Option<&SyncProviderRecord>,
    ) -> Result<(SyncProviderRecord, HistoryClient), Box<dyn Error>> {
        let uuid = scope_uuid(scope)?;
        if scope.starts_with("cloud:") {
            let (record, provider, credentials) =
                self.prepared_cloud_provider_checked(uuid, expected)?;
            let access_token = || {
                credentials
                    .values
                    .get("access_token")
                    .cloned()
                    .ok_or_else(|| Box::<dyn Error>::from("OAuth access token is missing."))
            };
            let history = match provider.scheme.as_str() {
                "s3" | "azblob" | "gcs" | "webdav" => HistoryClient::Object(Box::new(
                    cloud_sync::build_transport(&provider, credentials)?,
                )),
                "gdrive" => {
                    let history =
                        cloud_sync::GoogleDriveHistoryClient::new(&provider, access_token()?)?;
                    HistoryClient::Drive(
                        Box::new(history),
                        Box::new(cloud_sync::build_transport(&provider, credentials)?),
                    )
                }
                "onedrive" => {
                    let history =
                        cloud_sync::OneDriveHistoryClient::new(&provider, access_token()?)?;
                    HistoryClient::Onedrive(
                        Box::new(history),
                        Box::new(cloud_sync::build_transport(&provider, credentials)?),
                    )
                }
                "dropbox" => {
                    let history =
                        cloud_sync::DropboxHistoryClient::new(&provider, access_token()?)?;
                    HistoryClient::Dropbox(
                        Box::new(history),
                        Box::new(cloud_sync::build_transport(&provider, credentials)?),
                    )
                }
                _ => {
                    return Err(
                        "This storage backend does not support object version history.".into(),
                    )
                }
            };
            return Ok((record, history));
        }
        self.atomic(|database| -> Result<_, Box<dyn Error>> {
            let record = database
                .get_sync_provider_record(uuid)?
                .ok_or("This sync provider was removed.")?;
            if let Some(expected) = expected {
                super::sync_providers::verify(Some(&record), Some(expected))?;
            }
            let client = match scope {
                "github_repository" => HistoryClient::Github(Box::new(database.github_client()?)),
                "github_gist" => HistoryClient::Gist(Box::new(database.github_gist_client()?)),
                "s3" => HistoryClient::Object(Box::new(database.s3_client()?)),
                _ => unreachable!(),
            };
            Ok((record, client))
        })
    }
    /// Explicit remote history read. The returned stamp belongs to the client
    /// actually used, including any refresh-token rotation performed on entry.
    pub fn sync_provider_history(
        &mut self,
        scope: &str,
    ) -> Result<SyncProviderHistory, Box<dyn Error>> {
        self.with_sync_operation(|database| database.sync_provider_history_inner(scope))
    }
    fn sync_provider_history_inner(
        &mut self,
        scope: &str,
    ) -> Result<SyncProviderHistory, Box<dyn Error>> {
        let (provider, client) = self.pin_sync_history(scope, None)?;
        let versions = client.list()?;
        let current = self.get_sync_provider_record(&provider.uuid)?;
        super::sync_providers::verify(current.as_ref(), Some(&provider))?;
        Ok(SyncProviderHistory {
            scope: scope.into(),
            provider,
            versions,
            can_restore: self.has_cached_sync_key()?,
        })
    }
    /// Restore the reviewed version on its pinned destination, then pull it
    /// with RemoteWins. Neither current entries nor forged/stale rows qualify.
    pub fn restore_sync_history_version(
        &mut self,
        history: &SyncProviderHistory,
        selected: &SyncHistoryVersion,
        retention_count: usize,
    ) -> Result<Value, Box<dyn Error>> {
        self.with_sync_operation(|database| {
            database.restore_sync_history_version_inner(history, selected, retention_count)
        })
    }
    fn restore_sync_history_version_inner(
        &mut self,
        history: &SyncProviderHistory,
        selected: &SyncHistoryVersion,
        retention_count: usize,
    ) -> Result<Value, Box<dyn Error>> {
        if retention_count > 100 {
            return Err("Keep between 0 and 100 local backups.".into());
        }
        if !self.has_cached_sync_key()? {
            return Err("Complete a sync on this device before restoring cloud history.".into());
        }
        verify_version(&history.versions, selected)?;
        let (provider, client) = self.pin_sync_history(&history.scope, Some(&history.provider))?;
        verify_version(&client.list()?, selected)?;
        let current = self.get_sync_provider_record(&provider.uuid)?;
        super::sync_providers::verify(current.as_ref(), Some(&provider))?;
        self.create_local_sync_backup(retention_count)?;
        client.restore(self, &history.scope, &selected.version_id)
    }
    pub(in crate::database) fn restore_pinned_sync_history(
        &mut self,
        scope: &str,
        version_id: &str,
    ) -> Result<Value, Box<dyn Error>> {
        if !self.has_cached_sync_key()? {
            return Err("Complete a sync on this device before restoring cloud history.".into());
        }
        let (_, client) = self.pin_sync_history(scope, None)?;
        client.restore(self, scope, version_id)
    }
    pub(in crate::database) fn rotate_sync_provider_checked(
        &mut self,
        expected: &SyncProviderRecord,
        current: &str,
        new: &str,
    ) -> Result<Value, Box<dyn Error>> {
        let scope = if matches!(
            expected.uuid.as_str(),
            "github_repository" | "github_gist" | "s3"
        ) {
            expected.uuid.clone()
        } else {
            format!("cloud:{}", expected.uuid)
        };
        let (_, client) = self.pin_sync_history(&scope, Some(expected))?;
        client.rotate(self, &scope, current, new)
    }
    fn finish_object_history_restore(
        &mut self,
        client: object_sync::OpenDalSyncTransport,
        scope: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.clear_provider_revision_checkpoint(scope)?;
        self.object_provider_sync_with_master_key_rotation(
            client,
            scope,
            "history-restore",
            None,
            None,
            Some(crate::sync::SyncStrategy::RemoteWins),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_scopes_keep_builtin_and_cloud_namespaces_separate() {
        assert_eq!(
            scope_uuid("github_repository").unwrap(),
            "github_repository"
        );
        assert_eq!(
            scope_uuid("cloud:provider-test_1").unwrap(),
            "provider-test_1"
        );
        for scope in [
            "provider-test",
            "cloud:",
            "cloud:s3",
            "cloud:github_gist",
            "cloud:../test",
            "cloud:test\n",
            "cloud:测试",
        ] {
            assert!(scope_uuid(scope).is_err(), "{scope:?}");
        }
    }
    #[test]
    fn normalized_history_keeps_opaque_object_ids_and_marks_git_heads() {
        let git = github_versions(vec![github_sync::GithubRevision {
            sha: "a".repeat(40),
            message: "Encrypted sync".into(),
            committed_at: "2026-10-27T01:02:03Z".into(),
            author: "Test".into(),
        }]);
        assert!(git[0].current);
        assert_eq!(git[0].author, "Test");
        let objects = object_versions(vec![object_sync::ObjectVersionSummary {
            version_id: "opaque/id+token==".into(),
            etag: "etag".into(),
            content_length: 1024,
            last_modified: None,
            is_current: false,
        }]);
        assert_eq!(objects[0].version_id, "opaque/id+token==");
        assert_eq!(objects[0].bytes, Some(1024));
        assert!(!objects[0].current);
        validate_versions(&objects).unwrap();
    }
    #[test]
    fn restoring_requires_the_same_noncurrent_version_descriptor() {
        let original = SyncHistoryVersion {
            version_id: "opaque".into(),
            timestamp: None,
            message: String::new(),
            author: String::new(),
            etag: "etag".into(),
            bytes: Some(42),
            current: false,
        };
        assert!(verify_version(std::slice::from_ref(&original), &original).is_ok());
        assert!(verify_version(&[], &original).is_err());
        let mut changed = original.clone();
        changed.etag = "changed".into();
        assert!(verify_version(&[changed], &original).is_err());
        changed = original.clone();
        changed.current = true;
        assert!(verify_version(std::slice::from_ref(&changed), &changed).is_err());
        assert!(verify_version(&[changed], &original).is_err());
        assert!(validate_versions(&[original.clone(), original.clone()]).is_err());
        for id in [String::new(), "bad\0id".into(), "x".repeat(4097)] {
            changed = original.clone();
            changed.version_id = id;
            assert!(validate_versions(&[changed]).is_err());
        }
    }
}
