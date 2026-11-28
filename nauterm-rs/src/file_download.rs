//! Menu Download targets the system Downloads directory, not a native Save As
//! prompt or the other pane. Planning and directory I/O run off the UI thread.
use crate::{
    file_model::Entry,
    file_operation::Operation,
    native::{Core, SshOptions},
    transfer_queue::{Service, Spec},
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

pub fn directory(preferred: Option<PathBuf>, home: &Path) -> Result<PathBuf> {
    let path = preferred.unwrap_or_else(|| home.join("Downloads"));
    ensure!(
        path.is_absolute(),
        "Unable to resolve the system Downloads directory."
    );
    ensure!(
        path.to_str().is_some_and(|path| !path.contains('\0')),
        "The Downloads directory cannot be represented by SFTP."
    );
    Ok(path)
}
pub fn system_directory(home: &Path) -> Result<PathBuf> {
    directory(
        directories::UserDirs::new().and_then(|dirs| dirs.download_dir().map(Path::to_owned)),
        home,
    )
}
pub fn unique_target(directory: &Path, name: &str, reserved: &HashSet<PathBuf>) -> Result<PathBuf> {
    ensure!(
        directory.is_absolute(),
        "Choose an absolute download directory."
    );
    ensure!(
        !name.is_empty()
            && !name.contains(['/', '\0'])
            && ![".", ".."].contains(&name)
            && Path::new(name).components().count() == 1
            && Path::new(name).file_name().is_some(),
        "A remote filename is not valid on this local platform."
    );
    let exists = |path: &Path| -> Result<bool> {
        if reserved.contains(path) {
            return Ok(true);
        }
        match std::fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error).context("Could not check the local download destination."),
        }
    };
    let original = directory.join(name);
    if !exists(&original)? {
        return Ok(original);
    }
    let (stem, extension) = name
        .rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map(|(stem, extension)| (stem, format!(".{extension}")))
        .unwrap_or((name, String::new()));
    for index in 2.. {
        let path = directory.join(format!("{stem} {index}{extension}"));
        if !exists(&path)? {
            return Ok(path);
        }
    }
    unreachable!()
}
pub struct Request {
    pub entries: Vec<Entry>,
    pub source_directory: String,
    pub options: Arc<SshOptions>,
    pub owner: u64,
}
pub struct Plan {
    operations: Vec<Operation>,
    options: Arc<SshOptions>,
    owner: u64,
    core: Option<Arc<Core>>,
}
impl Request {
    pub fn prepare(
        self,
        destination: PathBuf,
        mut reserved: HashSet<PathBuf>,
        core: Option<Arc<Core>>,
    ) -> Result<Plan> {
        ensure!(!self.entries.is_empty(), "Select at least one file.");
        let directory = directory(Some(destination), Path::new(""))?;
        let mut sources = HashSet::new();
        // Validate every source/name before creating the Downloads directory.
        for entry in &self.entries {
            ensure!(
                !entry.parent && sources.insert(entry.path.clone()),
                "Invalid or duplicate download source."
            );
            ensure!(
                entry.path
                    == format!(
                        "{}/{}",
                        self.source_directory.trim_end_matches('/'),
                        entry.name
                    ),
                "The remote source directory changed."
            );
            let path = unique_target(&directory, &entry.name, &reserved)?;
            Operation::Download {
                remote_path: entry.path.clone(),
                local_path: path
                    .to_str()
                    .context("This filename cannot be represented by SFTP.")?
                    .into(),
                transfer_threads: 8,
            }
            .validate()?;
        }
        std::fs::create_dir_all(&directory)
            .context("Unable to prepare the system Downloads directory.")?;
        let mut operations = Vec::with_capacity(self.entries.len());
        for entry in self.entries {
            let path = unique_target(&directory, &entry.name, &reserved)?;
            reserved.insert(path.clone());
            operations.push(Operation::Download {
                remote_path: entry.path,
                local_path: path
                    .to_str()
                    .context("This filename cannot be represented by SFTP.")?
                    .into(),
                transfer_threads: 8,
            });
        }
        Ok(Plan {
            operations,
            options: Arc::new(self.options.for_new_connection()),
            owner: self.owner,
            core,
        })
    }
}
impl Plan {
    pub fn enqueue(&self, service: &Service) -> Result<()> {
        self.enqueue_authorized(service, None)
    }
    pub fn enqueue_authorized(
        &self,
        service: &Service,
        sudo: Option<&Arc<crate::sftp_client::Sudo>>,
    ) -> Result<()> {
        for operation in &self.operations {
            operation.validate()?;
        }
        for operation in &self.operations {
            let mut spec = Spec::new(self.owner, self.options.clone(), operation.clone());
            spec.sudo = sudo.cloned();
            if sudo.is_some() {
                spec.label = Some("Download with sudo".into());
            }
            spec.core = self.core.clone();
            spec.inherit_transfer_threads = true;
            spec.refresh_owner = false;
            spec.refresh_local = false;
            service.enqueue(spec)?;
        }
        Ok(())
    }
}
