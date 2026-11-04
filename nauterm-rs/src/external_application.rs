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

/// Installed editor discovery. Windows keeps this separate from the general
/// Open With list, whose handlers can include browsers and media players.
/// No subprocess or application is launched during detection.
pub fn editor_catalog(extensions: &[String]) -> Result<Vec<Editor>> {
    #[cfg(target_os = "windows")]
    {
        let _ = extensions;
        windows::editor_catalog()
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
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

pub(crate) fn application_location_valid(identifier: &str, path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    if identifier.starts_with("win32-assoc:") {
        return windows_identity::Association::parse(identifier)
            .is_ok_and(|association| association.matches_path(path));
    }
    #[cfg(not(target_os = "windows"))]
    let _ = identifier;
    path.is_absolute()
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
                && application_location_valid(&application.identifier, &application.path)
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

#[derive(Debug)]
#[cfg_attr(not(target_os = "windows"), derive(PartialEq, Eq))]
struct Identity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(target_os = "windows")]
    volume: u32,
    #[cfg(target_os = "windows")]
    index: u64,
    // Keep the original object alive so an unlink/recreate cannot reuse its ID
    // during a native application chooser. Sharing allows edits and renames.
    #[cfg(target_os = "windows")]
    _file: fs::File,
    #[cfg(not(any(unix, target_os = "windows")))]
    created: Option<std::time::SystemTime>,
}
#[cfg(target_os = "windows")]
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.volume == other.volume && self.index == other.index
    }
}
#[cfg(target_os = "windows")]
impl Eq for Identity {}
impl Identity {
    fn read(path: &Path, metadata: &fs::Metadata) -> Result<Self> {
        #[cfg(not(target_os = "windows"))]
        let _ = path;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(target_os = "windows")]
        {
            use ::windows::Win32::{
                Foundation::HANDLE,
                Storage::FileSystem::{
                    BY_HANDLE_FILE_INFORMATION, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
                    FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
                },
            };
            use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
            let _ = metadata;
            let file = fs::OpenOptions::new()
                .access_mode(FILE_READ_ATTRIBUTES.0)
                .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0)
                .open(path)?;
            ensure!(file.metadata()?.is_file(), "Choose a regular local file.");
            let mut info = BY_HANDLE_FILE_INFORMATION::default();
            // SAFETY: The borrowed file handle and output live throughout this
            // call. The retained File owns and eventually closes the handle.
            unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info)? };
            Ok(Self {
                volume: info.dwVolumeSerialNumber,
                index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
                _file: file,
            })
        }
        #[cfg(not(any(unix, target_os = "windows")))]
        {
            Ok(Self {
                created: metadata.created().ok(),
            })
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
        let identity = Identity::read(&resolved, &metadata)?;
        Ok(Self {
            path,
            resolved,
            identity,
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
                && Identity::read(&resolved, &metadata)? == self.identity,
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

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "Subprocess argv fixture used by the Windows literal filename test"]
    fn windows_literal_filename_fixture() -> Result<()> {
        let arguments: Vec<_> = std::env::args_os().collect();
        let Some(separator) = arguments.iter().position(|argument| argument == "--") else {
            return Ok(());
        };
        ensure!(
            arguments.len() == separator + 3,
            "Invalid fixture arguments"
        );
        let file = arguments.last().context("Missing fixture filename")?;
        let capture = &arguments[arguments.len() - 2];
        fs::write(capture, serde_json::to_vec(file)?)?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_configured_command_passes_unicode_and_metacharacters_literally() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let capture = temp.path().join("captured argv.json");
        let file = temp.path().join("中文 & %PATH% $(not-run) file.txt");
        fs::write(&file, b"synthetic content")?;
        let editor = Editor::Command {
            label: "Synthetic argv recorder".into(),
            executable: std::env::current_exe()?,
            arguments: vec![
                "--exact".into(),
                "external_application::tests::windows_literal_filename_fixture".into(),
                "--ignored".into(),
                "--nocapture".into(),
                "--".into(),
                capture.to_str().unwrap().into(),
            ],
        };
        open_editor(&file, Some(&editor))?.recv_blocking()??;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if fs::read(&capture).is_ok_and(|bytes| {
                serde_json::from_slice::<std::ffi::OsString>(&bytes)
                    .is_ok_and(|received| received == file.as_os_str())
            }) {
                break;
            }
            ensure!(
                std::time::Instant::now() < deadline,
                "The Windows process did not receive one literal filename."
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    }

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
