//! Export saved key material to a user-selected directory, like the reference.
//! Secret contents never enter UI messages, public metadata or export receipts.
use super::{Editor, Item, Keychain};
use crate::{
    components::{
        Button,
        menu::{DropdownMenu, MenuItem},
    },
    file_commit::DownloadTarget,
    vault::KeyRecord,
};
use anyhow::{Context as _, Result, ensure};
use gpui_kit::{
    AnyElement, Context, EventEmitter, PathPromptOptions, Window, assets::IconName,
    component::Disableable, prelude::*,
};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::LazyLock,
};
use zeroize::Zeroizing;

static EXTENSION: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)(?:-cert\.pub|\.pub)$").unwrap());
static INVALID: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"[\\/:*?"<>|\x00-\x1f]+"#).unwrap());
static SPACE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\s+").unwrap());
static RESERVED: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$").unwrap());

fn filename(name: &str) -> String {
    let value = EXTENSION.replace(name.trim(), "");
    let value = INVALID.replace_all(&value, "-");
    let value = SPACE.replace_all(&value, "-");
    let mut value = value
        .trim_start_matches('.')
        .trim_end_matches(['.', ' ', '-'])
        .to_owned();
    // Leave room for a shared collision suffix and -cert.pub on all platforms.
    if value.len() > 180 {
        let mut end = 180;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
        value = value.trim_end_matches(['.', ' ', '-']).to_owned();
    }
    if value.is_empty() {
        return "id_key".into();
    }
    if RESERVED.is_match(value.split('.').next().unwrap_or_default()) {
        value.insert(0, '_');
    }
    value
}
struct Material {
    suffix: &'static str,
    contents: Zeroizing<String>,
    mode: u32,
}
fn available_base(directory: &Path, name: &str, materials: &[Material]) -> Result<String> {
    for attempt in 1u64.. {
        let base = if attempt == 1 {
            name.to_owned()
        } else {
            format!("{name}-{attempt}")
        };
        let mut occupied = false;
        for material in materials {
            match fs::symlink_metadata(directory.join(format!("{base}{}", material.suffix))) {
                Ok(_) => {
                    occupied = true;
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).context("Could not check the key export destination.");
                }
            }
        }
        if !occupied {
            return Ok(base);
        }
    }
    unreachable!()
}

struct Report {
    directory: PathBuf,
    files: Vec<PathBuf>,
}
fn export(mut key: KeyRecord, directory: PathBuf) -> Result<Report> {
    ensure!(
        directory.is_absolute(),
        "Choose an absolute export directory."
    );
    let directory = directory
        .canonicalize()
        .context("Could not open the export directory.")?;
    ensure!(directory.is_dir(), "Choose a directory for the key export.");
    let name = filename(&key.name);
    let materials: Vec<_> = [
        ("", key.private_key.take(), 0o600),
        (".pub", key.public_key.take(), 0o644),
        ("-cert.pub", key.certificate.take(), 0o644),
    ]
    .into_iter()
    .filter_map(|(suffix, contents, mode)| {
        let mut contents = Zeroizing::new(contents?);
        if contents.trim().is_empty() {
            return None;
        }
        if !contents.ends_with('\n') {
            contents.push('\n');
        }
        Some(Material {
            suffix,
            contents,
            mode,
        })
    })
    .collect();
    drop(key);
    ensure!(
        !materials.is_empty(),
        "No key material is available to export."
    );
    let base = available_base(&directory, &name, &materials)?;
    let mut targets = Vec::with_capacity(materials.len());
    // Prepare every output privately before publishing any member of the set.
    for material in &materials {
        let path = directory.join(format!("{base}{}", material.suffix));
        let target = DownloadTarget::new(&path, false)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(not(unix))]
        let _ = material.mode;
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(material.mode);
        }
        let mut file = options
            .open(target.staged())
            .context("Could not prepare a key export file.")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(material.mode))?;
        }
        file.write_all(material.contents.as_bytes())?;
        file.sync_all()?;
        targets.push((path, target));
    }
    let mut files = Vec::with_capacity(targets.len());
    for (path, target) in targets {
        target.publish().with_context(|| {
            if files.is_empty() {
                "The export destination changed; no key files were published. Try again.".to_owned()
            } else {
                format!(
                    "Key export stopped after creating {}. The remaining files were not published.",
                    files
                        .iter()
                        .map(|path: &PathBuf| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        })?;
        files.push(path);
    }
    Ok(Report { directory, files })
}

pub struct Message(pub crate::components::message::Message);
impl EventEmitter<Message> for Keychain {}
impl Keychain {
    pub(super) fn editor_actions(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(item) = editor.item() else {
            return vec![];
        };
        let owner = cx.weak_entity();
        vec![
            Button::new("credential-editor-actions")
                .toolbar()
                .ghost()
                .icon(IconName::Ellipsis)
                .tooltip("Credential actions")
                .disabled(self.busy(cx))
                .dropdown_menu(move |menu, _, _| {
                    let host = owner.clone();
                    let file = owner.clone();
                    let duplicate = owner.clone();
                    let delete = owner.clone();
                    menu.width(180.)
                        .when(item.key, |menu| {
                            menu.item(
                                MenuItem::localized("Export to host")
                                    .icon(IconName::Upload)
                                    .on_click(move |_, window, cx| {
                                        let _ = host.update(cx, |this, cx| {
                                            this.start_export(item, window, cx)
                                        });
                                    }),
                            )
                            .item(
                                MenuItem::localized("Export to file")
                                    .icon(IconName::Download)
                                    .on_click(move |_, window, cx| {
                                        let _ = file.update(cx, |this, cx| {
                                            this.export_file(item, window, cx)
                                        });
                                    }),
                            )
                        })
                        .item(
                            MenuItem::localized("Duplicate")
                                .icon(IconName::Copy)
                                .on_click(move |_, _, cx| {
                                    let _ = duplicate.update(cx, |this, cx| {
                                        this.selection = BTreeSet::from([item]);
                                        this.duplicate(cx);
                                    });
                                }),
                        )
                        .separator()
                        .item(
                            MenuItem::localized("Delete")
                                .icon(IconName::Trash)
                                .destructive(true)
                                .on_click(move |_, _, cx| {
                                    let _ = delete.update(cx, |this, cx| {
                                        if !this.busy(cx) {
                                            this.selection = BTreeSet::from([item]);
                                            this.delete_pending = true;
                                            cx.notify();
                                        }
                                    });
                                }),
                        )
                })
                .into_any_element(),
        ]
    }
    pub(super) fn export_file(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        if !item.key || self.busy(cx) {
            return;
        }
        let Some(vault) = self.vault.clone() else {
            return;
        };
        self.exporting_file = true;
        let picker = crate::file_dialog::open(
            PathPromptOptions {
                files: false,
                directories: true,
                multiple: false,
                prompt: Some("Export key".into()),
            },
            Some(crate::file_dialog::InitialDirectory::SshExport),
            window,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let result = match picker.await {
                Ok(Ok(Some(mut paths))) if paths.len() == 1 => {
                    let path = paths.pop().unwrap();
                    match vault.key(item.id).await {
                        Ok(key) => cx
                            .background_spawn(async move { export(key, path) })
                            .await
                            .map(Some),
                        Err(error) => Err(error),
                    }
                }
                Ok(Ok(None)) => Ok(None),
                _ => Err(anyhow::anyhow!(
                    "Could not open the export directory picker."
                )),
            };
            let _ = this.update(cx, |this, cx| {
                this.exporting_file = false;
                match result {
                    Ok(Some(report)) => {
                        cx.emit(Message(crate::components::message::Message::info(format!(
                            "Exported {} key {} to {}.",
                            report.files.len(),
                            if report.files.len() == 1 {
                                "file"
                            } else {
                                "files"
                            },
                            report.directory.display()
                        ))))
                    }
                    Ok(None) => {}
                    Err(error) => cx.emit(Message(crate::components::message::Message::error(
                        format!("Failed to export key: {error:#}"),
                    ))),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_names_keep_unicode_and_are_portable_single_filenames() {
        for (source, expected) in [
            (" Work key-cert.pub ", "Work-key"),
            ("../bad/:name.PUB", "-bad-name"),
            ("...", "id_key"),
            ("CON.txt", "_CON.txt"),
            ("服务器 key", "服务器-key"),
        ] {
            assert_eq!(filename(source), expected);
        }
        let long = filename(&"中".repeat(100));
        assert!(long.len() <= 180 && !long.contains(['/', '\\']));
    }
    fn fixture() -> KeyRecord {
        KeyRecord {
            name: "fixture-cert.pub".into(),
            private_key: Some("ENCRYPTED PRIVATE FIXTURE".into()),
            public_key: Some("PUBLIC FIXTURE\n".into()),
            certificate: Some("CERTIFICATE FIXTURE".into()),
            passphrase: Some("not exported".into()),
            ..Default::default()
        }
    }
    #[test]
    fn exports_complete_sets_with_shared_collision_suffixes_and_private_modes() -> Result<()> {
        let directory = tempfile::tempdir()?;
        fs::write(directory.path().join("fixture-cert.pub"), b"keep existing")?;
        let report = export(fixture(), directory.path().into())?;
        assert_eq!(report.files.len(), 3);
        for (suffix, expected, mode) in [
            ("", "ENCRYPTED PRIVATE FIXTURE\n", 0o600),
            (".pub", "PUBLIC FIXTURE\n", 0o644),
            ("-cert.pub", "CERTIFICATE FIXTURE\n", 0o644),
        ] {
            let path = directory.path().join(format!("fixture-2{suffix}"));
            assert_eq!(fs::read_to_string(&path)?, expected);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, mode);
            }
            #[cfg(not(unix))]
            let _ = mode;
        }
        assert_eq!(
            fs::read(directory.path().join("fixture-cert.pub"))?,
            b"keep existing"
        );
        let next = export(fixture(), directory.path().into())?;
        assert!(next.files.iter().all(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("fixture-3")
        }));
        assert_eq!(fs::read_dir(directory.path())?.count(), 7);
        Ok(())
    }
    #[test]
    fn empty_material_and_invalid_directories_publish_nothing() -> Result<()> {
        let directory = tempfile::tempdir()?;
        assert!(export(KeyRecord::default(), directory.path().into()).is_err());
        assert_eq!(fs::read_dir(directory.path())?.count(), 0);
        let file = directory.path().join("existing");
        fs::write(&file, b"existing data")?;
        assert!(export(fixture(), file.clone()).is_err());
        assert_eq!(fs::read(&file)?, b"existing data");
        let report = export(
            KeyRecord {
                id: None,
                name: "public.pub".into(),
                private_key: None,
                public_key: Some("PUBLIC ONLY".into()),
                certificate: None,
                passphrase: None,
            },
            directory.path().into(),
        )?;
        assert_eq!(
            report.files,
            vec![directory.path().canonicalize()?.join("public.pub")]
        );
        assert_eq!(fs::read_to_string(&report.files[0])?, "PUBLIC ONLY\n");
        assert!(!directory.path().join("public").exists());
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn dangling_symlinks_reserve_the_entire_export_name() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let link = directory.path().join("fixture.pub");
        std::os::unix::fs::symlink("missing-target", &link)?;
        let report = export(fixture(), directory.path().into())?;
        assert!(report.files.iter().all(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("fixture-2")
        }));
        assert_eq!(fs::read_link(link)?, PathBuf::from("missing-target"));
        assert!(!directory.path().join("missing-target").exists());
        Ok(())
    }
}
