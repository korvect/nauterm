//! Shared, descriptor-scoped coordination for sync/key operations in both
//! clients. SQLite transactions stay short; the lease spans network work too.
use super::*;
use std::fs::{File, OpenOptions, TryLockError};

struct Lease {
    _file: File,
}
impl Lease {
    fn acquire(path: Option<&Path>) -> Result<Option<Self>, Box<dyn Error>> {
        let Some(path) = path.filter(|path| !path.as_os_str().is_empty()) else {
            return Ok(None);
        };
        let path = fs::canonicalize(path)?;
        let mut name = path
            .file_name()
            .ok_or("The sync database path is invalid.")?
            .to_os_string();
        name.push(".sync.lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(path.with_file_name(name))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(TryLockError::WouldBlock) => Err("Another Nauterm synchronization or key operation is running. Try again when it finishes.".into()),
            Err(TryLockError::Error(error)) => Err(error.into()),
        }
    }
}
impl NautermDatabase {
    pub(in crate::database) fn with_sync_operation<T>(
        &mut self,
        work: impl FnOnce(&mut Self) -> Result<T, Box<dyn Error>>,
    ) -> Result<T, Box<dyn Error>> {
        if self.sync_operation_depth != 0 {
            return work(self);
        }
        let lease = Lease::acquire(self.connection.path().map(Path::new))?;
        self.sync_operation_depth = 1;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(self)));
        self.sync_operation_depth = 0;
        // Never unlink the stable sidecar: another process may have opened it.
        drop(lease);
        match result {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
}
impl DatabaseRequest {
    pub(super) fn requires_sync_lease(&self) -> bool {
        if let Self::SetAppMeta { key, .. } = self {
            return key.starts_with("sync_");
        }
        matches!(
            self,
            Self::SetMasterKey { .. }
                | Self::ChangeMasterKey { .. }
                | Self::RemoveMasterKey { .. }
                | Self::ForgetSyncKey
                | Self::SaveSyncPreferences { .. }
                | Self::RefreshRemoteSyncStatus
                | Self::GithubSaveToken { .. }
                | Self::GithubDeleteToken
                | Self::GithubSaveConfig { .. }
                | Self::GithubDeleteConfig
                | Self::GithubGistSaveToken { .. }
                | Self::GithubGistDeleteToken
                | Self::GithubGistSaveConfig { .. }
                | Self::GithubGistDeleteConfig
                | Self::S3SaveCredentials { .. }
                | Self::S3DeleteCredentials
                | Self::S3SaveConfig { .. }
                | Self::S3DeleteConfig
                | Self::CloudSaveProvider { .. }
                | Self::CloudDeleteProvider { .. }
                | Self::GithubSync { .. }
                | Self::GithubRestoreRevision { .. }
                | Self::GithubChangeMasterKey { .. }
                | Self::GithubListHistory { .. }
                | Self::GithubGistSync { .. }
                | Self::GithubGistRestoreVersion { .. }
                | Self::GithubGistChangeMasterKey { .. }
                | Self::GithubGistListHistory { .. }
                | Self::S3Sync { .. }
                | Self::S3RestoreVersion { .. }
                | Self::S3ChangeMasterKey { .. }
                | Self::S3ListHistory { .. }
                | Self::CloudSync { .. }
                | Self::CloudRestoreVersion { .. }
                | Self::CloudChangeMasterKey { .. }
                | Self::CloudListHistory { .. }
                | Self::LocalSyncBackupRestore { .. }
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_handles_share_one_stable_lease_and_release_it_on_drop(
    ) -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("synthetic.data");
        File::create(&path)?;
        let owner = Lease::acquire(Some(&path))?;
        assert!(Lease::acquire(Some(&path)).is_err());
        drop(owner);
        let next = Lease::acquire(Some(&path))?;
        assert!(directory.path().join("synthetic.data.sync.lock").is_file());
        drop(next);
        assert!(Lease::acquire(None)?.is_none());
        Ok(())
    }
    #[test]
    fn legacy_mutations_and_remote_reads_use_the_same_operation_boundary() {
        for request in [
            r#"{"op":"github_sync"}"#,
            r#"{"op":"forget_sync_key"}"#,
            r#"{"op":"github_list_history"}"#,
            r#"{"op":"cloud_sync","provider_id":"synthetic"}"#,
            r#"{"op":"github_change_master_key","current_master_key":"synthetic-current","new_master_key":"synthetic-new"}"#,
            r#"{"op":"local_sync_backup_restore","backup_id":"synthetic"}"#,
        ] {
            let request: DatabaseRequest = serde_json::from_str(request).unwrap();
            assert!(request.requires_sync_lease());
        }
        for request in [
            r#"{"op":"list_hosts"}"#,
            r#"{"op":"sync_key_status"}"#,
            r#"{"op":"local_sync_backup_list"}"#,
        ] {
            let request: DatabaseRequest = serde_json::from_str(request).unwrap();
            assert!(!request.requires_sync_lease());
        }
    }
}
