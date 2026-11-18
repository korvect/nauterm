//! Windows and Linux launch another copy for a URL. The process holding the
//! workspace lease consumes the pending links via filesystem notifications.
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const MAX_LINK: usize = 8192;

fn directory(state: &Path) -> io::Result<PathBuf> {
    let path = state.join("pending-connection-links");
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.recursive(true).create(&path)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_dir() {
        return Err(io::Error::other("Connection links directory is invalid"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid takes no arguments and has no memory requirements.
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::other(
                "Connection links directory is not private",
            ));
        }
    }
    Ok(path)
}

fn without_password(value: &str) -> Option<url::Url> {
    if value.len() > MAX_LINK {
        return None;
    }
    crate::quick_connect::connection_url(value)?;
    let mut url = url::Url::parse(value.trim()).ok()?;
    url.set_password(None).ok()?;
    (url.as_str().len() <= MAX_LINK).then_some(url)
}

/// Enqueue only validated connection URLs with passwords already stripped.
pub(crate) fn forward(state: &Path, links: &[String]) -> io::Result<bool> {
    let mut forwarded = false;
    for url in links.iter().filter_map(|value| without_password(value)) {
        let dir = directory(state)?;
        let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
        temp.write_all(url.as_str().as_bytes())?;
        let destination = dir.join(format!("{}.link", uuid::Uuid::new_v4()));
        temp.persist_noclobber(destination)?;
        forwarded = true;
    }
    Ok(forwarded)
}

pub(crate) struct Pending {
    path: PathBuf,
    _watcher: RecommendedWatcher,
}

impl Pending {
    pub(crate) fn watch(state: &Path) -> anyhow::Result<(Self, async_channel::Receiver<()>)> {
        let path = directory(state)?;
        let (send, receive) = async_channel::bounded(1);
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event.is_ok_and(|event| !matches!(event.kind, EventKind::Access(_))) {
                    let _ = send.try_send(());
                }
            })?;
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
        Ok((
            Self {
                path,
                _watcher: watcher,
            },
            receive,
        ))
    }

    pub(crate) async fn consume(
        self,
        wake: async_channel::Receiver<()>,
        send: async_channel::Sender<Vec<String>>,
    ) {
        loop {
            let path = self.path.clone();
            let links = smol::unblock(move || take_links(&path)).await;
            if !links.is_empty() && send.send(links).await.is_err() {
                break;
            }
            if wake.recv().await.is_err() {
                break;
            }
        }
    }
}

fn take_links(path: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(path) else {
        return vec![];
    };
    let mut entries: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "link")
                && entry.file_type().is_ok_and(|ty| ty.is_file())
        })
        .collect();
    entries.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    entries
        .iter()
        .filter_map(|entry| {
            let path = entry.path();
            let mut link = String::new();
            let result = File::open(&path)
                .and_then(|file| file.take((MAX_LINK + 1) as u64).read_to_string(&mut link));
            fs::remove_file(path).ok()?;
            result.ok()?;
            without_password(&link).map(|url| url.into())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_only_connection_links_and_strips_passwords() -> anyhow::Result<()> {
        let state = tempfile::tempdir()?;
        assert!(!forward(state.path(), &["file:///tmp/test".into()])?);
        assert!(forward(
            state.path(),
            &["ssh://admin:secret@example.com:2022".into()]
        )?);
        let dir = directory(state.path())?;
        let file = fs::read_dir(&dir)?.next().unwrap()?.path();
        assert_eq!(fs::read_to_string(file)?, "ssh://admin@example.com:2022");
        assert_eq!(take_links(&dir), ["ssh://admin@example.com:2022"]);
        assert!(take_links(&dir).is_empty());
        Ok(())
    }

    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "macOS uses LaunchServices for live URL delivery"
    )]
    fn delivers_links_queued_before_and_after_the_receiver_starts() -> anyhow::Result<()> {
        use std::time::Duration;

        let state = tempfile::tempdir()?;
        assert!(forward(state.path(), &["ssh://example.com".into()])?);
        let (pending, wake) = Pending::watch(state.path())?;
        let (send, receive) = async_channel::bounded(1);
        smol::block_on(async {
            let _consume = smol::spawn(pending.consume(wake, send));
            let received = async {
                assert_eq!(receive.recv().await?, ["ssh://example.com"]);
                assert!(forward(
                    state.path(),
                    &["sftp://example.com/path%20with%20spaces".into()]
                )?);
                assert_eq!(
                    receive.recv().await?,
                    ["sftp://example.com/path%20with%20spaces"]
                );
                Ok(())
            };
            smol::future::race(received, async {
                smol::Timer::after(Duration::from_secs(5)).await;
                anyhow::bail!("timed out waiting for a forwarded connection link")
            })
            .await
        })
    }
}
