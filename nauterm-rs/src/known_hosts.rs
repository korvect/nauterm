//! RS-owned OpenSSH trust-file management. Never reads the original app's file
//! or the user's ~/.ssh file unless explicitly chosen for import.
use anyhow::{Context as _, Result, ensure};
use std::{collections::BTreeSet, fs, io::Write, path::Path};

mod view;
pub use view::{ConvertToHost, KnownHosts};

const MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Entry {
    pub index: usize,
    pub line: String,
    pub pattern: String,
    pub algorithm: String,
    pub fingerprint: String,
    pub marker: Option<String>,
    pub endpoint: Option<(String, u16)>,
}
impl Entry {
    fn parse(index: usize, line: &str) -> Result<Self> {
        let fields: Vec<_> = line.split_whitespace().collect();
        let offset = usize::from(fields.first().is_some_and(|field| field.starts_with('@')));
        ensure!(fields.len() >= offset + 3, "Incomplete known-host entry");
        let normalized = fields.join(" ");
        let parsed: ssh_key::known_hosts::Entry = normalized.parse()?;
        let pattern = fields[offset].to_owned();
        Ok(Self {
            index,
            line: line.to_owned(),
            endpoint: endpoint(&pattern),
            pattern,
            algorithm: parsed.public_key().algorithm().to_string(),
            fingerprint: parsed
                .public_key()
                .fingerprint(ssh_key::HashAlg::Sha256)
                .to_string(),
            marker: (offset == 1).then(|| fields[0].to_owned()),
        })
    }
    pub fn matches(&self, query: &str) -> bool {
        let text = format!("{} {} {}", self.line, self.algorithm, self.fingerprint).to_lowercase();
        query
            .split_whitespace()
            .all(|token| text.contains(&token.to_lowercase()))
    }
    pub fn detail(&self) -> String {
        format!(
            "{}{}",
            self.algorithm,
            self.marker
                .as_ref()
                .map(|m| format!(", {m}"))
                .unwrap_or_default()
        )
    }
}
fn endpoint(pattern: &str) -> Option<(String, u16)> {
    pattern.split(',').find_map(|host| {
        if host.is_empty() || host.contains(['*', '?', '!', '|']) {
            return None;
        }
        if let Some(host) = host.strip_prefix('[') {
            let (host, port) = host.rsplit_once("]:")?;
            let port = port.parse::<u16>().ok().filter(|port| *port > 0)?;
            (!host.is_empty()).then(|| (host.to_owned(), port))
        } else {
            Some((host.to_owned(), 22))
        }
    })
}

#[derive(Clone, Default)]
pub struct Document {
    text: String,
    pub entries: Vec<Entry>,
    pub invalid_lines: usize,
}
impl Document {
    pub fn parse(text: String) -> Self {
        let mut document = Self {
            text,
            ..Self::default()
        };
        for (index, line) in document.text.lines().enumerate() {
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            match Entry::parse(index, line) {
                Ok(entry) => document.entries.push(entry),
                Err(_) => document.invalid_lines += 1,
            }
        }
        document
    }
    pub fn read(path: &Path) -> Result<Self> {
        match fs::metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error.into()),
            Ok(metadata) => ensure!(
                metadata.is_file() && metadata.len() <= MAX_BYTES,
                "The known_hosts file is not a regular file or exceeds 4 MiB."
            ),
        }
        let text = fs::read_to_string(path).context("Could not read known_hosts")?;
        ensure!(
            text.len() as u64 <= MAX_BYTES,
            "The known_hosts file exceeds 4 MiB."
        );
        Ok(Self::parse(text))
    }
    pub fn merge(&self, imported: &Self) -> Result<(Self, usize)> {
        ensure!(
            imported.invalid_lines == 0,
            "The import contains {} invalid or unsupported known-host entries. Nothing was imported.",
            imported.invalid_lines
        );
        let key = |entry: &Entry| {
            entry
                .line
                .split_whitespace()
                .take(if entry.marker.is_some() { 4 } else { 3 })
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut seen: BTreeSet<_> = self.entries.iter().map(&key).collect();
        let mut text = self.text.clone();
        let mut added = 0;
        for entry in &imported.entries {
            if seen.insert(key(entry)) {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&entry.line);
                text.push('\n');
                added += 1;
            }
        }
        ensure!(
            text.len() as u64 <= MAX_BYTES,
            "The merged known_hosts file would exceed 4 MiB."
        );
        Ok((Self::parse(text), added))
    }
    pub fn remove(&self, selected: &[Entry]) -> Result<Self> {
        let lines: Vec<_> = self.text.split_inclusive('\n').collect();
        for entry in selected {
            ensure!(
                lines
                    .get(entry.index)
                    .map(|line| line.trim_end_matches(['\r', '\n']))
                    == Some(entry.line.as_str()),
                "Known hosts changed since selection. Refresh the list before deleting."
            );
        }
        let selected: BTreeSet<_> = selected.iter().map(|entry| entry.index).collect();
        Ok(Self::parse(
            lines
                .into_iter()
                .enumerate()
                .filter(|(index, _)| !selected.contains(index))
                .map(|(_, line)| line)
                .collect(),
        ))
    }
    /// Publish from a fresh snapshot, rejecting detected concurrent trust edits.
    /// Existing unrecognized lines/comments are preserved verbatim.
    pub fn publish(&self, previous: &Self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .context("known_hosts needs an application directory")?;
        fs::create_dir_all(parent)?;
        let mut staged = tempfile::NamedTempFile::new_in(parent)?;
        staged.write_all(self.text.as_bytes())?;
        staged.as_file().sync_all()?;
        ensure!(
            Self::read(path)?.text == previous.text,
            "Known hosts changed while saving. Refresh and try again; the file was not replaced."
        );
        staged.persist(path).context("Could not save known_hosts")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const KEY: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGDreR6D12Sb4FmVCmq3BJQu7VxMYgIX1vnDSEVvzC3S";
    #[test]
    fn trust_management_preserves_unrelated_lines_and_rejects_stale_deletes() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("known_hosts");
        let original = Document::parse(format!(
            "# keep me\r\nexample.test {KEY}\nunknown old-format\n[::1]:2222 {KEY}\n"
        ));
        assert_eq!(original.entries.len(), 2);
        assert_eq!(original.invalid_lines, 1);
        assert_eq!(original.entries[1].endpoint, Some(("::1".into(), 2222)));
        assert!(original.entries[0].fingerprint.starts_with("SHA256:"));
        original.publish(&Document::default(), &path)?;
        let imported = Document::parse(format!(
            "example.test {KEY} new-comment\n@cert-authority *.example.test {KEY}\n"
        ));
        let (merged, added) = original.merge(&imported)?;
        assert_eq!(added, 1);
        assert_eq!(merged.entries[2].endpoint, None);
        merged.publish(&original, &path)?;
        let selected = vec![merged.entries[1].clone()];
        let removed = merged.remove(&selected)?;
        removed.publish(&merged, &path)?;
        assert!(removed.text.starts_with("# keep me\r\n"));
        assert!(removed.text.contains("unknown old-format\n"));
        assert!(removed.remove(&selected).is_err());
        assert!(merged.publish(&original, &path).is_err());
        assert_eq!(Document::read(&path)?.text, removed.text);
        assert!(original.merge(&Document::parse("invalid".into())).is_err());
        assert_eq!(endpoint("!deny,*.test,|1|hash|value"), None);
        assert_eq!(
            endpoint("*.test,concrete.test"),
            Some(("concrete.test".into(), 22))
        );
        Ok(())
    }
}
