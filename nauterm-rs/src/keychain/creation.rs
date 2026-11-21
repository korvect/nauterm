use super::{CREDENTIAL_FIELD_GAP, Editor, Keychain, Kind};
use crate::components::{ControlMetrics, Field, motion::Transition, panel};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use gpui_kit::component::{Disableable, Size};
use gpui_kit::{AnyElement, App, Context, IntoElement, Window, div, prelude::*, px};
use std::time::Instant;
use zeroize::Zeroizing;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Method {
    Paste,
    Import,
    Generate,
}

impl Method {
    const ALL: [Self; 3] = [Self::Paste, Self::Import, Self::Generate];
    fn label(self) -> &'static str {
        match self {
            Self::Paste => "Paste",
            Self::Import => "Import",
            Self::Generate => "Generate",
        }
    }
}
fn wire_label(wire: &str) -> Option<&'static str> {
    if wire.starts_with("sk-ssh-ed25519") {
        Some("FIDO2 ED25519")
    } else if wire.starts_with("sk-ecdsa-") {
        Some("FIDO2 ECDSA")
    } else if wire.starts_with("ssh-ed25519") {
        Some("ED25519")
    } else if wire.starts_with("ecdsa-") {
        Some("ECDSA")
    } else if wire.starts_with("ssh-rsa") {
        Some("RSA")
    } else if wire.starts_with("ssh-dss") {
        Some("DSA")
    } else {
        None
    }
}
pub(super) fn is_certificate(value: &str) -> bool {
    value
        .split_whitespace()
        .next()
        .is_some_and(|wire| wire.ends_with("-cert-v01@openssh.com") && wire_label(wire).is_some())
}

fn private_key_requires_passphrase(value: &str) -> bool {
    let value = value.trim();
    let mut lines = value.lines();
    match lines.next().unwrap_or_default() {
        "-----BEGIN OPENSSH PRIVATE KEY-----" => openssh_requires_passphrase(value),
        "-----BEGIN ENCRYPTED PRIVATE KEY-----" => true,
        "-----BEGIN RSA PRIVATE KEY-----"
        | "-----BEGIN DSA PRIVATE KEY-----"
        | "-----BEGIN EC PRIVATE KEY-----" => {
            lines.any(|line| line.trim().eq_ignore_ascii_case("Proc-Type: 4,ENCRYPTED"))
        }
        header
            if header.starts_with("PuTTY-User-Key-File-2:")
                || header.starts_with("PuTTY-User-Key-File-3:") =>
        {
            lines
                .find_map(|line| line.strip_prefix("Encryption:"))
                .map(str::trim)
                .is_some_and(|cipher| !cipher.is_empty() && cipher != "none")
        }
        _ => false,
    }
}

fn openssh_requires_passphrase(value: &str) -> bool {
    let Some((body, _)) = value
        .strip_prefix("-----BEGIN OPENSSH PRIVATE KEY-----")
        .and_then(|body| body.split_once("-----END OPENSSH PRIVATE KEY-----"))
    else {
        return false;
    };
    // The first 48 decoded bytes fit the magic, size, and longest supported
    // cipher name (chacha20-poly1305@openssh.com). Do not decode key material.
    let prefix: Vec<_> = body
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .take(64)
        .collect();
    let Ok(decoded) = BASE64_STANDARD.decode(prefix).map(Zeroizing::new) else {
        return false;
    };
    let Some((length, rest)) = decoded
        .strip_prefix(b"openssh-key-v1\0")
        .and_then(|header| header.split_first_chunk::<4>())
    else {
        return false;
    };
    rest.get(..u32::from_be_bytes(*length) as usize)
        .and_then(|name| std::str::from_utf8(name).ok())
        .is_some_and(|name| !name.is_empty() && name != "none")
}

fn display_name(private: &str, public: &str, certificate: &str) -> String {
    if is_certificate(certificate) {
        return format!(
            "{} SSH Certificate",
            wire_label(certificate.split_whitespace().next().unwrap()).unwrap()
        );
    }
    let header = private.trim_start().lines().next().unwrap_or_default();
    let kind = public
        .split_whitespace()
        .next()
        .and_then(wire_label)
        .or_else(|| {
            header
                .split_once(':')
                .and_then(|(_, wire)| wire_label(wire.trim()))
        })
        .unwrap_or_else(|| {
            if header.contains("BEGIN EC ") {
                "ECDSA"
            } else if header.contains("BEGIN RSA ") {
                "RSA"
            } else if header.contains("BEGIN DSA ") {
                "DSA"
            } else {
                "SSH"
            }
        });
    format!("{kind} key")
}
impl Editor {
    pub(super) fn requires_passphrase(&self, cx: &App) -> bool {
        self.kind != Kind::Identity
            && private_key_requires_passphrase(&self.private.read(cx).value())
    }

    pub(super) fn suggested_name(&self, inventory: &crate::vault::Inventory, cx: &App) -> String {
        if self.kind == Kind::Identity {
            let username = self.username.read(cx).value();
            let username = username.trim();
            let key = self
                .key_id
                .and_then(|id| inventory.keys.iter().find(|key| key.id == id))
                .map(|key| key.name.trim())
                .unwrap_or_default();
            match (username.is_empty(), key.is_empty()) {
                (false, false) => format!("{username}:{key}"),
                (false, true) => username.to_owned(),
                (true, false) => key.to_owned(),
                (true, true) => String::new(),
            }
        } else if self.is_generating() {
            self.generation.kind.suggested_name()
        } else {
            display_name(
                self.private.read(cx).value().as_ref(),
                self.public.read(cx).value().as_ref(),
                self.certificate.read(cx).value().as_ref(),
            )
        }
    }
}
impl Keychain {
    pub(super) fn detected_passphrase_field(
        &self,
        editor: &Editor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if editor.kind == Kind::Identity || editor.is_generating() {
            return None;
        }
        let visible = editor.requires_passphrase(cx);
        let target = f32::from(visible);
        let motion = window.use_keyed_state(
            ("key-passphrase-height", editor.passphrase.entity_id()),
            cx,
            |_, _| Transition::new(target),
        );
        let reduce_motion = cx.reduce_motion();
        let duration = if visible { panel::ENTER } else { panel::EXIT };
        let (amount, moving) = motion.update(cx, |motion, _| {
            motion.set(target, Instant::now(), duration, reduce_motion)
        });
        if moving {
            window.request_animation_frame();
        }
        (amount > 0.).then(|| {
            let height = ControlMetrics::input(Size::Large).height + CREDENTIAL_FIELD_GAP;
            div()
                .h(px(height * amount))
                .flex_shrink_0()
                .overflow_hidden()
                .child(
                    div().pt(px(CREDENTIAL_FIELD_GAP)).child(
                        Field::input(&editor.passphrase, "common.label.passphrase")
                            .secret(true)
                            .disabled(self.saving || !visible),
                    ),
                )
                .into_any_element()
        })
    }

    pub(super) fn refresh_automatic_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.editor
            && editor.automatic_name
        {
            let name = editor.suggested_name(&self.inventory, cx);
            if editor.name.read(cx).value().as_ref() != name {
                editor
                    .name
                    .update(cx, |field, cx| field.set_value(name, window, cx));
            }
        }
    }
    fn select_creation_method(
        &mut self,
        method: Method,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving || self.exporting_file {
            return;
        }
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.id.is_some() || editor.kind == Kind::Identity || editor.method == method {
            return;
        }
        editor.method = method;
        editor.import_generation = editor.import_generation.wrapping_add(1);
        editor.importing = false;
        editor.picking = false;
        editor.error = None;
        self.refresh_automatic_name(window, cx);
        cx.notify();
    }
    pub(super) fn creation_methods(&self, editor: &Editor, cx: &mut Context<Self>) -> AnyElement {
        crate::components::Segmented::new(
            "credential-creation-method",
            editor.method,
            Method::ALL.map(|method| (method, method.label().into())),
        )
        .disabled(self.saving || self.exporting_file)
        .on_change(
            cx.listener(|this, method, window, cx| {
                this.select_creation_method(*method, window, cx)
            }),
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{BASE64_STANDARD, Engine, display_name, private_key_requires_passphrase};

    #[test]
    fn reads_openssh_cipher_metadata_without_unlocking_the_key() {
        for (cipher, expected) in [
            ("none", false),
            ("aes256-ctr", true),
            ("aes256-cbc", true),
            ("chacha20-poly1305@openssh.com", true),
            ("", false),
        ] {
            let mut header = b"openssh-key-v1\0".to_vec();
            header.extend_from_slice(&(cipher.len() as u32).to_be_bytes());
            header.extend_from_slice(cipher.as_bytes());
            let body = BASE64_STANDARD.encode(header);
            let key = format!(
                "-----BEGIN OPENSSH PRIVATE KEY-----\r\n{}\r\n{}\r\n-----END OPENSSH PRIVATE KEY-----",
                &body[..16],
                &body[16..]
            );
            assert_eq!(private_key_requires_passphrase(&key), expected, "{cipher}");
        }
        assert!(!private_key_requires_passphrase(
            "-----BEGIN OPENSSH PRIVATE KEY-----\ninvalid!\n-----END OPENSSH PRIVATE KEY-----"
        ));
    }

    #[test]
    fn reads_legacy_pem_pkcs8_and_putty_encryption_headers() {
        for algorithm in ["RSA", "DSA", "EC"] {
            assert!(private_key_requires_passphrase(&format!(
                "-----BEGIN {algorithm} PRIVATE KEY-----\r\nProc-Type: 4,ENCRYPTED\r\n"
            )));
            assert!(!private_key_requires_passphrase(&format!(
                "-----BEGIN {algorithm} PRIVATE KEY-----\r\ndGVzdA==\r\n"
            )));
        }
        assert!(private_key_requires_passphrase(
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\ndGVzdA==\n-----END ENCRYPTED PRIVATE KEY-----"
        ));
        assert!(!private_key_requires_passphrase(
            "-----BEGIN PRIVATE KEY-----\ndGVzdA==\n-----END PRIVATE KEY-----"
        ));
        for version in [2, 3] {
            assert!(private_key_requires_passphrase(&format!(
                "PuTTY-User-Key-File-{version}: ssh-ed25519\r\nEncryption: aes256-cbc\r\n"
            )));
            assert!(!private_key_requires_passphrase(&format!(
                "PuTTY-User-Key-File-{version}: ssh-ed25519\r\nEncryption: none\r\n"
            )));
        }
    }

    #[test]
    fn material_names_use_wire_types_and_certificate_precedence() {
        assert_eq!(
            display_name("", "ssh-ed25519 AAAA comment", ""),
            "ED25519 key"
        );
        assert_eq!(
            display_name("-----BEGIN RSA PRIVATE KEY-----", "", ""),
            "RSA key"
        );
        assert_eq!(
            display_name(
                "",
                "ecdsa-sha2-nistp256 AAAA",
                "sk-ssh-ed25519-cert-v01@openssh.com AAAA"
            ),
            "FIDO2 ED25519 SSH Certificate"
        );
        assert_eq!(display_name("opaque private text", "", ""), "SSH key");
    }
}
