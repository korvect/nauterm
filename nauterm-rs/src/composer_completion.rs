//! Reference composer completion policy, independent of GPUI/native sessions.
use std::collections::HashSet;

pub fn should_submit(input: &str, sensitive: bool) -> bool {
    sensitive || !input.trim().is_empty()
}

/// Newest-first history with a reversible draft. Observing a manual edit or
/// Undo leaves navigation; history replacement itself preserves the cursor.
#[derive(Default)]
pub struct History {
    index: Option<usize>,
    draft: Option<String>,
    displayed: Option<String>,
}
impl History {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn observe(&mut self, input: &str) {
        if self
            .displayed
            .as_deref()
            .is_some_and(|displayed| displayed != input)
        {
            self.reset();
        }
    }
    pub fn previous(&mut self, input: &str, commands: &[String]) -> Option<String> {
        self.observe(input);
        if commands.is_empty() {
            self.reset();
            return None;
        }
        let index = self
            .index
            .map_or(0, |index| index.saturating_add(1).min(commands.len() - 1));
        self.draft.get_or_insert_with(|| input.into());
        self.index = Some(index);
        let text = commands[index].clone();
        self.displayed = Some(text.clone());
        Some(text)
    }
    pub fn next(&mut self, input: &str, commands: &[String]) -> Option<String> {
        self.observe(input);
        let index = self.index?;
        if commands.is_empty() {
            self.reset();
            return None;
        }
        if index > 0 {
            let index = (index - 1).min(commands.len() - 1);
            self.index = Some(index);
            let text = commands[index].clone();
            self.displayed = Some(text.clone());
            Some(text)
        } else {
            let text = self.draft.take().unwrap_or_default();
            self.reset();
            Some(text)
        }
    }
}

pub fn candidates(input: &str, commands: &[String], limit: usize) -> Vec<String> {
    if input.is_empty() || input.contains(['\n', '\r']) {
        return vec![];
    }
    let prefix = input.to_lowercase();
    let length = input.chars().count();
    let mut seen = HashSet::new();
    commands
        .iter()
        .map(|command| command.trim())
        .filter(|command| {
            !command.is_empty()
                && seen.insert(*command)
                && command.chars().count() > length
                && command.to_lowercase().starts_with(&prefix)
        })
        .take(limit.max(1))
        .map(str::to_owned)
        .collect()
}
#[derive(Default)]
pub struct Completion {
    input: String,
    items: Vec<String>,
    selected: Option<usize>,
    armed: bool,
    suppressed: Option<String>,
}
impl Completion {
    pub fn items(&self) -> &[String] {
        &self.items
    }
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
    pub fn armed(&self) -> bool {
        self.armed
    }
    pub fn refresh(
        &mut self,
        input: &str,
        commands: &[String],
        enabled: bool,
        sensitive: bool,
        limit: usize,
    ) {
        self.refresh_resolved(input, &[], commands, enabled, sensitive, limit);
    }
    pub fn refresh_resolved(
        &mut self,
        input: &str,
        resolved: &[String],
        commands: &[String],
        enabled: bool,
        sensitive: bool,
        limit: usize,
    ) {
        if self.input != input {
            self.input = input.into();
            self.armed = false;
            self.selected = None;
        }
        if self.suppressed.as_deref() != Some(input) {
            self.suppressed = None;
        }
        self.items = if enabled
            && !sensitive
            && !input.is_empty()
            && !input.contains(['\n', '\r'])
            && self.suppressed.is_none()
        {
            let mut seen = HashSet::new();
            resolved
                .iter()
                .map(|value| value.trim().to_owned())
                .chain(candidates(input, commands, limit))
                .filter(|value| !value.is_empty() && seen.insert(value.clone()))
                .take(limit.clamp(1, 999))
                .collect()
        } else {
            vec![]
        };
        if self.items.is_empty() {
            self.armed = false;
            self.selected = None;
        } else {
            self.selected = self.selected.map(|index| index.min(self.items.len() - 1));
        }
    }
    /// First Tab/Right activates; a second fills. Neither submits a command.
    pub fn activate(&mut self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }
        if !self.armed {
            self.armed = true;
            self.selected = Some(0);
            None
        } else {
            self.items.get(self.selected.unwrap_or(0)).cloned()
        }
    }
    pub fn step(&mut self, delta: isize) -> bool {
        if !self.armed || self.items.is_empty() {
            return false;
        }
        self.selected = Some(
            (self.selected.unwrap_or(0) as isize + delta).rem_euclid(self.items.len() as isize)
                as usize,
        );
        true
    }
    pub fn dismiss(&mut self) -> bool {
        if self.items.is_empty() && !self.armed {
            return false;
        }
        self.suppressed = Some(self.input.clone());
        self.items.clear();
        self.selected = None;
        self.armed = false;
        true
    }
}

/// Mirrors the reference's case-insensitive prompt suffixes, including optional
/// `for user/key` qualifiers. No terminal text is retained by this check.
fn sensitive_prompt(line: &str) -> bool {
    let folded = line
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let trimmed = folded.trim();
    let text = trimmed.strip_suffix(':').unwrap_or(trimmed).trim_end();
    const METHODS: [&str; 17] = [
        "password",
        "passphrase",
        "pin",
        "otp",
        "one-time",
        "one time",
        "onetime",
        "one-time password",
        "one time password",
        "onetime password",
        "one-time code",
        "one time code",
        "onetime code",
        "verification code",
        "security code",
        "auth code",
        "authentication code",
    ];
    if METHODS.iter().any(|method| text.ends_with(method)) || text.ends_with("token") {
        return true;
    }
    ["password for ", "passphrase for "].iter().any(|method| {
        text.rfind(method).is_some_and(|start| {
            let qualifier = &text[start + method.len()..];
            !qualifier.is_empty() && !qualifier.contains(':')
        })
    })
}
pub fn sensitive(echo: bool, current: &str, previous: Option<&str>) -> bool {
    !echo
        || sensitive_prompt(current)
        || (current.trim().is_empty() && previous.is_some_and(sensitive_prompt))
}
