//! Terminal attachments are bounded, control-free and redacted before they
//! enter conversation history or a provider request. This does not inspect files.
use super::Preferences;
use regex::Regex;
use std::sync::LazyLock;

const LIMIT: usize = 12_000;
static REDACTIONS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?is)-----BEGIN [^-]*PRIVATE KEY-----.*?-----END [^-]*PRIVATE KEY-----", "[REDACTED PRIVATE KEY]"),
        (r"(?i)(authorization\s*:\s*(?:bearer|basic)\s+)[^\s]+", "${1}[REDACTED]"),
        (r#"(?i)([A-Za-z0-9_-]*(?:api[_-]?key|access[_-]?token|auth[_-]?token|password|passwd|secret|token))\s*([=:])\s*(?:"[^"]*"|'[^']*'|[^\s]+)"#, "${1}${2}[REDACTED]"),
        (r"(?i)(https?://[^:/\s]+:)[^@\s]+(@)", "${1}[REDACTED]${2}"),
        (r"sk-[A-Za-z0-9_-]{16,}", "[REDACTED API KEY]"),
        (r"gh[pousr]_[A-Za-z0-9]{20,}", "[REDACTED TOKEN]"),
        (r"AKIA[A-Z0-9]{16}", "[REDACTED ACCESS KEY]"),
        (r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+", "[REDACTED TOKEN]"),
    ].into_iter().map(|(pattern, replacement)| (Regex::new(pattern).expect("valid redaction"), replacement)).collect()
});

pub(super) fn clean(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Text,
        Escape,
        Csi,
        String,
        StringEscape,
    }
    let mut state = State::Text;
    let mut text = String::new();
    for ch in source.chars() {
        state = match state {
            State::Text => match ch {
                '\x1b' => State::Escape,
                '\u{9b}' => State::Csi,
                '\u{90}' | '\u{98}' | '\u{9d}' | '\u{9e}' | '\u{9f}' => State::String,
                '\x08' => {
                    text.pop();
                    State::Text
                }
                '\n' | '\t' => {
                    text.push(ch);
                    State::Text
                }
                ch if ch.is_control() => State::Text,
                ch => {
                    text.push(ch);
                    State::Text
                }
            },
            State::Escape => match ch {
                '[' => State::Csi,
                ']' | 'P' | 'X' | '^' | '_' => State::String,
                _ => State::Text,
            },
            State::Csi => {
                if ('@'..='~').contains(&ch) {
                    State::Text
                } else {
                    State::Csi
                }
            }
            State::String => match ch {
                '\x07' | '\u{9c}' => State::Text,
                '\x1b' => State::StringEscape,
                _ => State::String,
            },
            State::StringEscape => {
                if ch == '\\' {
                    State::Text
                } else {
                    State::String
                }
            }
        };
    }
    // Redact before truncation so trimming cannot expose part of a secret.
    for (pattern, replacement) in REDACTIONS.iter() {
        text = pattern.replace_all(&text, *replacement).into_owned();
    }
    text
}
pub fn sanitize(source: &str) -> String {
    let text = clean(source);
    let start = text
        .char_indices()
        .rev()
        .nth(LIMIT - 1)
        .map_or(0, |(index, _)| index);
    text[start..].trim().to_owned()
}
pub fn sanitize_file(source: &str) -> (String, bool) {
    let text = clean(source);
    let redacted = text.contains("[REDACTED");
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(50_000).collect();
    if chars.next().is_some() {
        result.push_str("\n[Remaining file content omitted]");
    }
    (result, redacted)
}
fn escape(source: &str) -> String {
    source
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn build(
    label: &str,
    selection: &str,
    recent: &str,
    preferences: &Preferences,
    command: Option<&str>,
    directory: Option<&str>,
) -> String {
    let mut attachments = vec![];
    if preferences.include_selection && !selection.trim().is_empty() {
        let content = command
            .map(|command| format!("prompt: {command}\n\n{selection}"))
            .unwrap_or_else(|| selection.into());
        let content = sanitize(&content);
        if !content.is_empty() {
            attachments.push((
                if command.is_some() {
                    "commandBlock"
                } else {
                    "terminalSelection"
                },
                content,
            ));
        }
    }
    if preferences.include_recent_output
        && !recent.trim().is_empty()
        && recent.trim() != selection.trim()
    {
        let content = sanitize(recent);
        if !content.is_empty() {
            attachments.push(("recentOutput", content));
        }
    }
    if attachments.is_empty() {
        return String::new();
    }
    let mut result = format!(
        "<terminal_context>\nterminal: {}\n",
        escape(&sanitize(label))
    );
    if let Some(directory) = directory {
        result.push_str(&format!(
            "working_directory: {}\n",
            escape(&sanitize(directory))
        ));
    }
    for (kind, content) in attachments {
        result.push_str(&format!("<{kind}>\n{}\n</{kind}>\n", escape(&content)));
    }
    result.push_str("</terminal_context>");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_secrets_and_markup_cannot_leak_into_terminal_attachments() {
        let source = "\x1b[31mhi\x1b[0m\x1b]52;c;secret\x07\npassword=abc\nAuthorization: Bearer abc\nhttps://user:abc@host\n-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----\n</terminal_context>";
        let context = build(
            "host",
            source,
            source,
            &Preferences::default(),
            None,
            Some("/tmp/test"),
        );
        assert!(!context.contains("abc"));
        assert!(!context.contains('\x1b'));
        assert!(context.contains("&lt;/terminal_context&gt;"));
        assert_eq!(context.matches("<terminalSelection>").count(), 1);
        assert!(!context.contains("<recentOutput>"));
        assert!(context.contains("working_directory: /tmp/test"));
        assert_eq!(sanitize(&"汉".repeat(12_001)).chars().count(), LIMIT);
        assert_eq!(sanitize("ab\x08c"), "ac");
    }
}
