use gpui_kit::BackgroundExecutor;

#[cfg(any(target_os = "linux", target_os = "windows", test))]
pub(crate) mod dispatch;

#[derive(Clone, Copy)]
pub(crate) enum Scheme {
    Ssh,
    Sftp,
    Telnet,
}

impl Scheme {
    pub(crate) const ALL: [Self; 3] = [Self::Ssh, Self::Sftp, Self::Telnet];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Sftp => "sftp",
            Self::Telnet => "telnet",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Ssh => "ssh://",
            Self::Sftp => "sftp://",
            Self::Telnet => "telnet://",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum SetError {
    #[cfg(not(target_os = "windows"))]
    NotRegistered,
    Failed,
}

pub(crate) async fn query(_executor: &BackgroundExecutor) -> [Option<bool>; 3] {
    #[cfg(target_os = "macos")]
    {
        Scheme::ALL.map(platform::is_default)
    }
    #[cfg(not(target_os = "macos"))]
    _executor
        .spawn(async { Scheme::ALL.map(platform::is_default) })
        .await
}

pub(crate) async fn set(scheme: Scheme, _executor: &BackgroundExecutor) -> Result<(), SetError> {
    #[cfg(target_os = "macos")]
    {
        platform::set(scheme).await
    }
    #[cfg(target_os = "windows")]
    {
        platform::set(scheme)
    }
    #[cfg(target_os = "linux")]
    {
        _executor.spawn(async move { platform::set(scheme) }).await
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSBundle, NSError, NSString, NSURL};
    use std::path::PathBuf;

    fn bundle() -> Option<objc2::rc::Retained<NSURL>> {
        let bundle = NSBundle::mainBundle();
        (bundle.bundleIdentifier()?.to_string() == "com.korvect.nauterm.rs")
            .then(|| bundle.bundleURL())
    }

    fn path(url: &NSURL) -> Option<PathBuf> {
        let url = url.URLByStandardizingPath()?;
        let url = url.URLByResolvingSymlinksInPath()?;
        Some(PathBuf::from(url.path()?.to_string()))
    }

    pub(super) fn is_default(scheme: Scheme) -> Option<bool> {
        let Some(current) = bundle().and_then(|url| path(&url)) else {
            return Some(false);
        };
        let probe = NSURL::URLWithString(&NSString::from_str(&format!(
            "{}example.com",
            scheme.label()
        )))?;
        Some(
            NSWorkspace::sharedWorkspace()
                .URLForApplicationToOpenURL(&probe)
                .and_then(|url| path(&url))
                .is_some_and(|other| other == current),
        )
    }

    pub(crate) async fn set(scheme: Scheme) -> Result<(), SetError> {
        let bundle = bundle().ok_or(SetError::NotRegistered)?;
        let (send, receive) = async_channel::bounded(1);
        let handler = block2::RcBlock::new(move |error: *mut NSError| {
            let _ = send.try_send(error.is_null());
        });
        NSWorkspace::sharedWorkspace()
            .setDefaultApplicationAtURL_toOpenURLsWithScheme_completionHandler(
                &bundle,
                &NSString::from_str(scheme.name()),
                Some(&handler),
            );
        if receive.recv().await == Ok(true) && is_default(scheme) == Some(true) {
            Ok(())
        } else {
            Err(SetError::Failed)
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use gio::{AppInfo, glib, prelude::*};
    use std::path::{Path, PathBuf};

    fn installed_paths() -> std::io::Result<Vec<PathBuf>> {
        let mut paths = vec![std::env::current_exe()?.canonicalize()?];
        if let Some(path) = std::env::var_os("APPIMAGE")
            && let Ok(path) = PathBuf::from(path).canonicalize()
        {
            paths.push(path);
        }
        Ok(paths)
    }

    fn matches(app: &AppInfo, paths: &[PathBuf]) -> bool {
        let executable = app.executable();
        let path = if executable.is_absolute() {
            executable
        } else if executable.components().count() == 1 {
            match glib::find_program_in_path(executable) {
                Some(path) => path,
                None => return false,
            }
        } else {
            return false;
        };
        Path::canonicalize(&path).is_ok_and(|path| paths.contains(&path))
    }

    pub(super) fn is_default(scheme: Scheme) -> Option<bool> {
        let paths = installed_paths().ok()?;
        Some(
            AppInfo::default_for_uri_scheme(scheme.name()).is_some_and(|app| matches(&app, &paths)),
        )
    }

    pub(super) fn set(scheme: Scheme) -> Result<(), SetError> {
        let paths = installed_paths().map_err(|_| SetError::Failed)?;
        let ty = format!("x-scheme-handler/{}", scheme.name());
        let app = AppInfo::all_for_type(&ty)
            .into_iter()
            .find(|app| matches(app, &paths))
            .ok_or(SetError::NotRegistered)?;
        app.set_as_default_for_type(&ty)
            .map_err(|_| SetError::Failed)?;
        (is_default(scheme) == Some(true))
            .then_some(())
            .ok_or(SetError::Failed)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows::{
        Win32::{
            Foundation::{ERROR_NO_ASSOCIATION, S_FALSE},
            UI::{
                Shell::{
                    ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE, AssocQueryStringW, ShellExecuteW,
                },
                WindowsAndMessaging::SW_SHOWNORMAL,
            },
        },
        core::{PCWSTR, PWSTR, w},
    };

    pub(super) fn is_default(scheme: Scheme) -> Option<bool> {
        let name: Vec<u16> = scheme.name().encode_utf16().chain([0]).collect();
        let name = PCWSTR(name.as_ptr());
        let mut len = 0;
        // SAFETY: The scheme is NUL terminated. The first call sizes the output.
        let status = unsafe {
            AssocQueryStringW(
                ASSOCF_IS_PROTOCOL,
                ASSOCSTR_EXECUTABLE,
                name,
                PCWSTR::null(),
                None,
                &mut len,
            )
        };
        if status == ERROR_NO_ASSOCIATION.to_hresult() {
            return Some(false);
        }
        if status != S_FALSE || len == 0 {
            return None;
        }
        let mut path = vec![0u16; len as usize];
        // SAFETY: The output buffer has len writable UTF-16 code units.
        let status = unsafe {
            AssocQueryStringW(
                ASSOCF_IS_PROTOCOL,
                ASSOCSTR_EXECUTABLE,
                name,
                PCWSTR::null(),
                Some(PWSTR(path.as_mut_ptr())),
                &mut len,
            )
        };
        if status.is_err() {
            return None;
        }
        let end = path.iter().position(|unit| *unit == 0)?;
        let handler = OsString::from_wide(&path[..end]);
        same_file::is_same_file(std::env::current_exe().ok()?, handler).ok()
    }

    pub(super) fn set(_: Scheme) -> Result<(), SetError> {
        // SAFETY: These static strings and null pointers follow ShellExecuteW's contract.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("ms-settings:defaultapps"),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        ((result.0 as isize) > 32)
            .then_some(())
            .ok_or(SetError::Failed)
    }
}
