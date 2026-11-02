use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use russh::client::{self, Msg};
use russh::keys::known_hosts;
use russh::keys::HashAlg;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::pty::WakeupCallback;
use crate::session::SessionEvent;
use crate::ssh::{authenticate, connect_ssh_tcp_stream, SshProxyConfig};

const FORWARD_POLL_INTERVAL: Duration = Duration::from_millis(80);
const FORWARD_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const SOCKS5_REPLY_SUCCEEDED: u8 = 0x00;
const SOCKS5_REPLY_CONNECTION_REFUSED: u8 = 0x05;
const SOCKS5_REPLY_COMMAND_NOT_SUPPORTED: u8 = 0x07;
const SOCKS5_REPLY_ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;

#[derive(Clone, Debug)]
pub struct PortForwardConfig {
    pub id: u64,
    pub forward_type: String,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub certificate: Option<String>,
    pub passphrase: Option<String>,
    pub known_hosts_path: Option<String>,
    pub proxy: Option<SshProxyConfig>,
    pub bind_address: String,
    pub bind_port: u16,
    pub destination_host: Option<String>,
    pub destination_port: u16,
}

#[derive(Clone, Debug, Serialize)]
pub struct PortForwardStatus {
    pub id: u64,
    pub state: String,
    pub error: Option<String>,
    pub bound_port: Option<u16>,
    pub active_connections: u32,
}

impl PortForwardStatus {
    fn starting(id: u64) -> Self {
        Self {
            id,
            state: "starting".to_owned(),
            error: None,
            bound_port: None,
            active_connections: 0,
        }
    }
}

struct PortForwardRuntime {
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<PortForwardStatus>>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub struct PortForwardManager {
    runtimes: HashMap<u64, PortForwardRuntime>,
}

impl PortForwardManager {
    pub fn start(&mut self, config: PortForwardConfig) -> PortForwardStatus {
        self.stop(config.id);

        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(PortForwardStatus::starting(config.id)));
        let worker_stop = stop.clone();
        let worker_status = status.clone();
        let id = config.id;
        let worker = thread::Builder::new()
            .name(format!("nauterm-forward-{id}"))
            .spawn(move || run_forward_worker(config, worker_stop, worker_status));

        match worker {
            Ok(worker) => {
                self.runtimes.insert(
                    id,
                    PortForwardRuntime {
                        stop,
                        status,
                        worker: Some(worker),
                    },
                );
                self.wait_for_initial_status(id)
            }
            Err(error) => PortForwardStatus {
                id,
                state: "error".to_owned(),
                error: Some(format!("failed to spawn forwarding worker: {error}")),
                bound_port: None,
                active_connections: 0,
            },
        }
    }

    pub fn stop(&mut self, id: u64) -> bool {
        let Some(mut runtime) = self.runtimes.remove(&id) else {
            return false;
        };
        runtime.stop.store(true, Ordering::Release);
        crate::pty::join_worker(&mut runtime.worker, "port forwarding");
        set_status(&runtime.status, "stopped", None, None);
        true
    }

    pub fn stop_all(&mut self) -> usize {
        let ids = self.runtimes.keys().copied().collect::<Vec<_>>();
        let mut stopped = 0;
        for id in ids {
            if self.stop(id) {
                stopped += 1;
            }
        }
        stopped
    }

    pub fn status(&self, id: u64) -> PortForwardStatus {
        self.runtimes
            .get(&id)
            .and_then(|runtime| runtime.status.lock().ok().map(|status| status.clone()))
            .unwrap_or(PortForwardStatus {
                id,
                state: "stopped".to_owned(),
                error: None,
                bound_port: None,
                active_connections: 0,
            })
    }

    fn wait_for_initial_status(&self, id: u64) -> PortForwardStatus {
        for _ in 0..20 {
            let status = self.status(id);
            if status.state != "starting" {
                return status;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        self.status(id)
    }
}

pub fn port_forward_manager() -> &'static Mutex<PortForwardManager> {
    static MANAGER: OnceLock<Mutex<PortForwardManager>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(PortForwardManager::default()))
}

fn run_forward_worker(
    config: PortForwardConfig,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<PortForwardStatus>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            set_status(
                &status,
                "error",
                Some(format!("failed to start forwarding runtime: {error}")),
                None,
            );
            return;
        }
    };

    let result = runtime.block_on(run_forward(config.clone(), stop, status.clone()));
    if let Err(error) = result {
        set_status(&status, "error", Some(error), None);
    }
}

async fn run_forward(
    config: PortForwardConfig,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<PortForwardStatus>>,
) -> Result<(), String> {
    let remote_target = if config.forward_type == "remote" {
        Some(RemoteForwardTarget {
            destination_host: config
                .destination_host
                .clone()
                .filter(|host| !host.trim().is_empty())
                .ok_or_else(|| "remote forwarding requires a destination host".to_owned())?,
            destination_port: config.destination_port,
        })
    } else {
        None
    };
    let mut handle = connect_forward_ssh(&config, remote_target, status.clone()).await?;

    match config.forward_type.as_str() {
        "remote" => run_remote_forward(config, stop, status, &mut handle).await,
        "dynamic" => run_dynamic_forward(config, stop, status, Arc::new(handle)).await,
        _ => run_local_forward(config, stop, status, Arc::new(handle)).await,
    }
}

async fn connect_forward_ssh(
    config: &PortForwardConfig,
    remote_target: Option<RemoteForwardTarget>,
    status: Arc<Mutex<PortForwardStatus>>,
) -> Result<client::Handle<ForwardClientHandler>, String> {
    let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
    let wakeup = Arc::new(Mutex::new(None::<WakeupCallback>));
    let handler = ForwardClientHandler {
        host: config.ssh_host.clone(),
        port: config.ssh_port,
        known_hosts_path: config.known_hosts_path.clone(),
        remote_target,
        status,
    };
    let ssh_config = Arc::new(client::Config {
        nodelay: true,
        keepalive_interval: Some(Duration::from_secs(30)),
        ..client::Config::default()
    });
    let mut handle = tokio::time::timeout(FORWARD_CONNECT_TIMEOUT, async {
        let stream = connect_ssh_tcp_stream(
            config.ssh_host.as_str(),
            config.ssh_port,
            config.proxy.as_ref(),
        )
        .await?;
        client::connect_stream(ssh_config, stream, handler)
            .await
            .map_err(|error| format!("connect failed: {error}"))
    })
    .await
    .map_err(|_| {
        format!(
            "connect timed out after {} seconds",
            FORWARD_CONNECT_TIMEOUT.as_secs()
        )
    })??;
    authenticate(
        &mut handle,
        &config.username,
        config.private_key.as_deref(),
        config.certificate.as_deref(),
        config.passphrase.as_deref(),
        config.password.as_deref(),
        &events,
        &wakeup,
    )
    .await?;
    Ok(handle)
}

async fn run_local_forward(
    config: PortForwardConfig,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<PortForwardStatus>>,
    handle: Arc<client::Handle<ForwardClientHandler>>,
) -> Result<(), String> {
    let destination_host = config
        .destination_host
        .clone()
        .filter(|host| !host.trim().is_empty())
        .ok_or_else(|| "local forwarding requires a destination host".to_owned())?;
    let listener = TcpListener::bind((config.bind_address.as_str(), config.bind_port))
        .await
        .map_err(|error| format!("failed to bind local forward: {error}"))?;
    let bound_port = listener.local_addr().ok().map(|address| address.port());
    set_status(&status, "running", None, bound_port);

    loop {
        if stop.load(Ordering::Acquire) {
            set_status(&status, "stopped", None, bound_port);
            return Ok(());
        }
        match tokio::time::timeout(FORWARD_POLL_INTERVAL, listener.accept()).await {
            Ok(Ok((stream, origin))) => {
                if handle.is_closed() {
                    return Err(
                        "SSH connection closed while local forwarding was active".to_owned()
                    );
                }
                let handle = Arc::clone(&handle);
                let status = status.clone();
                let destination_host = destination_host.clone();
                let destination_port = config.destination_port;
                adjust_active_connections(&status, 1);
                tokio::spawn(async move {
                    let result = handle_local_forward_connection(
                        stream,
                        origin,
                        handle,
                        destination_host,
                        destination_port,
                    )
                    .await;
                    adjust_active_connections(&status, -1);
                    if let Err(error) = result {
                        set_status(&status, "running", Some(error), bound_port);
                    }
                });
            }
            Ok(Err(error)) => return Err(format!("failed to accept local forward: {error}")),
            Err(_) => {}
        }
    }
}

async fn run_dynamic_forward(
    config: PortForwardConfig,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<PortForwardStatus>>,
    handle: Arc<client::Handle<ForwardClientHandler>>,
) -> Result<(), String> {
    let listener = TcpListener::bind((config.bind_address.as_str(), config.bind_port))
        .await
        .map_err(|error| format!("failed to bind SOCKS forward: {error}"))?;
    let bound_port = listener.local_addr().ok().map(|address| address.port());
    set_status(&status, "running", None, bound_port);

    loop {
        if stop.load(Ordering::Acquire) {
            set_status(&status, "stopped", None, bound_port);
            return Ok(());
        }
        match tokio::time::timeout(FORWARD_POLL_INTERVAL, listener.accept()).await {
            Ok(Ok((stream, origin))) => {
                if handle.is_closed() {
                    return Err(
                        "SSH connection closed while SOCKS forwarding was active".to_owned()
                    );
                }
                let handle = Arc::clone(&handle);
                let status = status.clone();
                adjust_active_connections(&status, 1);
                tokio::spawn(async move {
                    let result = handle_socks_connection(stream, origin, handle).await;
                    adjust_active_connections(&status, -1);
                    if let Err(error) = result {
                        set_status(&status, "running", Some(error), bound_port);
                    }
                });
            }
            Ok(Err(error)) => return Err(format!("failed to accept SOCKS forward: {error}")),
            Err(_) => {}
        }
    }
}

async fn run_remote_forward(
    config: PortForwardConfig,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<PortForwardStatus>>,
    handle: &mut client::Handle<ForwardClientHandler>,
) -> Result<(), String> {
    let bound_port = handle
        .tcpip_forward(config.bind_address.clone(), config.bind_port as u32)
        .await
        .map_err(|error| format!("remote forwarding request failed: {error}"))?;
    let bound_port = if bound_port == 0 {
        config.bind_port
    } else {
        bound_port as u16
    };
    set_status(&status, "running", None, Some(bound_port));

    while !stop.load(Ordering::Acquire) && !handle.is_closed() {
        tokio::time::sleep(FORWARD_POLL_INTERVAL).await;
    }
    let _ = handle
        .cancel_tcpip_forward(config.bind_address, bound_port as u32)
        .await;
    set_status(&status, "stopped", None, Some(bound_port));
    Ok(())
}

async fn handle_local_forward_connection(
    stream: TcpStream,
    origin: std::net::SocketAddr,
    handle: Arc<client::Handle<ForwardClientHandler>>,
    destination_host: String,
    destination_port: u16,
) -> Result<(), String> {
    let channel = handle
        .channel_open_direct_tcpip(
            destination_host,
            destination_port as u32,
            origin.ip().to_string(),
            origin.port() as u32,
        )
        .await
        .map_err(|error| format!("failed to open SSH direct-tcpip channel: {error}"))?;
    proxy_streams(stream, channel.into_stream()).await;
    Ok(())
}

async fn handle_socks_connection(
    mut stream: TcpStream,
    origin: std::net::SocketAddr,
    handle: Arc<client::Handle<ForwardClientHandler>>,
) -> Result<(), String> {
    let (host, port) = read_socks5_destination(&mut stream).await?;
    let channel = match handle
        .channel_open_direct_tcpip(
            host,
            port as u32,
            origin.ip().to_string(),
            origin.port() as u32,
        )
        .await
    {
        Ok(channel) => channel,
        Err(error) => {
            let _ = write_socks5_reply(&mut stream, SOCKS5_REPLY_CONNECTION_REFUSED).await;
            return Err(format!("failed to open SOCKS SSH channel: {error}"));
        }
    };
    write_socks5_reply(&mut stream, SOCKS5_REPLY_SUCCEEDED)
        .await
        .map_err(|error| format!("failed to write SOCKS response: {error}"))?;
    proxy_streams(stream, channel.into_stream()).await;
    Ok(())
}

async fn write_socks5_reply(stream: &mut TcpStream, reply: u8) -> std::io::Result<()> {
    stream
        .write_all(&[0x05, reply, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await
}

async fn read_socks5_destination(stream: &mut TcpStream) -> Result<(String, u16), String> {
    let mut greeting = [0u8; 2];
    stream
        .read_exact(&mut greeting)
        .await
        .map_err(|error| format!("failed to read SOCKS greeting: {error}"))?;
    if greeting[0] != 0x05 {
        return Err("only SOCKS5 is supported".to_owned());
    }
    let mut methods = vec![0u8; greeting[1] as usize];
    stream
        .read_exact(&mut methods)
        .await
        .map_err(|error| format!("failed to read SOCKS methods: {error}"))?;
    if !methods.contains(&0x00) {
        stream
            .write_all(&[0x05, 0xff])
            .await
            .map_err(|error| format!("failed to write SOCKS method rejection: {error}"))?;
        return Err("SOCKS client did not offer no-auth method".to_owned());
    }
    stream
        .write_all(&[0x05, 0x00])
        .await
        .map_err(|error| format!("failed to write SOCKS method: {error}"))?;

    let mut header = [0u8; 4];
    stream
        .read_exact(&mut header)
        .await
        .map_err(|error| format!("failed to read SOCKS request: {error}"))?;
    if header[0] != 0x05 {
        let _ = write_socks5_reply(stream, SOCKS5_REPLY_COMMAND_NOT_SUPPORTED).await;
        return Err("only SOCKS5 CONNECT is supported".to_owned());
    }
    let command = header[1];

    let host = match header[3] {
        0x01 => {
            let mut address = [0u8; 4];
            stream
                .read_exact(&mut address)
                .await
                .map_err(|error| format!("failed to read SOCKS IPv4 address: {error}"))?;
            std::net::Ipv4Addr::from(address).to_string()
        }
        0x03 => {
            let mut length = [0u8; 1];
            stream
                .read_exact(&mut length)
                .await
                .map_err(|error| format!("failed to read SOCKS domain length: {error}"))?;
            let mut domain = vec![0u8; length[0] as usize];
            stream
                .read_exact(&mut domain)
                .await
                .map_err(|error| format!("failed to read SOCKS domain: {error}"))?;
            String::from_utf8(domain).map_err(|error| format!("invalid SOCKS domain: {error}"))?
        }
        0x04 => {
            let mut address = [0u8; 16];
            stream
                .read_exact(&mut address)
                .await
                .map_err(|error| format!("failed to read SOCKS IPv6 address: {error}"))?;
            std::net::Ipv6Addr::from(address).to_string()
        }
        _ => {
            let _ = write_socks5_reply(stream, SOCKS5_REPLY_ADDRESS_TYPE_NOT_SUPPORTED).await;
            return Err("unsupported SOCKS address type".to_owned());
        }
    };
    let mut port = [0u8; 2];
    stream
        .read_exact(&mut port)
        .await
        .map_err(|error| format!("failed to read SOCKS port: {error}"))?;
    if command != 0x01 {
        // Consume the complete SOCKS request before replying. Dropping a
        // Windows socket with unread request bytes can send an abortive RST
        // that discards this failure reply before the client receives it.
        let _ = write_socks5_reply(stream, SOCKS5_REPLY_COMMAND_NOT_SUPPORTED).await;
        return Err("only SOCKS5 CONNECT is supported".to_owned());
    }
    Ok((host, u16::from_be_bytes(port)))
}

async fn proxy_streams<A, B>(mut left: A, mut right: B)
where
    A: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    B: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let _ = tokio::io::copy_bidirectional(&mut left, &mut right).await;
    let _ = left.shutdown().await;
    let _ = right.shutdown().await;
}

#[derive(Clone)]
struct RemoteForwardTarget {
    destination_host: String,
    destination_port: u16,
}

#[derive(Clone)]
struct ForwardClientHandler {
    host: String,
    port: u16,
    known_hosts_path: Option<String>,
    remote_target: Option<RemoteForwardTarget>,
    status: Arc<Mutex<PortForwardStatus>>,
}

impl client::Handler for ForwardClientHandler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> impl std::future::Future<Output = Result<bool, Self::Error>> + Send {
        let host = self.host.clone();
        let port = self.port;
        let known_hosts_path = self.known_hosts_path.clone();
        let server_public_key = server_public_key.public_key();
        async move {
            let Some(known_hosts_path) = known_hosts_path else {
                return Ok(false);
            };
            let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();
            let verified = known_hosts::check_known_hosts_path(
                &host,
                port,
                &server_public_key,
                known_hosts_path,
            )
            .map_err(|_| russh::Error::Disconnect)?;
            if !verified {
                eprintln!("Untrusted host key for {host}:{port}: {fingerprint}");
            }
            Ok(verified)
        }
    }

    fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<Msg>,
        _connected_address: &str,
        _connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        let target = self.remote_target.clone();
        let status = self.status.clone();
        async move {
            if let Some(target) = target {
                reply.accept().await;
                adjust_active_connections(&status, 1);
                tokio::spawn(async move {
                    match TcpStream::connect((
                        target.destination_host.as_str(),
                        target.destination_port,
                    ))
                    .await
                    {
                        Ok(stream) => proxy_streams(stream, channel.into_stream()).await,
                        Err(_) => {
                            let _ = channel.close().await;
                        }
                    }
                    adjust_active_connections(&status, -1);
                });
            } else {
                let _ = channel.close().await;
            }
            Ok(())
        }
    }
}

fn set_status(
    status: &Arc<Mutex<PortForwardStatus>>,
    state: &str,
    error: Option<String>,
    bound_port: Option<u16>,
) {
    if let Ok(mut status) = status.lock() {
        status.state = state.to_owned();
        status.error = error;
        status.bound_port = bound_port;
        if state != "running" {
            status.active_connections = 0;
        }
    }
}

fn adjust_active_connections(status: &Arc<Mutex<PortForwardStatus>>, delta: i32) {
    if let Ok(mut status) = status.lock() {
        status.active_connections = if delta.is_negative() {
            status
                .active_connections
                .saturating_sub(delta.unsigned_abs())
        } else {
            status.active_connections.saturating_add(delta as u32)
        };
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn socks_rejects_missing_no_auth_method() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_socks5_destination(&mut stream).await.unwrap_err()
        });

        let mut client = TcpStream::connect(address).await.unwrap();
        client.write_all(&[0x05, 0x01, 0x02]).await.unwrap();
        let mut reply = [0u8; 2];
        client.read_exact(&mut reply).await.unwrap();

        assert_eq!(reply, [0x05, 0xff]);
        assert!(server.await.unwrap().contains("no-auth"));
    }

    #[tokio::test]
    async fn socks_rejects_unsupported_command_with_failure_reply() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_socks5_destination(&mut stream).await.unwrap_err()
        });

        let mut client = TcpStream::connect(address).await.unwrap();
        client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut method_reply = [0u8; 2];
        client.read_exact(&mut method_reply).await.unwrap();
        assert_eq!(method_reply, [0x05, 0x00]);

        client
            .write_all(&[0x05, 0x02, 0x00, 0x01, 127, 0, 0, 1, 0, 80])
            .await
            .unwrap();
        let mut failure_reply = [0u8; 10];
        client.read_exact(&mut failure_reply).await.unwrap();

        assert_eq!(failure_reply[0], 0x05);
        assert_eq!(failure_reply[1], SOCKS5_REPLY_COMMAND_NOT_SUPPORTED);
        assert!(server.await.unwrap().contains("CONNECT"));
    }

    #[tokio::test]
    async fn socks_rejects_unsupported_address_type_with_failure_reply() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_socks5_destination(&mut stream).await.unwrap_err()
        });

        let mut client = TcpStream::connect(address).await.unwrap();
        client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut method_reply = [0u8; 2];
        client.read_exact(&mut method_reply).await.unwrap();
        assert_eq!(method_reply, [0x05, 0x00]);

        client.write_all(&[0x05, 0x01, 0x00, 0x09]).await.unwrap();
        let mut failure_reply = [0u8; 10];
        client.read_exact(&mut failure_reply).await.unwrap();

        assert_eq!(failure_reply[0], 0x05);
        assert_eq!(failure_reply[1], SOCKS5_REPLY_ADDRESS_TYPE_NOT_SUPPORTED);
        assert!(server.await.unwrap().contains("address type"));
    }
}
