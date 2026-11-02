//! Host names follow the reference form until the user supplies their own name.
use super::*;
use crate::vault::Inventory;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Endpoint {
    username: String,
    port: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Inputs {
    kind: HostKind,
    address: String,
    ssh: Option<Endpoint>,
    telnet: Option<Endpoint>,
}
impl Inputs {
    fn name(&self) -> String {
        let address = self.address.trim();
        if self.kind != HostKind::Remote || address.is_empty() {
            return String::new();
        }
        let Some(endpoint) = self.ssh.as_ref().or(self.telnet.as_ref()) else {
            return String::new();
        };
        let username = endpoint.username.trim();
        let port = endpoint.port.trim();
        let target = if username.is_empty() {
            address.to_owned()
        } else {
            format!("{username}@{address}")
        };
        if port.is_empty() {
            target
        } else {
            format!("{target}:{port}")
        }
    }
}
struct Automatic {
    automatic: bool,
    observed: String,
    inputs: Option<Inputs>,
    refresh: bool,
}
impl Automatic {
    fn new(name: String) -> Self {
        Self {
            automatic: name.trim().is_empty(),
            observed: name,
            inputs: None,
            refresh: false,
        }
    }
    fn update(&mut self, name: &str, inputs: Inputs, saving: bool) -> Option<String> {
        // Programmatic changes may emit Change after this call returns. Keep
        // their expected value so they cannot disable automatic naming.
        if name != self.observed {
            self.automatic = name.trim().is_empty();
            self.observed = name.to_owned();
        }
        let refresh = saving || self.refresh || self.inputs.as_ref() != Some(&inputs);
        self.refresh = false;
        let suggested = (self.automatic && refresh).then(|| inputs.name());
        self.inputs = Some(inputs);
        let suggested = suggested?;
        if name == suggested {
            return None;
        }
        self.observed = suggested.clone();
        Some(suggested)
    }
}
fn username(reference: Option<CredentialRef>, typed: &str, inventory: &Inventory) -> String {
    if let Some(CredentialRef::Identity(id)) = reference
        && let Some(identity) = inventory
            .identities
            .iter()
            .find(|identity| identity.id == id)
    {
        identity.username.clone()
    } else {
        typed.to_owned()
    }
}
pub(super) struct Draft {
    state: Automatic,
    _events: Vec<Subscription>,
}
impl Draft {
    pub fn new(editor: &Editor, cx: &mut Context<Nauterm>) -> Self {
        let name = &editor.fields[0].1;
        let token = name.entity_id();
        let host = editor.host.as_ref().unwrap();
        let inputs = [
            name,
            &editor.fields[1].1,
            &editor.fields[2].1,
            &editor.fields[3].1,
            &host.telnet.port,
            &host.telnet.username,
        ];
        let events = inputs
            .into_iter()
            .map(|input| {
                cx.subscribe(input, move |this, source, event: &InputEvent, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let Some(draft) = this
                        .editor
                        .as_mut()
                        .and_then(|editor| {
                            editor.find_mut(&|editor| {
                                editor.kind == ItemKind::Host
                                    && editor.fields[0].1.entity_id() == token
                            })
                        })
                        .and_then(|editor| editor.host.as_mut())
                        .and_then(|host| host.automatic_name.as_mut())
                    else {
                        return;
                    };
                    if source.entity_id() != token {
                        draft.state.refresh = true;
                    }
                    cx.notify();
                })
            })
            .collect();
        Self {
            state: Automatic::new(name.read(cx).value().to_string()),
            _events: events,
        }
    }
}
impl Nauterm {
    pub(super) fn refresh_host_name(
        &mut self,
        saving: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = self
            .editor
            .as_mut()
            .filter(|editor| editor.kind == ItemKind::Host)
        else {
            return;
        };
        let host = editor.host.as_mut().unwrap();
        if host.busy {
            return;
        }
        let Some(automatic) = &mut host.automatic_name else {
            return;
        };
        let empty = Inventory::default();
        let inventory = self
            .keychain
            .as_ref()
            .map(|keychain| keychain.read(cx).inventory_ref())
            .unwrap_or(&empty);
        let inputs = Inputs {
            kind: host.kind,
            address: editor.fields[1].1.read(cx).value().to_string(),
            ssh: host.ssh_enabled.then(|| Endpoint {
                username: username(
                    editor.credential,
                    &editor.fields[3].1.read(cx).value(),
                    inventory,
                ),
                port: editor.fields[2].1.read(cx).value().to_string(),
            }),
            telnet: host.telnet.enabled.then(|| Endpoint {
                username: username(
                    host.telnet.profile.identity_id.map(CredentialRef::Identity),
                    &host.telnet.username.read(cx).value(),
                    inventory,
                ),
                port: host.telnet.port.read(cx).value().to_string(),
            }),
        };
        let current = editor.fields[0].1.read(cx).value().to_string();
        let next = automatic.state.update(&current, inputs, saving);
        let has_name = !next.as_deref().unwrap_or(&current).trim().is_empty();
        if let Some(name) = next {
            editor.fields[0]
                .1
                .update(cx, |input, cx| input.set_value(name, window, cx));
        }
        if has_name
            && editor
                .error
                .as_ref()
                .is_some_and(|error| error.starts_with("Name is required"))
        {
            editor.error = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Automatic, Endpoint, Inputs, username};
    use crate::{
        model::{CredentialRef, HostKind},
        vault::{IdentitySummary, Inventory},
    };
    fn remote() -> Inputs {
        Inputs {
            kind: HostKind::Remote,
            address: " example.test ".into(),
            ssh: Some(Endpoint {
                username: " admin ".into(),
                port: " 22 ".into(),
            }),
            telnet: Some(Endpoint {
                username: "guest".into(),
                port: "23".into(),
            }),
        }
    }
    #[test]
    fn protocol_order_and_local_names_match_the_reference() {
        let mut inputs = remote();
        assert_eq!(inputs.name(), "admin@example.test:22");
        inputs.ssh = None;
        assert_eq!(inputs.name(), "guest@example.test:23");
        inputs.telnet.as_mut().unwrap().username.clear();
        inputs.telnet.as_mut().unwrap().port.clear();
        assert_eq!(inputs.name(), "example.test");
        inputs.kind = HostKind::Local;
        assert_eq!(inputs.name(), "");
        inputs.kind = HostKind::Remote;
        inputs.address.clear();
        assert_eq!(inputs.name(), "");
        inputs.address = "example.test".into();
        inputs.telnet = None;
        assert_eq!(inputs.name(), "");
    }
    #[test]
    fn manual_names_stop_following_and_clearing_restores_save_fallback() {
        let mut state = Automatic::new(String::new());
        let mut inputs = remote();
        let generated = state.update("", inputs.clone(), false).unwrap();
        assert_eq!(state.update(&generated, inputs.clone(), false), None);
        inputs.ssh.as_mut().unwrap().port = "2022".into();
        assert_eq!(
            state.update(&generated, inputs.clone(), false).as_deref(),
            Some("admin@example.test:2022")
        );
        assert_eq!(state.update("Production", inputs.clone(), false), None);
        inputs.address = "other.test".into();
        assert_eq!(state.update("Production", inputs.clone(), false), None);
        assert_eq!(state.update("", inputs.clone(), false), None);
        assert_eq!(
            state.update("", inputs.clone(), true).as_deref(),
            Some("admin@other.test:2022")
        );
        let mut existing = Automatic::new("Saved name".into());
        assert_eq!(existing.update("Saved name", inputs, false), None);
    }
    #[test]
    fn identity_names_use_the_login_username_and_manual_keys_keep_typed_text() {
        let inventory = Inventory {
            identities: vec![IdentitySummary {
                id: 4,
                name: "Production login".into(),
                username: "root".into(),
                key_id: None,
            }],
            ..Default::default()
        };
        assert_eq!(
            username(
                Some(CredentialRef::Identity(4)),
                "Production login",
                &inventory
            ),
            "root"
        );
        assert_eq!(
            username(Some(CredentialRef::Key(4)), "admin", &inventory),
            "admin"
        );
        assert_eq!(username(None, "guest", &inventory), "guest");
        let mut state = Automatic::new(String::new());
        let inputs = remote();
        let generated = state.update("", inputs.clone(), false).unwrap();
        assert_eq!(state.update("", inputs.clone(), false), None);
        state.refresh = true;
        assert_eq!(state.update("", inputs, false), Some(generated));
    }
}
