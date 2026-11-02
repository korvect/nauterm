use anyhow::{Result, ensure};
use serde::Serialize;

pub const DEFAULT_SCRIPT: &str = r#"if test ! -e "$1"; then
  mkdir -p "$1"
  chmod 700 "$1"
fi
if test ! -e "$1/$2"; then
  touch "$1/$2"
  chmod 600 "$1/$2"
fi
printf '%s\n' "$3" >> "$1/$2"
"#;

/// Public-only payload for an explicitly selected, authenticated SSH session.
/// No host credentials or private key can be serialized through this request.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub public_key: String,
    pub location: String,
    pub filename: String,
    pub script: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormErrors {
    pub location: Option<&'static str>,
    pub filename: Option<&'static str>,
    pub script: Option<&'static str>,
}
impl FormErrors {
    pub fn is_empty(self) -> bool {
        self.location.is_none() && self.filename.is_none() && self.script.is_none()
    }
}

pub fn location_error(value: &str) -> Option<&'static str> {
    if value.trim().is_empty() {
        Some("Location is required.")
    } else if value.len() > 4096 || value.contains(['\0', '\r', '\n']) {
        Some("Location must be a single line of at most 4096 bytes.")
    } else {
        None
    }
}
pub fn filename_error(value: &str) -> Option<&'static str> {
    let value = value.trim();
    (value.is_empty()
        || [".", ".."].contains(&value)
        || value.len() > 255
        || value.contains(['\0', '\r', '\n', '/', '\\']))
    .then_some("Enter a valid filename.")
}
pub fn script_error(value: &str) -> Option<&'static str> {
    if value.trim().is_empty() {
        Some("Script is required.")
    } else if value.len() > 1024 * 1024 || value.contains(['\0', '\r']) {
        Some("A valid UNIX script is required.")
    } else {
        None
    }
}

impl ExportRequest {
    fn validate_public_key(&self) -> Result<()> {
        ensure!(
            self.public_key.len() <= 64 * 1024,
            "Public key is too large."
        );
        ensure!(
            self.public_key
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count()
                == 1,
            "A single valid OpenSSH public key is required."
        );
        ensure!(
            ssh_key::PublicKey::from_openssh(self.public_key.trim()).is_ok(),
            "A valid OpenSSH public key is required."
        );
        Ok(())
    }
    pub fn form_errors(&self) -> FormErrors {
        FormErrors {
            location: location_error(&self.location),
            filename: filename_error(&self.filename),
            script: script_error(&self.script),
        }
    }
    pub fn validate(&self) -> Result<()> {
        self.validate_public_key()?;
        let errors = self.form_errors();
        if let Some(error) = errors.location.or(errors.filename).or(errors.script) {
            anyhow::bail!(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ExportRequest {
        let key = ssh_key::PrivateKey::from(ssh_key::private::Ed25519Keypair::from_seed(&[17; 32]));
        ExportRequest {
            public_key: key.public_key().to_openssh().unwrap(),
            location: ".ssh".into(),
            filename: "authorized_keys".into(),
            script: DEFAULT_SCRIPT.into(),
        }
    }
    #[test]
    fn public_only_request_matches_reference_script_and_wire_fields() {
        let request = request();
        request.validate().unwrap();
        let source = include_str!("../../lib/workspace/workspace_editor_key_export.dart");
        assert!(
            source.contains(DEFAULT_SCRIPT),
            "Reconcile the reference default script before changing it"
        );
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 4);
        assert!(json.get("publicKey").is_some());
        assert!(json.get("password").is_none());
        assert!(json.get("privateKey").is_none());
    }
    #[test]
    fn rejects_invalid_or_multiple_keys_and_path_control_characters() {
        for value in [
            "",
            "ssh-ed25519 not-base64",
            "ssh-ed25519 AAAA\nssh-ed25519 AAAA",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
        ] {
            let mut request = request();
            request.public_key = value.into();
            assert!(request.validate().is_err());
        }
        for value in [
            "",
            ".",
            "..",
            "../authorized_keys",
            "a\\b",
            "a\nb",
            "a\rb",
            "a\0b",
        ] {
            let mut request = request();
            request.filename = value.into();
            assert!(request.validate().is_err());
        }
        for value in ["", "a\nb", "a\rb", "a\0b"] {
            let mut request = request();
            request.location = value.into();
            assert!(request.validate().is_err());
        }
        for value in ["", "exit 0\r", "exit 0\0"] {
            let mut request = request();
            request.script = value.into();
            assert!(request.validate().is_err());
        }
    }
    #[test]
    fn export_form_reports_all_fields_and_clears_each_independently() {
        let mut request = request();
        request.location.clear();
        request.filename = "../authorized_keys".into();
        request.script.clear();
        let errors = request.form_errors();
        assert!(errors.location.is_some());
        assert!(errors.filename.is_some());
        assert!(errors.script.is_some());
        assert!(!errors.is_empty());
        request.location = ".ssh".into();
        let errors = request.form_errors();
        assert!(errors.location.is_none());
        assert!(errors.filename.is_some());
        assert!(errors.script.is_some());
        request.filename = "authorized_keys".into();
        request.script = DEFAULT_SCRIPT.into();
        assert!(request.form_errors().is_empty());
        request.validate().unwrap();
    }
}
