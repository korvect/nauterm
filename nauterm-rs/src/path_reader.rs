//! Background filesystem/SFTP listing for composer paths. No shell evaluation,
//! no trust acceptance, no writes and no application-data import.
use crate::{
    native::{Core, SessionLease, SshOptions},
    path_completion::{Entry, resolve},
    sftp_client,
};
use anyhow::{Result, ensure};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(crate) enum Source {
    #[default]
    Disabled,
    Local {
        home: Option<String>,
        lease: SessionLease,
    },
    Remote {
        core: Arc<Core>,
        options: Box<SshOptions>,
        lease: SessionLease,
    },
}
impl Source {
    pub(crate) fn local(lease: SessionLease) -> Self {
        Self::Local {
            home: std::env::var("HOME").ok().or_else(|| {
                cfg!(windows)
                    .then(|| std::env::var("USERPROFILE").ok())
                    .flatten()
            }),
            lease,
        }
    }
    pub(crate) fn remote(core: Arc<Core>, options: SshOptions, lease: SessionLease) -> Self {
        Self::Remote {
            core,
            options: Box::new(options),
            lease,
        }
    }
    pub fn context(&self, cwd: Option<&str>) -> Option<(String, Option<&str>, bool)> {
        match self {
            Self::Disabled => None,
            Self::Local { home, .. } => cwd.map(|cwd| {
                (
                    resolve(cwd, "", home.as_deref(), true),
                    home.as_deref(),
                    true,
                )
            }),
            Self::Remote { .. } => Some((cwd.unwrap_or("~").into(), None, false)),
        }
    }
    pub fn request(&self, directory: String) -> Result<(Request, Cancellation)> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (kind, remote, lease) = match self {
            Self::Disabled => anyhow::bail!("Path completion source unavailable"),
            Self::Local { lease, .. } => (Kind::Local, None, lease.clone()),
            Self::Remote {
                core,
                options,
                lease,
            } => {
                let (request, cancel) = sftp_client::Request::new(core.clone())?;
                (
                    Kind::Remote {
                        request,
                        options: options.clone(),
                    },
                    Some(cancel),
                    lease.clone(),
                )
            }
        };
        lease.validate()?;
        Ok((
            Request {
                kind,
                directory,
                lease,
                cancelled: cancelled.clone(),
            },
            Cancellation { cancelled, remote },
        ))
    }
}
enum Kind {
    Local,
    Remote {
        request: sftp_client::Request,
        options: Box<SshOptions>,
    },
}
pub(crate) struct Request {
    kind: Kind,
    directory: String,
    lease: SessionLease,
    cancelled: Arc<AtomicBool>,
}
pub(crate) struct Cancellation {
    cancelled: Arc<AtomicBool>,
    remote: Option<sftp_client::Cancellation>,
}
impl Drop for Cancellation {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(remote) = &self.remote {
            remote.cancel();
        }
    }
}
impl Request {
    pub fn read(self) -> Result<Vec<Entry>> {
        self.lease.validate()?;
        ensure!(
            !self.cancelled.load(Ordering::Acquire),
            "Path completion cancelled"
        );
        let entries = match self.kind {
            Kind::Local => read_local(&self.directory, &self.cancelled)?,
            Kind::Remote { request, options } => {
                let listing = request.list(&options, &self.directory)?;
                ensure!(
                    listing.error.is_none(),
                    "Remote path listing is unavailable"
                );
                listing
                    .entries
                    .into_iter()
                    .map(|entry| Entry {
                        name: entry.name,
                        directory: entry.is_directory,
                    })
                    .collect()
            }
        };
        self.lease.validate()?;
        ensure!(
            !self.cancelled.load(Ordering::Acquire),
            "Path completion cancelled"
        );
        Ok(entries)
    }
}
fn read_local(directory: &str, cancelled: &AtomicBool) -> Result<Vec<Entry>> {
    ensure!(
        std::path::Path::new(directory).is_absolute(),
        "The local terminal directory is unavailable"
    );
    let mut entries = vec![];
    for entry in std::fs::read_dir(directory)? {
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "Path completion cancelled"
        );
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let directory = entry.path().is_dir(); // Follows a directory symlink like the reference.
        entries.push(Entry { name, directory });
        ensure!(
            entries.len() <= 20_000,
            "Path completion directory is too large"
        );
    }
    Ok(entries)
}
