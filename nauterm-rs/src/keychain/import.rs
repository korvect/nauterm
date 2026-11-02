//! Import only a selected private key or a certificate's adjacent private key.
//! Companion reads stay bounded and never scan an SSH directory or original DB.
use super::{Editor, Keychain, creation::is_certificate};
use crate::{
    components::{Button, drag},
    connection::read_key_file,
    palette::Palette,
};
use anyhow::{Context as _, Result, ensure};
use gpui_kit::{
    AnyElement, Context, ExternalPaths, IntoElement, PathPromptOptions, Window,
    assets::IconName,
    component::{Disableable, Icon, Sizable},
    div,
    prelude::*,
    px, rgb,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

struct Imported {
    private: Zeroizing<String>,
    public: Option<Zeroizing<String>>,
    certificate: Option<Zeroizing<String>>,
}
fn read(path: &Path) -> Result<Zeroizing<String>> {
    let metadata = fs::metadata(path)?;
    ensure!(
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= 4 * 1024 * 1024,
        "Choose a nonempty regular key file smaller than 4 MB."
    );
    let value = read_key_file(path.to_owned())?;
    ensure!(!value.trim().is_empty(), "The selected file is empty.");
    Ok(value)
}
fn companion(path: &Path, suffix: &str) -> PathBuf {
    let mut path = path.as_os_str().to_os_string();
    path.push(suffix);
    path.into()
}
fn optional(path: &Path) -> Result<Option<Zeroizing<String>>> {
    match fs::metadata(path) {
        Ok(_) => read(path).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("Could not read an adjacent credential file."),
    }
}
fn public(value: &str) -> bool {
    let header = value.trim_start().lines().next().unwrap_or_default();
    ["ssh-", "ecdsa-", "sk-ssh-", "sk-ecdsa-"]
        .into_iter()
        .any(|prefix| header.starts_with(prefix))
}
fn private(value: &str) -> bool {
    let header = value.trim_start().lines().next().unwrap_or_default().trim();
    (header.starts_with("-----BEGIN ") && header.ends_with(" PRIVATE KEY-----"))
        || header.starts_with("PuTTY-User-Key-File-")
}
fn import(path: PathBuf) -> Result<Imported> {
    let selected = read(&path)?;
    let selected_certificate = is_certificate(&selected);
    let (private_path, private, certificate) = if selected_certificate {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix("-cert.pub"))
            .filter(|name| !name.is_empty())
            .context(
                "The certificate filename must end with -cert.pub so its private key can be found.",
            )?;
        let private_path = path.with_file_name(name);
        let private = read(&private_path)
            .context("The matching private key could not be read next to the certificate.")?;
        (private_path, private, Some(selected))
    } else {
        (path, selected, None)
    };
    ensure!(
        !public(&private),
        "Select a private key file, not a public key."
    );
    ensure!(
        self::private(&private),
        "The selected file is not a private key."
    );
    let certificate = match certificate {
        Some(certificate) => Some(certificate),
        None => {
            optional(&companion(&private_path, "-cert.pub"))?.filter(|value| is_certificate(value))
        }
    };
    let public = optional(&companion(&private_path, ".pub"))?
        .filter(|value| self::public(value) && !is_certificate(value));
    Ok(Imported {
        private,
        public,
        certificate,
    })
}

impl Keychain {
    pub(super) fn choose_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.exporting_file {
            return;
        }
        let Some(editor) = self.editor.as_mut().filter(|editor| {
            editor.can_import(self.related) && !editor.importing && !editor.picking
        }) else {
            return;
        };
        editor.picking = true;
        editor.import_generation = editor.import_generation.wrapping_add(1);
        let generation = editor.import_generation;
        let anchor = editor.name.entity_id();
        let picker = crate::file_dialog::open(
            PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Import private key or certificate".into()),
            },
            Some(crate::file_dialog::InitialDirectory::SshImport),
            window,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let result = match picker.await {
                Ok(Ok(Some(mut paths))) => Ok(paths.pop()),
                Ok(Ok(None)) => Ok(None),
                _ => Err(anyhow::anyhow!("Could not open the key import picker.")),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(editor) = this.editor.as_mut().filter(|editor| {
                    editor.name.entity_id() == anchor
                        && editor.import_generation == generation
                        && editor.can_import(this.related)
                }) else {
                    return;
                };
                editor.picking = false;
                match result {
                    Ok(Some(path)) => this.import_path(path, window, cx),
                    Ok(None) => {}
                    Err(error) => editor.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn import_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.exporting_file {
            return;
        }
        let Some(editor) = self.editor.as_mut().filter(|editor| {
            editor.can_import(self.related) && !editor.importing && !editor.picking
        }) else {
            return;
        };
        editor.import_generation = editor.import_generation.wrapping_add(1);
        editor.importing = true;
        editor.error = None;
        let generation = editor.import_generation;
        let anchor = editor.name.entity_id();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { import(path) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(editor) = this.editor.as_mut().filter(|editor| {
                    editor.name.entity_id() == anchor
                        && editor.import_generation == generation
                        && editor.can_import(this.related)
                }) else {
                    return;
                };
                editor.importing = false;
                match result {
                    Ok(value) => {
                        editor.private.update(cx, |field, cx| {
                            field.set_value(value.private.as_str(), window, cx)
                        });
                        editor.public.update(cx, |field, cx| {
                            field.set_value(
                                value
                                    .public
                                    .as_ref()
                                    .map(|value| value.as_str())
                                    .unwrap_or_default(),
                                window,
                                cx,
                            )
                        });
                        editor.certificate.update(cx, |field, cx| {
                            field.set_value(
                                value
                                    .certificate
                                    .as_ref()
                                    .map(|value| value.as_str())
                                    .unwrap_or_default(),
                                window,
                                cx,
                            )
                        });
                        editor.error = None;
                        editor.private_error = None;
                        this.refresh_automatic_name(window, cx);
                    }
                    Err(error) => editor.error = Some(format!("Unable to import key: {error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn import_panel(&self, editor: &Editor, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.dark);
        let busy = self.saving || self.exporting_file || editor.importing || editor.picking;
        let owner = cx.weak_entity();
        let panel = Button::new("key-import-drop-button")
            .localized_accessibility_label("Import private key file")
            .ghost()
            .w_full()
            .h_full()
            .p(px(10.))
            .rounded(px(9.))
            .border_1()
            .border_color(if editor.private_error.is_some() {
                rgb(0xe5453d)
            } else {
                p.border
            })
            .bg(p.sidebar)
            .disabled(busy)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(9.))
                    .text_color(p.text)
                    .child(Icon::new(IconName::FileUp).with_size(px(28.)))
                    .child(crate::localization::view(if editor.importing {
                        "Importing…"
                    } else {
                        "Drag and drop a private key file"
                    })),
            )
            .on_click(cx.listener(|this, _, window, cx| this.choose_import(window, cx)))
            .into_any_element();
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .h(px(112.))
                    .w_full()
                    .child(drag::external_target_with_label(
                        "key-file-import-drop",
                        panel,
                        self.dark,
                        "Drop the private key to import",
                        move |paths, cx| {
                            owner.upgrade().is_some_and(|owner| {
                                let this = owner.read(cx);
                                !paths.0.is_empty()
                                    && !this.saving
                                    && !this.exporting_file
                                    && this.editor.as_ref().is_some_and(|editor| {
                                        editor.can_import(this.related)
                                            && !editor.importing
                                            && !editor.picking
                                    })
                            })
                        },
                        cx.listener(|this, paths: &ExternalPaths, window, cx| {
                            if let Some(path) = paths.0.first() {
                                this.import_path(path.clone(), window, cx);
                            }
                        }),
                    )),
            )
            .child(
                Button::new("import-credential")
                    .w_full()
                    .large()
                    .primary()
                    .icon(IconName::FileKey)
                    .label(if editor.importing {
                        "Importing…"
                    } else {
                        "Import from key file"
                    })
                    .loading(editor.importing)
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, window, cx| this.choose_import(window, cx))),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::import;
    use anyhow::Result;
    use std::fs;
    const PRIVATE: &str =
        "-----BEGIN OPENSSH PRIVATE KEY-----\nOPAQUE FIXTURE\n-----END OPENSSH PRIVATE KEY-----\n";
    const PUBLIC: &str = "ssh-ed25519 AAAA fixture";
    const CERTIFICATE: &str = "ssh-ed25519-cert-v01@openssh.com AAAA fixture";

    #[test]
    fn private_or_certificate_selection_loads_only_the_matching_companions() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let private = directory.path().join("fixture");
        fs::write(&private, PRIVATE)?;
        fs::write(directory.path().join("fixture.pub"), PUBLIC)?;
        fs::write(directory.path().join("fixture-cert.pub"), CERTIFICATE)?;
        let unrelated = directory.path().join("original-app-fixture");
        fs::write(&unrelated, "leave untouched")?;
        for selected in [private, directory.path().join("fixture-cert.pub")] {
            let value = import(selected)?;
            assert_eq!(value.private.as_str(), PRIVATE);
            assert_eq!(
                value.public.as_deref().map(|value| value.as_str()),
                Some(PUBLIC)
            );
            assert_eq!(
                value.certificate.as_deref().map(|value| value.as_str()),
                Some(CERTIFICATE)
            );
        }
        assert_eq!(fs::read_to_string(unrelated)?, "leave untouched");
        Ok(())
    }
    #[test]
    fn public_keys_and_unpaired_or_misnamed_certificates_are_rejected() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let public = directory.path().join("fixture.pub");
        fs::write(&public, PUBLIC)?;
        assert!(
            import(public)
                .err()
                .unwrap()
                .to_string()
                .contains("not a public key")
        );
        let certificate = directory.path().join("certificate.pub");
        fs::write(&certificate, CERTIFICATE)?;
        assert!(
            import(certificate)
                .err()
                .unwrap()
                .to_string()
                .contains("must end with -cert.pub")
        );
        let certificate = directory.path().join("missing-cert.pub");
        fs::write(&certificate, CERTIFICATE)?;
        assert!(import(certificate).is_err());
        Ok(())
    }
    #[test]
    fn empty_binary_and_oversize_files_are_rejected_before_publication() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("fixture");
        for bytes in [
            vec![],
            b"binary\0data".to_vec(),
            vec![b'x'; 4 * 1024 * 1024 + 1],
            b"not a private key".to_vec(),
        ] {
            fs::write(&path, bytes)?;
            assert!(import(path.clone()).is_err());
        }
        fs::write(&path, PRIVATE)?;
        let value = import(path)?;
        assert_eq!(value.private.as_str(), PRIVATE);
        assert!(value.public.is_none() && value.certificate.is_none());
        assert_eq!(fs::read_dir(directory.path())?.count(), 1);
        Ok(())
    }
}
