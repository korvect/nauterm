//! A bounded loopback-only HTTP callback. Invalid requests never consume the
//! authorization; a valid state is required before acknowledging a code/error.
use super::*;
use config::Config;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub(super) struct Server {
    v4: TcpListener,
    v6: Option<TcpListener>,
    authority: String,
    path: &'static str,
    pub redirect: String,
}
enum Decision {
    Reject(u16),
    Error(&'static str),
    Code(Zeroizing<String>),
}
impl Server {
    pub async fn bind(config: &Config) -> Result<Self> {
        for _ in 0..8 {
            let v4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, config.port()))).await
                .map_err(|_| anyhow::anyhow!("Could not open the local sign-in callback. Close another pending sign-in and try again."))?;
            let port = v4
                .local_addr()
                .map_err(|_| anyhow::anyhow!("Could not open the local sign-in callback."))?
                .port();
            let v6 = if config.host() == "localhost" {
                match TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port))).await {
                    Ok(listener) => Some(listener),
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::Unsupported
                        ) =>
                    {
                        None
                    }
                    Err(error)
                        if error.kind() == std::io::ErrorKind::AddrInUse && config.port() == 0 =>
                    {
                        continue;
                    }
                    Err(_) => {
                        return Err(anyhow::anyhow!(
                            "Could not open the local sign-in callback."
                        ));
                    }
                }
            } else {
                None
            };
            let authority = format!("{}:{port}", config.host());
            return Ok(Self {
                v4,
                v6,
                redirect: format!("http://{authority}{}", config.path()),
                authority,
                path: config.path(),
            });
        }
        anyhow::bail!("Could not reserve a local sign-in callback. Try again.")
    }
    async fn accept(&self) -> std::io::Result<TcpStream> {
        if let Some(v6) = &self.v6 {
            match select(Box::pin(self.v4.accept()), Box::pin(v6.accept())).await {
                Either::Left((result, _)) | Either::Right((result, _)) => {
                    result.map(|(stream, _)| stream)
                }
            }
        } else {
            self.v4.accept().await.map(|(stream, _)| stream)
        }
    }
    pub async fn code(
        &self,
        state: &str,
        language: crate::localization::Language,
    ) -> Result<Zeroizing<String>> {
        tokio::time::timeout(Duration::from_secs(300), self.wait(state, language))
            .await
            .map_err(|_| anyhow::anyhow!("Authorization timed out. Try signing in again."))?
    }
    async fn wait(
        &self,
        state: &str,
        language: crate::localization::Language,
    ) -> Result<Zeroizing<String>> {
        for _ in 0..128 {
            let mut stream = self
                .accept()
                .await
                .map_err(|_| anyhow::anyhow!("The sign-in callback stopped."))?;
            let request =
                match tokio::time::timeout(Duration::from_secs(3), read_headers(&mut stream)).await
                {
                    Ok(Some(request)) => request,
                    _ => continue,
                };
            match parse(&request, &self.authority, self.path, state) {
                Decision::Reject(status) => {
                    respond(&mut stream, status, "Request rejected.", language).await
                }
                Decision::Error(message) => {
                    respond(
                        &mut stream,
                        200,
                        "Authorization was not completed. Return to Nauterm RS.",
                        language,
                    )
                    .await;
                    anyhow::bail!("{message}");
                }
                Decision::Code(code) => {
                    respond(
                        &mut stream,
                        200,
                        "Authorization received. Return to Nauterm RS to finish signing in.",
                        language,
                    )
                    .await;
                    return Ok(code);
                }
            }
        }
        anyhow::bail!("Too many invalid sign-in callbacks. Start a new sign-in.")
    }
}
async fn read_headers(stream: &mut TcpStream) -> Option<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    loop {
        let mut chunk = Zeroizing::new([0u8; 1024]);
        let length = stream.read(chunk.as_mut_slice()).await.ok()?;
        if length == 0 || bytes.len().saturating_add(length) > 16 * 1024 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..length]);
        if bytes.windows(4).any(|value| value == b"\r\n\r\n") {
            return Some(bytes);
        }
    }
}
fn parse(bytes: &[u8], authority: &str, path: &str, expected_state: &str) -> Decision {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Decision::Reject(400);
    };
    let Some((headers, _)) = text.split_once("\r\n\r\n") else {
        return Decision::Reject(400);
    };
    let mut lines = headers.split("\r\n");
    let parts: Vec<_> = lines.next().unwrap_or_default().split(' ').collect();
    if parts.len() != 3 || !matches!(parts[2], "HTTP/1.1" | "HTTP/1.0") {
        return Decision::Reject(400);
    }
    if parts[0] != "GET" {
        return Decision::Reject(405);
    }
    let mut host = None;
    let mut content_length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Decision::Reject(400);
        };
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.chars().any(|c| c.is_control() && c != '\t')
        {
            return Decision::Reject(400);
        }
        if name.eq_ignore_ascii_case("host") && host.replace(value.trim()).is_some() {
            return Decision::Reject(400);
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Decision::Reject(400);
        }
        if name.eq_ignore_ascii_case("content-length")
            && content_length.replace(value.trim()).is_some()
        {
            return Decision::Reject(400);
        }
    }
    if host.is_none_or(|host| !host.eq_ignore_ascii_case(authority))
        || content_length.is_some_and(|value| value != "0")
    {
        return Decision::Reject(400);
    }
    let (request_path, query) = parts[1].split_once('?').unwrap_or((parts[1], ""));
    if request_path != path {
        return Decision::Reject(404);
    }
    let mut parameters = std::collections::BTreeMap::new();
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        if parameters.insert(key, value).is_some() {
            return Decision::Reject(400);
        }
    }
    if parameters.get("state").map(|value| value.as_ref()) != Some(expected_state) {
        return Decision::Reject(403);
    }
    if parameters.contains_key("error") {
        return if parameters.contains_key("code") {
            Decision::Reject(400)
        } else {
            Decision::Error(authorization_error(parameters["error"].as_ref()))
        };
    }
    let Some(code) = parameters.get("code").filter(|value| {
        !value.trim().is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
    }) else {
        return Decision::Reject(400);
    };
    Decision::Code(Zeroizing::new(code.to_string()))
}
async fn respond(
    stream: &mut TcpStream,
    status: u16,
    message: &str,
    language: crate::localization::Language,
) {
    let title = if status == 200 {
        "OK"
    } else {
        "Request Rejected"
    };
    let message = crate::localization::text_for(language, message);
    let body =
        format!("<!doctype html><meta charset=utf-8><title>Nauterm RS</title><p>{message}</p>");
    let response = format!(
        "HTTP/1.1 {status} {title}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'\r\n\r\n{body}",
        body.len()
    );
    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        stream.write_all(response.as_bytes()),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(target: &str, headers: &str) -> Vec<u8> {
        format!("GET {target} HTTP/1.1\r\nHost: localhost:1234\r\n{headers}\r\n").into_bytes()
    }
    #[test]
    fn only_the_expected_single_state_code_and_authority_can_complete_a_callback() {
        let Decision::Code(code) = parse(
            &request("/oauth/callback?state=nonce&code=opaque%2Bcode", ""),
            "localhost:1234",
            "/oauth/callback",
            "nonce",
        ) else {
            panic!("valid callback rejected");
        };
        assert_eq!(code.as_str(), "opaque+code");
        for target in [
            "/oauth/callback?state=wrong&code=x",
            "/oauth/callback?code=x",
            "/oauth/callback?state=nonce&code=x&code=y",
            "/oauth/callback?state=nonce&state=nonce&code=x",
            "/oauth/callback?state=nonce&code=x&error=denied",
            "/favicon.ico",
        ] {
            assert!(matches!(
                parse(
                    &request(target, ""),
                    "localhost:1234",
                    "/oauth/callback",
                    "nonce"
                ),
                Decision::Reject(_)
            ));
        }
        for headers in [
            "Host: attacker.invalid\r\n",
            "Content-Length: 20\r\n",
            "Transfer-Encoding: chunked\r\n",
        ] {
            assert!(matches!(
                parse(
                    &request("/oauth/callback?state=nonce&code=x", headers),
                    "localhost:1234",
                    "/oauth/callback",
                    "nonce"
                ),
                Decision::Reject(_)
            ));
        }
        assert!(matches!(
            parse(
                &request("/oauth/callback?state=nonce&error=access_denied", ""),
                "localhost:1234",
                "/oauth/callback",
                "nonce"
            ),
            Decision::Error(_)
        ));
        let Decision::Error(message) = parse(
            &request(
                "/oauth/callback?state=nonce&error=invalid_scope&error_description=synthetic-token",
                "",
            ),
            "localhost:1234",
            "/oauth/callback",
            "nonce",
        ) else {
            panic!("provider error was not classified");
        };
        assert!(message.contains("configured"));
        assert!(!message.contains("synthetic-token"));
    }
}
