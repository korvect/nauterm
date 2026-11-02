//! Shell-owned history readers and RS's application-observed command history.
//! Filesystem/process/native reads run off the UI thread. Tests pass explicit
//! fixture roots; they never read the developer's home or execute login profiles.
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{
    collections::HashSet,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

pub mod store;

pub const LIMIT: usize = 2000;
const MAX_BYTES: usize = 4 * 1024 * 1024;

pub fn live_command_allowed(command: &str) -> bool {
    let lower = command.trim().to_lowercase();
    !lower.is_empty()
        && !lower.contains('\0')
        && !lower.starts_with("password:")
        && !lower.starts_with("passphrase:")
        && ![" token=", " password=", " passwd="]
            .iter()
            .any(|pattern| lower.contains(pattern))
}

/// Only shell-reported CommandStarted metadata is collected, never keystrokes
/// or screen text. Prefix/payload/terminators may span native output pumps.
#[derive(Default)]
pub struct Capture {
    pub reported_shell: Option<String>,
    matched: usize,
    in_payload: bool,
    escaped: bool,
    discarded: bool,
    payload: Zeroizing<Vec<u8>>,
    commands: std::collections::VecDeque<String>,
}
impl Capture {
    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.feed(*byte);
        }
    }
    fn reset(&mut self) {
        use zeroize::Zeroize;
        self.matched = 0;
        self.in_payload = false;
        self.escaped = false;
        self.discarded = false;
        self.payload.zeroize();
        self.payload.clear();
    }
    fn feed(&mut self, byte: u8) {
        const PREFIX: &[u8] = b"\x1b]4545;";
        if !self.in_payload {
            if byte == PREFIX[self.matched] {
                self.matched += 1;
            } else {
                self.matched = usize::from(byte == 0x1b);
            }
            if self.matched == PREFIX.len() {
                self.in_payload = true;
                self.matched = 0;
            }
            return;
        }
        if byte == 7 || (self.escaped && byte == b'\\') {
            if !self.discarded {
                self.finish();
            }
            self.reset();
            return;
        }
        if self.payload.len() >= 16 * 1024 {
            self.discarded = true;
            use zeroize::Zeroize;
            self.payload.zeroize();
            self.payload.clear();
        }
        if !self.discarded {
            if self.escaped {
                self.payload.push(0x1b);
            }
            if byte != 0x1b {
                self.payload.push(byte);
            }
        }
        self.escaped = byte == 0x1b;
    }
    fn finish(&mut self) {
        use base64::Engine;
        if let Some(shell) = self.payload.strip_prefix(b"Shell;") {
            self.reported_shell = std::str::from_utf8(shell)
                .ok()
                .map(str::trim)
                .filter(|shell| {
                    !shell.is_empty() && shell.len() <= 4096 && !shell.chars().any(char::is_control)
                })
                .map(str::to_owned);
            return;
        }
        let Some(value) = self.payload.strip_prefix(b"CommandStarted;") else {
            return;
        };
        let encoded = value
            .splitn(2, |byte| *byte == b';')
            .last()
            .unwrap_or_default();
        let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
            return;
        };
        let decoded = Zeroizing::new(decoded);
        let Ok(command) = std::str::from_utf8(&decoded) else {
            return;
        };
        let command = command.trim();
        if live_command_allowed(command) {
            self.commands.push_back(command.into());
            if self.commands.len() > LIMIT {
                self.commands.pop_front();
            }
        }
    }
    pub fn take(&mut self) -> Vec<String> {
        self.commands.drain(..).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Zsh,
    Bash,
    Fish,
    PowerShell,
    Ksh,
    Csh,
    Nushell,
}
impl Format {
    pub fn for_shell(shell: &str) -> Option<Self> {
        match shell
            .trim()
            .rsplit(['/', '\\'])
            .next()?
            .to_ascii_lowercase()
            .as_str()
        {
            "zsh" => Some(Self::Zsh),
            "bash" | "bash.exe" => Some(Self::Bash),
            "fish" => Some(Self::Fish),
            "pwsh" | "pwsh.exe" | "powershell" | "powershell.exe" => Some(Self::PowerShell),
            "ksh" | "ksh93" | "mksh" => Some(Self::Ksh),
            "csh" | "tcsh" => Some(Self::Csh),
            "nu" | "nu.exe" => Some(Self::Nushell),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub command: String,
    /// Milliseconds since Unix epoch, shared by imported and observed history.
    pub created_at: i64,
}
impl Entry {
    pub fn observed(command: String, time: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            command,
            created_at: time.timestamp_millis(),
        }
    }
    pub fn time(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        (self.created_at > 0)
            .then(|| chrono::DateTime::from_timestamp_millis(self.created_at))
            .flatten()
    }
}
fn add(entries: &mut Vec<Entry>, command: &str, created_at: i64) {
    let command = command.trim();
    if !command.is_empty() && !command.contains('\0') {
        entries.push(Entry {
            command: command.into(),
            created_at,
        });
    }
}
fn seconds(text: &str) -> Option<i64> {
    (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse::<i64>().ok()?.checked_mul(1000))
        .flatten()
}
fn zsh_marker(line: &str) -> Option<(&str, &str)> {
    let (stamp, rest) = line.strip_prefix(": ")?.split_once(':')?;
    let (duration, command) = rest.split_once(';')?;
    if stamp.is_empty()
        || duration.is_empty()
        || !stamp
            .bytes()
            .chain(duration.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some((stamp, command))
}
pub fn parse(content: &str, format: Format, read_at: i64, limit: usize) -> Vec<Entry> {
    let mut entries = vec![];
    match format {
        Format::Zsh => {
            let mut lines = content.split('\n');
            while let Some(line) = lines.next() {
                if let Some((stamp, command)) = zsh_marker(line) {
                    let mut command = command.to_owned();
                    while command.ends_with('\\') {
                        let Some(next) = lines.next() else {
                            break;
                        };
                        command.pop();
                        command.push('\n');
                        command.push_str(next);
                    }
                    add(&mut entries, &command, seconds(stamp).unwrap_or(read_at));
                } else {
                    add(&mut entries, line, read_at);
                }
            }
        }
        Format::Bash => {
            let mut stamp = read_at;
            for line in content.split('\n') {
                if let Some(timestamp) = line.strip_prefix('#').and_then(seconds) {
                    stamp = timestamp;
                } else {
                    add(&mut entries, line, stamp);
                }
            }
        }
        Format::Fish => {
            let mut command = None;
            let mut stamp = read_at;
            for line in content.split('\n') {
                if let Some(next) = line.strip_prefix("- cmd:") {
                    if let Some(previous) = command {
                        add(&mut entries, previous, stamp);
                    }
                    command = Some(next.strip_prefix(' ').unwrap_or(next));
                    stamp = read_at;
                } else if command.is_some()
                    && let Some(timestamp) =
                        line.trim_start().strip_prefix("when: ").and_then(seconds)
                {
                    stamp = timestamp;
                }
            }
            if let Some(command) = command {
                add(&mut entries, command, stamp);
            }
        }
        Format::Ksh => {
            for line in content.split('\n') {
                let trimmed = line.trim_start();
                let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
                let command = if digits > 0 && trimmed[digits..].starts_with(char::is_whitespace) {
                    trimmed[digits..].trim_start()
                } else {
                    line
                };
                add(&mut entries, command, read_at);
            }
        }
        Format::Nushell => {
            #[derive(Deserialize)]
            struct Row {
                command: String,
                start_timestamp: Option<Stamp>,
            }
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum Stamp {
                Millis(i64),
                Text(String),
                Other(serde::de::IgnoredAny),
            }
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum Item {
                Command(Row),
                Other(serde::de::IgnoredAny),
            }
            if let Ok(rows) = serde_json::from_str::<Vec<Item>>(content) {
                for row in rows {
                    if let Item::Command(row) = row {
                        let stamp = match row.start_timestamp {
                            Some(Stamp::Millis(stamp)) => Some(stamp),
                            Some(Stamp::Text(text)) => chrono::DateTime::parse_from_rfc3339(&text)
                                .ok()
                                .map(|stamp| stamp.timestamp_millis()),
                            _ => None,
                        };
                        add(&mut entries, &row.command, stamp.unwrap_or(read_at));
                    }
                }
            }
        }
        Format::PowerShell | Format::Csh => {
            for line in content.split('\n') {
                add(&mut entries, line, read_at);
            }
        }
    }
    let keep = limit.min(LIMIT);
    if entries.len() > keep {
        entries.drain(..entries.len() - keep);
    }
    entries
}
pub fn newest_first(entries: &[Entry]) -> Vec<String> {
    let mut seen = HashSet::new();
    entries
        .iter()
        .rev()
        .filter_map(|entry| {
            let command = entry.command.trim();
            (!command.is_empty() && seen.insert(command)).then(|| command.to_owned())
        })
        .collect()
}

pub struct Locations {
    pub home: PathBuf,
    pub app_data: Option<PathBuf>,
    pub windows: bool,
}
impl Locations {
    pub fn environment() -> Option<Self> {
        let home = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .or_else(|| {
                cfg!(windows)
                    .then(|| std::env::var_os("USERPROFILE"))
                    .flatten()
            })?;
        Some(Self {
            home: home.into(),
            app_data: std::env::var_os("APPDATA").map(Into::into),
            windows: cfg!(windows),
        })
    }
    fn file(&self, format: Format) -> Option<PathBuf> {
        Some(match format {
            Format::Zsh => self.home.join(".zsh_history"),
            Format::Bash => self.home.join(".bash_history"),
            Format::Fish => self.home.join(".local/share/fish/fish_history"),
            Format::PowerShell if self.windows => self
                .app_data
                .as_ref()
                .unwrap_or(&self.home)
                .join("Microsoft/Windows/PowerShell/PSReadLine/ConsoleHost_history.txt"),
            Format::PowerShell => self
                .home
                .join(".local/share/powershell/PSReadLine/ConsoleHost_history.txt"),
            _ => return None,
        })
    }
}
fn read_file(path: &Path) -> Result<String> {
    match std::fs::metadata(path) {
        Ok(metadata) => ensure!(metadata.is_file(), "Shell history is not a regular file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.into()),
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    ensure!(metadata.is_file(), "Shell history is not a regular file");
    let skipped = metadata.len() > MAX_BYTES as u64;
    if skipped {
        file.seek(SeekFrom::End(-(MAX_BYTES as i64)))?;
    }
    let mut bytes = Zeroizing::new(vec![]);
    file.take(MAX_BYTES as u64).read_to_end(&mut bytes)?;
    let bytes = if skipped {
        bytes.splitn(2, |b| *b == b'\n').nth(1).unwrap_or_default()
    } else {
        &bytes
    };
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

/// Profiles can hang or print unbounded data. Bound both without touching a PTY,
/// inheriting stdin, writing a plaintext temporary file, or interpolating input.
async fn managed_output(shell: &str, args: &[String], timeout: Duration) -> Result<String> {
    use smol::io::AsyncReadExt;
    use std::process::Stdio;
    let mut child = smol::process::Command::new(shell)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("Cannot start the shell history reader")?;
    let stdout = child
        .stdout
        .take()
        .context("Shell history reader has no output")?;
    smol::future::or(
        async {
            let mut bytes = Zeroizing::new(vec![]);
            stdout
                .take(MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .await?;
            ensure!(
                bytes.len() <= MAX_BYTES,
                "Shell history reader output exceeded its limit"
            );
            ensure!(
                child.status().await?.success(),
                "Shell history reader failed"
            );
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        },
        async {
            smol::Timer::after(timeout).await;
            bail!("Shell history reader timed out")
        },
    )
    .await
}
pub fn read_local(shell: &str, locations: &Locations) -> Result<Vec<Entry>> {
    let Some(format) = Format::for_shell(shell) else {
        return Ok(vec![]);
    };
    let content = match format {
        Format::Ksh | Format::Csh | Format::Nushell => {
            let args: Vec<String> = match format {
                Format::Ksh => vec!["-ic", "fc -l -2000"],
                Format::Csh => vec!["-ic", "history -h 2000"],
                _ => vec!["-c", "history --long | last 2000 | to json -r"],
            }
            .into_iter()
            .map(Into::into)
            .collect();
            smol::block_on(managed_output(shell, &args, Duration::from_secs(8)))?
        }
        Format::PowerShell => {
            let args = [
                "-NoLogo",
                "-NoProfile",
                "-Command",
                "(Get-PSReadLineOption).HistorySavePath",
            ]
            .map(String::from);
            let configured =
                smol::block_on(managed_output(shell, &args, Duration::from_secs(8))).ok();
            let path = configured
                .as_deref()
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(PathBuf::from);
            let path = path
                .filter(|p| p.is_file())
                .or_else(|| locations.file(format));
            path.map(|p| read_file(&p)).transpose()?.unwrap_or_default()
        }
        _ => read_file(
            &locations
                .file(format)
                .context("Unavailable shell history path")?,
        )?,
    };
    let content = Zeroizing::new(content);
    Ok(parse(
        &content,
        format,
        chrono::Utc::now().timestamp_millis(),
        LIMIT,
    ))
}

/// Event-driven refreshes coalesce while a request is running and throttle
/// subsequent reads. A failure retains the previous inventory and can retry.
#[derive(Default)]
pub struct Refresh {
    loading: bool,
    last: Option<Instant>,
}
impl Refresh {
    pub fn begin(&mut self, now: Instant, interval: Duration) -> bool {
        if self.loading
            || self
                .last
                .is_some_and(|last| now.duration_since(last) < interval)
        {
            return false;
        }
        self.loading = true;
        self.last = Some(now);
        true
    }
    pub fn finish(&mut self) {
        self.loading = false;
    }
}

#[cfg(test)]
mod capture_tests {
    use super::Capture;
    #[test]
    fn raw_capture_retains_only_shell_reported_commands_across_every_chunk_boundary() {
        let bytes = b"ordinary pwd output\x1b]4545;Shell;/bin/zsh\x07\x1b]4545;CommandStarted;123;cHdk\x1b\\";
        for split in 0..=bytes.len() {
            let mut capture = Capture::default();
            capture.feed_bytes(&bytes[..split]);
            capture.feed_bytes(&bytes[split..]);
            assert_eq!(capture.take(), vec!["pwd"], "split at {split}");
            assert!(capture.take().is_empty());
        }
    }
    #[test]
    fn raw_capture_recovers_after_oversized_metadata_without_retaining_input() {
        let mut capture = Capture::default();
        capture.feed_bytes(b"\x1b]4545;CommandStarted;");
        capture.feed_bytes(&[b'x'; 20_000]);
        assert!(capture.payload.is_empty());
        capture.feed_bytes(b"\x07plain input is not history\x1b]4545;CommandStarted;cHdk\x07");
        assert_eq!(capture.take(), vec!["pwd"]);
    }
}

#[cfg(test)]
mod time_tests {
    use super::{Entry, Format, parse};
    #[test]
    fn imported_and_observed_commands_use_the_same_millisecond_timestamp() {
        let time = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let observed = Entry::observed("pwd".into(), time);
        for (content, format) in [
            (": 1700000000:0;pwd", Format::Zsh),
            ("#1700000000\npwd", Format::Bash),
            ("- cmd: pwd\n  when: 1700000000", Format::Fish),
        ] {
            let imported = parse(content, format, 1_800_000_000_000, 10);
            assert_eq!(imported[0].created_at, observed.created_at);
            assert_eq!(imported[0].time(), Some(time));
        }
        assert_eq!(observed.time(), Some(time));
        assert!(
            Entry {
                command: "pwd".into(),
                created_at: 0
            }
            .time()
            .is_none()
        );
    }
}
