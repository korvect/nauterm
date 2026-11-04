//! Public RS-only editor preferences and argument planning. No file contents,
//! credentials, application launching or shell evaluation happen in this model.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Editor {
    Application {
        label: String,
        identifier: String,
        path: PathBuf,
    },
    Command {
        label: String,
        executable: PathBuf,
        arguments: Vec<String>,
    },
}
impl Editor {
    pub fn label(&self) -> &str {
        match self {
            Self::Application { label, .. } | Self::Command { label, .. } => label,
        }
    }
    pub fn id(&self) -> String {
        match self {
            Self::Application { identifier, .. } => format!("app:{identifier}"),
            Self::Command {
                executable,
                arguments,
                ..
            } => serde_json::to_string(&(executable, arguments))
                .expect("public command fields are JSON encodable"),
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.label().trim().is_empty()
                && self.label().len() <= 256
                && !self.label().chars().any(char::is_control),
            "Invalid editor label."
        );
        match self {
            Self::Application {
                identifier, path, ..
            } => ensure!(
                !identifier.is_empty()
                    && identifier.len()
                        <= (if identifier.starts_with("win32-assoc:") {
                            64 * 1024
                        } else {
                            512
                        })
                    && !identifier.chars().any(char::is_control)
                    && path.is_absolute()
                    && path.to_str().is_some()
                    && !path.as_os_str().to_string_lossy().contains('\0'),
                "Invalid application editor."
            ),
            Self::Command {
                executable,
                arguments,
                ..
            } => {
                let text = executable.to_str().unwrap_or_default();
                ensure!(
                    !text.is_empty()
                        && text.len() <= 16 * 1024
                        && !text.chars().any(char::is_control),
                    "Invalid editor executable."
                );
                ensure!(
                    executable.is_absolute()
                        || (!text.contains(['/', '\\']) && text != "." && text != ".."),
                    "Use an absolute editor path or an executable name."
                );
                ensure!(
                    arguments.len() <= 128
                        && arguments
                            .iter()
                            .all(|arg| arg.len() <= 16 * 1024 && !arg.contains('\0')),
                    "Invalid editor arguments."
                );
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Editors {
    pub ssh_editor: String,
    pub external_editor: Option<Editor>,
    pub text_extensions: Vec<String>,
}
impl Default for Editors {
    fn default() -> Self {
        Self { ssh_editor: "vim".into(), external_editor: None,
            text_extensions: "txt md markdown json jsonc yaml yml xml toml ini conf cfg log env properties sh bash zsh fish py rb pl php lua js jsx ts tsx dart swift m mm c h cc cpp hpp java kt kts go rs sql html css scss sass less vue svelte gradle".split_whitespace().map(str::to_owned).collect() }
    }
}
impl Editors {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.ssh_editor.len() <= 4096 && !self.ssh_editor.chars().any(char::is_control),
            "The SSH editor must be a single-line command."
        );
        ensure!(
            self.text_extensions.len() <= 512
                && parse_extensions(&self.text_extensions.join(",")) == self.text_extensions,
            "Invalid text file extensions."
        );
        if let Some(editor) = &self.external_editor {
            editor.validate()?;
        }
        Ok(())
    }
    pub fn for_name(&self, name: &str) -> Option<&Editor> {
        let name = name.rsplit(['/', '\\']).next().unwrap_or_default();
        let supported = name.rsplit_once('.').is_none_or(|(_, suffix)| {
            suffix.is_empty()
                || self
                    .text_extensions
                    .iter()
                    .any(|extension| extension.eq_ignore_ascii_case(suffix))
        });
        supported.then_some(self.external_editor.as_ref()).flatten()
    }
    /// Executed only by an explicit SSH-editor action. The configured command
    /// is user intent; the untrusted remote filename is always a single literal.
    pub fn ssh_command(&self, remote_path: &str) -> Result<String> {
        self.validate()?;
        ensure!(
            !self.ssh_editor.trim().is_empty(),
            "Configure an SSH editor in Settings first."
        );
        ensure!(
            remote_path.starts_with('/')
                && !remote_path.chars().any(char::is_control)
                && remote_path.len() <= 64 * 1024,
            "Invalid remote editor path."
        );
        Ok(format!(
            "{} '{}'",
            self.ssh_editor.trim(),
            remote_path.replace('\'', "'\"'\"'")
        ))
    }
    /// Sudo runs inside the already-bound SSH terminal, which owns its own
    /// interactive password prompt. Never forward an SFTP password into it.
    pub fn sudo_ssh_command(&self, remote_path: &str) -> Result<String> {
        Ok(format!("sudo -- {}", self.ssh_command(remote_path)?))
    }
    /// Matches the reference: detected ID/name resolves a saved selection;
    /// a missing saved editor stays available, and first detection initializes
    /// an unset choice. Results are never treated as executable shell text.
    pub fn reconcile(&mut self, mut detected: Vec<Editor>) -> Vec<Editor> {
        let mut seen = HashSet::new();
        detected.retain(|editor| {
            editor.validate().is_ok() && seen.insert(editor.label().to_lowercase())
        });
        if let Some(saved) = &self.external_editor {
            if let Some(found) = detected
                .iter()
                .find(|editor| editor.id() == saved.id())
                .or_else(|| {
                    detected
                        .iter()
                        .find(|editor| editor.label().eq_ignore_ascii_case(saved.label()))
                })
            {
                self.external_editor = Some(found.clone());
            } else {
                detected.push(saved.clone());
            }
        } else {
            self.external_editor = detected.first().cloned();
        }
        detected
    }
}
pub fn parse_extensions(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    value
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter_map(|extension| {
            let extension = extension.trim().to_ascii_lowercase();
            let extension = if extension.starts_with("*.") {
                extension[1..].trim_start_matches('.')
            } else {
                extension.trim_start_matches('.')
            };
            (!extension.is_empty()
                && extension.len() <= 128
                && extension.as_bytes()[0].is_ascii_alphanumeric()
                && extension
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"+_-".contains(&byte))
                && seen.insert(extension.to_owned()))
            .then(|| extension.to_owned())
        })
        .take(512)
        .collect()
}

/// The file is appended as one argument, never interpolated into a shell.
pub fn command_arguments(
    editor: &Editor,
    path: &Path,
) -> Result<(PathBuf, Vec<std::ffi::OsString>)> {
    editor.validate()?;
    ensure!(
        path.is_absolute() && !path.as_os_str().to_string_lossy().contains('\0'),
        "Choose an absolute editor file path."
    );
    let Editor::Command {
        executable,
        arguments,
        ..
    } = editor
    else {
        anyhow::bail!("This editor uses the system application API.");
    };
    let mut args: Vec<_> = arguments.iter().map(std::ffi::OsString::from).collect();
    args.push(path.as_os_str().to_owned());
    Ok((executable.clone(), args))
}
