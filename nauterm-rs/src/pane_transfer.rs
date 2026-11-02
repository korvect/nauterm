//! Cross-pane plans capture both SSH endpoints. Relay stages share the actual
//! transfer queue; a failed/paused stage retains its private payload lease.
use crate::{
    file_model::Entry,
    file_operation::Operation,
    native::{Core, SshOptions},
    transfer_queue::{Service, Spec},
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub enum Endpoint {
    Local(PathBuf),
    Remote {
        directory: String,
        options: Arc<SshOptions>,
    },
}
#[derive(Clone)]
enum Work {
    Direct {
        operation: Operation,
        options: Arc<SshOptions>,
    },
    Relay {
        download: Operation,
        upload: Operation,
        source: Arc<SshOptions>,
        target: Arc<SshOptions>,
        lease: Arc<tempfile::TempDir>,
    },
}
impl Work {
    fn target_operation(&self) -> &Operation {
        match self {
            Self::Direct { operation, .. } => operation,
            Self::Relay { upload, .. } => upload,
        }
    }
    fn validate(&self) -> Result<()> {
        self.target_operation().validate()?;
        if let Self::Relay { download, .. } = self {
            download.validate()?;
        }
        Ok(())
    }
    fn accepts(&self, resolved: &Operation) -> bool {
        let original = self.target_operation();
        match (original, resolved) {
            (
                Operation::Upload {
                    local_path,
                    remote_path,
                    ..
                },
                Operation::Upload {
                    local_path: source,
                    remote_path: target,
                    ..
                },
            ) => {
                local_path == source
                    && target
                        .rsplit_once('/')
                        .is_some_and(|(_, name)| !name.is_empty())
                    && remote_path.rsplit_once('/').map(|p| p.0)
                        == target.rsplit_once('/').map(|p| p.0)
            }
            (
                Operation::Download {
                    remote_path,
                    local_path,
                    ..
                },
                Operation::Download {
                    remote_path: source,
                    local_path: target,
                    ..
                },
            ) => {
                remote_path == source
                    && Path::new(local_path).parent() == Path::new(target).parent()
            }
            _ => false,
        }
    }
}
#[derive(Clone)]
pub struct Batch {
    work: Vec<Work>,
    source_owner: u64,
    target_owner: u64,
    core: Option<Arc<Core>>,
    pub kinds: BTreeMap<String, bool>,
}
fn text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .context("This filename cannot be represented by SFTP. No items were transferred.")
}
fn remote_child(directory: &str, name: &str) -> String {
    format!("{}/{name}", directory.trim_end_matches('/'))
}
fn private_relay() -> Result<Arc<tempfile::TempDir>> {
    let mut builder = tempfile::Builder::new();
    builder.prefix("nauterm-rs-relay-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    Ok(Arc::new(builder.tempdir()?))
}
fn protect(path: &Path) -> Result<()> {
    let mut pending = vec![path.to_owned()];
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path)?;
        ensure!(
            !metadata.file_type().is_symlink() && (metadata.is_file() || metadata.is_dir()),
            "Refusing a nonregular relay item."
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &path,
                std::fs::Permissions::from_mode(if metadata.is_dir() { 0o700 } else { 0o600 }),
            )?;
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
        }
    }
    Ok(())
}
impl Batch {
    pub fn plan(
        source: Endpoint,
        target: Endpoint,
        entries: Vec<Entry>,
        source_owner: u64,
        target_owner: u64,
        core: Option<Arc<Core>>,
    ) -> Result<Self> {
        ensure!(!entries.is_empty(), "Select at least one file.");
        ensure!(
            !matches!((&source, &target), (Endpoint::Local(_), Endpoint::Local(_))),
            "Local-to-local pane transfers are not available."
        );
        let strict = |endpoint| match endpoint {
            Endpoint::Remote { directory, options } => Endpoint::Remote {
                directory,
                options: Arc::new(options.for_new_connection()),
            },
            local => local,
        };
        let source = strict(source);
        let target = strict(target);
        let mut work = vec![];
        let mut kinds = BTreeMap::new();
        let mut targets = HashSet::new();
        for entry in entries {
            ensure!(
                !entry.parent
                    && !entry.name.is_empty()
                    && !entry.name.contains(['/', '\0'])
                    && ![".", ".."].contains(&entry.name.as_str()),
                "Invalid transfer filename."
            );
            let item = match (&source, &target) {
                (
                    Endpoint::Local(directory),
                    Endpoint::Remote {
                        directory: target,
                        options,
                    },
                ) => {
                    let path = entry
                        .native_path
                        .context("A local transfer needs an exact native path.")?;
                    ensure!(
                        path.parent() == Some(directory.as_path())
                            && path.file_name().and_then(|name| name.to_str()) == Some(&entry.name),
                        "The selected source directory changed."
                    );
                    let local_path = text(&path)?;
                    kinds.insert(local_path.clone(), entry.directory);
                    Work::Direct {
                        operation: Operation::Upload {
                            local_path,
                            remote_path: remote_child(target, &entry.name),
                            replace_existing: false,
                            transfer_threads: 8,
                        },
                        options: options.clone(),
                    }
                }
                (Endpoint::Remote { directory, options }, Endpoint::Local(target)) => {
                    ensure!(
                        entry.path == remote_child(directory, &entry.name),
                        "The remote source directory changed."
                    );
                    ensure!(
                        Path::new(&entry.name).components().count() == 1
                            && Path::new(&entry.name).file_name().is_some(),
                        "This filename cannot be represented on the local platform."
                    );
                    kinds.insert(entry.path.clone(), entry.directory);
                    Work::Direct {
                        operation: Operation::Download {
                            remote_path: entry.path,
                            local_path: text(&target.join(entry.name))?,
                            transfer_threads: 8,
                        },
                        options: options.clone(),
                    }
                }
                (
                    Endpoint::Remote {
                        directory,
                        options: source,
                    },
                    Endpoint::Remote {
                        directory: target,
                        options,
                    },
                ) => {
                    ensure!(
                        entry.path == remote_child(directory, &entry.name),
                        "The remote source directory changed."
                    );
                    let lease = private_relay()?;
                    // A fixed local leaf avoids interpreting a remote filename
                    // as a local-platform path; only its remote target keeps it.
                    let local_path = text(&lease.path().join("payload"))?;
                    kinds.insert(local_path.clone(), entry.directory);
                    Work::Relay {
                        download: Operation::Download {
                            remote_path: entry.path,
                            local_path: local_path.clone(),
                            transfer_threads: 8,
                        },
                        upload: Operation::Upload {
                            local_path,
                            remote_path: remote_child(target, &entry.name),
                            replace_existing: false,
                            transfer_threads: 8,
                        },
                        source: source.clone(),
                        target: options.clone(),
                        lease,
                    }
                }
                _ => unreachable!(),
            };
            item.validate()?;
            ensure!(
                targets.insert(item.target_operation().target().to_owned()),
                "Two selected items have the same destination."
            );
            work.push(item);
        }
        Ok(Self {
            work,
            source_owner,
            target_owner,
            core,
            kinds,
        })
    }
    pub fn download(&self) -> bool {
        matches!(self.work[0].target_operation(), Operation::Download { .. })
    }
    pub fn operations(&self) -> Vec<Operation> {
        self.work
            .iter()
            .map(|work| work.target_operation().clone())
            .collect()
    }
    pub fn display_source(&self, index: usize) -> Option<&str> {
        self.work.get(index).map(|work| match work {
            Work::Relay { download, .. } => download.source(),
            Work::Direct { operation, .. } => operation.source(),
        })
    }
    pub fn options(&self) -> Arc<SshOptions> {
        match &self.work[0] {
            Work::Direct { options, .. } => options.clone(),
            Work::Relay { target, .. } => target.clone(),
        }
    }
    pub fn enqueue(
        &self,
        queue: &Service,
        operations: Vec<Operation>,
        replacements: &HashSet<String>,
    ) -> Result<()> {
        self.enqueue_authorized(queue, operations, replacements, None)
    }
    pub fn enqueue_authorized(
        &self,
        queue: &Service,
        operations: Vec<Operation>,
        replacements: &HashSet<String>,
        sudo: Option<&Arc<crate::sftp_client::Sudo>>,
    ) -> Result<()> {
        ensure!(
            sudo.is_none()
                || self.work.iter().all(|work| matches!(
                    work,
                    Work::Direct {
                        operation: Operation::Upload { .. },
                        ..
                    } | Work::Relay { .. }
                )),
            "Destination sudo cannot elevate a source-side download."
        );
        ensure!(
            operations.len() == self.work.len(),
            "Transfer plan changed during confirmation."
        );
        let mut targets = HashSet::new();
        // Validate the whole confirmed plan before submitting its first job.
        // Conflict resolution may rename a destination, never its source,
        // direction or destination directory.
        for (work, operation) in self.work.iter().zip(&operations) {
            operation.validate()?;
            ensure!(
                work.accepts(operation),
                "Transfer source or destination directory changed during confirmation."
            );
            ensure!(
                targets.insert(operation.target()),
                "Two selected items have the same destination."
            );
        }
        for (work, operation) in self.work.iter().zip(operations) {
            match work {
                Work::Direct { options, .. } => {
                    let mut spec = Spec::new(self.target_owner, options.clone(), operation);
                    spec.sudo = sudo.cloned();
                    if sudo.is_some() {
                        spec.label = Some("Upload with sudo".into());
                    }
                    spec.replace_local = replacements.contains(spec.operation.target());
                    spec.core = self.core.clone();
                    spec.inherit_transfer_threads = true;
                    queue.enqueue(spec)?;
                }
                Work::Relay {
                    download,
                    source,
                    target,
                    lease,
                    ..
                } => {
                    let mut upload = Spec::new(self.target_owner, target.clone(), operation);
                    upload.sudo = sudo.cloned();
                    upload.core = self.core.clone();
                    upload.inherit_transfer_threads = true;
                    upload.label = Some(
                        if sudo.is_some() {
                            "Transfer upload with sudo"
                        } else {
                            "Transfer upload"
                        }
                        .into(),
                    );
                    upload.display_source = Some(download.source().into());
                    upload.payload = Some(lease.clone());
                    let upload = Mutex::new(Some(upload));
                    let weak = queue.downgrade();
                    let local = lease.path().join("payload");
                    let mut spec = Spec::new(self.source_owner, source.clone(), download.clone());
                    spec.core = self.core.clone();
                    spec.inherit_transfer_threads = true;
                    spec.label = Some("Transfer download".into());
                    spec.display_target = Some("Private transfer copy".into());
                    spec.payload = Some(lease.clone());
                    spec.refresh_owner = false;
                    spec.refresh_local = false;
                    spec.after = Some(Arc::new(move || {
                        protect(&local)?;
                        let upload = upload
                            .lock()
                            .unwrap()
                            .take()
                            .context("This transfer stage was already submitted.")?;
                        weak.upgrade()
                            .context("The transfer queue closed before upload.")?
                            .enqueue(upload)?;
                        Ok(())
                    }));
                    queue.enqueue(spec)?;
                }
            }
        }
        Ok(())
    }
}
