//! Snippet execution semantics shared by the library and its terminal tool.
use crate::model::{AppState, Id, Snippet};
use anyhow::{Result, ensure};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolOrigin {
    pub tab: u64,
    pub session: u64,
    pub host: Option<Id>,
}
pub fn tool_draft(
    name: &str,
    script: &str,
    package: Option<Id>,
    origin: ToolOrigin,
    current: ToolOrigin,
    data: &AppState,
) -> Result<Snippet> {
    ensure!(
        origin == current,
        "Select the original terminal pane before saving this snippet."
    );
    ensure!(
        origin
            .host
            .is_none_or(|id| data.hosts.iter().any(|host| host.id == id)),
        "The original SSH host is no longer available."
    );
    draft(
        name,
        script,
        BTreeSet::new(),
        origin.host.into_iter().collect(),
        package,
        None,
    )
}

impl Snippet {
    pub fn targeted(&self) -> bool {
        self.scope == crate::model::SnippetScope::Targeted
            || !self.target_group_ids.is_empty()
            || !self.target_host_ids.is_empty()
    }
    pub fn matches_host(&self, host: Option<Id>, data: &AppState) -> bool {
        if !self.targeted() {
            return true;
        }
        let Some(host_id) = host else {
            return false;
        };
        if self.target_host_ids.contains(&host_id) {
            return true;
        }
        let mut group = data
            .hosts
            .iter()
            .find(|host| host.id == host_id)
            .and_then(|host| host.group_id);
        let mut seen = BTreeSet::new();
        while let Some(id) = group.filter(|id| seen.insert(*id)) {
            if self.target_group_ids.contains(&id) {
                return true;
            }
            group = data
                .groups
                .iter()
                .find(|group| group.id == id)
                .and_then(|group| group.parent);
        }
        false
    }
    pub fn matches_query(&self, query: &str) -> bool {
        let text = format!(
            "{} {} {} {} {} {}",
            self.name,
            self.command,
            self.description,
            if self.targeted() {
                "targeted"
            } else {
                "global"
            },
            self.target_group_ids
                .iter()
                .map(|id| format!("group:{id}"))
                .collect::<Vec<_>>()
                .join(" "),
            self.target_host_ids
                .iter()
                .map(|id| format!("host:{id}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        text.to_lowercase().contains(&query.trim().to_lowercase())
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.name.trim().is_empty(), "Description is required.");
        execution(&self.command)?;
        ensure!(
            self.target_group_ids.len() <= 2048
                && self.target_host_ids.len() <= 2048
                && self
                    .target_group_ids
                    .iter()
                    .chain(&self.target_host_ids)
                    .all(|id| *id > 0 && *id <= i64::MAX as u64),
            "Invalid or excessive snippet target IDs"
        );
        Ok(())
    }
}

pub fn command(text: &str) -> Result<&str> {
    let text = text.trim();
    ensure!(!text.is_empty(), "Script is required.");
    ensure!(!text.contains('\0'), "Script contains a NUL character.");
    Ok(text)
}

/// The actual editor's normalized draft; catalog allocation/persistence is
/// deliberately separate so invalid input cannot consume an ID.
pub fn draft(
    name: &str,
    script: &str,
    groups: BTreeSet<Id>,
    hosts: BTreeSet<Id>,
    package: Option<Id>,
    previous: Option<&Snippet>,
) -> Result<Snippet> {
    let scope = if !groups.is_empty()
        || !hosts.is_empty()
        || previous.is_some_and(|old| {
            old.scope == crate::model::SnippetScope::Targeted
                && old.target_group_ids == groups
                && old.target_host_ids == hosts
        }) {
        crate::model::SnippetScope::Targeted
    } else {
        crate::model::SnippetScope::Global
    };
    let snippet = Snippet {
        scope,
        id: previous.map_or(0, |s| s.id),
        name: if name.trim().is_empty() {
            "Untitled snippet".into()
        } else {
            name.trim().into()
        },
        command: command(script)?.to_owned(),
        description: previous.map(|s| s.description.clone()).unwrap_or_default(),
        target_group_ids: groups,
        target_host_ids: hosts,
        package_id: package,
    };
    snippet.validate()?;
    Ok(snippet)
}
/// The reference Run action sends CR-delimited input, not bracketed paste.
pub fn execution(text: &str) -> Result<String> {
    let mut normalized = command(text)?.replace("\r\n", "\n").replace('\n', "\r");
    if !normalized.ends_with('\r') {
        normalized.push('\r');
    }
    Ok(normalized)
}
/// History Run replaces the current shell line, including text after a cursor
/// moved into its middle. End-of-line followed by backward-kill-line is the
/// shell editor's operation; never erase terminal cells or submit the old draft.
/// Send the replacement and command together so output cannot split the input.
pub fn replace_prompt_execution(text: &str) -> Result<String> {
    Ok(format!("\x05\x15{}", execution(text)?))
}
/// Copy to prompt must not execute multiline commands when paste mode is off.
/// Native paste owns escape encoding when bracketed mode is on.
pub fn prompt(text: &str, keyboard_mode: u32) -> Result<(String, bool)> {
    let normalized = command(text)?.replace("\r\n", "\n").replace('\r', "\n");
    let multiline = normalized.contains('\n');
    let bracketed = multiline && keyboard_mode & (1 << 2) != 0;
    Ok((
        if multiline && !bracketed {
            normalized.replace('\n', " ")
        } else {
            normalized
        },
        bracketed,
    ))
}

/// Native mode can have changed since the rendered frame. Never send bare
/// line delimiters just because a stale snapshot said bracketed paste was on.
pub fn encoded_prompt(text: &str, native: String) -> Result<String> {
    if native.starts_with("\x1b[200~") && native.ends_with("\x1b[201~") {
        Ok(native)
    } else {
        Ok(prompt(text, 0)?.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_replaces_the_line_before_submitting_and_validates_before_clearing() {
        assert_eq!(
            replace_prompt_execution(" printf first\r\nprintf second\n ").unwrap(),
            "\x05\x15printf first\rprintf second\r"
        );
        assert!(replace_prompt_execution(" \r\n ").is_err());
        assert!(replace_prompt_execution("printf a\0b").is_err());
        // Other Run actions retain their existing append/submit semantics.
        assert_eq!(execution("ls").unwrap(), "ls\r");
    }
}
