//! System-shell discovery without launching a process or interpolating commands.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shell {
    pub path: String,
    pub label: String,
}
/// Resolve a configured local directory without shell interpolation or I/O.
/// Existence and executable checks remain in the native local-open path.
pub fn expand_directory(directory: Option<&str>) -> anyhow::Result<Option<String>> {
    validate_directory(directory)?;
    let Some(directory) = directory.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    anyhow::ensure!(
        crate::recovery::safe_directory(directory),
        "Work dir contains invalid control characters."
    );
    if directory == "~" || directory.starts_with("~/") {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .ok_or_else(|| anyhow::anyhow!("The home directory is unavailable."))?;
        let path = PathBuf::from(home).join(directory.strip_prefix("~/").unwrap_or_default());
        anyhow::ensure!(path.is_absolute(), "The home directory must be absolute.");
        return Ok(Some(
            path.to_str()
                .ok_or_else(|| anyhow::anyhow!("Work dir is not UTF-8."))?
                .into(),
        ));
    }
    anyhow::ensure!(
        Path::new(directory).is_absolute(),
        "Work dir must be an absolute path or start with ~/."
    );
    Ok(Some(directory.into()))
}
pub fn validate_directory(directory: Option<&str>) -> anyhow::Result<()> {
    if let Some(directory) = directory.map(str::trim).filter(|value| !value.is_empty()) {
        anyhow::ensure!(
            crate::recovery::safe_directory(directory),
            "Work dir contains invalid control characters."
        );
        anyhow::ensure!(
            directory == "~" || directory.starts_with("~/") || Path::new(directory).is_absolute(),
            "Work dir must be an absolute path or start with ~/."
        );
    }
    Ok(())
}

fn git_bash(path: &str) -> bool {
    let path = path.to_lowercase().replace('\\', "/");
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() < 3 || parts.last() != Some(&"bash.exe") || parts[parts.len() - 2] != "bin" {
        return false;
    }
    let root = if parts[parts.len() - 3] == "usr" {
        parts.len().checked_sub(4)
    } else {
        parts.len().checked_sub(3)
    };
    root.is_some_and(|i| parts[i] == "git" || parts[i].starts_with("portablegit"))
}
pub fn label(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.to_ascii_lowercase().as_str() {
        "pwsh" | "pwsh.exe" => "PowerShell".into(),
        "powershell" | "powershell.exe" => "Windows PowerShell".into(),
        "cmd" | "cmd.exe" => "Command Prompt".into(),
        "bash.exe" if git_bash(path) => "Git Bash".into(),
        _ => name.into(),
    }
}
pub fn available(path: &str) -> bool {
    if !Path::new(path).is_absolute() || path.as_bytes().contains(&0) {
        return false;
    }
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return false;
        }
    }
    true
}
fn normalize(
    candidates: Vec<String>,
    default: Option<&str>,
    windows: bool,
    valid: impl Fn(&str) -> bool,
) -> Vec<Shell> {
    let mut paths = BTreeSet::new();
    let mut by_label = BTreeMap::new();
    for path in candidates {
        let path = path.trim();
        let key = if windows {
            path.to_lowercase().replace('/', "\\")
        } else {
            path.into()
        };
        if path.is_empty() || !valid(path) || !paths.insert(key) {
            continue;
        }
        by_label
            .entry(label(path))
            .or_insert_with(|| path.to_owned());
    }
    let priority = |path: &str| match path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase()
        .as_str()
    {
        "pwsh.exe" => 0,
        "powershell.exe" => 1,
        "cmd.exe" => 2,
        "bash.exe" => 3,
        _ => 100,
    };
    let mut shells: Vec<_> = by_label
        .into_iter()
        .map(|(label, path)| Shell { path, label })
        .collect();
    shells.sort_by_key(|shell| {
        (
            if windows { priority(&shell.path) } else { 100 },
            shell.label.clone(),
        )
    });
    let first = default
        .and_then(|default| {
            shells.iter().position(|shell| {
                if windows {
                    shell.path.eq_ignore_ascii_case(default)
                } else {
                    shell.path == default
                }
            })
        })
        .unwrap_or(0);
    if first < shells.len() {
        let shell = shells.remove(first);
        shells.insert(0, shell);
    }
    shells
}
pub fn discover() -> Vec<Shell> {
    let mut candidates = vec![];
    let default;
    if cfg!(windows) {
        for (variable, suffix) in [
            ("ProgramFiles", "PowerShell/7/pwsh.exe"),
            (
                "SystemRoot",
                "System32/WindowsPowerShell/v1.0/powershell.exe",
            ),
            ("SystemRoot", "System32/cmd.exe"),
            ("ProgramFiles", "Git/bin/bash.exe"),
            ("ProgramFiles(x86)", "Git/bin/bash.exe"),
            ("LOCALAPPDATA", "Programs/Git/bin/bash.exe"),
        ] {
            if let Some(root) = std::env::var_os(variable)
                && let Some(path) = PathBuf::from(root).join(suffix).to_str()
            {
                candidates.push(path.to_owned());
            }
        }
        if let Ok(path) = std::env::var("COMSPEC") {
            candidates.push(path);
        }
        if let Some(path) = std::env::var_os("PATH") {
            for directory in std::env::split_paths(&path) {
                for name in ["pwsh.exe", "powershell.exe", "cmd.exe", "bash.exe"] {
                    if let Some(path) = directory.join(name).to_str()
                        && (name != "bash.exe" || git_bash(path))
                    {
                        candidates.push(path.to_owned());
                    }
                }
            }
        }
        default = None;
    } else {
        default = std::env::var("SHELL")
            .ok()
            .filter(|shell| !shell.trim().is_empty());
        candidates.extend(default.clone());
        if let Ok(content) = std::fs::read_to_string("/etc/shells") {
            candidates.extend(
                content
                    .lines()
                    .map(str::trim)
                    .filter(|line| line.starts_with('/'))
                    .map(str::to_owned),
            );
        }
    }
    normalize(candidates, default.as_deref(), cfg!(windows), available)
}

#[cfg(test)]
mod tests {
    use super::{label, normalize};
    #[test]
    fn configured_directory_is_literal_and_validation_does_not_require_its_existence()
    -> anyhow::Result<()> {
        assert_eq!(super::expand_directory(None)?, None);
        assert_eq!(super::expand_directory(Some("  "))?, None);
        let directory = std::env::temp_dir()
            .join("literal $(not-run); directory")
            .to_string_lossy()
            .into_owned();
        assert_eq!(super::expand_directory(Some(&directory))?, Some(directory));
        assert!(super::validate_directory(Some("~/work dir")).is_ok());
        assert!(super::validate_directory(Some("~other/work")).is_err());
        assert!(super::expand_directory(Some("relative/path")).is_err());
        assert!(super::expand_directory(Some("/tmp/invalid\npath")).is_err());
        Ok(())
    }
    #[test]
    fn catalog_deduplicates_labels_and_puts_the_environment_shell_first() {
        let shells = normalize(
            vec![
                "/bin/bash",
                "/bin/zsh",
                "/usr/local/bin/bash",
                "/missing",
                "/bin/sh",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            Some("/bin/zsh"),
            false,
            |path| path != "/missing",
        );
        assert_eq!(
            shells
                .iter()
                .map(|shell| shell.label.as_str())
                .collect::<Vec<_>>(),
            ["zsh", "bash", "sh"]
        );
        assert_eq!(shells[1].path, "/bin/bash");
    }
    #[test]
    fn windows_names_and_priorities_match_the_reference() {
        assert_eq!(label("C:\\Program Files\\Git\\bin\\bash.exe"), "Git Bash");
        assert_eq!(label("C:/tools/PortableGit/usr/bin/bash.exe"), "Git Bash");
        assert_eq!(label("C:/other/bin/bash.exe"), "bash.exe");
        let shells = normalize(
            vec![
                "C:/cmd.exe",
                "C:/powershell.exe",
                "C:/pwsh.exe",
                "c:/PWSH.EXE",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            None,
            true,
            |_| true,
        );
        assert_eq!(
            shells
                .iter()
                .map(|shell| shell.label.as_str())
                .collect::<Vec<_>>(),
            ["PowerShell", "Windows PowerShell", "Command Prompt"]
        );
    }
}
