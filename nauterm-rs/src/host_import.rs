//! Selected-file/folder imports. Secret key material is transient and zeroized;
//! no referenced IdentityFile, Include or command is opened/executed implicitly.
mod formats;
pub mod keys;
use crate::host_transfer::{self, Candidate};
use anyhow::{Context, Result, ensure};
pub use keys::{KeyBatch, KeyCandidate, KeyReport};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Csv,
    OpenSsh,
    Putty,
    MobaXterm,
    SecureCrt,
}
impl Source {
    pub const ALL: [Self; 5] = [
        Self::Csv,
        Self::OpenSsh,
        Self::Putty,
        Self::MobaXterm,
        Self::SecureCrt,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::OpenSsh => "OpenSSH",
            Self::Putty => "PuTTY",
            Self::MobaXterm => "MobaXterm",
            Self::SecureCrt => "SecureCRT",
        }
    }
    pub fn directory(self) -> bool {
        matches!(self, Self::OpenSsh | Self::SecureCrt)
    }
    pub fn detail(self) -> &'static str {
        match self {
            Self::Csv => "Import Hosts from a CSV file",
            Self::OpenSsh => "Choose an SSH folder containing config, known_hosts and keys",
            Self::Putty => "Choose a PuTTY registry export (.reg)",
            Self::MobaXterm => "Choose a session export (.mxtsessions or .ini)",
            Self::SecureCrt => "Choose a Sessions folder containing .ini files",
        }
    }
}
#[derive(Default)]
pub struct Bundle {
    pub hosts: Vec<Candidate>,
    pub keys: Vec<KeyCandidate>,
    pub skipped_files: usize,
}

pub(crate) fn read_text(path: &Path, limit: u64) -> Result<Zeroizing<String>> {
    let file = fs::File::open(path)?;
    ensure!(file.metadata()?.is_file(), "Choose a regular file.");
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "The selected import file is too large."
    );
    let text = Zeroizing::new(
        if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
            ensure!(
                bytes.len().is_multiple_of(2),
                "The UTF-16 import file is truncated."
            );
            let little = bytes[0] == 0xff;
            let units = Zeroizing::new(
                bytes[2..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| {
                        if little {
                            u16::from_le_bytes([pair[0], pair[1]])
                        } else {
                            u16::from_be_bytes([pair[0], pair[1]])
                        }
                    })
                    .collect::<Vec<_>>(),
            );
            String::from_utf16(&units).context("The import file is not valid UTF-16.")?
        } else {
            std::str::from_utf8(&bytes)
                .context("Choose UTF-8 or BOM-marked UTF-16 text.")?
                .trim_start_matches('\u{feff}')
                .to_owned()
        },
    );
    ensure!(
        !text.contains('\0'),
        "The selected import file contains NUL bytes."
    );
    Ok(text)
}
pub fn normalize_path(value: &str, base: Option<&Path>, home: Option<&Path>) -> String {
    let value = value.trim().trim_matches('"').replace('\\', "/");
    let path = if let Some(rest) = value.strip_prefix("~/") {
        home.map(|home| home.join(rest))
            .unwrap_or_else(|| PathBuf::from(&value))
    } else if Path::new(&value).is_absolute() || value.as_bytes().get(1) == Some(&b':') {
        PathBuf::from(&value)
    } else {
        base.map(|base| base.join(&value))
            .unwrap_or_else(|| PathBuf::from(&value))
    };
    let value = path.to_string_lossy().replace('\\', "/");
    let absolute = value.starts_with('/');
    let mut parts = vec![];
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|part| *part != "..") => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    format!("{}{}", if absolute { "/" } else { "" }, parts.join("/"))
}
pub fn load(source: Source, path: &Path) -> Result<Bundle> {
    let mut bundle = Bundle::default();
    match source {
        Source::Csv => bundle.hosts = host_transfer::read(path)?,
        Source::Putty => bundle.hosts = formats::putty(&read_text(path, 16 * 1024 * 1024)?),
        Source::MobaXterm => bundle.hosts = formats::moba(&read_text(path, 16 * 1024 * 1024)?),
        Source::OpenSsh => {
            ensure!(path.is_dir(), "Choose an SSH folder.");
            let config = path.join("config");
            if config.is_file() {
                bundle.hosts = formats::openssh(&read_text(&config, 16 * 1024 * 1024)?);
            }
            let mut endpoints: BTreeSet<_> = bundle
                .hosts
                .iter()
                .map(|host| (host.address.to_lowercase(), host.port))
                .collect();
            let known = path.join("known_hosts");
            if known.is_file() {
                bundle.hosts.extend(
                    formats::known_hosts(&read_text(&known, 16 * 1024 * 1024)?)
                        .into_iter()
                        .filter(|host| endpoints.insert((host.address.to_lowercase(), host.port))),
                );
            }
            let mut files = vec![];
            let mut key_bytes = 0usize;
            for (index, entry) in fs::read_dir(path)?.enumerate() {
                ensure!(index < 10000, "Import at most 10,000 files at a time.");
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if matches!(
                    name.as_str(),
                    "config"
                        | "known_hosts"
                        | "known_hosts.old"
                        | "authorized_keys"
                        | "authorized_keys2"
                        | "environment"
                        | "rc"
                ) || name.starts_with('.')
                {
                    continue;
                }
                match read_text(&entry.path(), 1024 * 1024) {
                    Ok(text) => {
                        if keys::recognized(&text) {
                            key_bytes += text.len();
                            ensure!(
                                key_bytes <= 64 * 1024 * 1024,
                                "Import at most 64 MiB of key material at a time."
                            );
                            files.push((entry.path(), text));
                        }
                    }
                    Err(_) => bundle.skipped_files += 1,
                }
            }
            bundle.keys = keys::collect(files);
        }
        Source::SecureCrt => {
            let mut pending = vec![(path.to_owned(), 0)];
            let mut count = 0;
            while let Some((directory, depth)) = pending.pop() {
                ensure!(depth <= 32, "The Sessions folder is nested too deeply.");
                for entry in fs::read_dir(&directory)? {
                    count += 1;
                    ensure!(
                        count <= 10000,
                        "Import at most 10,000 session files at a time."
                    );
                    let entry = entry?;
                    let kind = entry.file_type()?;
                    if kind.is_dir() {
                        pending.push((entry.path(), depth + 1));
                        continue;
                    }
                    if !kind.is_file()
                        || !entry
                            .path()
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("ini"))
                    {
                        continue;
                    }
                    let file = entry.path();
                    let relative = file.strip_prefix(path)?.with_extension("");
                    let name = relative
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    let group = relative
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .map(|parent| parent.to_string_lossy().into_owned());
                    if let Some(host) = formats::securecrt(
                        &read_text(&file, 16 * 1024 * 1024)?,
                        name,
                        group.as_deref(),
                    ) {
                        bundle.hosts.push(host);
                    }
                }
            }
        }
    }
    ensure!(
        bundle.hosts.len() <= 10000,
        "Import at most 10,000 Hosts at a time."
    );
    let home = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_owned());
    let base = if source.directory() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    for host in &mut bundle.hosts {
        host.identity_file = host
            .identity_file
            .as_deref()
            .map(|value| normalize_path(value, Some(base), home.as_deref()));
    }
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folder_discovery_pairs_selected_keys_without_following_identity_references() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        fs::write(
            root.join("config"),
            "Host work\n HostName host.test\n Port 2222\n IdentityFile ./id\nHost external\n HostName external.test\n IdentityFile /synthetic/outside/key\n",
        )?;
        fs::write(
            root.join("known_hosts"),
            "[host.test]:2222 ssh-ed25519 AAAA\nother.test ssh-ed25519 BBBB\n",
        )?;
        fs::write(
            root.join("id"),
            "-----BEGIN OPENSSH PRIVATE KEY-----\nSYNTHETIC_PRIVATE\n-----END OPENSSH PRIVATE KEY-----",
        )?;
        fs::write(root.join("id.pub"), "ssh-ed25519 AAAA comment")?;
        fs::write(root.join("authorized_keys"), "ssh-rsa BBBB ignored")?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("id"), root.join("linked-key"))?;
        let bundle = load(Source::OpenSsh, root)?;
        assert_eq!(bundle.hosts.len(), 3);
        assert_eq!(bundle.keys.len(), 1);
        assert_eq!(bundle.keys[0].paths.len(), 2);
        assert_eq!(
            bundle.hosts[0].identity_file.as_deref(),
            Some(normalize_path(&root.join("id").to_string_lossy(), None, None).as_str())
        );
        assert_eq!(
            bundle.hosts[1].identity_file.as_deref(),
            Some("/synthetic/outside/key")
        );
        let imported = host_transfer::merge(
            &crate::model::AppState::default(),
            &bundle.hosts,
            &vec![None; bundle.hosts.len()],
        )?;
        assert_eq!(imported.key_bindings.len(), 2);
        assert!(!serde_json::to_string(&imported.data)?.contains("SYNTHETIC_PRIVATE"));
        Ok(())
    }
    #[test]
    fn utf16_session_folders_keep_relative_groups_and_reads_are_bounded() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        fs::create_dir_all(root.join("Network/Core"))?;
        let text = "S:\"Hostname\"=router.test\nS:\"Username\"=网络\nD:\"[SSH2] Port\"=000008ae\n";
        let bytes: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        fs::write(root.join("Network/Core/Router.ini"), bytes)?;
        let bundle = load(Source::SecureCrt, root)?;
        assert_eq!(bundle.hosts.len(), 1);
        assert_eq!(bundle.hosts[0].groups, ["Network", "Core"]);
        assert_eq!(bundle.hosts[0].port, 2222);
        assert_eq!(bundle.hosts[0].username, "网络");
        assert!(read_text(&root.join("Network/Core/Router.ini"), 4).is_err());
        fs::write(root.join("truncated"), [0xff, 0xfe, 1])?;
        assert!(read_text(&root.join("truncated"), 10).is_err());
        assert_eq!(
            normalize_path(
                "~/keys/../.ssh/id",
                None,
                Some(Path::new("/synthetic/home"))
            ),
            "/synthetic/home/.ssh/id"
        );
        Ok(())
    }
}
