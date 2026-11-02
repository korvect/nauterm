//! RS-owned public SFTP preferences. Defaults affect new views; transfer limits
//! affect admission without cancelling already-running work.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub concurrent_tasks: usize,
    pub transfer_threads: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            concurrent_tasks: 3,
            transfer_threads: 8,
        }
    }
}
impl Limits {
    pub fn validate(self) -> Result<()> {
        ensure!(
            (1..=8).contains(&self.concurrent_tasks),
            "Concurrent Tasks must be between 1 and 8."
        );
        ensure!(
            (1..=32).contains(&self.transfer_threads),
            "Transfer Concurrency must be between 1 and 32."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub show_tab: bool,
    pub initial_local_directory: String,
    pub show_hidden_files: bool,
    #[serde(flatten)]
    pub limits: Limits,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_tab: true,
            initial_local_directory: String::new(),
            show_hidden_files: false,
            limits: Limits::default(),
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<()> {
        self.limits.validate()?;
        let directory = self.initial_local_directory.trim();
        ensure!(
            directory.len() <= 16 * 1024
                && !directory.contains('\0')
                && (directory.is_empty() || Path::new(directory).is_absolute()),
            "Enter an absolute initial local directory, or leave it empty for the home directory."
        );
        Ok(())
    }
    pub fn initial_directory(&self, home: &Path) -> PathBuf {
        if self.initial_local_directory.trim().is_empty() {
            home.to_owned()
        } else {
            self.initial_local_directory.trim().into()
        }
    }
}
