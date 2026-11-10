use super::*;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use russh::keys::ssh_key::{PrivateKey, PublicKey};
use std::sync::Arc;

use crate::fido2::{self, Fido2SshAlgorithm};

fn push_auth_event(
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    kind: &str,
    method: &str,
    message: impl Into<String>,
) {
    push_event(
        events,
        wakeup,
        SessionEvent::new(kind, message).with_method(method),
    );
}

fn push_auth_method_event(
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    kind: &str,
    method: &str,
    message: impl Into<String>,
    methods: &russh::MethodSet,
) {
    let mut event = SessionEvent::new(kind, message).with_method(method);
    event.authentication_methods = Some(
        methods
            .iter()
            .map(|method| <&str>::from(method).to_owned())
            .collect(),
    );
    push_event(events, wakeup, event);
}

type DynamicAgentClient = AgentClient<Box<dyn AgentStream + Send + Unpin + 'static>>;

const OPENSSH_CERTIFICATE_SUFFIX: &str = "-cert-v01@openssh.com";
const SSH_SK_USER_PRESENCE_REQUIRED: u8 = 0x01;
const SSH_SK_USER_VERIFICATION_REQUIRED: u8 = 0x04;
const AUTH_SECRETS_PREFIX: &str = "nauterm-auth-secrets:v1:";

struct AuthenticationSecrets {
    key_passphrase: Option<String>,
    fido2_pin: Option<String>,
    packed: bool,
}

fn authentication_secrets(passphrase: Option<&str>) -> AuthenticationSecrets {
    let Some(value) = passphrase else {
        return AuthenticationSecrets {
            key_passphrase: None,
            fido2_pin: None,
            packed: false,
        };
    };
    let Some(json) = value.strip_prefix(AUTH_SECRETS_PREFIX) else {
        return AuthenticationSecrets {
            key_passphrase: Some(value.to_owned()),
            fido2_pin: None,
            packed: false,
        };
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return AuthenticationSecrets {
            key_passphrase: Some(value.to_owned()),
            fido2_pin: None,
            packed: false,
        };
    };
    let string = |field: &str| {
        value
            .get(field)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    AuthenticationSecrets {
        key_passphrase: string("keyPassphrase"),
        fido2_pin: string("fido2Pin"),
        packed: true,
    }
}

// Unknown hosts may receive unsigned public-key probes. All authentication
// paths must pass this gate before signing, sending a password, or opening a
// session, including servers which accept "none" authentication.
#[derive(Clone, Default)]
pub(crate) struct HostKeyVerification {
    pending: Arc<Mutex<Option<SessionEvent>>>,
    host_key_seen: Arc<AtomicBool>,
    interaction: Option<AuthenticationInteraction>,
}

impl HostKeyVerification {
    pub(super) fn with_interaction(interaction: AuthenticationInteraction) -> Self {
        Self {
            interaction: Some(interaction),
            ..Self::default()
        }
    }

    pub(super) fn begin_host_key_check(&self) -> bool {
        !self.host_key_seen.swap(true, Ordering::AcqRel)
    }
    pub(super) fn defer(&self, event: SessionEvent) {
        *self.pending.lock().unwrap() = Some(event);
    }

    fn require_trusted(
        &self,
        events: &Arc<Mutex<Vec<SessionEvent>>>,
        wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    ) -> Result<(), String> {
        let pending = self.pending.lock().unwrap().as_ref().cloned();
        if let Some(event) = pending {
            push_event(events, wakeup, event);
            return Err("host key confirmation is required".to_owned());
        }
        Ok(())
    }
}

pub(crate) struct PreparedPrivateKey<'a> {
    encoded: &'a str,
    public_key: Option<PublicKey>,
}

pub(crate) fn prepare_private_key<'a>(
    encoded: Option<&'a str>,
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
) -> Result<Option<PreparedPrivateKey<'a>>, String> {
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    // OpenSSH retains the public key outside the encrypted private section.
    // Reading it must not perform a KDF or require a passphrase.
    if let Ok(key) = PrivateKey::from_openssh(encoded) {
        return Ok(Some(PreparedPrivateKey {
            encoded,
            public_key: Some(key.public_key().clone()),
        }));
    }
    match decode_secret_key(encoded, None) {
        Ok(key) => Ok(Some(PreparedPrivateKey {
            encoded,
            public_key: Some(key.public_key().clone()),
        })),
        Err(error) if encrypted_key(encoded, &error) => {
            let public_key = if encoded.trim_start().starts_with("PuTTY-User-Key-File-") {
                let mut lines = encoded.lines();
                let count = lines.find_map(|line| {
                    line.strip_prefix("Public-Lines: ")?
                        .trim()
                        .parse::<usize>()
                        .ok()
                });
                count.and_then(|count| {
                    let body = lines.by_ref().take(count).collect::<String>();
                    let bytes = BASE64_STANDARD.decode(body).ok()?;
                    PublicKey::from_bytes(&bytes).ok()
                })
            } else {
                None // Encrypted PKCS#8/PKCS#5 does not expose the public key.
            };
            Ok(Some(PreparedPrivateKey {
                encoded,
                public_key,
            }))
        }
        Err(error) => {
            push_auth_event(
                events,
                wakeup,
                "auth_key_failed",
                "public_key",
                format!("Failed to decode configured private key: {error}"),
            );
            Err(format!("failed to decode private key: {error}"))
        }
    }
}

fn encrypted_key(encoded: &str, error: &KeyError) -> bool {
    matches!(error, KeyError::KeyIsEncrypted)
        || encoded.lines().any(|line| {
            line == "-----BEGIN ENCRYPTED PRIVATE KEY-----" || line == "Encryption: aes256-cbc"
        })
}

fn unlock_private_key(
    encoded: &str,
    passphrase: Option<&str>,
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
) -> Result<PrivateKey, String> {
    let error = match decode_secret_key(encoded, None) {
        Ok(key) => return Ok(key),
        Err(error) => error,
    };
    if encrypted_key(encoded, &error) {
        let message = if let Some(passphrase) = passphrase {
            if let Ok(key) = decode_secret_key(encoded, Some(passphrase)) {
                return Ok(key);
            }
            "Unable to unlock the configured private key. Check its passphrase and try again."
        } else {
            "Configured private key requires a passphrase."
        };
        push_auth_event(
            events,
            wakeup,
            "auth_passphrase_required",
            "public_key",
            message,
        );
        return Err(message.to_owned());
    }
    push_auth_event(
        events,
        wakeup,
        "auth_key_failed",
        "public_key",
        format!("Failed to decode configured private key: {error}"),
    );
    Err(format!("failed to decode private key: {error}"))
}

#[derive(Debug)]
struct Fido2SignerError(String);

impl std::fmt::Display for Fido2SignerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Fido2SignerError {}

impl From<russh::SendError> for Fido2SignerError {
    fn from(_: russh::SendError) -> Self {
        Self("SSH signing request was cancelled.".to_owned())
    }
}

struct Fido2Signer {
    application: String,
    key_handle: Vec<u8>,
    algorithm: Fido2SshAlgorithm,
    pin: Option<String>,
    require_user_presence: bool,
}

impl russh::Signer for Fido2Signer {
    type Error = Fido2SignerError;

    fn auth_sign(
        &mut self,
        _key: &russh::keys::agent::AgentIdentity,
        _hash_alg: Option<HashAlg>,
        mut to_sign: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, Self::Error>> + Send {
        let application = self.application.clone();
        let key_handle = self.key_handle.clone();
        let algorithm = self.algorithm;
        let pin = self.pin.clone();
        let require_user_presence = self.require_user_presence;
        let message = to_sign.clone();
        async move {
            let signature = tokio::task::spawn_blocking(move || {
                fido2::sign_ssh(
                    &application,
                    &key_handle,
                    algorithm,
                    &message,
                    pin.as_deref(),
                    require_user_presence,
                )
                .map_err(Fido2SignerError)
            })
            .await
            .map_err(|error| Fido2SignerError(error.to_string()))??;
            to_sign.extend_from_slice(&signature);
            Ok(to_sign)
        }
    }
}

#[derive(Debug)]
enum KeySignError {
    Intervention(String),
    Failed(String),
}

impl std::fmt::Display for KeySignError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Intervention(message) | Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for KeySignError {}

impl From<russh::SendError> for KeySignError {
    fn from(_: russh::SendError) -> Self {
        Self::Failed("SSH signing request was cancelled.".to_owned())
    }
}

struct ConfiguredKeySigner<'a> {
    key: PreparedPrivateKey<'a>,
    public_key: PublicKey,
    unlocked: Option<PrivateKey>,
    secrets: AuthenticationSecrets,
    events: &'a Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &'a Arc<Mutex<Option<WakeupCallback>>>,
    host_key_verification: &'a HostKeyVerification,
}

impl ConfiguredKeySigner<'_> {
    async fn request_unlock(&mut self, message: String) -> Result<(), KeySignError> {
        let Some(interaction) = &self.host_key_verification.interaction else {
            return Err(KeySignError::Intervention(message));
        };
        let intervention = self
            .events
            .lock()
            .unwrap()
            .last()
            .filter(|event| event.kind == "auth_passphrase_required")
            .cloned()
            .ok_or(KeySignError::Failed(message))?;
        // Keep the pending signing request alive: restarting authentication here
        // would discard a previously accepted password or public-key factor.
        let credentials = interaction
            .request_key_unlock(intervention, self.events, self.wakeup)
            .await
            .map_err(KeySignError::Failed)?;
        if credentials.private_key.as_deref() != Some(self.key.encoded) {
            return Err(KeySignError::Failed(
                "Cannot change a key while its signature is pending.".to_owned(),
            ));
        }
        self.secrets = authentication_secrets(credentials.passphrase.as_deref());
        Ok(())
    }
}

impl russh::Signer for ConfiguredKeySigner<'_> {
    type Error = KeySignError;

    async fn auth_sign(
        &mut self,
        identity: &russh::keys::agent::AgentIdentity,
        hash_alg: Option<HashAlg>,
        mut to_sign: Vec<u8>,
    ) -> Result<Vec<u8>, Self::Error> {
        use russh::keys::{signature::Signer as _, ssh_encoding::Encode as _};

        // russh calls this only after USERAUTH_PK_OK. A rejected public key
        // never prompts for its passphrase or accesses a security key.
        let key = if let Some(key) = self.unlocked.take() {
            key
        } else {
            loop {
                match unlock_private_key(
                    self.key.encoded,
                    self.secrets.key_passphrase.as_deref(),
                    self.events,
                    self.wakeup,
                ) {
                    Ok(key) => break key,
                    Err(message) => self.request_unlock(message).await?,
                }
            }
        };
        if key.public_key().key_data() != self.public_key.key_data() {
            return Err(KeySignError::Failed(
                "Private key does not match the offered public key.".to_owned(),
            ));
        }
        self.host_key_verification
            .require_trusted(self.events, self.wakeup)
            .map_err(KeySignError::Intervention)?;

        let descriptor = key
            .key_data()
            .sk_ecdsa_p256()
            .map(|key| {
                (
                    key.public().application().to_owned(),
                    key.key_handle().to_vec(),
                    key.flags(),
                    Fido2SshAlgorithm::Ecdsa,
                )
            })
            .or_else(|| {
                key.key_data().sk_ed25519().map(|key| {
                    (
                        key.public().application().to_owned(),
                        key.key_handle().to_vec(),
                        key.flags(),
                        Fido2SshAlgorithm::Ed25519,
                    )
                })
            });
        if let Some((application, key_handle, flags, algorithm)) = descriptor {
            while flags & SSH_SK_USER_VERIFICATION_REQUIRED != 0
                && if self.secrets.packed {
                    self.secrets.fido2_pin.is_none()
                } else {
                    self.secrets.key_passphrase.is_none()
                }
            {
                let message = "FIDO2 security key requires its PIN.";
                push_auth_event(
                    self.events,
                    self.wakeup,
                    "auth_passphrase_required",
                    "fido2",
                    message,
                );
                self.request_unlock(message.to_owned()).await?;
            }
            let pin = if self.secrets.packed {
                self.secrets.fido2_pin.as_deref()
            } else {
                self.secrets.key_passphrase.as_deref()
            };
            let require_user_presence = flags & SSH_SK_USER_PRESENCE_REQUIRED != 0;
            push_auth_event(
                self.events,
                self.wakeup,
                "auth_fido2_start",
                "fido2",
                if require_user_presence {
                    "Touch or confirm the FIDO2 security key."
                } else {
                    "Signing with the FIDO2 security key."
                },
            );
            let mut signer = Fido2Signer {
                application,
                key_handle,
                algorithm,
                pin: pin.map(str::to_owned),
                require_user_presence,
            };
            return russh::Signer::auth_sign(&mut signer, identity, hash_alg, to_sign)
                .await
                .map_err(|error| KeySignError::Failed(error.to_string()));
        }

        let signature: russh::keys::ssh_key::Signature = match key.key_data().rsa() {
            Some(rsa) => (rsa, hash_alg).try_sign(&to_sign),
            None => key.try_sign(&to_sign),
        }
        .map_err(|error| KeySignError::Failed(error.to_string()))?;
        let mut encoded = Vec::new();
        signature
            .encode(&mut encoded)
            .map_err(|error| KeySignError::Failed(error.to_string()))?;
        encoded
            .encode(&mut to_sign)
            .map_err(|error| KeySignError::Failed(error.to_string()))?;
        Ok(to_sign)
    }
}

fn decode_openssh_certificate(encoded: &str) -> Result<russh::keys::ssh_key::Certificate, String> {
    use russh::keys::ssh_key::Certificate;

    let trimmed = encoded.trim().trim_start_matches('\u{feff}');
    let direct_error = match Certificate::from_openssh(trimmed) {
        Ok(certificate) => return Ok(certificate),
        Err(error) => error,
    };

    // OpenSSH writes certificates on one line, but text fields, clipboard
    // managers, and RFC4716-style exports can replace the separating space or
    // wrap the Base64 payload. Comments are not needed for authentication, so
    // rebuild the canonical two-field representation and stop as soon as a
    // complete certificate can be decoded.
    let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        let algorithm = token.trim_start_matches('\u{feff}');
        if !algorithm.ends_with(OPENSSH_CERTIFICATE_SUFFIX) {
            continue;
        }

        let mut base64_data = String::new();
        for token in &tokens[index + 1..] {
            if !token.bytes().all(is_base64_character) {
                break;
            }
            base64_data.push_str(token);
            let candidate = format!("{algorithm} {base64_data}");
            if let Ok(certificate) = Certificate::from_openssh(&candidate) {
                return Ok(certificate);
            }
        }
    }

    // Some importers expose only the Base64 body or use RFC4716 markers. The
    // certificate's algorithm is encoded inside the blob, so parsing the raw
    // SSH bytes is sufficient and does not weaken the private-key match check.
    let mut raw_base64 = String::new();
    let mut raw_base64_only = true;
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("----")
            || line.to_ascii_lowercase().starts_with("comment:")
        {
            continue;
        }
        for token in line.split_whitespace() {
            if token.bytes().all(is_base64_character) {
                raw_base64.push_str(token);
            } else {
                raw_base64_only = false;
                break;
            }
        }
        if !raw_base64_only {
            break;
        }
    }
    if raw_base64_only && !raw_base64.is_empty() {
        if let Ok(bytes) = BASE64_STANDARD.decode(&raw_base64) {
            if let Ok(certificate) = Certificate::from_bytes(&bytes) {
                return Ok(certificate);
            }
        }
    }

    Err(format!(
        "failed to decode OpenSSH certificate: expected a complete `*-cert-v01@openssh.com <base64-data>` certificate ({direct_error})"
    ))
}

fn is_base64_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
}

pub(crate) fn openssh_certificate_type(encoded: &str) -> Option<String> {
    if let Ok(certificate) = decode_openssh_certificate(encoded) {
        return Some(certificate.algorithm().to_certificate_type());
    }

    // Keep a useful type hint for a certificate that has not been validated
    // yet. Authentication still performs the full decode and key-match check.
    encoded
        .split_whitespace()
        .map(|token| token.trim_start_matches('\u{feff}'))
        .find(|token| token.ends_with(OPENSSH_CERTIFICATE_SUFFIX))
        .map(str::to_owned)
}

fn decode_openssh_certificate_for_key(
    encoded: &str,
    public_key: &PublicKey,
) -> Result<russh::keys::ssh_key::Certificate, String> {
    let certificate = decode_openssh_certificate(encoded)?;
    if certificate.public_key() != public_key.key_data() {
        return Err("OpenSSH certificate does not match the configured private key".to_owned());
    }
    Ok(certificate)
}

#[cfg(unix)]
async fn connect_agent_client() -> Result<DynamicAgentClient, String> {
    AgentClient::connect_env()
        .await
        .map(|agent| agent.dynamic())
        .map_err(|error| error.to_string())
}

#[cfg(windows)]
async fn connect_agent_client() -> Result<DynamicAgentClient, String> {
    if let Ok(agent) = AgentClient::connect_pageant().await {
        return Ok(agent.dynamic());
    }

    let pipe = std::env::var("SSH_AUTH_SOCK")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .unwrap_or_else(|| r"\\.\pipe\openssh-ssh-agent".to_owned());
    AgentClient::<tokio::net::windows::named_pipe::NamedPipeClient>::connect_named_pipe(
        std::ffi::OsString::from(pipe),
    )
    .await
    .map(|agent| agent.dynamic())
    .map_err(|error| error.to_string())
}

pub(crate) async fn authenticate<H: client::Handler>(
    handle: &mut client::Handle<H>,
    username: &str,
    private_key: Option<PreparedPrivateKey<'_>>,
    certificate: Option<&str>,
    passphrase: Option<&str>,
    password: Option<&str>,
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    host_key_verification: &HostKeyVerification,
) -> Result<(), String> {
    push_auth_event(
        events,
        wakeup,
        "auth_none_start",
        "none",
        "Trying SSH none authentication.",
    );
    let mut remaining_methods = match handle.authenticate_none(username.to_owned()).await {
        Ok(AuthResult::Success) => {
            host_key_verification.require_trusted(events, wakeup)?;
            push_auth_event(
                events,
                wakeup,
                "auth_success",
                "none",
                "SSH none authentication succeeded.",
            );
            return Ok(());
        }
        Ok(AuthResult::Failure {
            remaining_methods, ..
        }) => {
            push_auth_method_event(
                events,
                wakeup,
                "auth_none_rejected",
                "none",
                "SSH none authentication was rejected.",
                &remaining_methods,
            );
            remaining_methods
        }
        Err(error) => {
            push_auth_event(
                events,
                wakeup,
                "auth_none_failed",
                "none",
                format!("SSH none authentication failed: {error}"),
            );
            return Err(error.to_string());
        }
    };
    if certificate.is_some() && private_key.is_none() {
        let error = "OpenSSH certificate authentication requires a private key".to_owned();
        push_auth_event(
            events,
            wakeup,
            "auth_certificate_failed",
            "certificate",
            error.clone(),
        );
        return Err(error);
    }
    let mut credentials = AuthenticationCredentials {
        method: String::new(),
        private_key: private_key.map(|key| key.encoded.to_owned()),
        certificate: certificate.map(str::to_owned),
        passphrase: passphrase.map(str::to_owned),
        password: password.map(str::to_owned),
    };
    let mut agent_attempted = false;
    let mut partially_authenticated = false;
    loop {
        if remaining_methods.contains(&russh::MethodKind::PublicKey) {
            if let Some(encoded) = credentials.private_key.take() {
                let prepared = prepare_private_key(Some(&encoded), events, wakeup)?.unwrap();
                let result = authenticate_configured_key(
                    handle,
                    username,
                    prepared,
                    credentials.certificate.as_deref(),
                    credentials.passphrase.as_deref(),
                    events,
                    wakeup,
                    host_key_verification,
                )
                .await;
                match result {
                    Ok(AuthResult::Success) => return Ok(()),
                    Ok(AuthResult::Failure {
                        remaining_methods: methods,
                        partial_success,
                    }) => {
                        remaining_methods = methods;
                        partially_authenticated |= partial_success;
                    }
                    Err(error) => {
                        let intervention = events
                            .lock()
                            .unwrap()
                            .last()
                            .filter(|event| event.kind == "auth_passphrase_required")
                            .cloned();
                        if let (Some(interaction), Some(intervention)) =
                            (&host_key_verification.interaction, intervention)
                        {
                            credentials = interaction
                                .request_credentials(
                                    handle,
                                    &remaining_methods,
                                    Some(intervention),
                                    events,
                                    wakeup,
                                )
                                .await?;
                        } else {
                            return Err(error);
                        }
                    }
                }
                continue;
            }
        }
        host_key_verification.require_trusted(events, wakeup)?;
        if remaining_methods.contains(&russh::MethodKind::Password) {
            if let Some(password) = credentials.password.take() {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_password_start",
                    "password",
                    "Trying password authentication.",
                );
                match handle
                    .authenticate_password(username.to_owned(), password)
                    .await
                {
                    Ok(AuthResult::Success) => {
                        push_auth_event(
                            events,
                            wakeup,
                            "auth_success",
                            "password",
                            "Password authentication succeeded.",
                        );
                        return Ok(());
                    }
                    Ok(AuthResult::Failure {
                        remaining_methods: methods,
                        partial_success,
                    }) => {
                        push_auth_method_event(
                            events,
                            wakeup,
                            if partial_success {
                                "auth_partial_success"
                            } else {
                                "auth_password_rejected"
                            },
                            "password",
                            if partial_success {
                                "Password accepted; further authentication is required."
                            } else {
                                "Password authentication was rejected."
                            },
                            &methods,
                        );
                        remaining_methods = methods;
                        partially_authenticated |= partial_success;
                    }
                    Err(error) => {
                        push_auth_event(
                            events,
                            wakeup,
                            "auth_password_failed",
                            "password",
                            format!("Password authentication failed: {error}"),
                        );
                        return Err(error.to_string());
                    }
                }
                continue;
            }
        }
        if !agent_attempted && remaining_methods.contains(&russh::MethodKind::PublicKey) {
            agent_attempted = true;
            match authenticate_with_agent(handle, username, events, wakeup).await {
                Ok(Some(AuthResult::Success)) => return Ok(()),
                Ok(Some(AuthResult::Failure {
                    remaining_methods: methods,
                    partial_success,
                })) => {
                    remaining_methods = methods;
                    partially_authenticated |= partial_success;
                }
                Ok(None) => {}
                Err(error)
                    if host_key_verification.interaction.is_none()
                        && !remaining_methods.contains(&russh::MethodKind::KeyboardInteractive) =>
                {
                    return Err(error)
                }
                Err(_) => {}
            }
            continue;
        }
        let needs_credentials = remaining_methods.contains(&russh::MethodKind::Password)
            || remaining_methods.contains(&russh::MethodKind::PublicKey);
        if remaining_methods.contains(&russh::MethodKind::KeyboardInteractive)
            && !(partially_authenticated && needs_credentials)
        {
            return authenticate_keyboard_interactive(
                handle,
                username,
                events,
                wakeup,
                host_key_verification,
            )
            .await;
        }
        if needs_credentials {
            if let Some(interaction) = &host_key_verification.interaction {
                credentials = interaction
                    .request_credentials(handle, &remaining_methods, None, events, wakeup)
                    .await?;
                continue;
            }
        }
        push_auth_method_event(
            events,
            wakeup,
            "auth_failed",
            "ssh",
            "Server rejected all available SSH authentication methods.",
            &remaining_methods,
        );
        return Err("Server rejected all available SSH authentication methods.".to_owned());
    }
}

async fn authenticate_configured_key<H: client::Handler>(
    handle: &mut client::Handle<H>,
    username: &str,
    private_key: PreparedPrivateKey<'_>,
    certificate: Option<&str>,
    passphrase: Option<&str>,
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    host_key_verification: &HostKeyVerification,
) -> Result<AuthResult, String> {
    let secrets = authentication_secrets(passphrase);
    let auth_method = if certificate.is_some() {
        "certificate"
    } else {
        "public_key"
    };
    let certificate_value = certificate
        .map(decode_openssh_certificate)
        .transpose()
        .inspect_err(|error| {
            push_auth_event(
                events,
                wakeup,
                "auth_certificate_failed",
                "certificate",
                error.clone(),
            );
        })?;
    let mut unlocked = None;
    let public_key = match private_key.public_key.clone().or_else(|| {
        certificate_value
            .as_ref()
            .map(|certificate| PublicKey::new(certificate.public_key().clone(), ""))
    }) {
        Some(key) => key,
        None => {
            // Legacy encrypted PEM formats hide the public key as well.
            let key = unlock_private_key(
                private_key.encoded,
                secrets.key_passphrase.as_deref(),
                events,
                wakeup,
            )?;
            let public_key = key.public_key().clone();
            unlocked = Some(key);
            public_key
        }
    };
    if let Some(encoded) = certificate {
        decode_openssh_certificate_for_key(encoded, &public_key).inspect_err(|error| {
            push_auth_event(
                events,
                wakeup,
                "auth_certificate_failed",
                "certificate",
                error.clone(),
            );
        })?;
    }
    push_auth_event(
        events,
        wakeup,
        if certificate.is_some() {
            "auth_certificate_start"
        } else {
            "auth_key_start"
        },
        auth_method,
        "Checking whether the server accepts the configured public key.",
    );
    let hash_alg = if public_key.algorithm().is_rsa() {
        handle
            .best_supported_rsa_hash()
            .await
            .map_err(|error| error.to_string())?
            .flatten()
    } else {
        None
    };
    let hardware_key = matches!(
        public_key.algorithm(),
        russh::keys::ssh_key::Algorithm::SkEcdsaSha2NistP256
            | russh::keys::ssh_key::Algorithm::SkEd25519
    );
    let mut signer = ConfiguredKeySigner {
        key: private_key,
        public_key: public_key.clone(),
        unlocked,
        secrets,
        events,
        wakeup,
        host_key_verification,
    };
    let result = if let Some(certificate) = certificate_value {
        handle
            .authenticate_certificate_with(username.to_owned(), certificate, hash_alg, &mut signer)
            .await
    } else {
        handle
            .authenticate_publickey_with(username.to_owned(), public_key, hash_alg, &mut signer)
            .await
    };
    match result {
        Ok(AuthResult::Success) => {
            // Also gate a server that replies SUCCESS to an unsigned probe.
            host_key_verification.require_trusted(events, wakeup)?;
            push_auth_event(
                events,
                wakeup,
                "auth_success",
                if hardware_key { "fido2" } else { auth_method },
                "Configured key authentication succeeded.",
            );
            Ok(AuthResult::Success)
        }
        Err(KeySignError::Intervention(message)) => Err(message),
        Err(error) => {
            push_auth_event(
                events,
                wakeup,
                if certificate.is_some() {
                    "auth_certificate_failed"
                } else {
                    "auth_key_failed"
                },
                auth_method,
                format!("Configured key authentication failed: {error}"),
            );
            Err(error.to_string())
        }
        Ok(AuthResult::Failure {
            remaining_methods: methods,
            partial_success,
        }) => {
            push_auth_method_event(
                events,
                wakeup,
                if partial_success {
                    "auth_partial_success"
                } else if certificate.is_some() {
                    "auth_certificate_rejected"
                } else {
                    "auth_key_rejected"
                },
                auth_method,
                if partial_success {
                    "Configured key accepted; further authentication is required."
                } else {
                    "Server rejected the configured public key."
                },
                &methods,
            );
            Ok(AuthResult::Failure {
                remaining_methods: methods,
                partial_success,
            })
        }
    }
}

async fn authenticate_with_agent<H: client::Handler>(
    handle: &mut client::Handle<H>,
    username: &str,
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
) -> Result<Option<AuthResult>, String> {
    push_auth_event(
        events,
        wakeup,
        "auth_agent_start",
        "agent",
        "Trying SSH agent authentication.",
    );
    let mut agent = connect_agent_client().await.map_err(|error| {
        push_auth_event(
            events,
            wakeup,
            "auth_agent_unavailable",
            "agent",
            format!("No usable SSH agent was available: {error}"),
        );
        format!("server rejected none/password authentication and no usable SSH agent was available: {error}")
    })?;
    let identities = agent.request_identities().await.map_err(|error| {
        push_auth_event(
            events,
            wakeup,
            "auth_agent_failed",
            "agent",
            format!("Failed to list SSH agent identities: {error}"),
        );
        format!("failed to list SSH agent identities: {error}")
    })?;
    push_auth_event(
        events,
        wakeup,
        "auth_agent_identities",
        "agent",
        format!("SSH agent returned {} identities.", identities.len()),
    );
    let mut last_result = None;
    for (index, identity) in identities.into_iter().enumerate() {
        let key = identity.public_key().into_owned();
        push_auth_event(
            events,
            wakeup,
            "auth_agent_identity_start",
            "agent",
            format!("Trying SSH agent identity {}.", index + 1),
        );
        match handle
            .authenticate_publickey_with(username.to_owned(), key, None, &mut agent)
            .await
        {
            Ok(AuthResult::Success) => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_success",
                    "agent",
                    format!("SSH agent identity {} succeeded.", index + 1),
                );
                return Ok(Some(AuthResult::Success));
            }
            Ok(AuthResult::Failure {
                partial_success,
                remaining_methods,
            }) => {
                push_auth_method_event(
                    events,
                    wakeup,
                    if partial_success {
                        "auth_partial_success"
                    } else {
                        "auth_agent_identity_rejected"
                    },
                    "agent",
                    if partial_success {
                        "SSH agent key accepted; further authentication is required.".to_owned()
                    } else {
                        format!("SSH agent identity {} was rejected.", index + 1)
                    },
                    &remaining_methods,
                );
                let stop =
                    partial_success || !remaining_methods.contains(&russh::MethodKind::PublicKey);
                let result = AuthResult::Failure {
                    partial_success,
                    remaining_methods,
                };
                if stop {
                    return Ok(Some(result));
                }
                last_result = Some(result);
            }
            Err(error) => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_agent_failed",
                    "agent",
                    format!("SSH agent authentication failed: {error}"),
                );
                return Err(error.to_string());
            }
        }
    }

    Ok(last_result)
}

async fn authenticate_keyboard_interactive<H: client::Handler>(
    handle: &mut client::Handle<H>,
    username: &str,
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    verification: &HostKeyVerification,
) -> Result<(), String> {
    verification.require_trusted(events, wakeup)?;
    let result = async {
        let interaction = verification.interaction.as_ref().ok_or_else(|| {
            "This SSH operation requires keyboard-interactive authentication, but cannot display interactive prompts.".to_owned()
        })?;
        push_auth_event(
            events, wakeup, "auth_interactive_start", "keyboard-interactive",
            "Trying SSH keyboard-interactive authentication.",
        );
        let mut response = handle
            .authenticate_keyboard_interactive_start(username, None)
            .await
            .map_err(|error| error.to_string())?;
        loop {
            match response {
                client::KeyboardInteractiveAuthResponse::Success => {
                    push_auth_event(
                        events, wakeup, "auth_success", "keyboard-interactive",
                        "SSH keyboard-interactive authentication succeeded.",
                    );
                    return Ok(());
                }
                client::KeyboardInteractiveAuthResponse::Failure { .. } => {
                    return Err("Server rejected SSH keyboard-interactive authentication.".to_owned());
                }
                client::KeyboardInteractiveAuthResponse::InfoRequest { name, instructions, prompts } => {
                    // Empty information requests still require an empty response.
                    let responses = if prompts.is_empty() && name.is_empty() && instructions.is_empty() {
                        Vec::new()
                    } else {
                        interaction.request(handle, name, instructions, prompts, events, wakeup).await?
                    };
                    push_auth_event(
                        events, wakeup, "auth_interactive_submitted", "keyboard-interactive",
                        "SSH keyboard-interactive response submitted.",
                    );
                    response = handle
                        .authenticate_keyboard_interactive_respond(responses)
                        .await
                        .map_err(|error| error.to_string())?;
                }
            }
        }
    }.await;
    if let Err(error) = &result {
        push_auth_event(
            events,
            wakeup,
            "auth_interactive_failed",
            "keyboard-interactive",
            error.clone(),
        );
    }
    result
}

#[cfg(test)]
#[path = "auth_probe_tests.rs"]
mod probe_tests;

#[cfg(test)]
mod tests {
    use super::{
        decode_openssh_certificate_for_key, openssh_certificate_type, prepare_private_key,
        unlock_private_key,
    };
    use russh::client;
    use russh::keys::ssh_key::{
        certificate::Builder, private::Ed25519Keypair, PrivateKey, PublicKey,
    };
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };

    fn private_key(seed: u8) -> PrivateKey {
        Ed25519Keypair::from_seed(&[seed; 32]).into()
    }

    fn encrypted_private_key(key: &PrivateKey) -> String {
        use russh::keys::ssh_key::{Cipher, Kdf, LineEnding};

        key.encrypt_with(
            Cipher::Aes256Ctr,
            Kdf::Bcrypt {
                salt: vec![7; 16],
                rounds: 1,
            },
            123,
            "test-passphrase",
        )
        .unwrap()
        .to_openssh(LineEnding::LF)
        .unwrap()
        .to_string()
    }

    #[test]
    fn encrypted_private_key_requests_missing_or_incorrect_passphrase() {
        let encoded = encrypted_private_key(&private_key(1));
        for passphrase in [None, Some("wrong-passphrase")] {
            let events = Arc::new(Mutex::new(Vec::new()));
            let wakeup = Arc::new(Mutex::new(None));
            assert!(unlock_private_key(&encoded, passphrase, &events, &wakeup).is_err());
            let events = events.lock().unwrap();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].kind, "auth_passphrase_required");
            assert_eq!(events[0].method.as_deref(), Some("public_key"));
            assert!(!events[0].message.contains("wrong-passphrase"));
        }
    }

    #[test]
    fn encrypted_private_key_accepts_plain_and_packed_passphrases() {
        let key = private_key(1);
        let encoded = encrypted_private_key(&key);
        for passphrase in [
            "test-passphrase",
            r#"nauterm-auth-secrets:v1:{"keyPassphrase":"test-passphrase","fido2Pin":"1234"}"#,
        ] {
            let events = Arc::new(Mutex::new(Vec::new()));
            let wakeup = Arc::new(Mutex::new(None));
            let prepared = prepare_private_key(Some(&encoded), &events, &wakeup)
                .unwrap()
                .unwrap();
            assert_eq!(prepared.public_key.as_ref().unwrap(), key.public_key());
            assert!(events.lock().unwrap().is_empty());
            let secrets = super::authentication_secrets(Some(passphrase));
            let unlocked = unlock_private_key(
                &encoded,
                secrets.key_passphrase.as_deref(),
                &events,
                &wakeup,
            )
            .unwrap();
            assert_eq!(unlocked.public_key(), key.public_key());
            assert!(!unlocked.is_encrypted());
            assert!(events.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn encrypted_pkcs8_private_key_can_be_unlocked() {
        let key = private_key(1);
        let mut encoded = Vec::new();
        russh::keys::encode_pkcs8_pem_encrypted(&key, b"test-passphrase", 1, &mut encoded).unwrap();
        let encoded = String::from_utf8(encoded).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let wakeup = Arc::new(Mutex::new(None));
        assert!(unlock_private_key(&encoded, None, &events, &wakeup).is_err());
        assert_eq!(events.lock().unwrap()[0].kind, "auth_passphrase_required");
        let unlocked =
            unlock_private_key(&encoded, Some("test-passphrase"), &events, &wakeup).unwrap();
        assert_eq!(unlocked.public_key(), key.public_key());
    }

    #[test]
    fn malformed_private_key_is_not_a_passphrase_request() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let wakeup = Arc::new(Mutex::new(None));
        assert!(prepare_private_key(Some("invalid key"), &events, &wakeup).is_err());
        assert_eq!(events.lock().unwrap()[0].kind, "auth_key_failed");
        assert!(prepare_private_key(None, &events, &wakeup)
            .unwrap()
            .is_none());
    }

    fn certificate_for(subject: &PrivateKey) -> String {
        let ca = private_key(99);
        let mut builder = Builder::new(
            vec![7; Builder::RECOMMENDED_NONCE_SIZE],
            subject.public_key(),
            0,
            u64::MAX,
        )
        .unwrap();
        builder.key_id("nauterm-auth-test").unwrap();
        builder.valid_principal("testuser").unwrap();
        builder.sign(&ca).unwrap().to_openssh().unwrap()
    }

    #[test]
    fn accepts_certificate_matching_private_key() {
        let subject = private_key(1);
        let encoded = certificate_for(&subject);

        let decoded = decode_openssh_certificate_for_key(&encoded, subject.public_key()).unwrap();

        assert_eq!(decoded.public_key(), subject.public_key().key_data());
    }

    #[test]
    fn accepts_wrapped_certificate_text_with_nonstandard_whitespace() {
        let subject = private_key(1);
        let encoded = certificate_for(&subject);
        let mut fields = encoded.split_whitespace();
        let algorithm = fields.next().unwrap();
        let base64_data = fields.next().unwrap();
        let midpoint = base64_data.len() / 2;
        let wrapped = format!(
            "\u{feff}\n{algorithm}\t{}\n{} copied-comment",
            &base64_data[..midpoint],
            &base64_data[midpoint..]
        );

        let decoded = decode_openssh_certificate_for_key(&wrapped, subject.public_key()).unwrap();

        assert_eq!(decoded.public_key(), subject.public_key().key_data());
    }

    #[test]
    fn accepts_rfc4716_wrapped_certificate_body() {
        let subject = private_key(1);
        let encoded = certificate_for(&subject);
        let base64_data = encoded.split_whitespace().nth(1).unwrap();
        let midpoint = base64_data.len() / 2;
        let wrapped = format!(
            "---- BEGIN SSH2 PUBLIC KEY ----\nComment: \"nauterm test\"\n{}\n{}\n---- END SSH2 PUBLIC KEY ----",
            &base64_data[..midpoint],
            &base64_data[midpoint..]
        );

        let decoded = decode_openssh_certificate_for_key(&wrapped, subject.public_key()).unwrap();

        assert_eq!(decoded.public_key(), subject.public_key().key_data());
    }

    #[test]
    fn reports_certificate_wire_type_without_exposing_the_body() {
        assert_eq!(
            openssh_certificate_type("ecdsa-sha2-nistp384-cert-v01@openssh.com not-yet-validated")
                .as_deref(),
            Some("ecdsa-sha2-nistp384-cert-v01@openssh.com")
        );
    }

    #[test]
    fn rejects_certificate_for_different_private_key() {
        let subject = private_key(1);
        let other = private_key(2);
        let encoded = certificate_for(&subject);

        let error = decode_openssh_certificate_for_key(&encoded, other.public_key()).unwrap_err();

        assert!(error.contains("does not match"));
    }

    #[test]
    fn rejects_invalid_certificate_text() {
        let subject = private_key(1);

        let error =
            decode_openssh_certificate_for_key("ssh-ed25519-cert invalid", subject.public_key())
                .unwrap_err();

        assert!(error.contains("failed to decode OpenSSH certificate"));
    }

    #[test]
    fn encrypted_key_is_used_in_real_key_and_certificate_auth_exchanges() {
        struct TestClient;

        impl client::Handler for TestClient {
            type Error = russh::Error;

            async fn check_server_key(
                &mut self,
                _server_public_key: &russh::keys::PublicKeyOrCertificate,
            ) -> Result<bool, Self::Error> {
                Ok(true)
            }
        }

        struct TestServer {
            certificate_seen: Arc<AtomicBool>,
            expected_key: PublicKey,
        }

        impl russh::server::Handler for TestServer {
            type Error = russh::Error;

            async fn auth_publickey_offered(
                &mut self,
                _user: &str,
                _public_key: &russh::keys::ssh_key::PublicKey,
            ) -> Result<russh::server::Auth, Self::Error> {
                Ok(russh::server::Auth::Accept)
            }

            async fn auth_publickey(
                &mut self,
                user: &str,
                public_key: &russh::keys::ssh_key::PublicKey,
            ) -> Result<russh::server::Auth, Self::Error> {
                assert_eq!(user, "testuser");
                assert_eq!(public_key, &self.expected_key);
                Ok(russh::server::Auth::Accept)
            }

            async fn auth_openssh_certificate(
                &mut self,
                _user: &str,
                certificate: &russh::keys::ssh_key::Certificate,
            ) -> Result<russh::server::Auth, Self::Error> {
                self.certificate_seen.store(true, Ordering::SeqCst);
                if certificate.key_id() == "nauterm-auth-test"
                    && certificate.public_key() == self.expected_key.key_data()
                {
                    Ok(russh::server::Auth::Accept)
                } else {
                    Ok(russh::server::Auth::Reject {
                        proceed_with_methods: None,
                        partial_success: false,
                    })
                }
            }
        }

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            use russh::keys::ssh_key::{Algorithm, EcdsaCurve};
            let mut rng = russh::keys::key::safe_rng();
            let keys = [
                private_key(1),
                PrivateKey::random(
                    &mut rng,
                    Algorithm::Ecdsa {
                        curve: EcdsaCurve::NistP256,
                    },
                )
                .unwrap(),
                PrivateKey::random(&mut rng, Algorithm::Rsa { hash: None }).unwrap(),
            ];
            for (user_key, use_certificate) in keys
                .into_iter()
                .flat_map(|key| [(key.clone(), false), (key, true)])
            {
                let user_key_text = encrypted_private_key(&user_key);
                let certificate_text = certificate_for(&user_key);
                let certificate_seen = Arc::new(AtomicBool::new(false));

                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let address = listener.local_addr().unwrap();
                let server_config = Arc::new(russh::server::Config {
                    auth_rejection_time: std::time::Duration::ZERO,
                    keys: vec![private_key(3)],
                    ..Default::default()
                });
                let server_certificate_seen = certificate_seen.clone();
                let expected_key = user_key.public_key().clone();
                let server_task = tokio::spawn(async move {
                    let (stream, _) = listener.accept().await.unwrap();
                    russh::server::run_stream(
                        server_config,
                        stream,
                        TestServer {
                            certificate_seen: server_certificate_seen,
                            expected_key,
                        },
                    )
                    .await
                    .unwrap();
                });

                let events = Arc::new(Mutex::new(Vec::new()));
                let wakeup = Arc::new(Mutex::new(None));
                let prepared_key =
                    prepare_private_key(Some(&user_key_text), &events, &wakeup).unwrap();
                let mut handle =
                    client::connect(Arc::new(client::Config::default()), address, TestClient)
                        .await
                        .unwrap();

                super::authenticate(
                    &mut handle,
                    "testuser",
                    prepared_key,
                    use_certificate.then_some(certificate_text.as_str()),
                    Some("test-passphrase"),
                    None,
                    &events,
                    &wakeup,
                    &super::HostKeyVerification::default(),
                )
                .await
                .unwrap();

                assert_eq!(certificate_seen.load(Ordering::SeqCst), use_certificate);
                let method = if use_certificate {
                    "certificate"
                } else {
                    "public_key"
                };
                assert!(events.lock().unwrap().iter().any(|event| {
                    event.kind == "auth_success" && event.method.as_deref() == Some(method)
                }));

                handle
                    .disconnect(russh::Disconnect::ByApplication, "", "")
                    .await
                    .unwrap();
                server_task.abort();
            }
        });
    }
}

#[cfg(test)]
#[path = "auth_interactive_tests.rs"]
mod interactive_tests;

#[cfg(test)]
#[path = "auth_stages_tests.rs"]
mod stages_tests;
