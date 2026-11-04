//! Real SSH channels and loopback TCP; no sshd, private hosts or saved credentials.
use super::*;
use russh::server;
use std::net::SocketAddr;

struct Fixture {
    address: SocketAddr,
    trust: tempfile::TempDir,
    sessions: Arc<Mutex<Vec<server::Handle>>>,
    server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn new() -> Self {
        let key: russh::keys::ssh_key::PrivateKey =
            russh::keys::ssh_key::private::Ed25519Keypair::from_seed(&[51; 32]).into();
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let trust = tempfile::tempdir().unwrap();
        known_hosts::learn_known_hosts_path(
            "127.0.0.1",
            address.port(),
            key.public_key(),
            trust.path().join("known_hosts"),
        )
        .unwrap();
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::ZERO,
            ..Default::default()
        });
        let sessions = Arc::new(Mutex::new(Vec::new()));
        let active = sessions.clone();
        let server = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let session = server::run_stream(config.clone(), stream, Handler)
                    .await
                    .unwrap();
                active.lock().unwrap().push(session.handle());
                tokio::spawn(async move {
                    let _ = session.await;
                });
            }
        });
        Self {
            address,
            trust,
            sessions,
            server,
        }
    }

    fn config(&self, kind: &str, destination: u16) -> PortForwardConfig {
        PortForwardConfig {
            id: 1,
            forward_type: kind.into(),
            ssh_host: self.address.ip().to_string(),
            ssh_port: self.address.port(),
            username: "fixture".into(),
            password: Some("fixture-only-password".into()),
            private_key: None,
            certificate: None,
            passphrase: None,
            known_hosts_path: Some(
                self.trust
                    .path()
                    .join("known_hosts")
                    .to_str()
                    .unwrap()
                    .into(),
            ),
            proxy: None,
            bind_address: "127.0.0.1".into(),
            bind_port: 0,
            destination_host: Some("127.0.0.1".into()),
            destination_port: destination,
        }
    }

    async fn disconnect(&self) {
        let sessions = self.sessions.lock().unwrap().clone();
        for session in sessions {
            session
                .disconnect(
                    russh::Disconnect::ByApplication,
                    "fixture disconnected".into(),
                    "en".into(),
                )
                .await
                .unwrap();
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

struct Handler;
impl server::Handler for Handler {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(
            if user == "fixture" && password == "fixture-only-password" {
                server::Auth::Accept
            } else {
                server::Auth::Reject {
                    proceed_with_methods: None,
                    partial_success: false,
                }
            },
        )
    }
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: russh::Channel<server::Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(host, "127.0.0.1");
        let stream = TcpStream::connect((host, port as u16)).await.unwrap();
        reply.accept().await;
        tokio::spawn(proxy_streams(stream, channel.into_stream()));
        Ok(())
    }
    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        assert_eq!(address, "127.0.0.1");
        let listener = TcpListener::bind((address, *port as u16)).await.unwrap();
        *port = u32::from(listener.local_addr().unwrap().port());
        let port = *port;
        let handle = session.handle();
        tokio::spawn(async move {
            let (stream, origin) = listener.accept().await.unwrap();
            let channel = handle
                .channel_open_forwarded_tcpip(
                    "127.0.0.1",
                    port,
                    origin.ip().to_string(),
                    u32::from(origin.port()),
                )
                .await
                .unwrap();
            proxy_streams(stream, channel.into_stream()).await;
        });
        Ok(true)
    }
    async fn cancel_tcpip_forward(
        &mut self,
        _: &str,
        _: u32,
        _: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

async fn wait_running(status: &Arc<Mutex<PortForwardStatus>>) -> u16 {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let current = status.lock().unwrap().clone();
            assert_ne!(current.state, "error", "{:?}", current.error);
            if current.state == "running" {
                return current.bound_port.unwrap();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("forward did not become ready")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_remote_and_dynamic_forward_real_tcp_payload_and_stop() {
    for kind in ["local", "dynamic", "remote"] {
        let fixture = Fixture::new().await;
        let echo = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let destination = echo.local_addr().unwrap().port();
        let echo_task = tokio::spawn(async move {
            let (mut stream, _) = echo.accept().await.unwrap();
            let mut payload = [0; 16];
            stream.read_exact(&mut payload).await.unwrap();
            stream.write_all(&payload).await.unwrap();
            stream.shutdown().await.unwrap();
        });
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(PortForwardStatus::starting(1)));
        let task = tokio::spawn(run_forward(
            fixture.config(kind, destination),
            stop.clone(),
            status.clone(),
        ));
        let bound_port = wait_running(&status).await;
        let mut stream = TcpStream::connect(("127.0.0.1", bound_port)).await.unwrap();
        if kind == "dynamic" {
            stream.write_all(&[5, 1, 0]).await.unwrap();
            let mut response = [0; 2];
            stream.read_exact(&mut response).await.unwrap();
            assert_eq!(response, [5, 0]);
            let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
            request.extend(destination.to_be_bytes());
            stream.write_all(&request).await.unwrap();
            let mut response = [0; 10];
            stream.read_exact(&mut response).await.unwrap();
            assert_eq!(response[1], 0);
        }
        stream.write_all(b"nauterm-forward!").await.unwrap();
        let mut reply = [0; 16];
        tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut reply))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&reply, b"nauterm-forward!");
        echo_task.await.unwrap();
        drop(stream);
        stop.store(true, Ordering::Release);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(status.lock().unwrap().state, "stopped");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_forwards_notice_ssh_disconnect_without_another_client_connection() {
    for kind in ["local", "dynamic", "remote"] {
        let fixture = Fixture::new().await;
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(PortForwardStatus::starting(1)));
        let task = tokio::spawn(run_forward(fixture.config(kind, 9), stop, status.clone()));
        wait_running(&status).await;
        fixture.disconnect().await;
        let error = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("an idle forward did not notice its SSH disconnect")
            .unwrap()
            .unwrap_err();
        assert!(error.contains("SSH connection closed"), "{kind}: {error}");
    }
}
