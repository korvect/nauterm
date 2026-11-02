use super::{Editor, Keychain, Kind};
use gpui_kit::component::Disableable;
use gpui_kit::{AnyElement, App, Context, IntoElement, Window};

#[derive(Clone, Copy, PartialEq, Eq)]
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
    use super::display_name;
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
