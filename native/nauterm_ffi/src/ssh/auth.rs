use super::*;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use std::sync::Arc;

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

type DynamicAgentClient = AgentClient<Box<dyn AgentStream + Send + Unpin + 'static>>;

const OPENSSH_CERTIFICATE_SUFFIX: &str = "-cert-v01@openssh.com";

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
    private_key: &russh::keys::ssh_key::PrivateKey,
) -> Result<russh::keys::ssh_key::Certificate, String> {
    let certificate = decode_openssh_certificate(encoded)?;
    if certificate.public_key() != private_key.public_key().key_data() {
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
    private_key: Option<&str>,
    certificate: Option<&str>,
    passphrase: Option<&str>,
    password: Option<&str>,
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
) -> Result<(), String> {
    push_auth_event(
        events,
        wakeup,
        "auth_none_start",
        "none",
        "Trying SSH none authentication.",
    );
    match handle.authenticate_none(username.to_owned()).await {
        Ok(result) if result.success() => {
            push_auth_event(
                events,
                wakeup,
                "auth_success",
                "none",
                "SSH none authentication succeeded.",
            );
            return Ok(());
        }
        Ok(AuthResult::Failure { .. }) => {
            push_auth_event(
                events,
                wakeup,
                "auth_none_rejected",
                "none",
                "SSH none authentication was rejected.",
            );
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
        Ok(AuthResult::Success) => {
            push_auth_event(
                events,
                wakeup,
                "auth_success",
                "none",
                "SSH none authentication succeeded.",
            );
            return Ok(());
        }
    }

    if let Some(private_key) = private_key {
        let auth_method = if certificate.is_some() {
            push_auth_event(
                events,
                wakeup,
                "auth_certificate_start",
                "certificate",
                "Trying configured OpenSSH certificate authentication.",
            );
            "certificate"
        } else {
            push_auth_event(
                events,
                wakeup,
                "auth_key_start",
                "public_key",
                "Trying configured private key authentication.",
            );
            "public_key"
        };
        let private_key = match decode_secret_key(private_key, passphrase) {
            Ok(private_key) => private_key,
            Err(KeyError::KeyIsEncrypted) if passphrase.is_none() => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_passphrase_required",
                    "public_key",
                    "Configured private key requires a passphrase.",
                );
                return Err("configured private key requires a passphrase".to_owned());
            }
            Err(error) => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_key_failed",
                    "public_key",
                    format!("Failed to decode configured private key: {error}"),
                );
                return Err(format!("failed to decode private key: {error}"));
            }
        };
        let private_key = Arc::new(private_key);
        let result = if let Some(certificate) = certificate {
            let certificate = decode_openssh_certificate_for_key(certificate, &private_key);
            match certificate {
                Ok(certificate) => {
                    handle
                        .authenticate_openssh_cert(username.to_owned(), private_key, certificate)
                        .await
                }
                Err(error) => {
                    push_auth_event(
                        events,
                        wakeup,
                        "auth_certificate_failed",
                        "certificate",
                        error.clone(),
                    );
                    return Err(error);
                }
            }
        } else {
            let hash_alg = handle
                .best_supported_rsa_hash()
                .await
                .map_err(|error| error.to_string())?
                .flatten();
            handle
                .authenticate_publickey(
                    username.to_owned(),
                    PrivateKeyWithHashAlg::new(private_key, hash_alg),
                )
                .await
        };
        match result {
            Ok(result) if result.success() => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_success",
                    auth_method,
                    if certificate.is_some() {
                        "OpenSSH certificate authentication succeeded."
                    } else {
                        "Configured private key authentication succeeded."
                    },
                );
                return Ok(());
            }
            Ok(AuthResult::Failure { .. }) => {
                push_auth_event(
                    events,
                    wakeup,
                    if certificate.is_some() {
                        "auth_certificate_rejected"
                    } else {
                        "auth_key_rejected"
                    },
                    auth_method,
                    if certificate.is_some() {
                        "OpenSSH certificate authentication was rejected."
                    } else {
                        "Configured private key authentication was rejected."
                    },
                );
            }
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
                    if certificate.is_some() {
                        format!("OpenSSH certificate authentication failed: {error}")
                    } else {
                        format!("Configured private key authentication failed: {error}")
                    },
                );
                return Err(error.to_string());
            }
            Ok(AuthResult::Success) => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_success",
                    auth_method,
                    if certificate.is_some() {
                        "OpenSSH certificate authentication succeeded."
                    } else {
                        "Configured private key authentication succeeded."
                    },
                );
                return Ok(());
            }
        }

        push_auth_event(
            events,
            wakeup,
            "auth_failed",
            auth_method,
            if certificate.is_some() {
                "Server rejected configured OpenSSH certificate authentication."
            } else {
                "Server rejected configured private key authentication."
            },
        );
        return Err(if certificate.is_some() {
            "server rejected configured OpenSSH certificate authentication".to_owned()
        } else {
            "server rejected configured private key authentication".to_owned()
        });
    }

    if certificate.is_some() {
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

    if let Some(password) = password {
        push_auth_event(
            events,
            wakeup,
            "auth_password_start",
            "password",
            "Trying password authentication.",
        );
        match handle
            .authenticate_password(username.to_owned(), password.to_owned())
            .await
        {
            Ok(result) if result.success() => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_success",
                    "password",
                    "Password authentication succeeded.",
                );
                return Ok(());
            }
            Ok(AuthResult::Failure { .. }) => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_password_rejected",
                    "password",
                    "Password authentication was rejected.",
                );
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
        }
    }

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
            Ok(result) if result.success() => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_success",
                    "agent",
                    format!("SSH agent identity {} succeeded.", index + 1),
                );
                return Ok(());
            }
            Ok(_) => {
                push_auth_event(
                    events,
                    wakeup,
                    "auth_agent_identity_rejected",
                    "agent",
                    format!("SSH agent identity {} was rejected.", index + 1),
                );
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

    push_auth_event(
        events,
        wakeup,
        "auth_failed",
        "agent",
        "Server rejected none/password/agent authentication.",
    );
    Err("server rejected none/password/agent authentication".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{decode_openssh_certificate_for_key, openssh_certificate_type};
    use russh::client;
    use russh::keys::ssh_key::{certificate::Builder, private::Ed25519Keypair, PrivateKey};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };

    fn private_key(seed: u8) -> PrivateKey {
        Ed25519Keypair::from_seed(&[seed; 32]).into()
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

        let decoded = decode_openssh_certificate_for_key(&encoded, &subject).unwrap();

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

        let decoded = decode_openssh_certificate_for_key(&wrapped, &subject).unwrap();

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

        let decoded = decode_openssh_certificate_for_key(&wrapped, &subject).unwrap();

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

        let error = decode_openssh_certificate_for_key(&encoded, &other).unwrap_err();

        assert!(error.contains("does not match"));
    }

    #[test]
    fn rejects_invalid_certificate_text() {
        let subject = private_key(1);

        let error =
            decode_openssh_certificate_for_key("ssh-ed25519-cert invalid", &subject).unwrap_err();

        assert!(error.contains("failed to decode OpenSSH certificate"));
    }

    #[test]
    fn certificate_is_used_in_a_real_ssh_auth_exchange() {
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

            async fn auth_openssh_certificate(
                &mut self,
                _user: &str,
                certificate: &russh::keys::ssh_key::Certificate,
            ) -> Result<russh::server::Auth, Self::Error> {
                self.certificate_seen.store(true, Ordering::SeqCst);
                if certificate.key_id() == "nauterm-auth-test" {
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
            let user_key = private_key(1);
            let user_key_text = user_key
                .to_openssh(russh::keys::ssh_key::LineEnding::LF)
                .unwrap();
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
            let server_task = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                russh::server::run_stream(
                    server_config,
                    stream,
                    TestServer {
                        certificate_seen: server_certificate_seen,
                    },
                )
                .await
                .unwrap();
            });

            let mut handle =
                client::connect(Arc::new(client::Config::default()), address, TestClient)
                    .await
                    .unwrap();
            let events = Arc::new(Mutex::new(Vec::new()));
            let wakeup = Arc::new(Mutex::new(None));

            super::authenticate(
                &mut handle,
                "testuser",
                Some(user_key_text.as_str()),
                Some(&certificate_text),
                None,
                None,
                &events,
                &wakeup,
            )
            .await
            .unwrap();

            assert!(certificate_seen.load(Ordering::SeqCst));
            assert!(events.lock().unwrap().iter().any(|event| {
                event.kind == "auth_success" && event.method.as_deref() == Some("certificate")
            }));

            handle
                .disconnect(russh::Disconnect::ByApplication, "", "")
                .await
                .unwrap();
            server_task.abort();
        });
    }
}
