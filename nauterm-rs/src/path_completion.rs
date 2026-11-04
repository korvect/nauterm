//! Shell-specific path-query/quoting policy. This never evaluates shell input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsShell {
    PowerShell,
    Cmd,
}
impl WindowsShell {
    pub fn from_path(path: &str) -> Option<Self> {
        let normalized = path.replace('\\', "/");
        match normalized.rsplit('/').next()?.to_ascii_lowercase().as_str() {
            "pwsh.exe" | "pwsh" | "powershell.exe" | "powershell" => Some(Self::PowerShell),
            "cmd.exe" | "cmd" => Some(Self::Cmd),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quote {
    None,
    Single,
    Double,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub directory: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub directory: String,
    prefix: String,
    parent: String,
    name: String,
    directories_only: bool,
    quote: Quote,
    cd: bool,
    windows: Option<WindowsShell>,
}
fn join(parent: &str, child: &str) -> String {
    if child.is_empty() {
        parent.into()
    } else {
        format!(
            "{}{child}",
            if parent.ends_with('/') {
                parent.into()
            } else {
                format!("{parent}/")
            }
        )
    }
}
pub fn resolve(path: &str, cwd: &str, home: Option<&str>, local: bool) -> String {
    let expanded = if local && path == "~" {
        home.unwrap_or(path).into()
    } else if local && path.starts_with("~/") {
        home.map(|h| join(h, &path[2..]))
            .unwrap_or_else(|| path.into())
    } else {
        path.to_owned()
    };
    if expanded.is_empty() {
        cwd.into()
    } else if expanded.starts_with('/')
        || expanded == "~"
        || expanded.starts_with("~/")
        || (local && cfg!(windows) && std::path::Path::new(&expanded).is_absolute())
    {
        expanded
    } else {
        join(cwd, &expanded)
    }
}
fn unescape(path: &str, keep_trailing: bool) -> String {
    let mut result = String::new();
    let mut escaped = false;
    for ch in path.chars() {
        if escaped {
            result.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            result.push(ch);
        }
    }
    if escaped && keep_trailing {
        result.push('\\');
    }
    result
}
fn escape(path: &str, quote: Quote, directory: bool) -> String {
    let mut result = String::new();
    for ch in path.chars() {
        match quote {
            Quote::Single if ch == '\'' => result.push_str("'\\''"),
            Quote::Double if ['\\', '"', '$', '`'].contains(&ch) => {
                result.push('\\');
                result.push(ch);
            }
            Quote::None if ch.is_whitespace() || "\\'\"$`!#&;|<>()[]{}*?".contains(ch) => {
                result.push('\\');
                result.push(ch);
            }
            _ => result.push(ch),
        }
    }
    if !directory {
        match quote {
            Quote::Single => result.push('\''),
            Quote::Double => result.push('"'),
            _ => {}
        }
    }
    result
}
fn token_start(input: &str) -> usize {
    let mut start = 0;
    let mut escaped = false;
    let mut quote = Quote::None;
    for (index, ch) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Quote::Single {
            if ch == '\'' {
                quote = Quote::None;
            }
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if quote == Quote::Double {
            if ch == '"' {
                quote = Quote::None;
            }
            continue;
        }
        match ch {
            '\'' => quote = Quote::Single,
            '"' => quote = Quote::Double,
            _ if ch.is_whitespace() => start = index + ch.len_utf8(),
            _ => {}
        }
    }
    start
}
fn words(input: &str) -> Option<Vec<String>> {
    let mut result = vec![];
    let mut word = String::new();
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch.is_whitespace() {
            if !word.is_empty() {
                result.push(std::mem::take(&mut word));
            }
        } else {
            word.push(ch);
        }
    }
    if escaped {
        return None;
    }
    if !word.is_empty() {
        result.push(word);
    }
    Some(result)
}
fn assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
fn command(words: &[String]) -> Option<&str> {
    let mut index = 0;
    while words.get(index).is_some_and(|w| assignment(w)) {
        index += 1;
    }
    while let Some(word) = words.get(index) {
        match word.as_str() {
            "sudo" => {
                index += 1;
                while let Some(option) = words.get(index).filter(|w| w.starts_with('-')) {
                    index += 1;
                    if [
                        "-u",
                        "-g",
                        "-h",
                        "-p",
                        "-C",
                        "--user",
                        "--group",
                        "--host",
                        "--prompt",
                        "--close-from",
                    ]
                    .contains(&option.as_str())
                    {
                        index += usize::from(index < words.len());
                    }
                }
            }
            "env" => {
                index += 1;
                while words.get(index).is_some_and(|w| assignment(w)) {
                    index += 1;
                }
            }
            "command" | "builtin" | "exec" | "nohup" => index += 1,
            _ => return word.trim_end_matches('/').rsplit('/').next(),
        }
    }
    None
}
fn option_mode(option: &str) -> Option<bool> {
    let option = option.split('=').next().unwrap_or(option);
    if [
        "-C",
        "--chdir",
        "--cwd",
        "--directory",
        "--working-directory",
    ]
    .contains(&option)
        || option.ends_with("-dir")
        || option.ends_with("-directory")
    {
        Some(true)
    } else if [
        "-c",
        "-f",
        "-i",
        "-o",
        "--cert",
        "--certificate",
        "--config",
        "--file",
        "--identity",
        "--identity-file",
        "--input",
        "--key",
        "--known-hosts",
        "--output",
        "--path",
    ]
    .contains(&option)
        || option.ends_with("-file")
        || option.ends_with("-path")
    {
        Some(false)
    } else {
        None
    }
}
impl Query {
    pub fn parse_windows(
        input: &str,
        cwd: &str,
        home: Option<&str>,
        shell: WindowsShell,
    ) -> Option<Self> {
        if input.is_empty()
            || input.len() > 64 * 1024
            || input.chars().any(|ch| ch.is_control() && ch != '\t')
            || cwd.is_empty()
            || cwd.chars().any(char::is_control)
        {
            return None;
        }
        let mut start = 0;
        let mut quote = Quote::None;
        let mut escaped = false;
        for (index, ch) in input.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if (shell == WindowsShell::PowerShell && ch == '`' && quote != Quote::Single)
                || (shell == WindowsShell::Cmd && ch == '^' && quote == Quote::None)
            {
                escaped = true;
                continue;
            }
            match ch {
                '\'' if shell == WindowsShell::PowerShell && quote != Quote::Double => {
                    quote = if quote == Quote::Single {
                        Quote::None
                    } else {
                        Quote::Single
                    };
                }
                '"' if quote != Quote::Single => {
                    quote = if quote == Quote::Double {
                        Quote::None
                    } else {
                        Quote::Double
                    };
                }
                ';' | '|' | '&' if quote == Quote::None => return None,
                _ if ch.is_whitespace() && quote == Quote::None => start = index + ch.len_utf8(),
                _ => {}
            }
        }
        let mut before = input[..start].to_owned();
        let mut raw = &input[start..];
        let command = input.split_whitespace().next()?.to_ascii_lowercase();
        let cd = command == "cd"
            || command == "chdir"
            || command == "set-location"
            || command == "pushd";
        // Keep a directory-change argument together while the user types its
        // spaces; generated native-shell suggestions add the required quotes.
        if cd {
            let rest = input.trim_start().get(command.len()..)?.trim_start();
            let rest = if shell == WindowsShell::Cmd
                && rest.get(..2).is_some_and(|s| s.eq_ignore_ascii_case("/d"))
                && (rest.len() == 2 || rest[2..].starts_with(char::is_whitespace))
            {
                rest[2..].trim_start()
            } else {
                rest
            };
            if !rest.starts_with('-') {
                raw = rest;
                before = input[..input.len() - rest.len()].to_owned();
            }
        }
        let (path, quote) = if let Some(value) = raw.strip_prefix('"') {
            let value = value.strip_suffix('"').unwrap_or(value);
            if value.contains('"') {
                return None;
            }
            (value.to_owned(), Quote::Double)
        } else if shell == WindowsShell::PowerShell {
            if let Some(value) = raw.strip_prefix('\'') {
                let value = value.strip_suffix('\'').unwrap_or(value);
                if value.replace("''", "").contains('\'') {
                    return None;
                }
                (value.replace("''", "'"), Quote::Single)
            } else {
                (raw.to_owned(), Quote::None)
            }
        } else {
            (raw.to_owned(), Quote::None)
        };
        let path = path.as_str();
        // CMD expands these even inside quotes. Do not offer a completion that
        // could execute a different path under the user's expansion settings.
        if shell == WindowsShell::Cmd && path.contains(['%', '!']) {
            return None;
        }
        if path.as_bytes().get(1) == Some(&b':') && !path.get(2..)?.starts_with(['/', '\\']) {
            return None; // Drive-relative paths need that drive's separate cwd.
        }
        let separator = if path.contains('\\') || !path.contains('/') {
            '\\'
        } else {
            '/'
        };
        let (parent, name) = path
            .rfind(['\\', '/'])
            .map_or(("", path), |i| (&path[..i + 1], &path[i + 1..]));
        let absolute = parent.starts_with(['/', '\\']) || parent.as_bytes().get(1) == Some(&b':');
        let directory = if parent.is_empty() {
            cwd.to_owned()
        } else if shell == WindowsShell::PowerShell
            && (parent.starts_with("~/") || parent.starts_with("~\\"))
        {
            format!(
                "{}{separator}{}",
                home.unwrap_or(cwd).trim_end_matches(['/', '\\']),
                &parent[2..]
            )
        } else if absolute {
            parent.to_owned()
        } else {
            format!("{}{separator}{parent}", cwd.trim_end_matches(['/', '\\']))
        };
        let mut prefix = before;
        if quote != Quote::None {
            prefix.push(if quote == Quote::Single { '\'' } else { '"' });
        }
        Some(Self {
            directory,
            prefix,
            parent: parent.to_owned(),
            name: name.to_owned(),
            directories_only: cd,
            quote,
            cd,
            windows: Some(shell),
        })
    }
    pub fn pending(&self) -> Vec<String> {
        if self.cd {
            self.candidates(&[], 1)
        } else {
            vec![]
        }
    }
    pub fn parse(input: &str, cwd: &str, home: Option<&str>, local: bool) -> Option<Self> {
        if input.is_empty()
            || input.len() > 64 * 1024
            || input.chars().any(|ch| ch.is_control() && ch != '\t')
            || input.contains([';', '|', '&'])
            || cwd.is_empty()
            || cwd.contains(['\n', '\r', '\0'])
        {
            return None;
        }
        // The reference's dedicated cd path normalizes the command and offers
        // `cd ..` without a slash before falling back to general quoted paths.
        let trimmed = input.trim_start();
        if let Some(raw) = trimmed
            .strip_prefix("cd")
            .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
        {
            let raw = raw.trim_start();
            if !raw.starts_with(['\'', '"']) && !raw.contains(['\'', '"']) {
                let path = unescape(raw, true);
                return Some(Self::from_path(
                    "cd ".into(),
                    path,
                    (cwd, home, local),
                    Quote::None,
                    true,
                    true,
                ));
            }
        }
        let start = token_start(input);
        let before = &input[..start];
        let raw = &input[start..];
        if before.contains(['\'', '"']) {
            return None;
        }
        let context = words(before)?;
        let mut prefix = before.to_owned();
        let mut token = raw;
        let mut directories_only = matches!(command(&context), Some("cd" | "pushd" | "rmdir"));
        if let Some((option, value)) = raw.split_once('=').filter(|_| raw.starts_with('-')) {
            directories_only = option_mode(option)?;
            prefix.push_str(option);
            prefix.push('=');
            token = value;
        } else {
            directories_only = context
                .last()
                .and_then(|word| option_mode(word))
                .unwrap_or(directories_only);
            if raw.starts_with('-') && !raw.contains('/') {
                return None;
            }
        }
        let (path, quote) = if let Some(value) = token.strip_prefix('\'') {
            if value.contains('\'') {
                return None;
            }
            prefix.push('\'');
            (value.into(), Quote::Single)
        } else if let Some(value) = token.strip_prefix('"') {
            let mut escaped = false;
            for ch in value.chars() {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    return None;
                }
            }
            prefix.push('"');
            (unescape(value, true), Quote::Double)
        } else {
            if token.contains(['\'', '"']) {
                return None;
            }
            (unescape(token, false), Quote::None)
        };
        if context.is_empty() && !path.starts_with(['/', '~']) {
            return None;
        }
        Some(Self::from_path(
            prefix,
            path,
            (cwd, home, local),
            quote,
            directories_only,
            false,
        ))
    }
    fn from_path(
        prefix: String,
        path: String,
        (cwd, home, local): (&str, Option<&str>, bool),
        quote: Quote,
        directories_only: bool,
        cd: bool,
    ) -> Self {
        let (parent, name) = path
            .rfind('/')
            .map_or(("", path.as_str()), |i| (&path[..i + 1], &path[i + 1..]));
        Self {
            directory: resolve(parent, cwd, home, local),
            prefix,
            parent: parent.into(),
            name: name.into(),
            directories_only,
            quote,
            cd,
            windows: None,
        }
    }
    pub fn candidates(&self, entries: &[Entry], limit: usize) -> Vec<String> {
        let limit = limit.clamp(1, 999);
        let mut result = vec![];
        if self.parent.is_empty() && "..".starts_with(&self.name) {
            result.push(if self.cd {
                "cd ..".into()
            } else {
                format!("{}{}", self.prefix, escape("../", self.quote, true))
            });
        }
        let name = self.name.to_lowercase();
        let mut entries = entries
            .iter()
            .filter(|entry| {
                !entry.name.is_empty()
                    && ![".", ".."].contains(&entry.name.as_str())
                    && !entry.name.contains(['/', '\0', '\n', '\r'])
                    && (self.windows.is_none() || !entry.name.contains(['\\', '"']))
                    && (self.windows != Some(WindowsShell::Cmd) || !entry.name.contains(['%', '!']))
                    && (!self.directories_only || entry.directory)
                    && (self.name.starts_with('.') || !entry.name.starts_with('.'))
                    && entry.name.to_lowercase().starts_with(&name)
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.name.to_lowercase());
        for entry in entries {
            if result.len() >= limit {
                break;
            }
            let path = format!(
                "{}{}{}",
                self.parent,
                entry.name,
                if entry.directory {
                    if self.windows.is_some() && !self.parent.contains('/') {
                        "\\"
                    } else {
                        "/"
                    }
                } else {
                    ""
                }
            );
            result.push(format!(
                "{}{}",
                self.prefix,
                if let Some(shell) = self.windows {
                    escape_windows(&path, self.quote, shell)
                } else {
                    escape(&path, self.quote, entry.directory)
                }
            ));
        }
        result
    }
}

fn escape_windows(path: &str, quote: Quote, shell: WindowsShell) -> String {
    let effective = if quote == Quote::None {
        if shell == WindowsShell::PowerShell {
            Quote::Single
        } else {
            Quote::Double
        }
    } else {
        quote
    };
    let delimiter = if effective == Quote::Single {
        '\''
    } else {
        '"'
    };
    let escaped = if shell == WindowsShell::PowerShell {
        if effective == Quote::Single {
            path.replace('\'', "''")
        } else {
            path.replace('`', "``")
                .replace('$', "`$")
                .replace('"', "`\"")
        }
    } else {
        path.to_owned()
    };
    let opening = if quote == Quote::None {
        delimiter.to_string()
    } else {
        String::new()
    };
    // Native completions are complete arguments, including directories. A cd
    // suggestion can run immediately; parsing accepts its closing quote when
    // offering the next directory level.
    format!("{opening}{escaped}{delimiter}")
}

#[cfg(test)]
mod windows_tests {
    use super::*;

    #[test]
    fn native_queries_preserve_drive_unc_and_relative_paths() {
        for shell in [WindowsShell::PowerShell, WindowsShell::Cmd] {
            for (input, expected) in [
                (r"type C:\Users\中文\fi", r"C:\Users\中文\"),
                (r"type \\server\share\fi", r"\\server\share\"),
                (r"type folder\fi", r"D:\work\folder\"),
                (r#"type "C:\with space\fi"#, r"C:\with space\"),
            ] {
                assert_eq!(
                    Query::parse_windows(input, r"D:\work", None, shell)
                        .unwrap()
                        .directory,
                    expected
                );
            }
            assert!(Query::parse_windows("type C:relative", r"D:\work", None, shell).is_none());
        }
        assert_eq!(
            Query::parse_windows(
                r"cd ~/中文/",
                r"D:\work",
                Some(r"C:\User Home"),
                WindowsShell::PowerShell
            )
            .unwrap()
            .directory,
            "C:\\User Home/中文/"
        );
    }

    #[test]
    fn native_suggestions_quote_metacharacters_and_do_not_use_posix_escapes() {
        let entries = [Entry {
            name: "中文 & O'Brien.txt".into(),
            directory: false,
        }];
        let input = "type 中";
        assert_eq!(
            Query::parse_windows(input, r"C:\", None, WindowsShell::PowerShell)
                .unwrap()
                .candidates(&entries, 4),
            ["type '中文 & O''Brien.txt'"]
        );
        assert_eq!(
            Query::parse_windows(input, r"C:\", None, WindowsShell::Cmd)
                .unwrap()
                .candidates(&entries, 4),
            ["type \"中文 & O'Brien.txt\""]
        );
        assert!(
            Query::parse_windows("type file & next", r"C:\", None, WindowsShell::Cmd).is_none()
        );
        assert!(
            Query::parse_windows("type %USERPROFILE%", r"C:\", None, WindowsShell::Cmd).is_none()
        );
        let folders = [Entry {
            name: "folder with space".into(),
            directory: true,
        }];
        assert_eq!(
            Query::parse_windows("cd /d C:\\fo", r"D:\work", None, WindowsShell::Cmd)
                .unwrap()
                .candidates(&folders, 4),
            ["cd /d \"C:\\folder with space\\\""]
        );
        assert_eq!(
            Query::parse_windows(
                r#"cd /d "C:\folder with space\""#,
                r"D:\work",
                None,
                WindowsShell::Cmd
            )
            .unwrap()
            .directory,
            r"C:\folder with space\"
        );
    }

    #[cfg(windows)]
    #[test]
    fn native_suggestions_open_real_unicode_files_in_both_shells() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let name = "中文 & O'Brien.txt";
        std::fs::write(
            root.path().join(name),
            b"nauterm-windows-completion-fixture",
        )?;
        let entries = [Entry {
            name: name.into(),
            directory: false,
        }];
        for (shell, executable, prefix) in [
            (
                WindowsShell::PowerShell,
                "powershell.exe",
                "Get-Content -LiteralPath ",
            ),
            (WindowsShell::Cmd, "cmd.exe", "type "),
        ] {
            let query = Query::parse_windows(
                &format!("{prefix}中"),
                root.path().to_str().unwrap(),
                None,
                shell,
            )
            .unwrap();
            let command = query.candidates(&entries, 4).remove(0);
            let mut process = std::process::Command::new(executable);
            match shell {
                WindowsShell::PowerShell => {
                    process.args(["-NoProfile", "-NonInteractive", "-Command", &command]);
                }
                WindowsShell::Cmd => {
                    use std::os::windows::process::CommandExt;
                    process
                        .args(["/d", "/s", "/c"])
                        .raw_arg(format!("\"{command}\""));
                }
            }
            let output = process.current_dir(root.path()).output()?;
            assert!(
                output.status.success(),
                "{command}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "nauterm-windows-completion-fixture"
            );
        }
        Ok(())
    }
}
