//! Immutable process identity for application-owned paths and OS credentials.
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Client {
    Nauterm,
    NautermRs,
}
impl Client {
    pub const fn keyring_service(self) -> &'static str {
        match self {
            Self::Nauterm => "com.korvect.nauterm",
            Self::NautermRs => "com.korvect.nauterm.rs",
        }
    }
}

static CLIENT: OnceLock<Client> = OnceLock::new();

/// Select before resolving application paths or loading any encryption key.
/// Flutter's C ABI defaults to Nauterm; RS selects its identity at startup.
pub fn configure(client: Client) -> Result<(), &'static str> {
    configure_selection(&CLIENT, client)
}

fn configure_selection(selection: &OnceLock<Client>, client: Client) -> Result<(), &'static str> {
    if selection.set(client).is_err() && selection.get() != Some(&client) {
        return Err("Application data identity is already in use.");
    }
    Ok(())
}

pub(crate) fn current() -> Client {
    *CLIENT.get_or_init(|| Client::Nauterm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_namespaces_are_separate() {
        assert_eq!(Client::Nauterm.keyring_service(), "com.korvect.nauterm");
        assert_eq!(
            Client::NautermRs.keyring_service(),
            "com.korvect.nauterm.rs"
        );
        assert_ne!(
            Client::Nauterm.keyring_service(),
            Client::NautermRs.keyring_service()
        );
    }

    #[test]
    fn identity_cannot_change_after_selection_or_default_resolution() {
        let selected = OnceLock::new();
        configure_selection(&selected, Client::NautermRs).unwrap();
        configure_selection(&selected, Client::NautermRs).unwrap();
        assert!(configure_selection(&selected, Client::Nauterm).is_err());
        let default = OnceLock::new();
        default.get_or_init(|| Client::Nauterm);
        assert!(configure_selection(&default, Client::NautermRs).is_err());
        assert_eq!(default.get(), Some(&Client::Nauterm));
    }
}
