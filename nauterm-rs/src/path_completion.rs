//! Reference POSIX path-query/quoting policy. This never evaluates shell input.
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
    } else if expanded.starts_with('/') || expanded == "~" || expanded.starts_with("~/") {
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
                if entry.directory { "/" } else { "" }
            );
            result.push(format!(
                "{}{}",
                self.prefix,
                escape(&path, self.quote, entry.directory)
            ));
        }
        result
    }
}
