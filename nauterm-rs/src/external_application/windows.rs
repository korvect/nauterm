//! Windows Shell association handlers. All COM objects stay on their creating
//! STA thread; only owned metadata and PNG bytes cross back to GPUI workers.
use super::windows_identity::{self, Association};
use super::*;
use ::windows::{
    Win32::{
        System::Com::{
            COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoTaskMemFree,
            CoUninitialize, IDataObject,
        },
        UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
    },
    core::{HSTRING, PCWSTR, PWSTR, w},
};
mod icons;

struct Apartment;
impl Apartment {
    fn new() -> Result<Self> {
        // SAFETY: Each caller runs on a dedicated, newly-created STA thread.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: Balanced successful initialization, on the same thread.
        unsafe {
            CoUninitialize();
        }
    }
}
fn in_apartment<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    std::thread::Builder::new()
        .name("nauterm-rs-shell-associations".into())
        .spawn(move || {
            let _apartment = Apartment::new()?;
            work()
        })?
        .join()
        .map_err(|_| anyhow::anyhow!("The Windows application lookup failed."))?
}
struct TaskString(PWSTR);
impl Drop for TaskString {
    fn drop(&mut self) {
        // SAFETY: Shell GetName/GetUIName/GetIconLocation allocate with CoTaskMem.
        unsafe {
            CoTaskMemFree(Some(self.0.0.cast()));
        }
    }
}
fn owned_string(value: PWSTR) -> Result<String> {
    let value = TaskString(value);
    ensure!(
        !value.0.is_null(),
        "The application returned an empty name."
    );
    // SAFETY: A Shell-owned NUL-terminated UTF-16 string, retained by value.
    unsafe { value.0.to_string() }.context("Invalid Unicode application metadata")
}
fn handlers(extension: &str) -> Result<Vec<IAssocHandler>> {
    filtered_handlers(extension, ASSOC_FILTER_NONE)
}
fn filtered_handlers(extension: &str, filter: ASSOC_FILTER) -> Result<Vec<IAssocHandler>> {
    let extension = HSTRING::from(extension);
    // SAFETY: COM is initialized on this thread and extension remains live.
    let enumeration = unsafe { SHAssocEnumHandlers(&extension, filter)? };
    let mut result = vec![];
    loop {
        let mut item = [None];
        let mut fetched = 0;
        unsafe {
            enumeration.Next(&mut item, Some(&mut fetched))?;
        }
        if fetched == 0 {
            return Ok(result);
        }
        if let Some(handler) = item[0].take() {
            result.push(handler);
        }
        ensure!(
            result.len() < 4096,
            "The system returned too many application handlers."
        );
    }
}
fn association_string(extension: &str, kind: ASSOCSTR) -> Option<String> {
    let extension = HSTRING::from(extension);
    let mut length = 0;
    // SAFETY: First query obtains the required UTF-16 buffer length; the second
    // call uses a live buffer of that size. No registry association is changed.
    unsafe {
        let _ = AssocQueryStringW(ASSOCF_NONE, kind, &extension, w!("open"), None, &mut length);
        if !(1..=32768).contains(&length) {
            return None;
        }
        let mut buffer = vec![0; length as usize];
        AssocQueryStringW(
            ASSOCF_NONE,
            kind,
            &extension,
            w!("open"),
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
        .ok()
        .ok()?;
        String::from_utf16(&buffer[..length.saturating_sub(1) as usize]).ok()
    }
}
fn handler_name(handler: &IAssocHandler) -> Result<String> {
    // SAFETY: handler lives in the current initialized COM apartment.
    owned_string(unsafe { handler.GetName()? })
}
fn handler_icon(handler: &IAssocHandler) -> Option<Vec<u8>> {
    let mut location = PWSTR::null();
    let mut index = 0;
    // SAFETY: The out parameters remain live throughout this COM call.
    let result = unsafe { handler.GetIconLocation(&mut location, &mut index) };
    let location = owned_string(location).ok();
    result.ok()?;
    icons::extract(&location?, index)
}
pub(super) fn list(path: &Path) -> Result<Vec<Application>> {
    let extension = format!(
        ".{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .context("Invalid file extension")?
    );
    list_association(extension, ASSOC_FILTER_NONE, true)
}

/// Settings use the OS-recommended plain-text handlers, not the general Open
/// With list for every configured extension. Icons are loaded after publishing.
pub(super) fn editor_catalog() -> Result<Vec<Editor>> {
    Ok(
        list_association(".txt".into(), ASSOC_FILTER_RECOMMENDED, false)?
            .into_iter()
            .map(Editor::from)
            .collect(),
    )
}

fn list_association(
    extension: String,
    filter: ASSOC_FILTER,
    load_icons: bool,
) -> Result<Vec<Application>> {
    in_apartment(move || {
        let default = association_string(&extension, ASSOCSTR_EXECUTABLE);
        let default_label = association_string(&extension, ASSOCSTR_FRIENDLYAPPNAME);
        let own = std::env::current_exe().ok();
        let mut applications = vec![];
        for handler in filtered_handlers(&extension, filter)? {
            let Ok(name) = handler_name(&handler) else {
                continue;
            };
            let path = PathBuf::from(&name);
            if own.as_ref().is_some_and(|own| {
                own.as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&name)
            }) {
                continue;
            }
            let Some(label) = (unsafe { handler.GetUIName() })
                .ok()
                .and_then(|label| owned_string(label).ok())
            else {
                continue;
            };
            let association = Association::new(extension.clone(), name.clone())?;
            let is_default = default
                .as_ref()
                .is_some_and(|default| default.eq_ignore_ascii_case(&name))
                || (!path.is_absolute()
                    && default_label
                        .as_ref()
                        .is_some_and(|default| default.eq_ignore_ascii_case(&label)));
            let app = Application {
                name: label,
                identifier: association.identifier(),
                path,
                is_default,
            };
            if load_icons {
                let icon = handler_icon(&handler).or_else(|| icons::file(&app.path));
                super::icons::remember(&Editor::from(app.clone()), icon);
            }
            applications.push(app);
        }
        Ok(normalized(applications, None))
    })
}
pub(super) fn from_path(path: &Path) -> Result<Application> {
    ensure!(
        path.is_absolute() && usable_executable(path),
        "Choose an installed application executable (.exe or .com)."
    );
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .context("Invalid application filename")?;
    let text = path.to_str().context("Invalid Unicode application path")?;
    Ok(Application {
        name: name.into(),
        identifier: windows_identity::executable_identifier(text),
        path: path.into(),
        is_default: false,
    })
}
fn find_handler(identity: &Association) -> Result<IAssocHandler> {
    handlers(&identity.extension)?
        .into_iter()
        .find(|handler| {
            handler_name(handler).is_ok_and(|name| name.eq_ignore_ascii_case(&identity.handler))
        })
        .context("The selected application is no longer registered. Choose it again.")
}
pub(super) fn open(
    path: &Path,
    application: Option<&Editor>,
) -> Result<async_channel::Receiver<Result<()>>> {
    if let Some(Editor::Application {
        identifier,
        path: executable,
        label,
    }) = application
        && windows_identity::is_executable(identifier, executable.to_str().unwrap_or_default())
    {
        return open_command(
            path,
            &Editor::Command {
                label: label.clone(),
                executable: executable.clone(),
                arguments: vec![],
            },
        );
    }
    let path = path.to_owned();
    let application = application.cloned();
    let (sender, receiver) = async_channel::bounded(1);
    std::thread::Builder::new()
        .name("nauterm-rs-shell-launch".into())
        .spawn(move || {
            let result = (|| {
                let _apartment = Apartment::new()?;
                let filename = HSTRING::from(path.as_os_str());
                // SAFETY: COM is initialized; every string and interface stays live
                // until invocation returns. No untrusted filename is command text.
                unsafe {
                    if let Some(Editor::Application {
                        identifier,
                        path: executable,
                        ..
                    }) = application
                    {
                        let identity = Association::parse(&identifier)?;
                        ensure!(
                            identity.matches_path(&executable),
                            "The selected application identity changed."
                        );
                        let handler = find_handler(&identity)?;
                        let item: IShellItem = SHCreateItemFromParsingName(&filename, None)?;
                        let data: IDataObject = item.BindToHandler(None, &BHID_DataObject)?;
                        handler.Invoke(&data)?;
                    } else {
                        let mut info = SHELLEXECUTEINFOW {
                            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
                            lpVerb: w!("open"),
                            lpFile: PCWSTR(filename.as_ptr()),
                            nShow: SW_SHOWNORMAL.0,
                            ..Default::default()
                        };
                        ShellExecuteExW(&mut info)?;
                    }
                }
                Ok(())
            })();
            let _ = sender.send_blocking(result);
        })?;
    Ok(receiver)
}
pub(super) fn icon(editor: &Editor) -> Option<Vec<u8>> {
    let editor = editor.clone();
    in_apartment(move || {
        Ok(match editor {
            Editor::Application {
                identifier, path, ..
            } => Association::parse(&identifier)
                .ok()
                .and_then(|identity| find_handler(&identity).ok())
                .and_then(|handler| handler_icon(&handler))
                .or_else(|| icons::file(&path)),
            Editor::Command { executable, .. } => resolve_executable(&executable)
                .ok()
                .and_then(|path| icons::file(&path)),
        })
    })
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_catalog_matches_the_system_recommended_plain_text_handlers() -> Result<()> {
        let started = std::time::Instant::now();
        // Query the full native list and inspect each recommendation separately
        // from the production enumeration filter. No application names are
        // assumed, and no application is launched or association changed.
        let expected = in_apartment(|| {
            let own = std::env::current_exe()?;
            let mut applications = vec![];
            for handler in handlers(".txt")? {
                // S_FALSE is also a successful HRESULT; only S_OK recommends.
                if unsafe { handler.IsRecommended() }.0 != 0 {
                    continue;
                }
                let name = handler_name(&handler)?;
                if own.to_string_lossy().eq_ignore_ascii_case(&name) {
                    continue;
                }
                let label = owned_string(unsafe { handler.GetUIName()? })?;
                let association = Association::new(".txt".into(), name.clone())?;
                applications.push(Application {
                    name: label,
                    identifier: association.identifier(),
                    path: PathBuf::from(name),
                    is_default: false,
                });
            }
            Ok(normalized(applications, None)
                .into_iter()
                .map(Editor::from)
                .map(|editor| editor.id())
                .collect::<std::collections::BTreeSet<_>>())
        })?;
        let actual = editor_catalog()?;
        assert_eq!(
            actual
                .iter()
                .map(Editor::id)
                .collect::<std::collections::BTreeSet<_>>(),
            expected,
        );
        eprintln!(
            "System-recommended text applications: {:?} ({:?})",
            actual.iter().map(Editor::label).collect::<Vec<_>>(),
            started.elapsed(),
        );
        Ok(())
    }

    #[test]
    fn native_catalog_keeps_packaged_and_executable_handler_identities() -> Result<()> {
        let extension = ".png";
        let registered = in_apartment(|| {
            let mut names = Vec::new();
            for handler in handlers(".png")? {
                let name = handler_name(&handler)?;
                let label = owned_string(unsafe { handler.GetUIName()? })?;
                if !label.trim().is_empty() && !label.chars().any(char::is_control) {
                    names.push(name);
                }
                let mut location = PWSTR::null();
                let mut index = 0;
                if unsafe { handler.GetIconLocation(&mut location, &mut index) }.is_ok() {
                    let location = owned_string(location)?;
                    if location.starts_with("@{") {
                        let source = HSTRING::from(&location);
                        let mut buffer = vec![0; 32768];
                        // Stale installed-app metadata need not resolve. If the
                        // OS does resolve a live resource, the catalog must be
                        // able to display it rather than falling back silently.
                        if unsafe { SHLoadIndirectString(&source, &mut buffer, None) }.is_ok() {
                            let png = icons::extract(&location, index)
                                .context("Installed application resource icon was lost")?;
                            let image =
                                image::load_from_memory_with_format(&png, image::ImageFormat::Png)?;
                            assert!(image.to_rgba8().pixels().any(|pixel| pixel[3] != 0));
                        }
                    }
                } else if !location.is_null() {
                    drop(TaskString(location));
                }
            }
            Ok(names)
        })?;
        let applications = list(Path::new("fixture.png"))?;
        let own = std::env::current_exe()?;
        for name in registered {
            if own.to_string_lossy().eq_ignore_ascii_case(&name) {
                continue;
            }
            let identifier = Association::new(extension.into(), name.clone())?.identifier();
            let app = applications
                .iter()
                .find(|app| app.identifier == identifier)
                .with_context(|| format!("Registered Shell handler was lost: {name}"))?;
            Editor::from(app.clone()).validate()?;
        }
        for app in applications {
            let identity = Association::parse(&app.identifier)?;
            assert!(identity.matches_path(&app.path));
            if let Some(png) = icon(&Editor::from(app)) {
                let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)?;
                assert!(image.to_rgba8().pixels().any(|pixel| pixel[3] != 0));
            }
        }
        Ok(())
    }

    #[test]
    fn chooser_selection_rejects_scripts_and_reports_removed_executable() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let executable = temp.path().join("编辑器 & tools.exe");
        fs::write(&executable, b"test fixture, removed before launch")?;
        let app = from_path(&executable)?;
        fs::remove_file(&executable)?;
        let result = open(
            &temp.path().join("文字 & content.txt"),
            Some(&Editor::from(app)),
        )?
        .recv_blocking()?;
        assert!(result.is_err());
        for script in ["editor.cmd", "editor.bat", "editor.ps1"] {
            let script = temp.path().join(script);
            fs::write(&script, b"test fixture")?;
            assert!(from_path(&script).is_err());
        }
        Ok(())
    }
}
