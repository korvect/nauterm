use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::*;

pub(super) async fn connect_ssh_with_timeout(
    config: Arc<client::Config>,
    host: &str,
    port: u16,
    proxy: Option<&SshProxyConfig>,
    handler: SshClientHandler,
) -> Result<client::Handle<SshClientHandler>, String> {
    tokio::time::timeout(SSH_CONNECT_TIMEOUT, async {
        let stream = connect_ssh_tcp_stream(host, port, proxy).await?;
        client::connect_stream(config, stream, handler)
            .await
            .map_err(|error| format!("connect failed: {error}"))
    })
    .await
    .map_err(|_| {
        format!(
            "connect timed out after {} seconds",
            SSH_CONNECT_TIMEOUT.as_secs()
        )
    })?
}

pub(super) async fn connect_ssh_with_timeout_and_udp_host(
    config: Arc<client::Config>,
    host: &str,
    port: u16,
    proxy: Option<&SshProxyConfig>,
    handler: SshClientHandler,
) -> Result<(client::Handle<SshClientHandler>, SelectedMoshUdpHost), String> {
    tokio::time::timeout(SSH_CONNECT_TIMEOUT, async {
        let (stream, udp_host) = connect_ssh_tcp_stream_with_udp_host(host, port, proxy).await?;
        let handle = client::connect_stream(config, stream, handler)
            .await
            .map_err(|error| format!("connect failed: {error}"))?;
        Ok((handle, udp_host))
    })
    .await
    .map_err(|_| {
        format!(
            "connect timed out after {} seconds",
            SSH_CONNECT_TIMEOUT.as_secs()
        )
    })?
}

pub(crate) async fn connect_ssh_tcp_stream(
    host: &str,
    port: u16,
    proxy: Option<&SshProxyConfig>,
) -> Result<TcpStream, String> {
    connect_ssh_tcp_stream_with_udp_host(host, port, proxy)
        .await
        .map(|(stream, _)| stream)
}

async fn connect_ssh_tcp_stream_with_udp_host(
    host: &str,
    port: u16,
    proxy: Option<&SshProxyConfig>,
) -> Result<(TcpStream, SelectedMoshUdpHost), String> {
    let Some(proxy) = proxy else {
        let stream = TcpStream::connect((host, port))
            .await
            .map_err(|error| format!("connect failed: {error}"))?;
        let udp_host = preferred_mosh_udp_host(host, None, stream.peer_addr().ok());
        return Ok((stream, udp_host));
    };
    let mut stream = TcpStream::connect((proxy.host.as_str(), proxy.port))
        .await
        .map_err(|error| {
            format!(
                "failed to connect {} proxy {}:{}: {error}",
                proxy.proxy_type.to_uppercase(),
                proxy.host,
                proxy.port
            )
        })?;
    match proxy.proxy_type.as_str() {
        "http" => connect_http_proxy(&mut stream, host, port, proxy).await?,
        "socks5" => connect_socks5_proxy(&mut stream, host, port, proxy).await?,
        _ => return Err(format!("unsupported proxy type: {}", proxy.proxy_type)),
    }
    Ok((stream, preferred_mosh_udp_host(host, Some(proxy), None)))
}

pub(super) fn preferred_mosh_udp_host(
    host: &str,
    proxy: Option<&SshProxyConfig>,
    peer_addr: Option<std::net::SocketAddr>,
) -> SelectedMoshUdpHost {
    if proxy.is_none() {
        if let Some(peer_addr) = peer_addr {
            return SelectedMoshUdpHost {
                host: peer_addr.ip().to_string(),
                source: MoshUdpHostSource::ConnectedPeerIp,
            };
        }
    }
    SelectedMoshUdpHost {
        host: normalize_mosh_udp_host(host),
        source: MoshUdpHostSource::OriginalHost,
    }
}

fn normalize_mosh_udp_host(host: &str) -> String {
    let host = host.trim();
    if let Some(stripped) = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        if !stripped.is_empty() {
            return stripped.to_owned();
        }
    }
    host.to_owned()
}

async fn connect_http_proxy(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    proxy: &SshProxyConfig,
) -> Result<(), String> {
    let authority = proxy_target_authority(host, port);
    let mut request = format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nProxy-Connection: Keep-Alive\r\n"
    );
    if let Some(username) = proxy.username.as_deref() {
        let credentials = format!("{}:{}", username, proxy.password.as_deref().unwrap_or(""));
        request.push_str("Proxy-Authorization: Basic ");
        request.push_str(&BASE64_STANDARD.encode(credentials));
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| format!("failed to write HTTP proxy request: {error}"))?;

    let mut response = Vec::new();
    let mut buffer = [0u8; 1024];
    while !response.windows(4).any(|window| window == b"\r\n\r\n") {
        // Peek to locate the exact header boundary without consuming an SSH
        // identification/packet coalesced after the CONNECT response.
        let available = stream
            .peek(&mut buffer)
            .await
            .map_err(|error| format!("failed to read HTTP proxy response: {error}"))?;
        if available == 0 {
            return Err("HTTP proxy closed before CONNECT completed.".to_owned());
        }
        let tail = &response[response.len().saturating_sub(3)..];
        let mut lookahead = Vec::with_capacity(tail.len() + available);
        lookahead.extend_from_slice(tail);
        lookahead.extend_from_slice(&buffer[..available]);
        let consume = lookahead
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|end| end + 4 - tail.len())
            .unwrap_or(available);
        stream
            .read_exact(&mut buffer[..consume])
            .await
            .map_err(|error| format!("failed to read HTTP proxy response: {error}"))?;
        response.extend_from_slice(&buffer[..consume]);
        if response.len() > 64 * 1024 {
            return Err("HTTP proxy response is too large.".to_owned());
        }
    }
    let response_text = String::from_utf8_lossy(&response);
    let status_line = response_text.lines().next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    if (200..300).contains(&status) {
        return Ok(());
    }
    Err(format!(
        "HTTP proxy CONNECT failed{}.",
        if status == 0 {
            "".to_owned()
        } else {
            format!(" with status {status}")
        }
    ))
}

async fn connect_socks5_proxy(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    proxy: &SshProxyConfig,
) -> Result<(), String> {
    let use_password = proxy.username.as_deref().is_some();
    if let Some(username) = proxy.username.as_deref() {
        if username.len() > u8::MAX as usize {
            return Err("SOCKS5 proxy username is too long.".to_owned());
        }
    }
    if let Some(password) = proxy.password.as_deref() {
        if password.len() > u8::MAX as usize {
            return Err("SOCKS5 proxy password is too long.".to_owned());
        }
    }
    let methods: &[u8] = if use_password {
        &[0x05, 0x02, 0x00, 0x02]
    } else {
        &[0x05, 0x01, 0x00]
    };
    stream
        .write_all(methods)
        .await
        .map_err(|error| format!("failed to write SOCKS5 greeting: {error}"))?;
    let mut greeting = [0u8; 2];
    stream
        .read_exact(&mut greeting)
        .await
        .map_err(|error| format!("failed to read SOCKS5 greeting: {error}"))?;
    if greeting[0] != 0x05 {
        return Err("SOCKS5 proxy returned an invalid version.".to_owned());
    }
    match greeting[1] {
        0x00 => {}
        0x02 => authenticate_socks5_proxy(stream, proxy).await?,
        0xff => return Err("SOCKS5 proxy rejected supported auth methods.".to_owned()),
        method => {
            return Err(format!(
                "SOCKS5 proxy selected unsupported auth method {method}."
            ))
        }
    }

    let host_bytes = host.as_bytes();
    if host_bytes.len() > u8::MAX as usize {
        return Err("SOCKS5 target host is too long.".to_owned());
    }
    let mut request = Vec::with_capacity(7 + host_bytes.len());
    request.extend_from_slice(&[0x05, 0x01, 0x00, 0x03, host_bytes.len() as u8]);
    request.extend_from_slice(host_bytes);
    request.extend_from_slice(&port.to_be_bytes());
    stream
        .write_all(&request)
        .await
        .map_err(|error| format!("failed to write SOCKS5 CONNECT request: {error}"))?;
    let mut header = [0u8; 4];
    stream
        .read_exact(&mut header)
        .await
        .map_err(|error| format!("failed to read SOCKS5 CONNECT response: {error}"))?;
    if header[0] != 0x05 {
        return Err("SOCKS5 proxy returned an invalid CONNECT response.".to_owned());
    }
    if header[1] != 0x00 {
        return Err(format!(
            "SOCKS5 proxy CONNECT failed with reply {}.",
            header[1]
        ));
    }
    let address_length = match header[3] {
        0x01 => 4,
        0x03 => {
            let mut length = [0u8; 1];
            stream
                .read_exact(&mut length)
                .await
                .map_err(|error| format!("failed to read SOCKS5 bound address: {error}"))?;
            length[0] as usize
        }
        0x04 => 16,
        atyp => {
            return Err(format!(
                "SOCKS5 proxy returned unsupported address type {atyp}."
            ))
        }
    };
    let mut discard = vec![0u8; address_length + 2];
    stream
        .read_exact(&mut discard)
        .await
        .map_err(|error| format!("failed to read SOCKS5 bound address: {error}"))?;
    Ok(())
}

async fn authenticate_socks5_proxy(
    stream: &mut TcpStream,
    proxy: &SshProxyConfig,
) -> Result<(), String> {
    let username = proxy.username.as_deref().unwrap_or("");
    let password = proxy.password.as_deref().unwrap_or("");
    let mut request = Vec::with_capacity(3 + username.len() + password.len());
    request.push(0x01);
    request.push(username.len() as u8);
    request.extend_from_slice(username.as_bytes());
    request.push(password.len() as u8);
    request.extend_from_slice(password.as_bytes());
    stream
        .write_all(&request)
        .await
        .map_err(|error| format!("failed to write SOCKS5 auth request: {error}"))?;
    let mut response = [0u8; 2];
    stream
        .read_exact(&mut response)
        .await
        .map_err(|error| format!("failed to read SOCKS5 auth response: {error}"))?;
    if response == [0x01, 0x00] {
        Ok(())
    } else {
        Err("SOCKS5 proxy authentication failed.".to_owned())
    }
}

fn proxy_target_authority(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}
