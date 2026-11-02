use super::*;
use russh::keys::ssh_key::{private::Ed25519Keypair, Cipher, Kdf, LineEnding};
use russh::{server, MethodKind, MethodSet};
use std::sync::atomic::AtomicUsize;

struct TestClient;
impl client::Handler for TestClient {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        _: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}
fn key() -> PrivateKey {
    Ed25519Keypair::from_seed(&[19; 32]).into()
}
struct StageServer {
    methods: Vec<MethodKind>,
    stage: usize,
    authenticated_factors: Arc<AtomicUsize>,
}
impl StageServer {
    fn reject(&self, partial_success: bool) -> server::Auth {
        server::Auth::Reject {
            proceed_with_methods: Some(MethodSet::from(&self.methods[self.stage..self.stage + 1])),
            partial_success,
        }
    }
    fn accept_factor(&mut self) -> server::Auth {
        self.authenticated_factors.fetch_add(1, Ordering::SeqCst);
        self.stage += 1;
        if self.stage == self.methods.len() {
            server::Auth::Accept
        } else {
            self.reject(true)
        }
    }
}
impl server::Handler for StageServer {
    type Error = russh::Error;
    async fn auth_none(&mut self, _: &str) -> Result<server::Auth, Self::Error> {
        Ok(self.reject(false))
    }
    async fn auth_password(
        &mut self,
        _: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        assert_eq!(self.methods[self.stage], MethodKind::Password);
        Ok(if password == "phase-password" {
            self.accept_factor()
        } else {
            self.reject(false)
        })
    }
    async fn auth_publickey_offered(
        &mut self,
        _: &str,
        public_key: &PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        assert_eq!(self.methods[self.stage], MethodKind::PublicKey);
        Ok(if public_key == key().public_key() {
            server::Auth::Accept
        } else {
            self.reject(false)
        })
    }
    async fn auth_publickey(
        &mut self,
        _: &str,
        public_key: &PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        assert_eq!(self.methods[self.stage], MethodKind::PublicKey);
        assert_eq!(public_key, key().public_key());
        Ok(self.accept_factor())
    }
    async fn auth_keyboard_interactive(
        &mut self,
        _: &str,
        _: &str,
        response: Option<server::Response<'_>>,
    ) -> Result<server::Auth, Self::Error> {
        assert_eq!(self.methods[self.stage], MethodKind::KeyboardInteractive);
        if let Some(mut response) = response {
            assert_eq!(response.next().unwrap().as_ref(), b"123456");
            Ok(self.accept_factor())
        } else {
            Ok(server::Auth::Partial {
                name: "".into(),
                instructions: "".into(),
                prompts: vec![("Verification code: ".into(), false)].into(),
            })
        }
    }
}

#[test]
fn credentials_follow_server_stages_without_reconnecting() {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for (methods, encrypted) in [
                (
                    vec![
                        MethodKind::Password,
                        MethodKind::PublicKey,
                        MethodKind::KeyboardInteractive,
                    ],
                    false,
                ),
                (
                    vec![
                        MethodKind::PublicKey,
                        MethodKind::Password,
                        MethodKind::KeyboardInteractive,
                    ],
                    false,
                ),
                (
                    vec![
                        MethodKind::Password,
                        MethodKind::PublicKey,
                        MethodKind::KeyboardInteractive,
                    ],
                    true,
                ),
                (vec![MethodKind::Password], false),
                (vec![MethodKind::PublicKey], false),
            ] {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let address = listener.local_addr().unwrap();
                let factors = Arc::new(AtomicUsize::new(0));
                let expected_factors = methods.len();
            let method_order = methods.clone();
                let observed = factors.clone();
                let server = tokio::spawn(async move {
                    let (stream, _) = listener.accept().await.unwrap();
                    let config = Arc::new(server::Config {
                        keys: vec![Ed25519Keypair::from_seed(&[20; 32]).into()],
                        auth_rejection_time: Duration::ZERO,
                        auth_rejection_time_initial: Some(Duration::ZERO),
                        ..Default::default()
                    });
                    let session = server::run_stream(
                        config,
                        stream,
                        StageServer {
                            methods,
                            stage: 0,
                            authenticated_factors: observed,
                        },
                    )
                    .await
                    .unwrap();
                    let _ = session.await;
                });
                let mut handle =
                    client::connect(Arc::new(client::Config::default()), address, TestClient)
                        .await
                        .unwrap();
                let events = Arc::new(Mutex::new(Vec::new()));
                let wakeup = Arc::new(Mutex::new(None));
                let interaction = AuthenticationInteraction::default();
                let verification = HostKeyVerification::with_interaction(interaction.clone());
                let user_key = if encrypted {
                    key()
                        .encrypt_with(
                            Cipher::Aes256Ctr,
                            Kdf::Bcrypt {
                                salt: vec![3; 16],
                                rounds: 1,
                            },
                            7,
                            "unlock-me",
                        )
                        .unwrap()
                } else {
                    key()
                }
                .to_openssh(LineEnding::LF)
                .unwrap()
                .to_string();
                let auth = authenticate(
                    &mut handle,
                    "testuser",
                    None,
                    None,
                    None,
                    None,
                    &events,
                    &wakeup,
                    &verification,
                );
                let respond = async {
                    let mut last_id = 0;
                    let mut unlock_attempts = 0;
                    loop {
                        let event = events
                            .lock()
                            .unwrap()
                            .iter()
                            .find(|event| {
                                event
                                    .authentication_request_id
                                    .is_some_and(|id| id > last_id)
                                    || event
                                        .challenge
                                        .as_ref()
                                        .is_some_and(|challenge| challenge.id > last_id)
                            })
                            .cloned();
                        if let Some(event) = event {
                            if let Some(request_id) = event.authentication_request_id {
                                let methods = event.authentication_methods.unwrap();
                                assert_eq!(methods.len(), 1);
                                let method = methods[0].clone();
                                let credentials = if method == "password" {
                                    AuthenticationCredentials {
                                        method,
                                        password: Some("phase-password".into()),
                                        private_key: None,
                                        certificate: None,
                                        passphrase: None,
                                    }
                                } else {
                                    AuthenticationCredentials {
                                        method,
                                        private_key: Some(user_key.clone()),
                                        passphrase: (event.kind == "auth_passphrase_required")
                                            .then(|| {
                                                unlock_attempts += 1;
                                                if unlock_attempts == 1 { "wrong-unlock".into() } else { "unlock-me".into() }
                                            }),
                                        password: None,
                                        certificate: None,
                                    }
                                };
                                assert!(interaction.respond_credentials(request_id, credentials));
                                assert!(!interaction.respond(request_id, None));
                                last_id = request_id;
                            } else if let Some(challenge) = event.challenge {
                                assert!(
                                    interaction.respond(challenge.id, Some(vec!["123456".into()]))
                                );
                                last_id = challenge.id;
                            }
                        }
                        if events
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|event| event.kind == "auth_success")
                        {
                            return;
                        }
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                };
                let (result, ()) = tokio::time::timeout(Duration::from_secs(5), async {
                    tokio::join!(auth, respond)
                })
                .await
                .unwrap_or_else(|_| panic!("all factors must complete: {method_order:?}, encrypted={encrypted}, events={:?}", events.lock().unwrap().iter().map(|event| (&event.kind, &event.authentication_request_id)).collect::<Vec<_>>()));
                result.unwrap();
                assert_eq!(factors.load(Ordering::SeqCst), expected_factors);
                let serialized = serde_json::to_string(&*events.lock().unwrap()).unwrap();
                for secret in ["phase-password", "unlock-me", "123456"] {
                    assert!(!serialized.contains(secret));
                }
                server.abort();
            }
        });
}
