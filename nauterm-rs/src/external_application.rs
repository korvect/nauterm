//! System file associations and configured editor launchers. Never reads the
//! original Nauterm application's preferences or user data.
use crate::editor_config::Editor;
use anyhow::{Context, Result, ensure};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::choose;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod chooser;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub use chooser::choose;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(any(target_os = "windows", test))]
mod windows_identity;

mod icons;
pub use icons::{cached as cached_icon, preload as preload_icon};

pub const CAN_CHOOSE: bool = cfg!(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "windows"
));

impl From<Application> for Editor {
    fn from(application: Application) -> Self {
        Self::Application {
            label: application.name,
            identifier: application.identifier,
            path: application.path,
        }
    }
}

/// The same association discovery as Open With, deduplicated by the settings
/// model. No subprocess or application is launched during detection.
pub fn editor_catalog(extensions: &[String]) -> Result<Vec<Editor>> {
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let mut applications = vec![];
        for extension in extensions {
            applications.extend(list_for_name(&format!("file.{extension}"))?);
        }
        Ok(normalized(applications, None)
            .into_iter()
            .map(Editor::from)
            .collect())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = extensions;
        let mut names = vec![
            ("Cursor", "cursor"),
            ("VS Code", "code"),
            ("VS Code Insiders", "code-insiders"),
            ("Zed", "zed"),
            ("Sublime Text", "subl"),
        ];
        #[cfg(target_os = "windows")]
        names.push(("Notepad", "notepad.exe"));
        Ok(names
            .into_iter()
            .filter_map(|(label, executable)| {
                resolve_executable(Path::new(executable))
                    .ok()
                    .map(|executable| Editor::Command {
                        label: label.into(),
                        executable,
                        arguments: vec![],
                    })
            })
            .collect())
    }
}

fn usable_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        path.extension().is_some_and(|suffix| {
            suffix.eq_ignore_ascii_case("exe") || suffix.eq_ignore_ascii_case("com")
        })
    }
}
fn resolve_executable(executable: &Path) -> Result<PathBuf> {
    if executable.is_absolute() {
        ensure!(
            usable_executable(executable),
            "The configured editor is not executable or is no longer installed."
        );
        return Ok(executable.to_owned());
    }
    let directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| {
            std::env::split_paths(&value)
                .filter(|path| path.is_absolute())
                .collect()
        })
        .unwrap_or_default();
    #[cfg(unix)]
    let directories = {
        let mut directories = directories;
        directories.extend(
            [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/opt/local/bin",
                "/usr/bin",
                "/bin",
            ]
            .map(PathBuf::from),
        );
        if let Some(user_dirs) = directories::UserDirs::new() {
            directories.push(user_dirs.home_dir().join(".local/bin"));
        }
        directories
    };
    for directory in directories {
        let candidate = directory.join(executable);
        if usable_executable(&candidate) {
            return Ok(candidate);
        }
        #[cfg(target_os = "windows")]
        if executable.extension().is_none() {
            for suffix in ["exe", "com"] {
                let candidate = candidate.with_extension(suffix);
                if usable_executable(&candidate) {
                    return Ok(candidate);
                }
            }
        }
    }
    anyhow::bail!("The configured editor executable could not be found.")
}

fn open_command(path: &Path, editor: &Editor) -> Result<async_channel::Receiver<Result<()>>> {
    let (executable, arguments) = crate::editor_config::command_arguments(editor, path)?;
    let (sender, receiver) = async_channel::bounded(1);
    std::thread::Builder::new()
        .name("nauterm-rs-editor-launch".into())
        .spawn(move || {
            let result = (|| {
                let executable = resolve_executable(&executable)?;
                // Windows command scripts implicitly invoke cmd.exe and cannot
                // safely receive an arbitrary filename as a literal argument.
                #[cfg(target_os = "windows")]
                ensure!(
                    !executable
                        .extension()
                        .is_some_and(|suffix| suffix.eq_ignore_ascii_case("bat")
                            || suffix.eq_ignore_ascii_case("cmd")),
                    "Choose an editor executable, not a command script."
                );
                let mut command = std::process::Command::new(executable);
                command
                    .args(arguments)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    command.process_group(0);
                }
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x00000008 | 0x00000200);
                }
                command
                    .spawn()
                    .context("Unable to start the configured editor.")
            })();
            match result {
                Ok(mut child) => {
                    let _ = sender.send_blocking(Ok(()));
                    let _ = child.wait();
                }
                Err(error) => {
                    let _ = sender.send_blocking(Err(error));
                }
            }
        })?;
    Ok(receiver)
}

pub fn open_editor(
    path: &Path,
    editor: Option<&Editor>,
) -> Result<async_channel::Receiver<Result<()>>> {
    if let Some(editor) = editor {
        editor.validate()?;
    }
    match editor {
        Some(Editor::Command { .. }) => open_command(path, editor.unwrap()),
        #[cfg(target_os = "macos")]
        Some(Editor::Application {
            label,
            identifier,
            path: application,
        }) => macos::open(
            path,
            Some(&Application {
                name: label.clone(),
                identifier: identifier.clone(),
                path: application.clone(),
                is_default: false,
            }),
        ),
        #[cfg(target_os = "linux")]
        Some(Editor::Application {
            identifier,
            path: application,
            ..
        }) => {
            let (sender, receiver) = async_channel::bounded(1);
            let path = path.to_owned();
            let application = application.clone();
            let identifier = identifier.clone();
            std::thread::Builder::new()
                .name("nauterm-rs-application-launch".into())
                .spawn(move || {
                    let _ = sender.send_blocking(linux::launch(&path, &application, &identifier));
                })?;
            Ok(receiver)
        }
        #[cfg(target_os = "windows")]
        Some(Editor::Application { .. }) => windows::open(path, editor),
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        Some(Editor::Application { .. }) => anyhow::bail!(
            "This saved application editor is only available on macOS. Select an installed editor in Settings."
        ),
        None => {
            #[cfg(target_os = "windows")]
            {
                windows::open(path, None)
            }
            #[cfg(target_os = "macos")]
            {
                macos::open(path, None)
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                open_command(
                    path,
                    &Editor::Command {
                        label: "System default".into(),
                        executable: if cfg!(target_os = "windows") {
                            "explorer.exe".into()
                        } else {
                            "xdg-open".into()
                        },
                        arguments: vec![],
                    },
                )
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Application {
    pub name: String,
    pub identifier: String,
    pub path: PathBuf,
    pub is_default: bool,
}

fn normalized(applications: Vec<Application>, own_identifier: Option<&str>) -> Vec<Application> {
    let mut seen = HashSet::new();
    let mut applications: Vec<_> = applications
        .into_iter()
        .filter(|application| {
            !application.name.trim().is_empty()
                && !application.name.chars().any(char::is_control)
                && !application.identifier.trim().is_empty()
                && !application.identifier.chars().any(char::is_control)
                && application.path.is_absolute()
                && Some(application.identifier.as_str()) != own_identifier
                && seen.insert(application.identifier.clone())
        })
        .collect();
    // Reference Dart's decoded application list is alphabetic, not default-first.
    applications.sort_by_key(|application| application.name.to_lowercase());
    applications
}

pub fn list(path: &Path) -> Result<Vec<Application>> {
    if path
        .extension()
        .is_none_or(|extension| extension.is_empty())
    {
        return Ok(vec![]);
    }
    #[cfg(target_os = "macos")]
    let applications = macos::list(path)?;
    #[cfg(target_os = "linux")]
    let applications = linux::list(path)?;
    #[cfg(target_os = "windows")]
    let applications = windows::list(path)?;
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let applications = normalized(vec![], None);
    Ok(applications)
}

/// Call from a worker after publishing association metadata to the view.
pub fn preload_application_icons(applications: &[Application]) {
    for application in applications {
        preload_icon(&Editor::from(application.clone()));
    }
}

pub fn list_for_name(name: &str) -> Result<Vec<Application>> {
    ensure!(
        !name.contains(['/', '\\', '\0'])
            && Path::new(name).file_name().is_some()
            && Path::new(name).components().count() == 1,
        "Invalid application lookup filename."
    );
    list(Path::new(name))
}

#[derive(Debug, PartialEq, Eq)]
struct Identity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(not(unix))]
    created: Option<std::time::SystemTime>,
}
impl Identity {
    fn read(metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                created: metadata.created().ok(),
            }
        }
    }
}

/// A chooser must not silently open a replacement of the file it was shown for.
/// No content is read and no implicit association is used for explicit editors.
pub struct FileTarget {
    pub path: PathBuf,
    resolved: PathBuf,
    identity: Identity,
    explicit: bool,
}
impl FileTarget {
    pub fn prepare(path: PathBuf, explicit: bool) -> Result<Self> {
        ensure!(path.is_absolute(), "Choose an absolute local file path.");
        let resolved = path
            .canonicalize()
            .context("The local file no longer exists.")?;
        let metadata = fs::metadata(&resolved)?;
        ensure!(metadata.is_file(), "Choose a regular local file.");
        ensure!(
            explicit || crate::file_model::can_open_local(&path)?,
            "This file cannot be opened through an implicit file association. Choose an explicit application instead."
        );
        Ok(Self {
            path,
            resolved,
            identity: Identity::read(&metadata),
            explicit,
        })
    }
    pub fn revalidate(&self) -> Result<()> {
        let resolved = self
            .path
            .canonicalize()
            .context("The local file no longer exists.")?;
        let metadata = fs::metadata(&resolved)?;
        ensure!(
            resolved == self.resolved
                && metadata.is_file()
                && Identity::read(&metadata) == self.identity,
            "The selected file changed while choosing an application. Select it again."
        );
        ensure!(
            self.explicit || crate::file_model::can_open_local(&self.path)?,
            "The file is no longer safe to open through its implicit association."
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn configured_command_launch_passes_the_filename_literally_to_a_synthetic_process() -> Result<()>
    {
        use std::{
            os::unix::fs::PermissionsExt,
            time::{Duration, Instant},
        };
        let temp = tempfile::tempdir()?;
        let executable = temp.path().join("synthetic editor");
        // A test-only argv recorder, not an installed editor or actual user file.
        fs::write(
            &executable,
            b"#!/bin/sh\ntask_capture=$2\nshift 2\nprintf '%s\\0' \"$@\" > \"$task_capture\"\n",
        )?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
        let capture = temp.path().join("argv.bin");
        let file = temp.path().join("-中文 '$(not-executed); file.txt");
        fs::write(&file, b"synthetic content")?;
        let editor = Editor::Command {
            label: "Synthetic argv recorder".into(),
            executable,
            arguments: vec!["--wait".into(), capture.to_str().unwrap().into()],
        };
        let target = FileTarget::prepare(file.clone(), true)?;
        target.revalidate()?;
        let launched = open_editor(&file, Some(&editor))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(result) = launched.try_recv() {
                result?;
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "Synthetic launcher did not report startup."
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut expected = file.as_os_str().as_encoded_bytes().to_vec();
        expected.push(0);
        loop {
            if fs::read(&capture).is_ok_and(|bytes| bytes == expected) {
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "Synthetic process did not receive one literal filename."
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    #[test]
    fn missing_configured_executable_returns_a_launch_error_without_fallback() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let editor = Editor::Command {
            label: "Missing synthetic editor".into(),
            executable: temp.path().join("does-not-exist"),
            arguments: vec![],
        };
        let launched = open_editor(&temp.path().join("synthetic.txt"), Some(&editor))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Ok(result) = launched.try_recv() {
                assert!(result.is_err());
                break;
            }
            ensure!(
                std::time::Instant::now() < deadline,
                "Missing editor did not fail."
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    }

    #[test]
    fn associations_are_sorted_deduplicated_and_exclude_our_own_application() {
        let temporary = tempfile::tempdir().unwrap();
        let app = |name: &str, id: &str| Application {
            name: name.into(),
            identifier: id.into(),
            path: temporary.path().join("Fixture.app"),
            is_default: false,
        };
        let values = normalized(
            vec![
                app("Zulu", "z"),
                app("Alpha", "a"),
                app("duplicate", "a"),
                app("Self", "self"),
                app("bad\nlabel", "bad"),
                app("", "empty"),
            ],
            Some("self"),
        );
        assert_eq!(
            values
                .iter()
                .map(|value| value.name.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "Zulu"]
        );
    }
    #[test]
    fn explicit_application_is_required_for_scripts_and_executable_files() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let script = temp.path().join("-file with spaces;$(not-executed).sh");
        fs::write(&script, b"synthetic")?;
        assert!(FileTarget::prepare(script.clone(), false).is_err());
        FileTarget::prepare(script, true)?.revalidate()?;
        assert!(FileTarget::prepare(temp.path().into(), true).is_err());
        assert!(FileTarget::prepare(PathBuf::from("relative.txt"), true).is_err());
        let text = temp.path().join("中文.txt");
        fs::write(&text, b"synthetic")?;
        FileTarget::prepare(text.clone(), false)?.revalidate()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let target = FileTarget::prepare(text.clone(), false)?;
            fs::set_permissions(&text, fs::Permissions::from_mode(0o700))?;
            assert!(target.revalidate().is_err());
            FileTarget::prepare(text, true)?.revalidate()?;
        }
        Ok(())
    }
    #[test]
    fn replacing_removing_or_retargeting_a_selected_file_is_rejected() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("file.txt");
        fs::write(&path, b"first")?;
        let target = FileTarget::prepare(path.clone(), false)?;
        fs::rename(&path, temp.path().join("previous.txt"))?;
        fs::write(&path, b"second")?;
        assert!(target.revalidate().is_err());
        let target = FileTarget::prepare(path.clone(), false)?;
        fs::remove_file(&path)?;
        assert!(target.revalidate().is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(temp.path().join("previous.txt"), &path)?;
            let target = FileTarget::prepare(path.clone(), false)?;
            fs::remove_file(&path)?;
            fs::write(temp.path().join("other.txt"), b"other")?;
            symlink(temp.path().join("other.txt"), &path)?;
            assert!(target.revalidate().is_err());
        }
        Ok(())
    }
}
