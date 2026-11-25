//! Import only a selected private key or a certificate's adjacent private key.
//! Companion reads stay bounded and never scan an SSH directory or original DB.
use super::{Editor, Keychain, creation::is_certificate};
use crate::{
    components::Button, connection::read_key_file, palette::Palette, typography::FontWeights,
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
    ensure!(metadata.is_file(), "Choose a regular file.");
    ensure!(metadata.len() > 0, "The selected file is empty.");
    ensure!(
        metadata.len() <= 4 * 1024 * 1024,
        "The selected file is too large."
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
                prompt: None,
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
                    Err(error) => {
                        use crate::localization::{format_for, language, message};
                        editor.error = Some(
                            format_for(
                                language(cx),
                                "workspace.keys.import.failure",
                                &[("error", &message(cx, &format!("{error:#}")))],
                            )
                            .to_string(),
                        );
                    }
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
        let accepts = move |paths: &ExternalPaths| !busy && !paths.0.is_empty();
        let content = |dragging: bool, label| {
            let color = if dragging { p.accent } else { p.muted };
            div()
                .id(if dragging { "drop-ready" } else { "drop-idle" })
                .absolute()
                .inset_0()
                .px(px(10.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .text_center()
                .text_size(px(12.))
                .font_weight(FontWeights::MEDIUM)
                .opacity(if dragging { 0. } else { 1. })
                .drag_over::<ExternalPaths>(move |style, paths, _, _| {
                    style.opacity(if dragging == accepts(paths) { 1. } else { 0. })
                })
                .child(
                    Icon::new(IconName::FileUp)
                        .with_size(px(22.))
                        .text_color(color),
                )
                .child(
                    div()
                        .text_color(if dragging { p.accent } else { p.text })
                        .child(crate::localization::view(label)),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .id("key-file-import-drop")
                    .h(px(112.))
                    .w_full()
                    .relative()
                    .rounded(px(9.))
                    .border_1()
                    .border_dashed()
                    .border_color(if editor.private_error.is_some() {
                        rgb(0xe5453d)
                    } else {
                        p.border
                    })
                    .bg(p.surface.opacity(0.48))
                    .can_drop(move |data, _, _| {
                        data.downcast_ref::<ExternalPaths>().is_some_and(accepts)
                    })
                    .drag_over::<ExternalPaths>(move |style, _, _, _| {
                        style.border_color(p.accent).bg(p.accent.opacity(0.07))
                    })
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                        if let Some(path) = paths.0.first() {
                            this.import_path(path.clone(), window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        Button::new("key-import-drop-button")
                            .localized_accessibility_label("workspace.keys.import.fromFile")
                            .ghost()
                            .w_full()
                            .h_full()
                            .p_0()
                            .rounded(px(9.))
                            .disabled(busy)
                            .child(content(
                                false,
                                if editor.importing {
                                    "workspace.keys.import.importing"
                                } else {
                                    "workspace.keys.import.drag"
                                },
                            ))
                            .child(content(true, "workspace.keys.import.drop"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.choose_import(window, cx)),
                            ),
                    ),
            )
            .child(
                Button::new("import-credential")
                    .w_full()
                    .large()
                    .primary()
                    .icon(IconName::FileKey)
                    .label(if editor.importing {
                        "workspace.keys.import.importing"
                    } else {
                        "workspace.keys.import.fromFile"
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
