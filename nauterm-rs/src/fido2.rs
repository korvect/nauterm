//! Hardware operations run off the UI thread through the existing native API.
//! PINs are transient; generated keys use the shared Vault storage.
use crate::{native::Core, vault::KeyRecord};
use anyhow::{Result, ensure};
use serde::Serialize;
use std::sync::Arc;
use zeroize::Zeroizing;

pub mod editor;

pub use nauterm_core::fido2::Fido2Device as Device;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum KeyType {
    #[default]
    Ecdsa,
    Ed25519,
}
impl KeyType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ecdsa => "ECDSA 256",
            Self::Ed25519 => "ED25519",
        }
    }
    fn native_name(self) -> &'static str {
        match self {
            Self::Ecdsa => "ecdsa",
            Self::Ed25519 => "ed25519",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ValidationError {
    Device,
    Label,
    Pin,
    Rounds,
    Cipher,
}
impl ValidationError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Device => "Select a FIDO2 device.",
            Self::Label => "Label is required.",
            Self::Pin => "Enter the security key PIN.",
            Self::Rounds => "Rounds must be between 1 and 1000000.",
            Self::Cipher => "Unsupported private key cipher.",
        }
    }
}
impl std::fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}
impl std::error::Error for ValidationError {}

pub struct Generate {
    pub device_id: String,
    pub label: String,
    pub key_type: KeyType,
    pub pin: Zeroizing<String>,
    pub require_presence: bool,
    pub require_pin: bool,
    pub passphrase: Zeroizing<String>,
    pub cipher: &'static str,
    pub rounds: u32,
    pub save_passphrase: bool,
}
impl Generate {
    fn validate(&self) -> std::result::Result<(), ValidationError> {
        if self.device_id.is_empty() {
            return Err(ValidationError::Device);
        }
        if self.label.trim().is_empty() {
            return Err(ValidationError::Label);
        }
        if self.require_pin && self.pin.is_empty() {
            return Err(ValidationError::Pin);
        }
        if !self.passphrase.is_empty() && !(1..=1_000_000).contains(&self.rounds) {
            return Err(ValidationError::Rounds);
        }
        if !matches!(self.cipher, "aes256-ctr" | "aes128-ctr") {
            return Err(ValidationError::Cipher);
        }
        Ok(())
    }
    fn native_request(&self) -> Result<nauterm_core::fido2::Fido2GenerateRequest> {
        self.validate()?;
        Ok(nauterm_core::fido2::Fido2GenerateRequest {
            device_id: self.device_id.clone(),
            label: self.label.trim().to_owned(),
            key_type: self.key_type.native_name().to_owned(),
            pin: if self.require_pin {
                self.pin.to_string()
            } else {
                String::new()
            },
            require_user_presence: self.require_presence,
            require_user_verification: self.require_pin,
            resident: false,
            passphrase: self.passphrase.to_string(),
            cipher: self.cipher.to_owned(),
            rounds: self.rounds,
        })
    }
}
/// The reference auth ABI uses one string for two independent secrets.
/// Packing even a file passphrase alone prevents it being mistaken for a PIN.
pub(crate) fn authentication(passphrase: &str, pin: &str) -> Result<Zeroizing<String>> {
    ensure!(
        !passphrase.contains('\0') && !pin.contains('\0'),
        "SSH credential contains a NUL character"
    );
    if passphrase.is_empty() && pin.is_empty() {
        return Ok(Zeroizing::new(String::new()));
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Secrets<'a> {
        key_passphrase: &'a str,
        fido2_pin: &'a str,
    }
    let json = Zeroizing::new(serde_json::to_string(&Secrets {
        key_passphrase: passphrase,
        fido2_pin: pin,
    })?);
    Ok(Zeroizing::new(format!(
        "nauterm-auth-secrets:v1:{}",
        json.as_str()
    )))
}

pub struct Client {
    _core: Arc<Core>,
}
impl Client {
    pub(crate) fn new(core: Arc<Core>) -> Self {
        Self { _core: core }
    }
    pub fn devices(&self) -> Vec<Device> {
        nauterm_core::fido2::list_devices()
    }
    pub fn verify(&self, device: &str, pin: Zeroizing<String>) -> Result<()> {
        ensure!(!pin.is_empty(), "Enter the security key PIN.");
        nauterm_core::fido2::verify_pin(device.to_owned(), pin.to_string())
            .map_err(anyhow::Error::msg)
    }
    pub fn generate(&self, request: Generate) -> Result<KeyRecord> {
        let mut generated = nauterm_core::fido2::generate_key(request.native_request()?)
            .map_err(anyhow::Error::msg)?;
        Ok(KeyRecord {
            id: None,
            name: request.label.trim().to_owned(),
            private_key: Some(std::mem::take(&mut generated.private_key)),
            public_key: Some(std::mem::take(&mut generated.public_key)),
            passphrase: (request.save_passphrase && !request.passphrase.is_empty())
                .then(|| request.passphrase.to_string()),
            certificate: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ssh_authentication_encodes_file_passphrase_and_device_pin_independently() -> Result<()> {
        let bytes = authentication("file-pass", "device-pin")?;
        let prefix = "nauterm-auth-secrets:v1:";
        assert!(bytes.starts_with(prefix));
        let values: serde_json::Value = serde_json::from_str(&bytes[prefix.len()..])?;
        assert_eq!(values["keyPassphrase"], "file-pass");
        assert_eq!(values["fido2Pin"], "device-pin");
        let bytes = authentication("file-only", "")?;
        let values: serde_json::Value = serde_json::from_str(&bytes[prefix.len()..])?;
        assert_eq!(values["fido2Pin"], "");
        assert!(authentication("", "")?.is_empty());
        assert!(authentication("invalid\0", "").is_err());
        Ok(())
    }
    #[test]
    fn native_request_keeps_pin_separate_from_file_protection() -> Result<()> {
        let mut request = Generate {
            device_id: "synthetic-device".into(),
            label: " Test key ".into(),
            key_type: KeyType::Ecdsa,
            pin: Zeroizing::new("test-pin".into()),
            require_presence: true,
            require_pin: true,
            passphrase: Zeroizing::new("test-passphrase".into()),
            cipher: "aes256-ctr",
            rounds: 100,
            save_passphrase: true,
        };
        let native = request.native_request()?;
        assert_eq!(native.pin, "test-pin");
        assert_eq!(native.passphrase, "test-passphrase");
        assert_eq!(native.label, "Test key");
        assert_eq!(native.key_type, "ecdsa");
        assert_eq!(native.cipher, "aes256-ctr");
        assert_eq!(native.rounds, 100);
        assert!(!native.resident);
        assert!(native.require_user_presence);
        assert!(native.require_user_verification);
        request.pin.clear();
        assert_eq!(request.validate(), Err(ValidationError::Pin));
        request.require_pin = false;
        request.rounds = 0;
        assert_eq!(request.validate(), Err(ValidationError::Rounds));
        request.passphrase.clear();
        request.pin.push_str("ignored-unless-required");
        request.key_type = KeyType::Ed25519;
        request.cipher = "aes128-ctr";
        request.require_presence = false;
        let native = request.native_request()?;
        assert!(native.pin.is_empty());
        assert!(!native.require_user_verification);
        assert_eq!(native.key_type, "ed25519");
        assert_eq!(native.cipher, "aes128-ctr");
        assert!(!native.require_user_presence);
        request.cipher = "invalid-cipher";
        assert_eq!(request.validate(), Err(ValidationError::Cipher));
        request.label.clear();
        assert_eq!(request.validate(), Err(ValidationError::Label));
        request.device_id.clear();
        assert_eq!(request.validate(), Err(ValidationError::Device));
        Ok(())
    }
}
