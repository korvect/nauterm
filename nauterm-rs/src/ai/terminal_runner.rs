//! Runs an explicitly approved command in its existing terminal. No auxiliary
//! SSH shell/process is created; output and completion are event driven.
use super::{client::Cancel, context};
use crate::native::CommandTarget;
use anyhow::{Result, ensure};
use futures_util::future::{Either, select};
use std::{
    future::Future,
    time::{Duration, Instant},
};

const SETUP_TIMEOUT: Duration = Duration::from_secs(4);
const CANCEL_GRACE: Duration = Duration::from_millis(1500);
const CAPTURE_LIMIT: usize = 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Zsh,
    Bash,
    Fish,
    PowerShell,
}
impl Kind {
    fn from_path(path: &str) -> Option<Self> {
        let path = path.replace('\\', "/").to_ascii_lowercase();
        match path.rsplit('/').next()?.trim_end_matches(".exe") {
            "zsh" | "rzsh" => Some(Self::Zsh),
            "bash" | "rbash" => Some(Self::Bash),
            "fish" => Some(Self::Fish),
            "pwsh" | "powershell" => Some(Self::PowerShell),
            _ => None,
        }
    }
}
#[derive(Clone)]
pub(crate) struct Integration {
    kind: Kind,
    pub token: String,
}
impl Integration {
    fn clean_line_keys(&self) -> &'static [u8] {
        match self.kind {
            // .NET's Unix console decoder can map Ctrl+] to a different key.
            // An alphabetic chord is stable on both PowerShell platforms.
            Kind::PowerShell => b"\x18\x15",
            _ => b"\x18\x1d",
        }
    }
    pub fn local(path: Option<&str>, interactive: bool) -> Option<Self> {
        if !interactive {
            return None;
        }
        let path = path?;
        let kind = Kind::from_path(path)?;
        let name = path.replace('\\', "/").to_ascii_lowercase();
        if !matches!(
            name.rsplit('/').next()?,
            "zsh"
                | "zsh.exe"
                | "bash"
                | "bash.exe"
                | "fish"
                | "fish.exe"
                | "pwsh"
                | "pwsh.exe"
                | "powershell"
                | "powershell.exe"
        ) {
            return None;
        }
        if cfg!(target_os = "macos")
            && kind == Kind::Bash
            && std::fs::canonicalize(path).ok().as_deref()
                == Some(std::path::Path::new("/bin/bash"))
        {
            return None;
        }
        Some(Self {
            kind,
            token: uuid::Uuid::new_v4().simple().to_string(),
        })
    }
    fn setup(&self) -> String {
        if self.kind == Kind::PowerShell {
            return nauterm_core::shell_integration::powershell_setup(&self.token)
                .expect("internally generated alphanumeric integration token");
        }
        // Ported from lib/terminal/terminal_shell_integration.dart; these are
        // shell functions/hooks, not user rc files or a second shell session.
        let (template, aid) = match self.kind {
            Kind::Zsh => (include_str!("shell/zsh.sh"), "$$"),
            Kind::Bash => (include_str!("shell/bash.sh"), "$BASHPID"),
            Kind::Fish => (include_str!("shell/fish.sh"), "$fish_pid"),
            Kind::PowerShell => unreachable!("PowerShell uses its own startup syntax"),
        };
        let ready = format!(
            "printf '\\033]777;nauterm-integration-ready={}\\007'; printf '\\r\\033[2K'",
            self.token
        );
        let setup = template
            .replace(r"\033]133;A\007", r"\033]133;A;cl=line;aid={{AID}}\007")
            .replace(r"\033]133;B\007", r"\033]133;B;aid={{AID}}\007")
            .replace("{{AID}}", aid)
            .replace("{{TOKEN}}", &self.token)
            .replace("{{READY}}", &ready);
        if self.kind == Kind::Bash {
            setup.replacen(
                r"printf '\033]777;nauterm-command-end=%s;%s\007'",
                r"printf '\033]777;nauterm-command-end=%s;%s\033\\'",
                1,
            )
        } else {
            setup
        }
    }
}
pub struct Execution {
    pub output: String,
    pub exit_code: Option<i64>,
    pub started_at: i64,
    pub finished_at: i64,
    pub cancelled: bool,
    pub tracked: bool,
}
#[derive(Debug)]
struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Stopped by user.")
    }
}
impl std::error::Error for Cancelled {}
async fn cancellable<T>(
    target: &CommandTarget,
    cancel: &Cancel,
    work: impl Future<Output = Result<T>>,
) -> Result<T> {
    let abort = async {
        let _ = select(Box::pin(cancel.wait()), Box::pin(target.interrupted.wait())).await;
    };
    match select(Box::pin(abort), Box::pin(work)).await {
        Either::Left(_) => Err(Cancelled.into()),
        Either::Right((result, _)) => result,
    }
}
async fn quiet(target: &CommandTarget) -> Result<()> {
    let deadline = Instant::now() + SETUP_TIMEOUT;
    loop {
        ensure!(
            Instant::now() < deadline,
            "The terminal did not reach a quiet shell prompt."
        );
        match select(
            Box::pin(target.next()),
            Box::pin(smol::Timer::after(Duration::from_millis(60))),
        )
        .await
        {
            Either::Left((output, _)) => {
                output?;
            }
            Either::Right(_) => return Ok(()),
        }
    }
}
async fn marker(target: &CommandTarget, marker: &[u8]) -> Result<()> {
    let work = async {
        let mut pending = vec![];
        loop {
            let output = target.next().await?;
            pending.extend_from_slice(&output.bytes);
            if find(&pending, marker).is_some() {
                return Ok(());
            }
            if pending.len() > 64 * 1024 {
                pending.drain(..pending.len() - marker.len().min(pending.len()));
            }
        }
    };
    match select(Box::pin(work), Box::pin(smol::Timer::after(SETUP_TIMEOUT))).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => anyhow::bail!("Could not prepare a clean shell prompt."),
    }
}
pub async fn run(target: CommandTarget, command: String, cancel: Cancel) -> Result<Execution> {
    let started_at = chrono::Utc::now().timestamp_millis();
    let mut touched = false;
    let mut submitted = false;
    let mut parser = None;
    let result = cancellable(&target, &cancel, async {
        target.validate()?;
        quiet(&target).await?;
        let integration = match target.integration().filter(|integration| {
            target
                .shell
                .as_deref()
                .is_none_or(|path| Kind::from_path(path) == Some(integration.kind))
        }) {
            Some(integration) => Some(integration),
            None => target
                .shell
                .as_deref()
                .and_then(Kind::from_path)
                .map(|kind| Integration {
                    kind,
                    token: uuid::Uuid::new_v4().simple().to_string(),
                }),
        };
        let Some(integration) = integration else {
            target.submit(&command, &cancel)?;
            return Ok(Execution {
                output: String::new(),
                exit_code: None,
                started_at,
                finished_at: chrono::Utc::now().timestamp_millis(),
                cancelled: false,
                tracked: false,
            });
        };
        if target.integration().as_ref().is_none_or(|cached| {
            cached.kind != integration.kind || cached.token != integration.token
        }) {
            target.send(b"\x15", &cancel)?;
            touched = true;
            quiet(&target).await?;
            let ready = format!(
                "\x1b]777;nauterm-integration-ready={}\x07",
                integration.token
            );
            target.suppress(ready.as_bytes())?;
            let setup = zeroize::Zeroizing::new(format!("{}\r", integration.setup()));
            target.send(setup.as_bytes(), &cancel)?;
            marker(&target, ready.as_bytes()).await?;
            target.cancel_suppression();
            target.remember(integration.clone())?;
        }
        quiet(&target).await?;
        target.send(integration.clean_line_keys(), &cancel)?;
        touched = true;
        if integration.kind != Kind::Bash {
            marker(
                &target,
                format!("\x1b]777;nauterm-line-ready={}\x07", integration.token).as_bytes(),
            )
            .await?;
        }
        parser = Some(Parser::new(
            &integration.token,
            integration.kind == Kind::Fish,
        ));
        target.submit(&command, &cancel)?;
        submitted = true;
        loop {
            let output = target.next().await?;
            let parser = parser.as_mut().expect("command parser");
            if let Some((output, exit_code)) = parser.push(&output.bytes) {
                return Ok(Execution {
                    output: parser.clean(&output, &command),
                    exit_code: Some(exit_code),
                    started_at,
                    finished_at: chrono::Utc::now().timestamp_millis(),
                    cancelled: false,
                    tracked: true,
                });
            }
        }
    })
    .await;
    target.cancel_suppression();
    match result {
        Err(error)
            if error.is::<Cancelled>() || cancel.cancelled() || target.interrupted.cancelled() =>
        {
            if touched && !target.interrupted.cancelled() {
                let _ = target.interrupt();
            }
            let mut captured = String::new();
            let mut exit_code = None;
            if submitted {
                let deadline = Instant::now() + CANCEL_GRACE;
                while Instant::now() < deadline {
                    let delay = deadline.saturating_duration_since(Instant::now());
                    match select(Box::pin(target.next()), Box::pin(smol::Timer::after(delay))).await
                    {
                        Either::Left((Ok(output), _)) => {
                            if let Some(parser) = &mut parser
                                && let Some((output, code)) = parser.push(&output.bytes)
                            {
                                captured = parser.clean(&output, &command);
                                exit_code = Some(code);
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                if exit_code.is_none()
                    && let Some(parser) = &parser
                {
                    captured = parser.clean(&parser.partial(), &command);
                }
            }
            Ok(Execution {
                output: captured,
                exit_code,
                started_at,
                finished_at: chrono::Utc::now().timestamp_millis(),
                cancelled: true,
                tracked: exit_code.is_some(),
            })
        }
        other => other,
    }
}

fn find(source: &[u8], needle: &[u8]) -> Option<usize> {
    source
        .windows(needle.len())
        .position(|window| window == needle)
}
struct Parser {
    prefix: Vec<u8>,
    pending: Vec<u8>,
    head: Vec<u8>,
    scan: usize,
    fish: bool,
}
impl Parser {
    fn new(token: &str, fish: bool) -> Self {
        Self {
            prefix: format!("\x1b]777;nauterm-command-end={token};").into_bytes(),
            pending: vec![],
            head: vec![],
            scan: 0,
            fish,
        }
    }
    fn push(&mut self, bytes: &[u8]) -> Option<(Vec<u8>, i64)> {
        self.pending.extend_from_slice(bytes);
        while let Some(relative) = find(&self.pending[self.scan..], &self.prefix) {
            let start = self.scan + relative;
            let code_start = start + self.prefix.len();
            let tail = &self.pending[code_start..self.pending.len().min(code_start + 32)];
            let bell = tail.iter().position(|byte| *byte == 7);
            let st = find(tail, b"\x1b\\");
            let end = match (bell, st) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) | (None, Some(a)) => a,
                _ => {
                    if self.pending.len() - code_start > 32
                        || tail.iter().any(|byte| {
                            !byte.is_ascii_digit()
                                && !matches!(*byte, b'+' | b'-' | b' ' | b'\t' | 0x1b)
                        })
                    {
                        self.scan = start + 1;
                        continue;
                    }
                    self.scan = start;
                    break;
                }
            };
            if let Some(code) = std::str::from_utf8(&tail[..end])
                .ok()
                .and_then(|value| value.trim().parse().ok())
            {
                let mut output_start = 0;
                if self.fish
                    && let Some(started) = self.pending[..start]
                        .windows(b"\x1b]4545;CommandStarted;".len())
                        .rposition(|window| window == b"\x1b]4545;CommandStarted;")
                {
                    let tail = &self.pending[started..start];
                    let bell = tail.iter().position(|byte| *byte == 7).map(|end| end + 1);
                    let st = find(tail, b"\x1b\\").map(|end| end + 2);
                    output_start = started
                        + match (bell, st) {
                            (Some(a), Some(b)) => a.min(b),
                            (Some(a), None) | (None, Some(a)) => a,
                            _ => 0,
                        };
                }
                let mut output = self.head.clone();
                if !output.is_empty() {
                    output.extend_from_slice(b"\n[Middle of terminal output omitted]\n");
                }
                output.extend_from_slice(&self.pending[output_start..start]);
                if output.len() > CAPTURE_LIMIT {
                    let end = output.len() - CAPTURE_LIMIT / 2;
                    output.splice(
                        CAPTURE_LIMIT / 2..end,
                        b"\n[Middle of terminal output omitted]\n".iter().copied(),
                    );
                }
                return Some((output, code));
            }
            self.scan = start + 1;
        }
        if self.pending.len() > CAPTURE_LIMIT {
            if self.head.is_empty() {
                self.head
                    .extend_from_slice(&self.pending[..CAPTURE_LIMIT / 2]);
            }
            let remove = self.pending.len() - CAPTURE_LIMIT / 2;
            self.pending.drain(..remove);
            self.scan = self.scan.saturating_sub(remove);
        }
        self.scan = self
            .scan
            .max(self.pending.len().saturating_sub(self.prefix.len() + 128));
        None
    }
    fn clean(&self, bytes: &[u8], command: &str) -> String {
        // Fish's CommandStarted boundary already excludes input echo.
        clean_output(bytes, (!self.fish).then_some(command))
    }
    fn partial(&self) -> Vec<u8> {
        let mut bytes = self.head.clone();
        bytes.extend_from_slice(&self.pending);
        bytes
    }
}
impl Drop for Parser {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.pending.zeroize();
        self.head.zeroize();
    }
}
fn clean_output(bytes: &[u8], command: Option<&str>) -> String {
    let text = context::clean(&String::from_utf8_lossy(bytes));
    let text = text.trim();
    let command = command.unwrap_or_default().replace("\r\n", "\n");
    let command = command.trim();
    let text = if command.is_empty() {
        text
    } else if let Some((line, rest)) = text.split_once('\n')
        && line.trim_end().ends_with(command)
    {
        rest.trim()
    } else if !text.contains('\n') && text.ends_with(command) {
        ""
    } else {
        echo_end(text, command, true)
            .or_else(|| echo_end(text, command, false))
            .map_or(text, |end| text[end..].trim())
    };
    let chars: Vec<_> = text.char_indices().collect();
    if chars.len() <= 12_000 {
        return text.into();
    }
    format!(
        "{}\n[Middle of terminal output omitted]\n{}",
        &text[..chars[6000].0],
        &text[chars[chars.len() - 6000].0..]
    )
}
fn echo_end(output: &str, command: &str, whitespace: bool) -> Option<usize> {
    let command = if whitespace {
        command.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        command.chars().filter(|ch| !ch.is_whitespace()).collect()
    };
    if command.is_empty() {
        return None;
    }
    let mut normalized = String::new();
    let mut positions = vec![];
    let mut previous_space = false;
    let limit = 4096.max(command.len().saturating_mul(4));
    for (index, ch) in output.char_indices().take(limit) {
        if ch.is_whitespace() {
            if whitespace && !previous_space {
                normalized.push(' ');
                positions.push(index + ch.len_utf8());
            }
        } else {
            normalized.push(ch);
            positions.extend(std::iter::repeat_n(index + ch.len_utf8(), ch.len_utf8()));
        }
        previous_space = ch.is_whitespace();
    }
    let start = normalized.find(&command)?;
    if normalized[..start].trim().len() > command.len() {
        return None;
    }
    positions.get(start + command.len() - 1).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn powershell_sessions_use_their_own_setup_syntax_and_in_process_tracking() {
        for path in [
            "pwsh",
            "powershell.exe",
            "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
        ] {
            let integration = Integration::local(Some(path), true).unwrap();
            assert_eq!(integration.kind, Kind::PowerShell);
            assert_eq!(integration.clean_line_keys(), b"\x18\x15");
            let setup = integration.setup();
            assert!(setup.contains(". ([scriptblock]::Create"));
            assert!(setup.contains(&format!("nauterm-integration-ready={}", integration.token)));
            assert!(!setup.contains("printf"));
        }
        assert!(Integration::local(Some("powershell.exe"), false).is_none());
        assert!(Integration::local(Some("cmd.exe"), true).is_none());
    }
    #[test]
    fn completion_markers_and_utf8_survive_every_boundary_and_both_terminators() {
        for end in [b"\x07".as_slice(), b"\x1b\\".as_slice()] {
            let mut bytes = "echo test\r\n结果\n".as_bytes().to_vec();
            bytes.extend_from_slice(b"\x1b]777;nauterm-command-end=token;23");
            bytes.extend_from_slice(end);
            for split in 0..=bytes.len() {
                let mut parser = Parser::new("token", false);
                let first = parser.push(&bytes[..split]);
                let result = first
                    .or_else(|| parser.push(&bytes[split..]))
                    .expect("complete marker");
                assert_eq!(result.1, 23);
                assert_eq!(parser.clean(&result.0, "echo test"), "结果");
            }
        }
    }
    #[test]
    fn malformed_or_partial_markers_cannot_hide_the_real_completion() {
        let mut parser = Parser::new("token", false);
        assert!(
            parser
                .push(b"output\x1b]777;nauterm-command-end=token;not-a-code")
                .is_none()
        );
        let (_, code) = parser
            .push(b"\x1b]777;nauterm-command-end=token;0\x07")
            .expect("real completion after malformed prefix");
        assert_eq!(code, 0);
        let mut parser = Parser::new("token", false);
        assert!(
            parser
                .push(b"\x1b]777;nauterm-command-end=other;0\x07")
                .is_none()
        );
        assert!(
            parser
                .push(b"\x1b]777;nauterm-command-end=token;0")
                .is_none()
        );
    }
    #[test]
    fn large_output_is_bounded_and_fish_skips_the_command_start_echo() {
        let mut parser = Parser::new("token", false);
        parser.push(b"first error\n");
        parser.push(&vec![b'x'; CAPTURE_LIMIT * 2]);
        assert!(parser.pending.len() + parser.head.len() <= CAPTURE_LIMIT);
        let (output, _) = parser
            .push(b"\nlast line\x1b]777;nauterm-command-end=token;0\x07")
            .unwrap();
        assert!(output.starts_with(b"first error"));
        assert!(output.ends_with(b"last line"));
        let mut parser = Parser::new("token", true);
        let (output,_)=parser.push(b"draft echo\x1b]4545;CommandStarted;cHdk\x1b\\actual output\x07\x1b]777;nauterm-command-end=token;0\x07").unwrap();
        assert_eq!(parser.clean(&output, "pwd"), "actual output");
    }
    #[test]
    fn echo_is_removed_once_and_fish_output_is_never_treated_as_echo() {
        assert_eq!(
            clean_output(b"pwd\npwd\nresult", Some("pwd")),
            "pwd\nresult"
        );
        assert_eq!(
            clean_output(b"$ echo hello\nhello", Some("echo hello")),
            "hello"
        );
        assert_eq!(
            clean_output(
                b"$ printf long\ncommand\nresult",
                Some("printf longcommand")
            ),
            "result"
        );
        assert_eq!(clean_output(b"pwd\nresult", None), "pwd\nresult");
        let mut parser = Parser::new("token", true);
        let (output, _) = parser.push(b"echo\x1b]4545;CommandStarted;cHdk\x07pwd\nresult\x1b]777;nauterm-command-end=token;0\x07").unwrap();
        assert_eq!(parser.clean(&output, "pwd"), "pwd\nresult");
    }
    #[test]
    fn returned_output_is_redacted_before_head_tail_summary() {
        let output = format!(
            "pwd\npassword=synthetic-secret\n{}\nlast",
            "文".repeat(20_000)
        );
        let cleaned = clean_output(output.as_bytes(), Some("pwd"));
        assert!(!cleaned.contains("synthetic-secret"));
        assert!(cleaned.starts_with("password=[REDACTED]"));
        assert!(cleaned.ends_with("last"));
        assert!(cleaned.contains("Middle of terminal output omitted"));
    }
}
