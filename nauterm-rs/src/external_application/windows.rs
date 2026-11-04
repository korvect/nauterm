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
    let extension = HSTRING::from(extension);
    // SAFETY: COM is initialized on this thread and extension remains live.
    let enumeration = unsafe { SHAssocEnumHandlers(&extension, ASSOC_FILTER_NONE)? };
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
fn default_executable(extension: &str) -> Option<String> {
    let extension = HSTRING::from(extension);
    let mut length = 0;
    // SAFETY: First query obtains the required UTF-16 buffer length; the second
    // call uses a live buffer of that size. No registry association is changed.
    unsafe {
        let _ = AssocQueryStringW(
            ASSOCF_NONE,
            ASSOCSTR_EXECUTABLE,
            &extension,
            w!("open"),
            None,
            &mut length,
        );
        if !(1..=32768).contains(&length) {
            return None;
        }
        let mut buffer = vec![0; length as usize];
        AssocQueryStringW(
            ASSOCF_NONE,
            ASSOCSTR_EXECUTABLE,
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
    in_apartment(move || {
        let default = default_executable(&extension);
        let own = std::env::current_exe().ok();
        let mut applications = vec![];
        for handler in handlers(&extension)? {
            let Ok(name) = handler_name(&handler) else {
                continue;
            };
            let path = PathBuf::from(&name);
            if !path.is_absolute()
                || own.as_ref().is_some_and(|own| {
                    own.as_os_str()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&name)
                })
            {
                continue;
            }
            let Some(label) = (unsafe { handler.GetUIName() })
                .ok()
                .and_then(|label| owned_string(label).ok())
            else {
                continue;
            };
            let association = Association::new(extension.clone(), name.clone())?;
            let app = Application {
                name: label,
                identifier: association.identifier(),
                path,
                is_default: default
                    .as_ref()
                    .is_some_and(|default| default.eq_ignore_ascii_case(&name)),
            };
            let icon = handler_icon(&handler).or_else(|| icons::file(&app.path));
            super::icons::remember(&Editor::from(app.clone()), icon);
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
                            executable
                                .to_str()
                                .is_some_and(|path| path.eq_ignore_ascii_case(&identity.handler)),
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
