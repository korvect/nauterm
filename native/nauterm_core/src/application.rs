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

    #[cfg(all(
        windows,
        feature = "integration-tests",
        not(feature = "test-credential-store")
    ))]
    #[test]
    fn windows_credential_store_keeps_client_secrets_independent(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let account = format!(
            "nauterm-validation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        );
        let flutter = keyring::Entry::new(Client::Nauterm.keyring_service(), &account)?;
        let rs = keyring::Entry::new(Client::NautermRs.keyring_service(), &account)?;
        for entry in [&flutter, &rs] {
            assert!(matches!(entry.get_password(), Err(keyring::Error::NoEntry)));
        }
        struct FixtureCredentials([keyring::Entry; 2]);
        impl Drop for FixtureCredentials {
            fn drop(&mut self) {
                for entry in &self.0 {
                    let _ = entry.delete_credential();
                }
            }
        }
        let entries = FixtureCredentials([flutter, rs]);
        entries.0[0].set_password("flutter-validation-value")?;
        entries.0[1].set_password("rs-validation-value")?;
        assert_eq!(entries.0[0].get_password()?, "flutter-validation-value");
        assert_eq!(entries.0[1].get_password()?, "rs-validation-value");
        entries.0[1].delete_credential()?;
        assert!(matches!(
            entries.0[1].get_password(),
            Err(keyring::Error::NoEntry)
        ));
        assert_eq!(entries.0[0].get_password()?, "flutter-validation-value");
        Ok(())
    }
}
