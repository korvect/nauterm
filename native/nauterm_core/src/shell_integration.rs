//! Shared local shell resources and the reference's managed startup blocks.
use std::{
    io,
    path::{Path, PathBuf},
};

const START: &str = "# >>> Nauterm shell integration >>>";
const END: &str = "# <<< Nauterm shell integration <<<";
const FISH_HEADER: &str = "# Managed by Nauterm. Use Settings to uninstall.";

pub fn write_resources(data_directory: &Path) -> io::Result<PathBuf> {
    let root = data_directory.join("shell-integration");
    write_if_changed(&root.join("nauterm.zsh"), crate::pty::ZSH_INTEGRATION)?;
    write_if_changed(&root.join("nauterm.bash"), crate::pty::BASH_INTEGRATION)?;
    write_if_changed(&root.join("nauterm.fish"), crate::pty::FISH_INTEGRATION)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in ["nauterm.zsh", "nauterm.bash", "nauterm.fish"] {
            std::fs::set_permissions(root.join(name), std::fs::Permissions::from_mode(0o644))?;
        }
    }
    Ok(root)
}

/// Explicit paths keep tests and isolated clients away from real user profiles.
/// Existing startup commands survive; only Nauterm's marked block is replaced.
pub fn synchronize(data_directory: &Path, home: &Path) -> io::Result<()> {
    if cfg!(windows) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Persistent shell integration is only available on Unix platforms",
        ));
    }
    let resources = write_resources(data_directory)?;
    for (profile, script) in [(".zshrc", "nauterm.zsh"), (".bashrc", "nauterm.bash")] {
        let path = home.join(profile);
        let original = read_optional(&path)?;
        let path_text = utf8(&resources.join(script))?.replace('\'', "'\\''");
        let block = format!("{START}\nif [ -n \"${{NAUTERM_SESSION:-}}\" ] && [ -z \"${{NAUTERM_SHELL_INTEGRATION_INJECT+x}}\" ]; then\n  . '{path_text}'\nfi\n{END}");
        let rest = without_block(&original);
        let rest = rest.trim_end();
        let next = if rest.is_empty() {
            format!("{block}\n")
        } else {
            format!("{rest}\n\n{block}\n")
        };
        write_if_changed(&path, &next)?;
    }
    let fish = home.join(".config/fish/conf.d/nauterm.fish");
    let original = read_optional(&fish)?;
    if !original.is_empty() && !original.starts_with(FISH_HEADER) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} is not managed by Nauterm", fish.display()),
        ));
    }
    let path = utf8(&resources.join("nauterm.fish"))?
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$");
    write_if_changed(
        &fish,
        &format!("{FISH_HEADER}\nif set -q NAUTERM_SESSION\n    source \"{path}\"\nend\n"),
    )
}

fn utf8(path: &Path) -> io::Result<&str> {
    path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Shell integration paths must be UTF-8",
        )
    })
}
fn read_optional(path: &Path) -> io::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}
fn without_block(text: &str) -> String {
    let Some(start) = text.find(START) else {
        return text.into();
    };
    let Some(end) = text[start + START.len()..].find(END) else {
        return text.into();
    };
    let mut end = start + START.len() + end + END.len();
    if text.as_bytes().get(end) == Some(&b'\r') {
        end += 1;
    }
    if text.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
    }
    format!("{}{}", &text[..start], &text[end..])
}
fn write_if_changed(path: &Path, text: &str) -> io::Result<()> {
    if read_optional(path)? == text {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn managed_profiles_preserve_user_commands_and_repeated_sync_is_stable() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let home = root.path().join("home");
        let data = root.path().join("Nauterm's $data 工作区");
        std::fs::create_dir_all(&home)?;
        std::fs::write(home.join(".zshrc"), "export BEFORE=1\n")?;
        std::fs::write(
            home.join(".bashrc"),
            format!("alias first=true\n{START}\nold block\n{END}\r\nalias last=true\n"),
        )?;
        synchronize(&data, &home)?;
        let zsh = std::fs::read_to_string(home.join(".zshrc"))?;
        let bash = std::fs::read_to_string(home.join(".bashrc"))?;
        let fish_path = home.join(".config/fish/conf.d/nauterm.fish");
        let fish = std::fs::read_to_string(&fish_path)?;
        assert!(zsh.starts_with("export BEFORE=1\n"));
        assert!(zsh.contains("Nauterm'\\''s $data 工作区"));
        assert!(zsh.contains("NAUTERM_SHELL_INTEGRATION_INJECT+x"));
        assert!(bash.starts_with("alias first=true\nalias last=true\n"));
        assert!(!bash.contains("old block"));
        assert_eq!(bash.matches(START).count(), 1);
        assert!(fish.contains("\\$data 工作区"));
        synchronize(&data, &home)?;
        assert_eq!(std::fs::read_to_string(home.join(".zshrc"))?, zsh);
        assert_eq!(std::fs::read_to_string(home.join(".bashrc"))?, bash);
        assert_eq!(std::fs::read_to_string(fish_path)?, fish);
        assert_eq!(
            std::fs::read_to_string(data.join("shell-integration/nauterm.zsh"))?,
            crate::pty::ZSH_INTEGRATION
        );
        Ok(())
    }

    #[test]
    fn an_incomplete_block_or_unmanaged_fish_file_is_preserved() -> io::Result<()> {
        let original = format!("user command\n{START}\nunfinished\n");
        assert_eq!(without_block(&original), original);
        let root = tempfile::tempdir()?;
        let home = root.path().join("home");
        let fish = home.join(".config/fish/conf.d/nauterm.fish");
        std::fs::create_dir_all(fish.parent().unwrap())?;
        std::fs::write(&fish, "user-owned fish configuration\n")?;
        assert_eq!(
            synchronize(&root.path().join("data"), &home)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            std::fs::read_to_string(fish)?,
            "user-owned fish configuration\n"
        );
        Ok(())
    }
}
