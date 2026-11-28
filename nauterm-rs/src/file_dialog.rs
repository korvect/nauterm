//! Native chooser directories follow the reference client, independently of the
//! last directory used by an unrelated chooser. No resource/keyring access.
use anyhow::Result;
use gpui_kit::{App, PathPromptOptions, Window};
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

#[derive(Clone, Copy)]
pub enum InitialDirectory {
    SshImport,
    SshExport,
}
fn ssh_directory(home: &Path, create: bool) -> PathBuf {
    let ssh = home.join(".ssh");
    if create {
        let prepared = std::fs::create_dir_all(&ssh);
        #[cfg(unix)]
        let prepared = prepared.and_then(|()| {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o700))
        });
        if prepared.is_err() {
            return home.to_owned();
        }
    }
    if ssh.is_dir() { ssh } else { home.to_owned() }
}
fn export_directory_in(home: &Path) -> PathBuf {
    [
        home.join("Downloads"),
        home.join("Documents"),
        home.to_owned(),
    ]
    .into_iter()
    .find(|path| path.is_dir())
    .unwrap_or_default()
}
pub fn export_directory() -> PathBuf {
    directories::UserDirs::new()
        .map(|dirs| export_directory_in(dirs.home_dir()))
        .unwrap_or_default()
}
// Keep the same cancellation/error shape as GPUI's platform chooser.
type PickResult = Result<Result<Option<Vec<PathBuf>>>>;
pub fn open(
    mut options: PathPromptOptions,
    initial: Option<InitialDirectory>,
    window: &Window,
    cx: &mut App,
) -> Pin<Box<dyn Future<Output = PickResult>>> {
    options.prompt = options
        .prompt
        .map(|prompt| crate::localization::text(cx, &prompt));
    // RFD's mixed file/folder picker is macOS-only. Other platforms must
    // delegate mixed selection to GPUI rather than reference unavailable APIs.
    #[cfg(not(target_os = "macos"))]
    if options.directories && options.files {
        let picker = cx.prompt_for_paths(options);
        return Box::pin(async move { picker.await.map_err(Into::into) });
    }
    let Some(initial) = initial else {
        let picker = cx.prompt_for_paths(options);
        return Box::pin(async move { picker.await.map_err(Into::into) });
    };
    let mut dialog = rfd::AsyncFileDialog::new().set_parent(window);
    if let Some(title) = options.prompt {
        dialog = dialog.set_title(title);
    }
    Box::pin(async move {
        let directory = smol::unblock(move || {
            directories::UserDirs::new().map(|dirs| {
                ssh_directory(
                    dirs.home_dir(),
                    matches!(initial, InitialDirectory::SshExport),
                )
            })
        })
        .await;
        if let Some(directory) = directory {
            dialog = dialog.set_directory(directory);
        }
        let paths = if options.directories && !options.files {
            if options.multiple {
                dialog.pick_folders().await
            } else {
                dialog.pick_folder().await.map(|file| vec![file])
            }
        } else if options.directories {
            #[cfg(target_os = "macos")]
            {
                if options.multiple {
                    dialog.pick_files_or_folders().await
                } else {
                    dialog.pick_file_or_folder().await.map(|file| vec![file])
                }
            }
            #[cfg(not(target_os = "macos"))]
            unreachable!("mixed selection uses the platform chooser above")
        } else if options.multiple {
            dialog.pick_files().await
        } else {
            dialog.pick_file().await.map(|file| vec![file])
        };
        Ok(Ok(paths.map(|files| {
            files
                .into_iter()
                .map(|file| file.path().to_owned())
                .collect()
        })))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chooser_directories_fall_back_and_prepare_only_ssh_exports() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(ssh_directory(home.path(), false), home.path());
        assert!(!home.path().join(".ssh").exists());
        assert_eq!(export_directory_in(home.path()), home.path());
        let documents = home.path().join("Documents");
        std::fs::create_dir(&documents).unwrap();
        assert_eq!(export_directory_in(home.path()), documents);
        let downloads = home.path().join("Downloads");
        std::fs::create_dir(&downloads).unwrap();
        assert_eq!(export_directory_in(home.path()), downloads);
        let ssh = ssh_directory(home.path(), true);
        assert_eq!(ssh, home.path().join(".ssh"));
        assert_eq!(ssh_directory(home.path(), false), ssh);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(ssh).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }
}
