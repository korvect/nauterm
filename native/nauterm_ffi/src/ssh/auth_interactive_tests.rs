use super::*;
use russh::keys::ssh_key::{private::Ed25519Keypair, LineEnding};
use russh::{server, MethodKind, MethodSet};

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

struct TestServer {
    round: usize,
    require_key: bool,
    password_first: bool,
    empty_round: bool,
}
fn methods(kinds: &[MethodKind], partial_success: bool) -> server::Auth {
    server::Auth::Reject {
        proceed_with_methods: Some(MethodSet::from(kinds)),
        partial_success,
    }
}
impl server::Handler for TestServer {
    type Error = russh::Error;
    async fn auth_none(&mut self, _: &str) -> Result<server::Auth, Self::Error> {
        Ok(methods(
            &[if self.require_key {
                MethodKind::PublicKey
            } else if self.password_first {
                MethodKind::Password
            } else {
                MethodKind::KeyboardInteractive
            }],
            false,
        ))
    }
    async fn auth_password(
        &mut self,
        _: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        assert_eq!(password, "configured-password");
        Ok(methods(&[MethodKind::KeyboardInteractive], true))
    }
    async fn auth_publickey_offered(
        &mut self,
        _: &str,
        _: &russh::keys::ssh_key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }
    async fn auth_publickey(
        &mut self,
        _: &str,
        _: &russh::keys::ssh_key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        Ok(methods(&[MethodKind::KeyboardInteractive], true))
    }
    async fn auth_keyboard_interactive(
        &mut self,
        _: &str,
        _: &str,
        response: Option<server::Response<'_>>,
    ) -> Result<server::Auth, Self::Error> {
        if let Some(response) = response {
            let responses: Vec<_> = response
                .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap())
                .collect();
            let expected: Vec<String> = if self.empty_round && self.round == 1 {
                vec![]
            } else if self.round == 1 {
                vec!["account".into(), "private-password".into()]
            } else {
                vec!["123456".into()]
            };
            if responses != expected {
                return Ok(methods(&[MethodKind::KeyboardInteractive], false));
            }
            if self.round == 2 {
                return Ok(server::Auth::Accept);
            }
        }
        self.round += 1;
        Ok(server::Auth::Partial {
            name: "Server authentication".into(),
            instructions: "Answer the server prompts".into(),
            prompts: if self.empty_round && self.round == 1 {
                vec![].into()
            } else if self.round == 1 {
                vec![("Account: ".into(), true), ("Password: ".into(), false)].into()
            } else {
                vec![("Verification code: ".into(), false)].into()
            },
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum Answer {
    Correct,
    Wrong,
    Cancel,
    DisconnectThenRespond,
}

async fn exercise(
    require_key: bool,
    password_first: bool,
    empty_round: bool,
    answer: Answer,
    trusted: bool,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let key: russh::keys::ssh_key::PrivateKey = Ed25519Keypair::from_seed(&[5; 32]).into();
    let config = Arc::new(server::Config {
        keys: vec![key.clone()],
        auth_rejection_time: Duration::ZERO,
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..Default::default()
    });
    let (server_socket_tx, server_socket_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let socket = stream.into_std().unwrap();
        let _ = server_socket_tx.send(socket.try_clone().unwrap());
        let stream = tokio::net::TcpStream::from_std(socket).unwrap();
        let session = server::run_stream(
            config,
            stream,
            TestServer {
                round: 0,
                require_key,
                password_first,
                empty_round,
            },
        )
        .await
        .unwrap();
        let _ = session.await;
    });
    let mut handle = client::connect(Arc::new(client::Config::default()), address, TestClient)
        .await
        .unwrap();
    let server_socket = server_socket_rx.await.unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let wakeup = Arc::new(Mutex::new(None));
    let interaction = AuthenticationInteraction::default();
    let verification = HostKeyVerification::with_interaction(interaction.clone());
    if !trusted {
        verification.defer(SessionEvent::new("host_key_unknown", "Confirm host key"));
    }
    let encoded = key.to_openssh(LineEnding::LF).unwrap();
    let prepared =
        prepare_private_key(require_key.then_some(encoded.as_str()), &events, &wakeup).unwrap();
    let auth = authenticate(
        &mut handle,
        "testuser",
        prepared,
        None,
        None,
        password_first.then_some("configured-password"),
        &events,
        &wakeup,
        &verification,
    );
    let respond = async {
        if !trusted {
            return;
        }
        let mut last_id = 0;
        let mut rounds = 0;
        loop {
            let request = events
                .lock()
                .unwrap()
                .iter()
                .filter_map(|event| event.challenge.as_ref())
                .find(|request| request.id > last_id)
                .cloned();
            if let Some(request) = request {
                assert!(request.id > last_id);
                if matches!(answer, Answer::DisconnectThenRespond) {
                    server_socket.shutdown(std::net::Shutdown::Both).unwrap();
                    // Russh stops reading the socket until AuthInfoResponse is sent.
                    assert!(interaction
                        .respond(request.id, Some(vec![String::new(); request.prompts.len()])));
                    return;
                }
                assert_eq!(request.name, "Server authentication");
                assert!(!interaction.respond(request.id + 100, Some(vec![])));
                let responses = if matches!(answer, Answer::Cancel) {
                    None
                } else if matches!(answer, Answer::Wrong) {
                    Some(vec!["incorrect-response".to_owned(); request.prompts.len()])
                } else if request.prompts.len() == 2 {
                    assert!(request.prompts[0].echo);
                    assert!(!request.prompts[1].echo);
                    Some(vec!["account".into(), "private-password".into()])
                } else if request.prompts.is_empty() {
                    Some(vec![])
                } else {
                    Some(vec!["123456".into()])
                };
                assert!(interaction.respond(request.id, responses));
                assert!(!interaction.respond(request.id, Some(vec![])));
                last_id = request.id;
                rounds += 1;
                if rounds == 2 || !matches!(answer, Answer::Correct) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(auth, respond)
    })
    .await
    .unwrap_or_else(|_| panic!("authentication must not hang: key={require_key}, password={password_first}, empty={empty_round}, answer={answer:?}, trusted={trusted}"));
    assert_eq!(result.is_ok(), trusted && matches!(answer, Answer::Correct));
    let events = events.lock().unwrap();
    let serialized = serde_json::to_string(&*events).unwrap();
    for secret in ["private-password", "123456", "incorrect-response"] {
        assert!(!serialized.contains(secret));
    }
    if !trusted {
        assert!(!events
            .iter()
            .any(|event| event.kind == "auth_interactive_request"));
    } else if matches!(answer, Answer::Correct) {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == "auth_success")
                .count(),
            1
        );
        if require_key || password_first {
            assert!(events
                .iter()
                .any(|event| event.kind == "auth_partial_success"));
        }
    } else {
        assert!(events
            .iter()
            .any(|event| event.kind == "auth_interactive_failed"));
    }
    drop(events);
    server.abort();
}

#[test]
fn keyboard_interactive_real_ssh_challenges() {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            exercise(false, false, false, Answer::Correct, true).await;
            exercise(true, false, false, Answer::Correct, true).await;
            exercise(false, true, false, Answer::Correct, true).await;
            exercise(false, false, true, Answer::Correct, true).await;
            exercise(false, false, false, Answer::Wrong, true).await;
            exercise(false, false, false, Answer::Cancel, true).await;
            exercise(false, false, false, Answer::DisconnectThenRespond, true).await;
            exercise(false, false, false, Answer::Correct, false).await;
        });
}

#[test]
fn keyboard_interactive_transport_roundtrip_and_shutdown() {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for stop_while_pending in [false, true] {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let address = listener.local_addr().unwrap();
                let config = Arc::new(server::Config {
                    keys: vec![Ed25519Keypair::from_seed(&[6; 32]).into()],
                    auth_rejection_time: Duration::ZERO,
                    auth_rejection_time_initial: Some(Duration::ZERO),
                    ..Default::default()
                });
                let server = tokio::spawn(async move {
                    let (stream, _) = listener.accept().await.unwrap();
                    let session = server::run_stream(
                        config,
                        stream,
                        TestServer {
                            round: 0,
                            require_key: false,
                            password_first: false,
                            empty_round: false,
                        },
                    )
                    .await
                    .unwrap();
                    let _ = session.await;
                });
                let known_hosts = std::env::temp_dir().join(format!(
                    "nauterm-kbd-{}-{}.known_hosts",
                    std::process::id(),
                    address.port()
                ));
                let mut transport = SshTransport::connect(
                    TerminalGeometry::new(80, 24),
                    TerminalOptions::default(),
                    "127.0.0.1",
                    address.port(),
                    "testuser",
                    None,
                    None,
                    None,
                    None,
                    known_hosts.to_str(),
                    HostKeyTrustMode::AcceptOnce,
                    None,
                    20,
                    "UTF-8",
                )
                .unwrap();
                let interaction = transport.authentication.clone();
                let last_id = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        for event in transport.drain_output().events {
                            if let Some(challenge) = event.challenge {
                                if stop_while_pending {
                                    return challenge.id;
                                }
                                let responses = if challenge.prompts.len() == 2 {
                                    vec!["account".into(), "private-password".into()]
                                } else {
                                    vec!["123456".into()]
                                };
                                assert!(transport
                                    .respond_to_authentication(challenge.id, Some(responses)));
                            }
                            if event.kind == "auth_success" {
                                return 0;
                            }
                            assert_ne!(event.kind, "auth_interactive_failed", "{}", event.message);
                            assert_ne!(event.kind, "error", "{}", event.message);
                        }
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                })
                .await
                .expect("SSH transport must deliver and accept challenges");
                drop(transport);
                assert!(
                    !interaction.respond(last_id, None),
                    "shutdown must clear the pending challenge"
                );
                server.abort();
            }
        });
}
