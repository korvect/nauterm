use super::*;
use russh::keys::ssh_key::{
    certificate::Builder, private::Ed25519Keypair, Cipher, Kdf, LineEnding,
};
use std::sync::atomic::AtomicUsize;

fn key(seed: u8) -> PrivateKey {
    Ed25519Keypair::from_seed(&[seed; 32]).into()
}

fn encrypted(key: &PrivateKey) -> String {
    key.encrypt_with(
        Cipher::Aes256Ctr,
        Kdf::Bcrypt {
            salt: vec![3; 16],
            rounds: 1,
        },
        7,
        "test-passphrase",
    )
    .unwrap()
    .to_openssh(LineEnding::LF)
    .unwrap()
    .to_string()
}

fn certificate(key: &PrivateKey) -> String {
    let mut builder = Builder::new(
        vec![9; Builder::RECOMMENDED_NONCE_SIZE],
        key.public_key(),
        0,
        u64::MAX,
    )
    .unwrap();
    builder.key_id("nauterm-probe-test").unwrap();
    builder.valid_principal("testuser").unwrap();
    builder.sign(&self::key(3)).unwrap().to_openssh().unwrap()
}

#[derive(Default)]
struct Observations {
    probes: AtomicUsize,
    signatures: AtomicUsize,
    passwords: AtomicUsize,
    keyboard_interactive: AtomicUsize,
}

struct TestServer {
    observations: Arc<Observations>,
    accept_none: bool,
    /// Mimic FreeBSD defaults: reject SSH `password`, accept PAM via kbd-interactive.
    reject_password: bool,
}

fn accepted(value: bool) -> russh::server::Auth {
    if value {
        russh::server::Auth::Accept
    } else {
        russh::server::Auth::Reject {
            proceed_with_methods: None,
            partial_success: false,
        }
    }
}

impl russh::server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_none(&mut self, _user: &str) -> Result<russh::server::Auth, Self::Error> {
        Ok(accepted(self.accept_none))
    }

    async fn auth_publickey_offered(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<russh::server::Auth, Self::Error> {
        self.observations.probes.fetch_add(1, Ordering::SeqCst);
        Ok(accepted(
            user == "testuser" && public_key == key(1).public_key(),
        ))
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<russh::server::Auth, Self::Error> {
        self.observations.signatures.fetch_add(1, Ordering::SeqCst);
        Ok(accepted(
            user == "testuser" && public_key == key(1).public_key(),
        ))
    }

    async fn auth_openssh_certificate(
        &mut self,
        user: &str,
        certificate: &russh::keys::ssh_key::Certificate,
    ) -> Result<russh::server::Auth, Self::Error> {
        self.observations.signatures.fetch_add(1, Ordering::SeqCst);
        Ok(accepted(
            user == "testuser" && certificate.public_key() == key(1).public_key().key_data(),
        ))
    }

    async fn auth_password(
        &mut self,
        _user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        self.observations.passwords.fetch_add(1, Ordering::SeqCst);
        if self.reject_password {
            return Ok(accepted(false));
        }
        Ok(accepted(password == "test-password"))
    }

    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        _user: &str,
        _submethods: &str,
        response: Option<russh::server::Response<'a>>,
    ) -> Result<russh::server::Auth, Self::Error> {
        self.observations
            .keyboard_interactive
            .fetch_add(1, Ordering::SeqCst);
        match response {
            None => Ok(russh::server::Auth::Partial {
                name: "".into(),
                instructions: "".into(),
                prompts: std::borrow::Cow::Borrowed(&[(
                    std::borrow::Cow::Borrowed("Password:"),
                    false,
                )]),
            }),
            Some(mut answers) => {
                let answer = answers.next().unwrap_or_default();
                Ok(accepted(answer.as_ref() == b"test-password"))
            }
        }
    }
}

struct Fixture {
    address: std::net::SocketAddr,
    known_hosts: PathBuf,
    observations: Arc<Observations>,
    server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn new(accept_none: bool) -> Self {
        Self::with_options(accept_none, false).await
    }

    async fn with_options(accept_none: bool, reject_password: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let known_hosts = std::env::temp_dir().join(format!(
            "nauterm-auth-probe-{}-{}",
            std::process::id(),
            address.port()
        ));
        let observations = Arc::new(Observations::default());
        let counters = observations.clone();
        let config = Arc::new(russh::server::Config {
            keys: vec![key(3)],
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        });
        let server = tokio::spawn(async move {
            let mut sessions = tokio::task::JoinSet::new();
            while let Ok((stream, _)) = listener.accept().await {
                let config = config.clone();
                let observations = counters.clone();
                sessions.spawn(async move {
                    if let Ok(session) = russh::server::run_stream(
                        config,
                        stream,
                        TestServer {
                            observations,
                            accept_none,
                            reject_password,
                        },
                    )
                    .await
                    {
                        let _ = session.await;
                    }
                });
            }
        });
        Self {
            address,
            known_hosts,
            observations,
            server,
        }
    }

    async fn attempt(
        &self,
        private_key: Option<&str>,
        certificate: Option<&str>,
        passphrase: Option<&str>,
        password: Option<&str>,
        user: &str,
        trust: HostKeyTrustMode,
    ) -> (Result<(), String>, Vec<SessionEvent>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let wakeup = Arc::new(Mutex::new(None));
        let host_key_verification = HostKeyVerification::default();
        let handler = SshClientHandler {
            host: self.address.ip().to_string(),
            port: self.address.port(),
            known_hosts_path: Some(self.known_hosts.clone()),
            host_key_trust_mode: trust,
            host_key_verification: host_key_verification.clone(),
            output: Arc::new(Mutex::new(Vec::new())),
            events: events.clone(),
            wakeup: wakeup.clone(),
        };
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            let prepared = prepare_private_key(private_key, &events, &wakeup)?;
            let mut handle = client::connect(ssh_client_config(), self.address, handler)
                .await
                .map_err(|error| error.to_string())?;
            let result = authenticate(
                &mut handle,
                user,
                prepared,
                certificate,
                passphrase,
                password,
                &events,
                &wakeup,
                &host_key_verification,
            )
            .await;
            if result.is_ok() {
                handle
                    .disconnect(russh::Disconnect::ByApplication, "", "")
                    .await
                    .unwrap();
            }
            result
        })
        .await
        .expect("authentication must complete or request an intervention");
        let captured = events.lock().unwrap().clone();
        (result, captured)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        let _ = fs::remove_file(&self.known_hosts);
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

fn has(events: &[SessionEvent], kind: &str) -> bool {
    events.iter().any(|event| event.kind == kind)
}

#[test]
fn rejected_keys_and_usernames_skip_unlock_then_require_host_trust_before_password() {
    runtime().block_on(async {
        for (seed, user, with_certificate) in [
            (2, "testuser", false),
            (1, "wrong-user", false),
            (2, "testuser", true),
        ] {
            let fixture = Fixture::new(false).await;
            let private_key = encrypted(&key(seed));
            let certificate = with_certificate.then(|| certificate(&key(seed)));
            let (result, events) = fixture
                .attempt(
                    Some(&private_key),
                    certificate.as_deref(),
                    None,
                    Some("test-password"),
                    user,
                    HostKeyTrustMode::Strict,
                )
                .await;
            assert!(result.is_err());
            assert!(has(&events, "host_key_unknown"));
            assert!(!has(&events, "auth_passphrase_required"));
            assert!(fixture.observations.probes.load(Ordering::SeqCst) > 0);
            assert_eq!(fixture.observations.signatures.load(Ordering::SeqCst), 0);
            assert_eq!(fixture.observations.passwords.load(Ordering::SeqCst), 0);

            let (result, events) = fixture
                .attempt(
                    Some(&private_key),
                    certificate.as_deref(),
                    None,
                    Some("test-password"),
                    user,
                    HostKeyTrustMode::AcceptOnce,
                )
                .await;
            assert!(result.is_ok(), "{result:?}");
            assert!(events
                .iter()
                .any(|event| event.kind == "auth_success"
                    && event.method.as_deref() == Some("password")));
            assert!(!has(&events, "auth_passphrase_required"));
            assert_eq!(fixture.observations.signatures.load(Ordering::SeqCst), 0);
            assert_eq!(fixture.observations.passwords.load(Ordering::SeqCst), 1);
        }
    });
}

#[test]
fn accepted_keys_unlock_before_host_confirmation_but_never_sign_before_trust() {
    runtime().block_on(async {
        for with_certificate in [false, true] {
            let fixture = Fixture::new(false).await;
            let private_key = encrypted(&key(1));
            let certificate = with_certificate.then(|| certificate(&key(1)));
            for passphrase in [None, Some("wrong-passphrase")] {
                let (result, events) = fixture
                    .attempt(
                        Some(&private_key),
                        certificate.as_deref(),
                        passphrase,
                        None,
                        "testuser",
                        HostKeyTrustMode::Strict,
                    )
                    .await;
                assert!(result.is_err());
                assert!(has(&events, "auth_passphrase_required"));
                assert!(!has(&events, "host_key_unknown"));
                assert!(!has(&events, "auth_key_failed"));
            }
            let (result, events) = fixture
                .attempt(
                    Some(&private_key),
                    certificate.as_deref(),
                    Some("test-passphrase"),
                    None,
                    "testuser",
                    HostKeyTrustMode::Strict,
                )
                .await;
            assert!(result.is_err());
            assert!(has(&events, "host_key_unknown"));
            assert!(!has(&events, "auth_passphrase_required"));
            assert_eq!(fixture.observations.signatures.load(Ordering::SeqCst), 0);
            let (result, events) = fixture
                .attempt(
                    Some(&private_key),
                    certificate.as_deref(),
                    Some("test-passphrase"),
                    None,
                    "testuser",
                    HostKeyTrustMode::AcceptOnce,
                )
                .await;
            assert!(result.is_ok(), "{result:?}");
            assert!(has(&events, "auth_success"));
            assert_eq!(fixture.observations.signatures.load(Ordering::SeqCst), 1);
        }
    });
}

#[test]
fn keyboard_interactive_succeeds_when_password_method_is_rejected() {
    runtime().block_on(async {
        let fixture = Fixture::with_options(false, true).await;
        let (result, events) = fixture
            .attempt(
                None,
                None,
                None,
                Some("test-password"),
                "testuser",
                HostKeyTrustMode::AcceptOnce,
            )
            .await;
        assert!(result.is_ok(), "{result:?} events={events:?}");
        assert!(has(&events, "auth_password_rejected"));
        assert!(has(&events, "auth_interactive_start"));
        // Saved password auto-answers the single echo=false kbd-int prompt;
        // no UI challenge is emitted.
        assert!(!has(&events, "auth_interactive_request"));
        assert!(events.iter().any(|event| {
            event.kind == "auth_success"
                && event.method.as_deref() == Some("keyboard-interactive")
        }));
        assert_eq!(fixture.observations.passwords.load(Ordering::SeqCst), 1);
        assert!(
            fixture
                .observations
                .keyboard_interactive
                .load(Ordering::SeqCst)
                >= 1
        );
    });
}

#[test]
fn password_and_none_auth_cannot_bypass_pending_host_confirmation() {
    runtime().block_on(async {
        for accept_none in [false, true] {
            let fixture = Fixture::new(accept_none).await;
            let (result, events) = fixture
                .attempt(
                    None,
                    None,
                    None,
                    Some("test-password"),
                    "testuser",
                    HostKeyTrustMode::Strict,
                )
                .await;
            assert!(result.is_err());
            assert!(has(&events, "host_key_unknown"));
            assert!(!has(&events, "auth_success"));
            assert_eq!(fixture.observations.passwords.load(Ordering::SeqCst), 0);
        }
    });
}

#[test]
fn changed_host_key_is_rejected_before_public_key_probe() {
    runtime().block_on(async {
        let fixture = Fixture::new(false).await;
        learn_known_hosts_path_without_leading_blank(
            "127.0.0.1",
            fixture.address.port(),
            key(9).public_key(),
            &fixture.known_hosts,
        )
        .unwrap();
        let private_key = encrypted(&key(1));
        let (result, events) = fixture
            .attempt(
                Some(&private_key),
                None,
                None,
                None,
                "testuser",
                HostKeyTrustMode::Strict,
            )
            .await;
        assert!(result.is_err());
        assert!(has(&events, "host_key_changed"));
        assert!(!has(&events, "auth_passphrase_required"));
        assert_eq!(fixture.observations.probes.load(Ordering::SeqCst), 0);
    });
}
