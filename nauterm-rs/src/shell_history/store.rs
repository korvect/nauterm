//! Application-observed commands, independent of shell-managed history files.
//! Only the recorder worker writes commands; reads/clear run off the UI thread.
use anyhow::{Context, Result, ensure};
use async_channel::{Receiver, Sender};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

const LIMIT: usize = 10_000;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const COMMAND_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub command: String,
    pub created_at: i64,
}
#[derive(Clone)]
pub struct Service(Arc<Shared>);
struct Shared {
    state: Mutex<State>,
    subscribers: Mutex<Vec<Sender<()>>>,
    epoch: AtomicU64,
}
struct State {
    path: PathBuf,
}
impl Service {
    pub fn new(path: PathBuf) -> Self {
        Self(Arc::new(Shared {
            state: Mutex::new(State { path }),
            subscribers: Mutex::new(vec![]),
            epoch: AtomicU64::new(0),
        }))
    }
    pub fn subscribe(&self) -> Receiver<()> {
        let (sender, receiver) = async_channel::bounded(1);
        let mut subscribers = self.0.subscribers.lock().unwrap();
        subscribers.retain(|subscriber| !subscriber.is_closed());
        subscribers.push(sender);
        receiver
    }
    fn notify(&self) {
        self.0.subscribers.lock().unwrap().retain(|subscriber| {
            let _ = subscriber.try_send(());
            !subscriber.is_closed()
        });
    }
    pub async fn entries(&self) -> Result<Vec<Entry>> {
        let service = self.clone();
        smol::unblock(move || {
            let state = service.0.state.lock().unwrap();
            nauterm_core::history_file::with_lock(&state.path, || state.load())
        })
        .await
    }
    pub async fn clear(&self) -> Result<()> {
        let service = self.clone();
        smol::unblock(move || {
            let state = service.0.state.lock().unwrap();
            nauterm_core::history_file::with_lock(&state.path, || state.save(&[]))?;
            // Commands already accepted by the recorder must not repopulate
            // this file if their queued disk writes run after Clear all.
            service.0.epoch.fetch_add(1, Ordering::AcqRel);
            drop(state);
            service.notify();
            Ok(())
        })
        .await
    }
    pub(crate) fn epoch(&self) -> u64 {
        self.0.epoch.load(Ordering::Acquire)
    }
    pub(crate) fn append(&self, commands: Vec<String>, created_at: i64, epoch: u64) -> Result<()> {
        let mut incoming: Vec<_> = commands
            .into_iter()
            .rev()
            .filter(|command| {
                command.len() <= COMMAND_BYTES && super::live_command_allowed(command)
            })
            .map(|command| Entry {
                command: command.trim().to_owned(),
                created_at,
            })
            .collect();
        if incoming.is_empty() {
            return Ok(());
        }
        let state = self.0.state.lock().unwrap();
        if epoch != self.epoch() {
            return Ok(());
        }
        nauterm_core::history_file::with_lock(&state.path, || {
            incoming.extend(state.load()?);
            state.save(&normalize(incoming))
        })?;
        drop(state);
        self.notify();
        Ok(())
    }
}
impl State {
    fn load(&self) -> Result<Vec<Entry>> {
        // Another client may have appended or cleared since our last read.
        // Always reload while the caller holds the shared sidecar lock.
        let metadata = match fs::symlink_metadata(&self.path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(error) => return Err(error.into()),
            Ok(metadata) => metadata,
        };
        ensure!(
            metadata.is_file(),
            "The shell history path is not a regular file."
        );
        let mut content = String::new();
        fs::File::open(&self.path)?
            .take(MAX_BYTES as u64 + 1)
            .read_to_string(&mut content)?;
        ensure!(
            content.len() <= MAX_BYTES,
            "The shell history file is too large."
        );
        Ok(normalize(
            content
                .lines()
                .filter_map(|line| {
                    let (timestamp, command) = line.split_once(';')?;
                    let created_at = timestamp.parse().ok()?;
                    let command = unescape(command).trim().to_owned();
                    (command.len() <= COMMAND_BYTES
                        && !command.is_empty()
                        && !command.contains('\0'))
                    .then_some(Entry {
                        command,
                        created_at,
                    })
                })
                .collect(),
        ))
    }
    fn save(&self, entries: &[Entry]) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("Shell history has no parent directory.")?;
        fs::create_dir_all(parent)?;
        ensure!(
            !fs::symlink_metadata(&self.path).is_ok_and(|meta| !meta.is_file()),
            "The shell history path is not a regular file."
        );
        let target = crate::file_commit::DownloadTarget::new(&self.path, true)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(target.staged())?;
        for entry in entries {
            writeln!(file, "{};{}", entry.created_at, escape(&entry.command))?;
        }
        file.sync_all()?;
        drop(file);
        target.publish()
    }
}
fn normalize(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.created_at));
    let mut seen = HashSet::new();
    let mut bytes = 0;
    entries.retain(|entry| {
        if !seen.insert(entry.command.clone()) || seen.len() > LIMIT {
            return false;
        }
        bytes += escape(&entry.command).len() + 32;
        bytes <= MAX_BYTES
    });
    entries
}
fn escape(command: &str) -> String {
    command
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
fn unescape(command: &str) -> String {
    let mut result = String::new();
    let mut chars = command.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observed_history_roundtrips_deduplicates_and_clears_only_its_file() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("shell_history");
        let external = temp.path().join(".zsh_history");
        fs::write(&external, "unrelated shell history")?;
        let service = Service::new(path.clone());
        let updates = service.subscribe();
        service.append(
            vec!["printf 'a\\nb'\nprintf '汉字'".into(), "ls -la".into()],
            10,
            service.epoch(),
        )?;
        // Populate one client's view, then commit from another instance.
        // The first writer must merge that disk update rather than its cache.
        let _ = smol::block_on(service.entries())?;
        let other = Service::new(path.clone());
        other.append(vec!["external client".into()], 15, other.epoch())?;
        service.append(
            vec!["ls -la".into(), "password: secret".into()],
            20,
            service.epoch(),
        )?;
        assert!(updates.try_recv().is_ok());
        let saved = smol::block_on(Service::new(path.clone()).entries())?;
        assert_eq!(saved.len(), 3);
        assert_eq!(
            saved[0],
            Entry {
                command: "ls -la".into(),
                created_at: 20
            }
        );
        assert_eq!(saved[1].command, "external client");
        assert_eq!(saved[2].command, "printf 'a\\nb'\nprintf '汉字'");
        assert!(!fs::read_to_string(&path)?.contains("secret"));
        let old_epoch = service.epoch();
        smol::block_on(service.clear())?;
        service.append(vec!["queued before clear".into()], 30, old_epoch)?;
        assert!(smol::block_on(Service::new(path).entries())?.is_empty());
        assert_eq!(fs::read_to_string(external)?, "unrelated shell history");
        service.append(vec!["new after clear".into()], 40, service.epoch())?;
        assert_eq!(
            smol::block_on(service.entries())?[0].command,
            "new after clear"
        );
        smol::block_on(other.clear())?;
        service.append(vec!["after external clear".into()], 50, service.epoch())?;
        let entries = smol::block_on(service.entries())?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "after external clear");
        Ok(())
    }
}
