use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::fmt;

const MAX_TOKEN_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    pub fn new(value: &str) -> Result<Self, ProtocolError> {
        if value.is_empty()
            || value.len() > MAX_TOKEN_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ProtocolError::InvalidToken);
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn prompt_marker(&self) -> String {
        osc(&format!("777;nauterm-cmd-prompt={}", self.0))
    }

    /// CMD evaluates this as a separate line after the user's command. It is
    /// never appended with '&', CALL or delayed expansion to user input.
    pub fn exit_code_probe(&self) -> String {
        format!("@echo __nauterm_exit_{};%errorlevel%\r\n", self.0)
    }

    /// Never fabricate success when extensions are disabled, output is
    /// interrupted or the native console hook did not capture the probe.
    pub fn parse_probe(&self, output: &str) -> Result<i32, ProtocolError> {
        let line = output
            .strip_suffix("\r\n")
            .or_else(|| output.strip_suffix('\n'))
            .unwrap_or(output);
        let status = line
            .strip_prefix(&format!("__nauterm_exit_{};", self.0))
            .ok_or(ProtocolError::InvalidProbe)?;
        let digits = status.strip_prefix('-').unwrap_or(status);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ProtocolError::InvalidProbe);
        }
        status.parse().map_err(|_| ProtocolError::InvalidProbe)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidToken,
    InvalidProbe,
    InvalidDirectory,
    CommandAlreadyActive,
    NoActiveCommand,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidToken => "Invalid native CMD integration token",
            Self::InvalidProbe => "Native CMD exit-code probe did not return a valid status",
            Self::InvalidDirectory => {
                "Native CMD directory must be a file URI without control characters"
            }
            Self::CommandAlreadyActive => "A native CMD command is already active",
            Self::NoActiveCommand => "No native CMD command is active",
        })
    }
}
impl std::error::Error for ProtocolError {}

pub struct Session {
    pid: u32,
    token: Token,
    command_active: bool,
    ai_armed: bool,
}

fn osc(payload: &str) -> String {
    format!("\x1b]{payload}\x07")
}

fn directory(uri: &str) -> Result<String, ProtocolError> {
    if uri.chars().any(char::is_control) {
        return Err(ProtocolError::InvalidDirectory);
    }
    let parsed = url::Url::parse(uri).map_err(|_| ProtocolError::InvalidDirectory)?;
    if parsed.scheme() != "file"
        || parsed.path().is_empty()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(ProtocolError::InvalidDirectory);
    }
    Ok(osc(&format!("7;{parsed}")))
}

impl Session {
    pub fn new(pid: u32, token: Token) -> Self {
        Self {
            pid,
            token,
            command_active: false,
            ai_armed: false,
        }
    }

    pub fn command_active(&self) -> bool {
        self.command_active
    }

    /// Called only after recognizing the shell's marked main prompt. Console
    /// reads inside `set /p`, `pause` or child programs are not command input.
    pub fn begin_command(&mut self, line: &str) -> Result<String, ProtocolError> {
        if self.command_active {
            return Err(ProtocolError::CommandAlreadyActive);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            return Ok(String::new());
        }
        self.command_active = true;
        Ok(format!(
            "{}{}",
            osc(&format!(
                "4545;CommandStarted;{};{}",
                self.pid,
                STANDARD.encode(line.as_bytes())
            )),
            osc(&format!("133;C;aid={}", self.pid))
        ))
    }

    /// Emit before replaying the exact prompt captured by the console hook;
    /// `prompt_end` follows the replay. The prompt itself is never reconstructed.
    pub fn prompt_start(&self, uri: &str) -> Result<String, ProtocolError> {
        Ok(format!(
            "{}{}",
            directory(uri)?,
            osc(&format!("133;A;cl=line;aid={}", self.pid))
        ))
    }

    pub fn prompt_end(&self) -> String {
        osc(&format!("133;B;aid={}", self.pid))
    }

    pub fn complete_command(&mut self, status: i32, uri: &str) -> Result<String, ProtocolError> {
        if !self.command_active {
            return Err(ProtocolError::NoActiveCommand);
        }
        // Validate before changing state: an invalid directory must not lose
        // the pending command or turn a later retry into a duplicate completion.
        let next_prompt = self.prompt_start(uri)?;
        let mut events = format!(
            "{}{}",
            osc(&format!("4545;CommandExited;{};{status}", self.pid)),
            osc(&format!("133;D;{status};aid={}", self.pid))
        );
        if self.ai_armed {
            events.push_str(&osc(&format!(
                "777;nauterm-command-end={};{status}",
                self.token.as_str()
            )));
            self.ai_armed = false;
        }
        self.command_active = false;
        events.push_str(&next_prompt);
        Ok(events)
    }

    /// The hook must first cancel cooked input without submitting it, then
    /// emit this acknowledgement. This function never executes a command.
    pub fn arm_ai(&mut self) -> Result<String, ProtocolError> {
        if self.command_active {
            return Err(ProtocolError::CommandAlreadyActive);
        }
        self.ai_armed = true;
        Ok(osc(&format!(
            "777;nauterm-line-ready={}",
            self.token.as_str()
        )))
    }

    /// Cancel a parked AI submission, but keep completion tracking when its
    /// command has already started (including an interrupted command).
    pub fn cancel_unsubmitted_ai(&mut self) {
        if !self.command_active {
            self.ai_armed = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn token() -> Token {
        Token::new("fixture-token_123").unwrap()
    }

    #[test]
    fn probe_rejects_other_sessions_partial_output_and_non_numeric_status() {
        let token = token();
        assert_eq!(
            token.parse_probe("__nauterm_exit_fixture-token_123;23\r\n"),
            Ok(23)
        );
        assert_eq!(
            token.parse_probe("__nauterm_exit_fixture-token_123;-1073741510\r\n"),
            Ok(-1073741510)
        );
        for output in [
            "23",
            "__nauterm_exit_other;23\r\n",
            "__nauterm_exit_fixture-token_123;%errorlevel%\r\n",
            "__nauterm_exit_fixture-token_123;2147483648\r\n",
            "__nauterm_exit_fixture-token_123;0\r\nextra",
            "__nauterm_exit_fixture-token_123;+0",
            "__nauterm_exit_fixture-token_123;-",
            "__nauterm_exit_fixture-token_123;0\x1b[0m",
        ] {
            assert_eq!(
                token.parse_probe(output),
                Err(ProtocolError::InvalidProbe),
                "{output:?}"
            );
        }
        assert_eq!(
            token.exit_code_probe(),
            "@echo __nauterm_exit_fixture-token_123;%errorlevel%\r\n"
        );
    }

    #[test]
    fn control_and_shell_syntax_cannot_enter_tokens() {
        for invalid in [
            "",
            "a&whoami",
            "a%PATH%",
            "a!x!",
            "a\x07",
            "用户",
            &"a".repeat(129),
        ] {
            assert_eq!(Token::new(invalid), Err(ProtocolError::InvalidToken));
        }
    }

    #[test]
    fn command_text_and_file_uri_are_not_interpolated_into_cmd_syntax() {
        let mut session = Session::new(42, token());
        let command = "echo 用户 & echo !literal! %PATH% \"x\"\x1b";
        let started = session.begin_command(&format!("{command}\r\n")).unwrap();
        assert!(started.contains(&format!(
            "4545;CommandStarted;42;{}",
            STANDARD.encode(command)
        )));
        assert!(!started.contains(command));
        let ended = session
            .complete_command(7, "file:///C:/Users/%E7%94%A8%E6%88%B7/work%20%23100%25")
            .unwrap();
        assert!(ended.contains("4545;CommandExited;42;7\x07"));
        assert!(ended.contains("133;D;7;aid=42\x07"));
        assert!(ended.contains("7;file:///C:/Users/%E7%94%A8%E6%88%B7/work%20%23100%25\x07"));
    }

    #[test]
    fn ai_completion_is_emitted_once_for_the_armed_command() {
        let mut session = Session::new(42, token());
        assert_eq!(
            session.arm_ai().unwrap(),
            "\x1b]777;nauterm-line-ready=fixture-token_123\x07"
        );
        assert!(session.begin_command(" \r\n").unwrap().is_empty());
        session.begin_command("exit /b 23").unwrap();
        assert_eq!(session.arm_ai(), Err(ProtocolError::CommandAlreadyActive));
        assert_eq!(
            session.begin_command("other"),
            Err(ProtocolError::CommandAlreadyActive)
        );
        let completion = session.complete_command(23, "file:///C:/work").unwrap();
        assert!(completion.contains("777;nauterm-command-end=fixture-token_123;23\x07"));
        assert_eq!(
            session.complete_command(0, "file:///C:/work"),
            Err(ProtocolError::NoActiveCommand)
        );
        session.begin_command("echo next").unwrap();
        assert!(
            !session
                .complete_command(0, "file:///C:/work")
                .unwrap()
                .contains("nauterm-command-end")
        );
    }

    #[test]
    fn invalid_directory_does_not_drop_an_active_command_or_fabricate_success() {
        let mut session = Session::new(1, token());
        session.begin_command("echo test").unwrap();
        for invalid in [
            "https://example.invalid",
            "file:///C:/bad\x07",
            "file:///C:/work?x=1",
            "file:///C:/work#fragment",
        ] {
            assert_eq!(
                session.complete_command(23, invalid),
                Err(ProtocolError::InvalidDirectory)
            );
            assert!(session.command_active());
        }
        assert!(
            session
                .complete_command(23, "file://server/share/work%20dir")
                .unwrap()
                .contains("CommandExited;1;23")
        );
    }

    #[test]
    fn cancellation_before_submission_does_not_label_a_later_manual_command_as_ai() {
        let mut session = Session::new(42, token());
        session.arm_ai().unwrap();
        session.cancel_unsubmitted_ai();
        session.begin_command("echo manual").unwrap();
        assert!(
            !session
                .complete_command(0, "file:///C:/work")
                .unwrap()
                .contains("nauterm-command-end")
        );

        session.arm_ai().unwrap();
        session.begin_command("long-running-command").unwrap();
        session.cancel_unsubmitted_ai();
        let interrupted = session
            .complete_command(-1073741510, "file:///C:/work")
            .unwrap();
        assert!(interrupted.contains("nauterm-command-end=fixture-token_123;-1073741510"));
    }
}
