use super::super::*;
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
use zeroize::Zeroizing;

const LOCAL_SYNC_BACKUP_PREFIX: &str = "nauterm-local-backup-";
const LOCAL_SYNC_BACKUP_SUFFIX: &str = ".sqlite";

type SyncMetadataSnapshot = Vec<(String, String, i64)>;
const SYNC_PREFERENCES_KEY: &str = "sync_preferences_v1";

fn load_sync_preferences(
    connection: &Connection,
) -> Result<crate::cloud_sync::SyncPreferences, Box<dyn Error>> {
    let value = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = ?",
            params![SYNC_PREFERENCES_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let mut preferences: crate::cloud_sync::SyncPreferences = value
        .map(|value| serde_json::from_str(&value))
        .transpose()?
        .unwrap_or_default();
    preferences.active_provider_id = connection
        .query_row(
            "SELECT uuid FROM sync_providers WHERE active = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|provider_id| {
            if matches!(
                provider_id.as_str(),
                "github_repository" | "github_gist" | "s3"
            ) {
                provider_id
            } else {
                format!("cloud:{provider_id}")
            }
        });
    Ok(preferences)
}

fn store_sync_preferences(
    connection: &Connection,
    preferences: &crate::cloud_sync::SyncPreferences,
) -> Result<(), Box<dyn Error>> {
    select_sync_provider(connection, preferences.active_provider_id.as_deref())?;
    let stored = serde_json::json!({
        "sync_snapshot": &preferences.sync_snapshot,
    });
    connection.execute(
        r#"INSERT INTO app_metadata (key, value) VALUES (?, ?)
           ON CONFLICT(key) DO UPDATE SET value = excluded.value,
             updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)"#,
        params![SYNC_PREFERENCES_KEY, serde_json::to_string(&stored)?],
    )?;
    Ok(())
}

fn select_sync_provider(
    connection: &Connection,
    provider_id: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let provider_id = provider_id.map(|value| value.strip_prefix("cloud:").unwrap_or(value));
    let current: Option<String> = connection
        .query_row(
            "SELECT uuid FROM sync_providers WHERE active = 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    // Saving a checkpoint must not look like a provider edit in another view.
    if current.as_deref() == provider_id {
        return Ok(());
    }
    connection.execute("UPDATE sync_providers SET active = 0 WHERE active != 0", [])?;
    if let Some(provider_id) = provider_id {
        let updated = connection.execute(
            "UPDATE sync_providers
             SET active = 1,
                 updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
             WHERE uuid = ?",
            params![provider_id],
        )?;
        if updated == 0 {
            return Err(Box::<dyn Error>::from(
                "The selected sync provider does not exist.",
            ));
        }
    }
    Ok(())
}

fn backup_prune_plan(files: &[PathBuf], retention: usize, protected: &[&Path]) -> Vec<PathBuf> {
    files
        .iter()
        .filter(|path| !protected.contains(&path.as_path()))
        .take(files.len().saturating_sub(retention))
        .cloned()
        .collect()
}

fn write_private_staging_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    file.write_all(bytes)
}

impl NautermDatabase {
    fn local_sync_backup_directory(&self) -> Result<PathBuf, Box<dyn Error>> {
        let database_path: String = self.connection.query_row(
            "SELECT file FROM pragma_database_list WHERE name = 'main'",
            [],
            |row| row.get(0),
        )?;
        let database_path = PathBuf::from(database_path);
        let parent = database_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| {
                Box::<dyn Error>::from("Local backups require a file-backed database.")
            })?;
        Ok(parent.join("sync-backups"))
    }

    fn local_sync_backup_files(&self) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        let directory = self.local_sync_backup_directory()?;
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut files = std::fs::read_dir(directory)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with(LOCAL_SYNC_BACKUP_PREFIX)
                            && name.ends_with(LOCAL_SYNC_BACKUP_SUFFIX)
                    })
            })
            .filter(|path| {
                std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
            })
            .collect::<Vec<_>>();
        files.sort();
        Ok(files)
    }

    pub(in crate::database) fn create_local_sync_backup(
        &mut self,
        retention_count: usize,
    ) -> Result<(), Box<dyn Error>> {
        self.create_local_sync_backup_preserving(retention_count, None)
            .map(|_| ())
    }

    fn create_local_sync_backup_preserving(
        &mut self,
        retention_count: usize,
        protected: Option<&Path>,
    ) -> Result<Option<PathBuf>, Box<dyn Error>> {
        if retention_count == 0 {
            return Ok(None);
        }
        let retention_count = retention_count.clamp(1, 101);
        let directory = self.local_sync_backup_directory()?;
        std::fs::create_dir_all(&directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let filename = format!("{LOCAL_SYNC_BACKUP_PREFIX}sync-{now}{LOCAL_SYNC_BACKUP_SUFFIX}");
        let path = directory.join(filename);
        Self::backup_connection_to(&self.connection, &path)?;
        let files = self.local_sync_backup_files()?;
        let protected: Vec<_> = protected
            .into_iter()
            .chain(std::iter::once(path.as_path()))
            .collect();
        for old in backup_prune_plan(&files, retention_count, &protected) {
            std::fs::remove_file(old)?;
        }
        Ok(Some(path))
    }

    pub fn local_sync_backups(&self) -> Result<Value, Box<dyn Error>> {
        let mut backups = self
            .local_sync_backup_files()?
            .into_iter()
            .map(|path| {
                let metadata = std::fs::metadata(&path)?;
                let created_at = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_millis();
                Ok(json!({
                    "id": path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
                    "reason": "sync",
                    "created_at": created_at,
                    "bytes": metadata.len(),
                }))
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        backups.reverse();
        Ok(Value::Array(backups))
    }

    pub fn restore_local_sync_backup(
        &mut self,
        backup_id: &str,
        retention_count: usize,
    ) -> Result<Value, Box<dyn Error>> {
        self.with_sync_operation(|database| {
            database.restore_local_sync_backup_inner(backup_id, retention_count)
        })
    }
    fn restore_local_sync_backup_inner(
        &mut self,
        backup_id: &str,
        retention_count: usize,
    ) -> Result<Value, Box<dyn Error>> {
        if !backup_id.starts_with(LOCAL_SYNC_BACKUP_PREFIX)
            || !backup_id.ends_with(LOCAL_SYNC_BACKUP_SUFFIX)
            || backup_id.contains('/')
            || backup_id.contains('\\')
        {
            return Err(Box::<dyn Error>::from("Invalid local backup identifier."));
        }
        let path = self.local_sync_backup_directory()?.join(backup_id);
        if !path.is_file() {
            return Err(Box::<dyn Error>::from("Local backup no longer exists."));
        }
        let source = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::key_database(&source)?;
        let version: i32 = source.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if !(1..=SCHEMA_VERSION).contains(&version) {
            return Err(Box::<dyn Error>::from(format!(
                "The backup uses unsupported database schema {version}; this application supports 1 through {SCHEMA_VERSION}."
            )));
        }
        let safety_backup = self.create_local_sync_backup_preserving(
            retention_count.saturating_add(1).clamp(2, 101),
            Some(&path),
        )?;
        {
            let backup = rusqlite::backup::Backup::new(&source, &mut self.connection)?;
            backup.run_to_completion(32, Duration::from_millis(10), None)?;
        }
        drop(source);
        Self::configure(&self.connection)?;
        Self::ensure_schema(&mut self.connection)?;
        let retention_count = retention_count.clamp(1, 100);
        let files = self.local_sync_backup_files()?;
        let protected: Vec<_> = safety_backup.as_deref().into_iter().collect();
        for path in backup_prune_plan(&files, retention_count, &protected) {
            std::fs::remove_file(path)?;
        }
        Ok(json!({"restored": true, "backup_id": backup_id}))
    }

    pub(in crate::database) fn sync_preferences(
        &self,
    ) -> Result<crate::cloud_sync::SyncPreferences, Box<dyn Error>> {
        load_sync_preferences(&self.connection)
    }

    pub(in crate::database) fn sync_preferences_status(&self) -> Result<Value, Box<dyn Error>> {
        let preferences = self.sync_preferences()?;
        let provider_id = preferences
            .active_provider_id
            .as_deref()
            .map(|value| value.strip_prefix("cloud:").unwrap_or(value));
        let checkpoint = provider_id
            .map(|provider_id| {
                self.connection
                    .query_row(
                        "SELECT last_seen_revision, last_seen_snapshot_id, last_sync_at
                         FROM sync_providers WHERE uuid = ?",
                        params![provider_id],
                        |row| {
                            Ok((
                                row.get::<_, Option<i64>>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, Option<i64>>(2)?,
                            ))
                        },
                    )
                    .optional()
            })
            .transpose()?
            .flatten();
        let mut value = serde_json::to_value(preferences)?;
        value["remote_revision"] = checkpoint
            .as_ref()
            .and_then(|value| value.0)
            .map(Value::from)
            .unwrap_or(Value::Null);
        value["remote_snapshot_id"] = checkpoint
            .as_ref()
            .and_then(|value| value.1.as_deref())
            .map(Value::from)
            .unwrap_or(Value::Null);
        value["remote_synced_at"] = checkpoint
            .and_then(|value| value.2)
            .map(Value::from)
            .unwrap_or(Value::Null);
        Ok(value)
    }

    /// Select a provider only if the active selection reviewed by this client
    /// still matches. The write reservation precedes the check; snapshot and
    /// remote checkpoint fields are read afresh rather than sent back by a UI.
    pub fn activate_sync_provider_checked(
        &mut self,
        expected: Option<&str>,
        selected: Option<&str>,
    ) -> Result<Value, Box<dyn Error>> {
        self.with_sync_operation(|database| {
            database.activate_sync_provider_checked_inner(expected, selected)
        })
    }
    fn activate_sync_provider_checked_inner(
        &mut self,
        expected: Option<&str>,
        selected: Option<&str>,
    ) -> Result<Value, Box<dyn Error>> {
        self.atomic(|database| {
            let preferences = load_sync_preferences(&database.connection)?;
            if preferences.active_provider_id.as_deref() != expected {
                return Err(Box::<dyn Error>::from(
                    "The active sync provider changed. Refresh before selecting it again.",
                ));
            }
            select_sync_provider(&database.connection, selected)?;
            database.sync_preferences_status()
        })
    }

    /// Background sync uses the current credentialed provider and cached DEK.
    /// Missing setup is a quiet no-op; the shared lease covers preflight and IO.
    pub fn sync_automatically(
        &mut self,
        strategy: crate::sync::SyncStrategy,
        retention_count: usize,
    ) -> Result<Option<Value>, Box<dyn Error>> {
        if retention_count > 100 {
            return Err("Keep between 0 and 100 local backups.".into());
        }
        self.with_sync_operation(|database| {
            let active = database.atomic(|database| -> Result<Option<String>, Box<dyn Error>> {
                if !database.has_cached_sync_key()? {
                    return Ok(None);
                }
                let Some(active) = load_sync_preferences(&database.connection)?.active_provider_id
                else {
                    return Ok(None);
                };
                let uuid = active.strip_prefix("cloud:").unwrap_or(&active);
                Ok(database
                    .get_sync_provider_record(uuid)?
                    .filter(|record| record.has_credentials)
                    .map(|_| active))
            })?;
            active
                .map(|active| {
                    database.sync_active_provider_inner(&active, None, strategy, retention_count)
                })
                .transpose()
        })
    }
    /// Explicit sync keeps the caller's reviewed active-provider identity. A
    /// failed upload may follow a local merge, so callers must refresh on error.
    pub fn sync_active_provider(
        &mut self,
        expected: &str,
        master_key: Option<&str>,
        strategy: crate::sync::SyncStrategy,
        retention_count: usize,
    ) -> Result<Value, Box<dyn Error>> {
        self.with_sync_operation(|database| {
            database.sync_active_provider_inner(expected, master_key, strategy, retention_count)
        })
    }
    fn sync_active_provider_inner(
        &mut self,
        expected: &str,
        master_key: Option<&str>,
        strategy: crate::sync::SyncStrategy,
        retention_count: usize,
    ) -> Result<Value, Box<dyn Error>> {
        if retention_count > 100 {
            return Err("Keep between 0 and 100 local backups.".into());
        }
        if let Some(key) = master_key {
            crate::crypto::validate_master_key(key)?;
        }
        self.atomic(|database| -> Result<(), Box<dyn Error>> {
            let current = load_sync_preferences(&database.connection)?;
            if current.active_provider_id.as_deref() != Some(expected) {
                return Err(
                    "The active sync provider changed. Refresh before syncing again.".into(),
                );
            }
            let uuid = expected.strip_prefix("cloud:").unwrap_or(expected);
            if !database
                .get_sync_provider_record(uuid)?
                .is_some_and(|provider| provider.has_credentials)
            {
                return Err("Configure this provider's credentials before syncing.".into());
            }
            if master_key.is_none() && !database.has_cached_sync_key()? {
                return Err("Enter a Master Key before syncing.".into());
            }
            Ok(())
        })?;
        self.create_local_sync_backup(retention_count)?;
        match expected {
            "github_repository" => self.github_sync(master_key, strategy),
            "github_gist" => self.github_gist_sync(master_key, strategy),
            "s3" => self.s3_sync(master_key, strategy),
            _ => self.cloud_provider_sync(
                expected
                    .strip_prefix("cloud:")
                    .ok_or("Unsupported sync provider.")?,
                master_key,
                strategy,
            ),
        }
    }

    pub(in crate::database) fn save_sync_preferences(
        &self,
        preferences: &crate::cloud_sync::SyncPreferences,
    ) -> Result<(), Box<dyn Error>> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut preferences = preferences.clone();
        if preferences.sync_snapshot.is_none() {
            preferences.sync_snapshot = load_sync_preferences(&transaction)?.sync_snapshot;
        }
        store_sync_preferences(&transaction, &preferences)?;
        transaction.commit()?;
        Ok(())
    }

    fn remember_sync_snapshot(
        &self,
        revision: u64,
        snapshot_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut preferences = load_sync_preferences(&transaction)?;
        preferences.sync_snapshot = Some(crate::cloud_sync::SyncSnapshotStatus {
            revision,
            snapshot_id: snapshot_id.to_string(),
        });
        store_sync_preferences(&transaction, &preferences)?;
        transaction.commit()?;
        Ok(())
    }

    pub(in crate::database) fn clear_sync_snapshot(&self) -> Result<(), Box<dyn Error>> {
        if !self.connection.is_autocommit() {
            let mut preferences = load_sync_preferences(&self.connection)?;
            preferences.sync_snapshot = None;
            return store_sync_preferences(&self.connection, &preferences);
        }
        let transaction = self.connection.unchecked_transaction()?;
        let mut preferences = load_sync_preferences(&transaction)?;
        preferences.sync_snapshot = None;
        store_sync_preferences(&transaction, &preferences)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn forget_cached_sync_key(
        &mut self,
        expected: Option<&SyncKeyState>,
    ) -> Result<SyncKeyState, Box<dyn Error>> {
        self.with_sync_operation(|database| database.atomic(|database| {
            if let Some(expected) = expected { database.sync_key_state()?.verify(expected)?; }
            database.connection.execute(
                "DELETE FROM app_metadata WHERE key IN ('sync_dek', 'sync_vault_id', 'sync_envelope_header') OR key LIKE 'sync_revision:%'", [],
            )?;
            database.clear_sync_snapshot()?;
            database.sync_key_state()
        }))
    }

    pub(in crate::database) fn clear_provider_revision_checkpoint(
        &self,
        scope: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.connection.execute(
            "DELETE FROM app_metadata WHERE key LIKE ?",
            params![format!("sync_revision:{scope}:%")],
        )?;
        Ok(())
    }

    fn remember_sync_provider_checkpoint(
        &self,
        provider_id: &str,
        revision: u64,
        snapshot_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        let revision = i64::try_from(revision)
            .map_err(|_| Box::<dyn Error>::from("Sync revision exceeds SQLite integer range."))?;
        self.connection.execute(
            r#"UPDATE sync_providers
               SET last_seen_revision = ?,
                   last_seen_snapshot_id = ?,
                   last_sync_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                   updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
               WHERE uuid = ?"#,
            params![revision, snapshot_id, provider_id],
        )?;
        Ok(())
    }

    pub(in crate::database) fn remember_remote_sync_status(
        &self,
        provider_id: &str,
        status: Option<&crate::sync::RemoteSyncStatus>,
    ) -> Result<(), Box<dyn Error>> {
        let revision = status
            .map(|value| i64::try_from(value.revision))
            .transpose()
            .map_err(|_| Box::<dyn Error>::from("Sync revision exceeds SQLite integer range."))?;
        let snapshot_id = status.map(|value| value.snapshot_id.as_str());
        self.connection.execute(
            r#"UPDATE sync_providers
               SET last_seen_revision = ?,
                   last_seen_snapshot_id = ?,
                   updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
               WHERE uuid = ?"#,
            params![revision, snapshot_id, provider_id],
        )?;
        Ok(())
    }

    pub(in crate::database) fn refresh_remote_sync_status(
        &mut self,
    ) -> Result<Value, Box<dyn Error>> {
        let preferences = self.sync_preferences()?;
        let Some(active_provider_id) = preferences.active_provider_id.as_deref() else {
            return self.sync_preferences_status();
        };
        let provider_id = active_provider_id
            .strip_prefix("cloud:")
            .unwrap_or(active_provider_id);
        let bytes = match active_provider_id {
            "github_repository" => self.github_client()?.fetch_current()?,
            "github_gist" => self.github_gist_client()?.fetch_current()?,
            "s3" => self.s3_client()?.fetch_current()?.map(|value| value.bytes),
            value if value.starts_with("cloud:") => self
                .cloud_provider_client(provider_id)?
                .fetch_current()?
                .map(|value| value.bytes),
            _ => return Err(Box::<dyn Error>::from("Active sync provider is invalid.")),
        };
        let status = bytes
            .as_deref()
            .map(|bytes| crate::sync::inspect_envelope(bytes, SCHEMA_VERSION))
            .transpose()?;
        self.remember_remote_sync_status(provider_id, status.as_ref())?;
        self.sync_preferences_status()
    }

    pub(in crate::database) fn save_provider_credentials(
        &self,
        provider_id: &str,
        provider: &str,
        name: &str,
        value: &str,
    ) -> Result<(), Box<dyn Error>> {
        if value.is_empty()
            || value.len() > 65_536
            || !serde_json::from_str::<Value>(value)?.is_object()
        {
            return Err(Box::<dyn Error>::from("Sync credential is invalid."));
        }
        self.connection.execute(
            r#"INSERT INTO sync_providers (
                 uuid, provider, name, config_json, credentials_json
               ) VALUES (?, ?, ?, '{}', ?)
               ON CONFLICT(uuid) DO UPDATE SET
                 credentials_json = excluded.credentials_json,
                 updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)"#,
            params![provider_id, provider, name, value],
        )?;
        Ok(())
    }

    fn load_provider_credentials(
        &self,
        provider_id: &str,
    ) -> Result<Option<Zeroizing<String>>, Box<dyn Error>> {
        Ok(self
            .connection
            .query_row(
                "SELECT credentials_json FROM sync_providers WHERE uuid = ?",
                params![provider_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .map(Zeroizing::new))
    }

    fn has_provider_credentials(&self, provider_id: &str) -> rusqlite::Result<bool> {
        self.connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sync_providers
               WHERE uuid = ? AND credentials_json IS NOT NULL
             )",
            params![provider_id],
            |row| row.get(0),
        )
    }

    pub(in crate::database) fn delete_provider_credentials(
        &self,
        provider_id: &str,
    ) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE sync_providers
             SET credentials_json = NULL,
                 updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
             WHERE uuid = ?",
            params![provider_id],
        )?;
        Ok(())
    }

    pub(in crate::database) fn save_github_pat(&self, token: &str) -> Result<(), Box<dyn Error>> {
        if token.is_empty() {
            return Err(Box::<dyn Error>::from("GitHub token is invalid."));
        }
        let encoded = Zeroizing::new(serde_json::to_string(&json!({"token": token}))?);
        self.save_provider_credentials(
            "github_repository",
            "github_repository",
            "GitHub Repository",
            encoded.as_str(),
        )
    }

    pub(in crate::database) fn load_github_pat(
        &self,
    ) -> Result<Option<Zeroizing<String>>, Box<dyn Error>> {
        let Some(encoded) = self.load_provider_credentials("github_repository")? else {
            return Ok(None);
        };
        let token = serde_json::from_str::<Value>(encoded.as_str())?
            .get("token")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        Ok(token.map(Zeroizing::new))
    }

    pub(in crate::database) fn save_github_gist_token(
        &self,
        token: &str,
    ) -> Result<(), Box<dyn Error>> {
        if token.is_empty() {
            return Err(Box::<dyn Error>::from("GitHub Gist token is invalid."));
        }
        let encoded = Zeroizing::new(serde_json::to_string(&json!({"token": token}))?);
        self.save_provider_credentials(
            "github_gist",
            "github_gist",
            "GitHub Gist",
            encoded.as_str(),
        )
    }

    pub(in crate::database) fn load_github_gist_token(
        &self,
    ) -> Result<Option<Zeroizing<String>>, Box<dyn Error>> {
        let Some(encoded) = self.load_provider_credentials("github_gist")? else {
            return Ok(None);
        };
        let token = serde_json::from_str::<Value>(encoded.as_str())?
            .get("token")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        Ok(token.map(Zeroizing::new))
    }

    pub(in crate::database) fn has_github_pat(&self) -> rusqlite::Result<bool> {
        self.has_provider_credentials("github_repository")
    }

    pub(in crate::database) fn delete_github_pat(&self) -> rusqlite::Result<()> {
        self.delete_provider_credentials("github_repository")
    }

    pub(in crate::database) fn has_github_gist_token(&self) -> rusqlite::Result<bool> {
        self.has_provider_credentials("github_gist")
    }

    pub(in crate::database) fn delete_github_gist_token(&self) -> rusqlite::Result<()> {
        self.delete_provider_credentials("github_gist")
    }

    pub(in crate::database) fn save_s3_credentials(
        &self,
        credentials: &crate::s3_sync::S3Credentials,
    ) -> Result<(), Box<dyn Error>> {
        if credentials.access_key_id.trim().is_empty() || credentials.secret_access_key.is_empty() {
            return Err(Box::<dyn Error>::from(
                "S3 Access Key ID and Secret Access Key are required.",
            ));
        }
        let encoded = Zeroizing::new(serde_json::to_string(credentials)?);
        self.save_provider_credentials("s3", "s3_compatible", "S3 Compatible", encoded.as_str())
    }

    pub(in crate::database) fn load_s3_credentials(
        &self,
    ) -> Result<Option<crate::s3_sync::S3Credentials>, Box<dyn Error>> {
        let Some(encoded) = self.load_provider_credentials("s3")? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_str(encoded.as_str())?))
    }

    pub(in crate::database) fn has_s3_credentials(&self) -> rusqlite::Result<bool> {
        self.has_provider_credentials("s3")
    }

    pub(in crate::database) fn delete_s3_credentials(&self) -> rusqlite::Result<()> {
        self.delete_provider_credentials("s3")
    }

    pub(in crate::database) fn save_cloud_credentials(
        &self,
        provider_id: &str,
        credentials: &crate::cloud_sync::CloudProviderCredentials,
    ) -> Result<(), Box<dyn Error>> {
        crate::cloud_sync::validate_credentials(credentials)?;
        let encoded = Zeroizing::new(serde_json::to_string(credentials)?);
        let provider = self.cloud_provider(provider_id)?;
        self.save_provider_credentials(
            provider_id,
            &provider.vendor,
            &provider.name,
            encoded.as_str(),
        )
    }

    pub(in crate::database) fn load_cloud_credentials(
        &self,
        provider_id: &str,
    ) -> Result<Option<crate::cloud_sync::CloudProviderCredentials>, Box<dyn Error>> {
        let Some(encoded) = self.load_provider_credentials(provider_id)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_str(encoded.as_str())?))
    }

    pub(in crate::database) fn has_cloud_credentials(
        &self,
        provider_id: &str,
    ) -> Result<bool, Box<dyn Error>> {
        Ok(self.has_provider_credentials(provider_id)?)
    }

    pub(in crate::database) fn delete_cloud_credentials(
        &self,
        provider_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        Ok(self.delete_provider_credentials(provider_id)?)
    }

    pub fn device_id(&self) -> rusqlite::Result<String> {
        self.connection.query_row(
            "SELECT value FROM app_metadata WHERE key = ?",
            params![DEVICE_ID_METADATA_KEY],
            |row| row.get(0),
        )
    }

    #[cfg(test)]
    pub(crate) fn sync_local_file(
        &mut self,
        path: &str,
        master_key: &str,
    ) -> Result<crate::sync::LocalSyncResult, crate::sync::SyncError> {
        crate::sync::sync_local_file(&mut self.connection, path, Some(master_key))
    }

    #[cfg(test)]
    pub(in crate::database) fn sync_local_file_with_saved_key(
        &mut self,
        path: &str,
        master_key: Option<&str>,
    ) -> Result<crate::sync::LocalSyncResult, crate::sync::SyncError> {
        crate::sync::sync_local_file(&mut self.connection, path, master_key)
    }

    #[cfg(test)]
    pub(crate) fn rotate_local_file_master_key(
        &mut self,
        path: &str,
        current_master_key: &str,
        new_master_key: &str,
    ) -> Result<crate::sync::LocalSyncResult, crate::sync::SyncError> {
        crate::sync::rotate_local_file_master_key(
            &mut self.connection,
            path,
            current_master_key,
            new_master_key,
        )
    }

    pub(in crate::database) fn sync_provider_staging_file(
        &mut self,
        path: &str,
        master_key: Option<&str>,
        revision_scope: &str,
        strategy: crate::sync::SyncStrategy,
    ) -> Result<crate::sync::LocalSyncResult, crate::sync::SyncError> {
        crate::sync::sync_provider_staging_file(
            &mut self.connection,
            path,
            master_key,
            revision_scope,
            strategy,
        )
    }

    pub(in crate::database) fn rotate_provider_staging_file_master_key(
        &mut self,
        path: &str,
        current_master_key: &str,
        new_master_key: &str,
        revision_scope: &str,
    ) -> Result<crate::sync::LocalSyncResult, crate::sync::SyncError> {
        crate::sync::rotate_provider_staging_file_master_key(
            &mut self.connection,
            path,
            current_master_key,
            new_master_key,
            revision_scope,
        )
    }

    pub(in crate::database) fn github_client(
        &self,
    ) -> Result<crate::github_sync::GithubClient, Box<dyn Error>> {
        let token = self
            .load_github_pat()?
            .ok_or_else(|| Box::<dyn Error>::from("GitHub token has not been configured."))?;
        let cfg_text = self.sync_provider_config("github_repository")?;
        let cfg_text = cfg_text
            .ok_or_else(|| Box::<dyn Error>::from("GitHub sync repository is not configured."))?;
        let cfg: crate::github_sync::GithubRepoConfig = serde_json::from_str(&cfg_text)?;
        Ok(crate::github_sync::GithubClient::new(
            token.to_string(),
            cfg,
        )?)
    }

    pub(in crate::database) fn github_gist_config(
        &self,
    ) -> Result<crate::github_gist_sync::GithubGistConfig, Box<dyn Error>> {
        let value = self.sync_provider_config("github_gist")?;
        match value {
            Some(text) => Ok(serde_json::from_str(&text)?),
            None => Ok(crate::github_gist_sync::GithubGistConfig::default()),
        }
    }

    pub(in crate::database) fn save_github_gist_config(
        &self,
        config: &crate::github_gist_sync::GithubGistConfig,
    ) -> Result<(), Box<dyn Error>> {
        let text = serde_json::to_string(config)?;
        self.save_sync_provider_config("github_gist", "github_gist", "GitHub Gist", &text)?;
        Ok(())
    }

    pub(in crate::database) fn github_gist_client(
        &self,
    ) -> Result<crate::github_gist_sync::GithubGistClient, Box<dyn Error>> {
        let token = self
            .load_github_gist_token()?
            .ok_or_else(|| Box::<dyn Error>::from("GitHub Gist is not connected."))?;
        Ok(crate::github_gist_sync::GithubGistClient::new(
            token.to_string(),
            self.github_gist_config()?,
        )?)
    }

    pub(in crate::database) fn s3_config(
        &self,
    ) -> Result<Option<crate::s3_sync::S3Config>, Box<dyn Error>> {
        let value = self.sync_provider_config("s3")?;
        value
            .map(|text| serde_json::from_str(&text).map_err(Box::<dyn Error>::from))
            .transpose()
    }

    pub(in crate::database) fn save_s3_config(
        &self,
        config: &crate::s3_sync::S3Config,
    ) -> Result<(), Box<dyn Error>> {
        crate::s3_sync::validate_config(config)?;
        let text = serde_json::to_string(config)?;
        self.save_sync_provider_config("s3", "s3_compatible", "S3 Compatible", &text)?;
        Ok(())
    }

    pub(in crate::database) fn sync_provider_config(
        &self,
        provider_id: &str,
    ) -> Result<Option<String>, Box<dyn Error>> {
        Ok(self
            .connection
            .query_row(
                "SELECT config_json FROM sync_providers WHERE uuid = ?",
                params![provider_id],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub(in crate::database) fn save_sync_provider_config(
        &self,
        uuid: &str,
        provider: &str,
        name: &str,
        config_json: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.connection.execute(
            r#"INSERT INTO sync_providers (uuid, provider, name, config_json)
               VALUES (?, ?, ?, ?)
               ON CONFLICT(uuid) DO UPDATE SET
                 provider = excluded.provider,
                 name = excluded.name,
                 config_json = excluded.config_json,
                 updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)"#,
            params![uuid, provider, name, config_json],
        )?;
        Ok(())
    }

    pub(in crate::database) fn delete_sync_provider_config(
        &self,
        provider_id: &str,
    ) -> Result<bool, Box<dyn Error>> {
        let deleted = self.connection.execute(
            "DELETE FROM sync_providers WHERE uuid = ?",
            params![provider_id],
        )? > 0;
        Ok(deleted)
    }

    pub(in crate::database) fn s3_client(
        &self,
    ) -> Result<crate::object_sync::OpenDalSyncTransport, Box<dyn Error>> {
        let config = self
            .s3_config()?
            .ok_or_else(|| Box::<dyn Error>::from("S3 sync is not configured."))?;
        let credentials = self
            .load_s3_credentials()?
            .ok_or_else(|| Box::<dyn Error>::from("S3 credentials have not been configured."))?;
        Ok(crate::s3_sync::build_transport(config, credentials)?)
    }

    pub(in crate::database) fn cloud_providers(
        &self,
    ) -> Result<Vec<crate::cloud_sync::CloudProviderConfig>, Box<dyn Error>> {
        let mut statement = self.connection.prepare(
            "SELECT uuid, provider, name, config_json FROM sync_providers
             WHERE uuid NOT IN ('github_repository', 'github_gist', 's3')
             ORDER BY created_at ASC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(id, vendor, name, encoded)| {
                let value: Value = serde_json::from_str(&encoded)?;
                let scheme = value
                    .get("scheme")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let config = value
                    .get("config")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?
                    .unwrap_or_default();
                Ok(crate::cloud_sync::CloudProviderConfig {
                    id,
                    scheme,
                    vendor,
                    name,
                    config,
                })
            })
            .collect()
    }

    pub(in crate::database) fn cloud_provider_summaries(
        &self,
    ) -> Result<Vec<crate::cloud_sync::CloudProviderSummary>, Box<dyn Error>> {
        self.cloud_providers()?
            .into_iter()
            .map(|provider| {
                Ok(crate::cloud_sync::CloudProviderSummary {
                    has_credentials: self.has_cloud_credentials(&provider.id)?,
                    provider,
                })
            })
            .collect()
    }

    pub(in crate::database) fn save_cloud_provider(
        &self,
        provider: crate::cloud_sync::CloudProviderConfig,
    ) -> Result<crate::cloud_sync::CloudProviderConfig, Box<dyn Error>> {
        crate::cloud_sync::validate_provider(&provider)?;
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sync_providers WHERE uuid = ?)",
            params![provider.id],
            |row| row.get(0),
        )?;
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM sync_providers
             WHERE uuid NOT IN ('github_repository', 'github_gist', 's3')",
            [],
            |row| row.get(0),
        )?;
        if count >= 32 && !exists {
            return Err(Box::<dyn Error>::from(
                "A maximum of 32 cloud providers can be configured.",
            ));
        }
        let encoded = serde_json::to_string(&json!({
            "scheme": &provider.scheme,
            "config": &provider.config,
        }))?;
        self.connection.execute(
            r#"INSERT INTO sync_providers (uuid, provider, name, config_json)
               VALUES (?, ?, ?, ?)
               ON CONFLICT(uuid) DO UPDATE SET
                 provider = excluded.provider,
                 name = excluded.name,
                 config_json = excluded.config_json,
                 updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)"#,
            params![provider.id, provider.vendor, provider.name, encoded],
        )?;
        Ok(provider)
    }

    pub(in crate::database) fn delete_cloud_provider(
        &self,
        provider_id: &str,
    ) -> Result<bool, Box<dyn Error>> {
        let deleted = self.connection.execute(
            "DELETE FROM sync_providers WHERE uuid = ?",
            params![provider_id],
        )? > 0;
        Ok(deleted)
    }

    pub(in crate::database) fn cloud_provider(
        &self,
        provider_id: &str,
    ) -> Result<crate::cloud_sync::CloudProviderConfig, Box<dyn Error>> {
        self.cloud_providers()?
            .into_iter()
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| Box::<dyn Error>::from("Cloud provider was not found."))
    }

    fn prepared_cloud_provider(
        &mut self,
        provider_id: &str,
    ) -> Result<
        (
            crate::cloud_sync::CloudProviderConfig,
            crate::cloud_sync::CloudProviderCredentials,
        ),
        Box<dyn Error>,
    > {
        let (_, provider, credentials) = self.prepared_cloud_provider_checked(provider_id, None)?;
        Ok((provider, credentials))
    }

    pub(in crate::database) fn prepared_cloud_provider_checked(
        &mut self,
        provider_id: &str,
        expected: Option<&SyncProviderRecord>,
    ) -> Result<
        (
            SyncProviderRecord,
            crate::cloud_sync::CloudProviderConfig,
            crate::cloud_sync::CloudProviderCredentials,
        ),
        Box<dyn Error>,
    > {
        let (mut record, provider, mut credentials) =
            self.atomic(|database| -> Result<_, Box<dyn Error>> {
                let record = database
                    .get_sync_provider_record(provider_id)?
                    .ok_or("Cloud provider was not found.")?;
                if let Some(expected) = expected {
                    super::super::sync_providers::verify(Some(&record), Some(expected))?;
                }
                let provider = database.cloud_provider(provider_id)?;
                let credentials = database
                    .load_cloud_credentials(provider_id)?
                    .ok_or("Cloud provider credentials are missing.")?;
                Ok((record, provider, credentials))
            })?;
        if matches!(
            provider.scheme.as_str(),
            "gcs" | "gdrive" | "onedrive" | "dropbox"
        ) {
            let mut refreshed =
                crate::cloud_sync::refresh_oauth_access_token(&provider, &credentials)?;
            if let Some(rotated_refresh_token) = refreshed.refresh_token.take() {
                let changed = credentials
                    .values
                    .get("refresh_token")
                    .is_none_or(|current| current != &rotated_refresh_token);
                credentials
                    .values
                    .insert("refresh_token".to_string(), rotated_refresh_token);
                if changed {
                    record = self.atomic(|database| -> Result<_, Box<dyn Error>> {
                        let current = database.get_sync_provider_record(provider_id)?;
                        super::super::sync_providers::verify(current.as_ref(), Some(&record))?;
                        database.save_cloud_credentials(provider_id, &credentials)?;
                        database
                            .get_sync_provider_record(provider_id)?
                            .ok_or_else(|| "Cloud provider was removed.".into())
                    })?;
                }
            }
            credentials.values.insert(
                "access_token".to_string(),
                std::mem::take(&mut refreshed.access_token),
            );
        }
        Ok((record, provider, credentials))
    }

    pub(in crate::database) fn cloud_provider_client(
        &mut self,
        provider_id: &str,
    ) -> Result<crate::object_sync::OpenDalSyncTransport, Box<dyn Error>> {
        let (provider, credentials) = self.prepared_cloud_provider(provider_id)?;
        Ok(crate::cloud_sync::build_transport(&provider, credentials)?)
    }

    pub(in crate::database) fn cloud_provider_list_history(
        &mut self,
        provider_id: &str,
        limit: usize,
    ) -> Result<Vec<crate::object_sync::ObjectVersionSummary>, Box<dyn Error>> {
        let (provider, mut credentials) = self.prepared_cloud_provider(provider_id)?;
        if matches!(provider.scheme.as_str(), "gdrive" | "onedrive" | "dropbox") {
            let access_token = credentials
                .values
                .remove("access_token")
                .ok_or_else(|| Box::<dyn Error>::from("OAuth access token is missing."))?;
            return match provider.scheme.as_str() {
                "gdrive" => Ok(crate::cloud_sync::GoogleDriveHistoryClient::new(
                    &provider,
                    access_token,
                )?
                .list_versions(limit)?),
                "onedrive" => Ok(crate::cloud_sync::OneDriveHistoryClient::new(
                    &provider,
                    access_token,
                )?
                .list_versions(limit)?),
                "dropbox" => Ok(crate::cloud_sync::DropboxHistoryClient::new(
                    &provider,
                    access_token,
                )?
                .list_versions(limit)?),
                _ => unreachable!(),
            };
        }
        Ok(crate::cloud_sync::build_transport(&provider, credentials)?.list_versions(limit)?)
    }

    /// End-to-end GitHub sync: pull remote → merge with local via the internal sync helper →
    /// push a new encrypted revision. Uses a private temp file that is deleted at the end so
    /// the caller never has to manage a local sync-file path.
    pub(crate) fn github_sync(
        &mut self,
        provided_master_key: Option<&str>,
        strategy: crate::sync::SyncStrategy,
    ) -> Result<Value, Box<dyn Error>> {
        self.github_sync_with_master_key_rotation(provided_master_key, None, Some(strategy))
    }

    pub(crate) fn github_restore_revision(
        &mut self,
        commit_sha: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.restore_pinned_sync_history("github_repository", commit_sha)
    }

    pub(crate) fn github_change_master_key(
        &mut self,
        current_master_key: &str,
        new_master_key: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.github_sync_with_master_key_rotation(
            Some(current_master_key),
            Some(new_master_key),
            None,
        )
    }

    fn github_sync_with_master_key_rotation(
        &mut self,
        provided_master_key: Option<&str>,
        new_master_key: Option<&str>,
        strategy_override: Option<crate::sync::SyncStrategy>,
    ) -> Result<Value, Box<dyn Error>> {
        if let Some(master_key) = provided_master_key {
            crate::crypto::validate_master_key(master_key)?;
        }

        let client = self.github_client()?;
        self.github_sync_with_client(
            client,
            provided_master_key,
            new_master_key,
            strategy_override,
        )
    }

    pub(in crate::database) fn github_sync_with_client(
        &mut self,
        client: crate::github_sync::GithubClient,
        provided_master_key: Option<&str>,
        new_master_key: Option<&str>,
        strategy_override: Option<crate::sync::SyncStrategy>,
    ) -> Result<Value, Box<dyn Error>> {
        let strategy = strategy_override.unwrap_or_default();

        let temp_dir = env::temp_dir();
        fs::create_dir_all(&temp_dir).ok();
        // A collision-resistant, non-persistent path. Only needs uniqueness per process run.
        let unique = format!(
            "nauterm-vault-{}-{}.enc",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let temp_path = temp_dir.join(unique);

        let outcome = (|| -> Result<Value, Box<dyn Error>> {
            let path_str = temp_path.to_string_lossy().to_string();
            let mut latest_stats = None;
            let sha = crate::github_sync::push_with_conflict_retry(
                &client,
                |remote| {
                    if let Some(remote_bytes) = remote {
                        fs::write(&temp_path, remote_bytes).map_err(|error| {
                            crate::github_sync::GithubSyncError::new(format!(
                                "Unable to stage the remote vault: {error}"
                            ))
                        })?;
                    } else if temp_path.exists() {
                        fs::remove_file(&temp_path).map_err(|error| {
                            crate::github_sync::GithubSyncError::new(format!(
                                "Unable to reset the temporary vault: {error}"
                            ))
                        })?;
                    }
                    let stats = if let Some(new_master_key) = new_master_key {
                        self.rotate_provider_staging_file_master_key(
                            &path_str,
                            provided_master_key.expect("rotation requires current Master Key"),
                            new_master_key,
                            "github_repository",
                        )
                    } else {
                        self.sync_provider_staging_file(
                            &path_str,
                            provided_master_key,
                            "github_repository",
                            strategy,
                        )
                    }
                    .map_err(|error| crate::github_sync::GithubSyncError::new(error.to_string()))?;
                    let bytes = fs::read(&temp_path).map_err(|error| {
                        crate::github_sync::GithubSyncError::new(format!(
                            "Unable to read the merged vault: {error}"
                        ))
                    })?;
                    latest_stats = Some(stats);
                    Ok(bytes)
                },
                "vault sync",
            )?;
            let stats = latest_stats.ok_or_else(|| {
                Box::<dyn Error>::from("GitHub sync completed without producing a vault.")
            })?;
            self.remember_sync_snapshot(stats.revision, &stats.snapshot_id)?;
            self.remember_sync_provider_checkpoint(
                "github_repository",
                stats.revision,
                &stats.snapshot_id,
            )?;
            Ok(json!({
                "commit_sha": sha,
                "revision": stats.revision,
                "snapshot_id": stats.snapshot_id,
                "imported_records": stats.imported_records,
                "total_records": stats.total_records,
                "created": stats.created,
                "synced_at": stats.synced_at,
            }))
        })();

        let _ = fs::remove_file(&temp_path);
        outcome
    }

    pub(crate) fn github_gist_sync(
        &mut self,
        provided_master_key: Option<&str>,
        strategy: crate::sync::SyncStrategy,
    ) -> Result<Value, Box<dyn Error>> {
        self.github_gist_sync_with_master_key_rotation(provided_master_key, None, Some(strategy))
    }

    pub(crate) fn github_gist_restore_version(
        &mut self,
        version: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.restore_pinned_sync_history("github_gist", version)
    }

    pub(crate) fn github_gist_change_master_key(
        &mut self,
        current_master_key: &str,
        new_master_key: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.github_gist_sync_with_master_key_rotation(
            Some(current_master_key),
            Some(new_master_key),
            None,
        )
    }

    fn github_gist_sync_with_master_key_rotation(
        &mut self,
        provided_master_key: Option<&str>,
        new_master_key: Option<&str>,
        strategy_override: Option<crate::sync::SyncStrategy>,
    ) -> Result<Value, Box<dyn Error>> {
        if let Some(master_key) = provided_master_key {
            crate::crypto::validate_master_key(master_key)?;
        }
        let client = self.github_gist_client()?;
        self.github_gist_sync_with_client(
            client,
            provided_master_key,
            new_master_key,
            strategy_override,
        )
    }

    pub(in crate::database) fn github_gist_sync_with_client(
        &mut self,
        client: crate::github_gist_sync::GithubGistClient,
        provided_master_key: Option<&str>,
        new_master_key: Option<&str>,
        strategy_override: Option<crate::sync::SyncStrategy>,
    ) -> Result<Value, Box<dyn Error>> {
        let temp_dir = env::temp_dir();
        fs::create_dir_all(&temp_dir).ok();
        let unique = format!(
            "nauterm-gist-{}-{}.enc",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        );
        let temp_path = temp_dir.join(unique);

        let outcome = (|| -> Result<Value, Box<dyn Error>> {
            match client.fetch_current()? {
                Some(remote_bytes) => fs::write(&temp_path, remote_bytes)?,
                None if temp_path.exists() => fs::remove_file(&temp_path)?,
                None => {}
            }
            let path = temp_path.to_string_lossy().to_string();
            let stats = if let Some(new_master_key) = new_master_key {
                self.rotate_provider_staging_file_master_key(
                    &path,
                    provided_master_key.expect("rotation requires current Master Key"),
                    new_master_key,
                    "github_gist",
                )
            } else {
                self.sync_provider_staging_file(
                    &path,
                    provided_master_key,
                    "github_gist",
                    strategy_override.unwrap_or_default(),
                )
            }?;
            let bytes = fs::read(&temp_path)?;
            let write = client.write(&bytes)?;
            let mut config = client.config().clone();
            if config.gist_id != write.gist_id {
                config.gist_id = write.gist_id.clone();
                self.save_github_gist_config(&config)?;
            }
            self.remember_sync_snapshot(stats.revision, &stats.snapshot_id)?;
            self.remember_sync_provider_checkpoint(
                "github_gist",
                stats.revision,
                &stats.snapshot_id,
            )?;
            Ok(json!({
                "gist_id": write.gist_id,
                "version": write.version,
                "revision": stats.revision,
                "snapshot_id": stats.snapshot_id,
                "imported_records": stats.imported_records,
                "total_records": stats.total_records,
                "created": stats.created,
                "synced_at": stats.synced_at,
            }))
        })();

        let _ = fs::remove_file(&temp_path);
        outcome
    }

    pub(crate) fn s3_sync(
        &mut self,
        provided_master_key: Option<&str>,
        strategy: crate::sync::SyncStrategy,
    ) -> Result<Value, Box<dyn Error>> {
        self.s3_sync_with_master_key_rotation(provided_master_key, None, Some(strategy))
    }

    pub(crate) fn s3_restore_version(&mut self, version_id: &str) -> Result<Value, Box<dyn Error>> {
        self.restore_pinned_sync_history("s3", version_id)
    }

    pub(crate) fn s3_change_master_key(
        &mut self,
        current_master_key: &str,
        new_master_key: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.s3_sync_with_master_key_rotation(Some(current_master_key), Some(new_master_key), None)
    }

    fn s3_sync_with_master_key_rotation(
        &mut self,
        provided_master_key: Option<&str>,
        new_master_key: Option<&str>,
        strategy_override: Option<crate::sync::SyncStrategy>,
    ) -> Result<Value, Box<dyn Error>> {
        let client = self.s3_client()?;
        self.object_provider_sync_with_master_key_rotation(
            client,
            "s3",
            "s3",
            provided_master_key,
            new_master_key,
            strategy_override,
        )
    }

    pub(crate) fn cloud_provider_sync(
        &mut self,
        provider_id: &str,
        provided_master_key: Option<&str>,
        strategy: crate::sync::SyncStrategy,
    ) -> Result<Value, Box<dyn Error>> {
        self.cloud_provider_sync_with_master_key_rotation(
            provider_id,
            provided_master_key,
            None,
            Some(strategy),
        )
    }

    pub(crate) fn cloud_provider_restore_version(
        &mut self,
        provider_id: &str,
        version_id: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.restore_pinned_sync_history(&format!("cloud:{provider_id}"), version_id)
    }

    pub(crate) fn cloud_provider_change_master_key(
        &mut self,
        provider_id: &str,
        current_master_key: &str,
        new_master_key: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.cloud_provider_sync_with_master_key_rotation(
            provider_id,
            Some(current_master_key),
            Some(new_master_key),
            None,
        )
    }

    fn cloud_provider_sync_with_master_key_rotation(
        &mut self,
        provider_id: &str,
        provided_master_key: Option<&str>,
        new_master_key: Option<&str>,
        strategy_override: Option<crate::sync::SyncStrategy>,
    ) -> Result<Value, Box<dyn Error>> {
        let client = self.cloud_provider_client(provider_id)?;
        let scope = format!("cloud:{provider_id}");
        let mut value = self.object_provider_sync_with_master_key_rotation(
            client,
            &scope,
            "cloud",
            provided_master_key,
            new_master_key,
            strategy_override,
        )?;
        value["provider_id"] = json!(provider_id);
        Ok(value)
    }

    pub(in crate::database) fn object_provider_sync_with_master_key_rotation(
        &mut self,
        client: crate::object_sync::OpenDalSyncTransport,
        revision_scope: &str,
        temp_prefix: &str,
        provided_master_key: Option<&str>,
        new_master_key: Option<&str>,
        strategy_override: Option<crate::sync::SyncStrategy>,
    ) -> Result<Value, Box<dyn Error>> {
        if let Some(master_key) = provided_master_key {
            crate::crypto::validate_master_key(master_key)?;
        }
        let temp_path = env::temp_dir().join(format!(
            "nauterm-{temp_prefix}-{}-{}.enc",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let outcome = (|| -> Result<Value, Box<dyn Error>> {
            let path = temp_path.to_string_lossy().to_string();
            let mut backoff = std::time::Duration::from_millis(250);
            for attempt in 0..3 {
                let remote = match client.fetch_current() {
                    Ok(remote) => remote,
                    Err(error)
                        if crate::object_sync::OpenDalSyncTransport::is_conflict(&error)
                            && attempt < 2 =>
                    {
                        std::thread::sleep(backoff);
                        backoff *= 2;
                        continue;
                    }
                    Err(error) => return Err(Box::new(error)),
                };
                match remote.as_ref() {
                    Some(remote) => write_private_staging_file(&temp_path, &remote.bytes)?,
                    None if temp_path.exists() => fs::remove_file(&temp_path)?,
                    None => {}
                }
                let metadata_snapshot = self.snapshot_sync_metadata()?;
                let stats = if let Some(new_master_key) = new_master_key {
                    self.rotate_provider_staging_file_master_key(
                        &path,
                        provided_master_key.expect("rotation requires current Master Key"),
                        new_master_key,
                        revision_scope,
                    )
                } else {
                    self.sync_provider_staging_file(
                        &path,
                        provided_master_key,
                        revision_scope,
                        strategy_override.unwrap_or_default(),
                    )
                };
                let stats = match stats {
                    Ok(stats) => stats,
                    Err(error) => {
                        self.restore_sync_metadata(&metadata_snapshot)?;
                        return Err(Box::new(error));
                    }
                };
                let bytes = match fs::read(&temp_path) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        self.restore_sync_metadata(&metadata_snapshot)?;
                        return Err(Box::new(error));
                    }
                };
                match client.write(&bytes, remote.as_ref().map(|value| value.etag.as_str())) {
                    Ok(write) => {
                        self.remember_sync_snapshot(stats.revision, &stats.snapshot_id)?;
                        let provider_id = revision_scope
                            .strip_prefix("cloud:")
                            .unwrap_or(revision_scope);
                        self.remember_sync_provider_checkpoint(
                            provider_id,
                            stats.revision,
                            &stats.snapshot_id,
                        )?;
                        return Ok(json!({
                            "etag": write.etag,
                            "version_id": write.version_id,
                            "revision": stats.revision,
                            "snapshot_id": stats.snapshot_id,
                            "imported_records": stats.imported_records,
                            "total_records": stats.total_records,
                            "created": stats.created,
                            "synced_at": stats.synced_at,
                        }));
                    }
                    Err(error) => {
                        self.restore_sync_metadata(&metadata_snapshot)?;
                        if !crate::object_sync::OpenDalSyncTransport::is_conflict(&error)
                            || attempt == 2
                        {
                            return Err(Box::new(error));
                        }
                        std::thread::sleep(backoff);
                        backoff *= 2;
                    }
                }
            }
            Err(Box::<dyn Error>::from(
                "Cloud storage sync retries exhausted.",
            ))
        })();
        let _ = fs::remove_file(&temp_path);
        outcome
    }

    fn snapshot_sync_metadata(&self) -> Result<SyncMetadataSnapshot, Box<dyn Error>> {
        let mut statement = self.connection.prepare(
            "SELECT key, value, updated_at FROM app_metadata
             WHERE key IN ('sync_dek', 'sync_vault_id', 'sync_envelope_header')
                OR key LIKE 'sync_revision:%'",
        )?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn restore_sync_metadata(
        &mut self,
        snapshot: &[(String, String, i64)],
    ) -> Result<(), Box<dyn Error>> {
        let transaction = self.connection.savepoint()?;
        transaction.execute(
            "DELETE FROM app_metadata
             WHERE key IN ('sync_dek', 'sync_vault_id', 'sync_envelope_header')
                OR key LIKE 'sync_revision:%'",
            [],
        )?;
        for (key, value, updated_at) in snapshot {
            transaction.execute(
                "INSERT INTO app_metadata (key, value, updated_at) VALUES (?, ?, ?)",
                params![key, value, updated_at],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod backup_policy_tests {
    use super::*;
    #[test]
    fn backup_pruning_keeps_the_restore_source_until_copying_finishes() {
        let files: Vec<_> = [
            "oldest.sqlite",
            "middle.sqlite",
            "recent.sqlite",
            "pre-restore.sqlite",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect();
        let protected = files[0].as_path();
        let before_copy = backup_prune_plan(&files, 2, &[protected, files[3].as_path()]);
        assert_eq!(before_copy, files[1..3]);
        assert!(!before_copy.contains(&files[0]));
        let after_copy = backup_prune_plan(
            &[files[0].clone(), files[3].clone()],
            1,
            &[files[3].as_path()],
        );
        assert_eq!(after_copy, files[..1]);
        assert!(backup_prune_plan(&files, 10, &[protected]).is_empty());
        let reversed = [files[3].clone(), files[0].clone()];
        assert_eq!(
            backup_prune_plan(&reversed, 1, &[reversed[0].as_path()]),
            files[..1]
        );
    }
}
