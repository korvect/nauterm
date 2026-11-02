use anyhow::{Context, Result, ensure};

#[derive(Debug, PartialEq, Eq)]
pub struct SshAddress {
    pub host: String,
    pub port: u16,
    pub username: String,
}
impl SshAddress {
    pub fn command(&self) -> String {
        let destination = if self.username.is_empty() {
            self.host.clone()
        } else {
            format!("{}@{}", self.username, self.host)
        };
        let argument = if !destination.is_empty()
            && destination
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-:/@%+".contains(&byte))
        {
            destination.clone()
        } else {
            format!("'{}'", destination.replace('\'', "'\\''"))
        };
        format!(
            "ssh -p {} {}{argument}",
            self.port,
            if destination.starts_with('-') {
                "-- "
            } else {
                ""
            }
        )
    }
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim().strip_prefix("ssh://").unwrap_or(input.trim());
        ensure!(
            !input.is_empty() && !input.chars().any(|c| c.is_whitespace() || c == '\0'),
            "Enter an SSH address"
        );
        let (username, authority) = match input.rsplit_once('@') {
            Some((user, authority)) => {
                ensure!(
                    !user.is_empty() && !user.contains(':'),
                    "Use username@host; do not enter passwords in an address"
                );
                (user, authority)
            }
            None => ("", input),
        };
        let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
            let (host, tail) = bracketed
                .split_once(']')
                .context("Close the IPv6 address with ]")?;
            host.parse::<std::net::Ipv6Addr>()
                .context("Invalid IPv6 address")?;
            let port = if tail.is_empty() {
                22
            } else {
                tail.strip_prefix(':')
                    .context("Expected :port after IPv6 address")?
                    .parse::<u16>()?
            };
            (host, port)
        } else if authority.parse::<std::net::Ipv6Addr>().is_ok() {
            (authority, 22)
        } else if let Some((host, port)) = authority.rsplit_once(':') {
            (host, port.parse::<u16>().context("Invalid SSH port")?)
        } else {
            (authority, 22)
        };
        ensure!(
            !host.is_empty() && !host.contains(['/', '\\', '?', '#', '[', ']']) && port != 0,
            "Invalid SSH host or port"
        );
        Ok(Self {
            host: host.to_owned(),
            port,
            username: username.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copied_commands_use_user_at_host_and_quote_only_when_needed() {
        assert_eq!(
            SshAddress::parse("admin@10.10.1.157:2022")
                .unwrap()
                .command(),
            "ssh -p 2022 admin@10.10.1.157"
        );
        assert_eq!(
            SshAddress::parse("example.test").unwrap().command(),
            "ssh -p 22 example.test"
        );
        assert_eq!(
            SshAddress::parse("admin@[::1]:2022").unwrap().command(),
            "ssh -p 2022 admin@::1"
        );
        let address = SshAddress {
            host: "example.test".into(),
            port: 22,
            username: "a'b$(id)".into(),
        };
        assert_eq!(address.command(), "ssh -p 22 'a'\\''b$(id)@example.test'");
        let address = SshAddress {
            host: "-example".into(),
            port: 22,
            username: String::new(),
        };
        assert_eq!(address.command(), "ssh -p 22 -- -example");
    }
    #[test]
    fn parses_dns_ipv4_bracketed_and_unbracketed_ipv6() {
        assert_eq!(
            SshAddress::parse("ssh://alice@example.test:2222").unwrap(),
            SshAddress {
                host: "example.test".into(),
                port: 2222,
                username: "alice".into()
            }
        );
        assert_eq!(SshAddress::parse("127.0.0.1").unwrap().port, 22);
        assert_eq!(SshAddress::parse("user@[::1]:2200").unwrap().host, "::1");
        assert_eq!(SshAddress::parse("::1").unwrap().port, 22);
    }
    #[test]
    fn rejects_credentials_search_filters_and_invalid_ports() {
        for input in [
            "",
            "host:linux",
            "tag:prod user:root",
            "user:password@host",
            "host:0",
            "host:65536",
            "host/path",
            "user@[::1]junk",
        ] {
            assert!(SshAddress::parse(input).is_err(), "accepted {input}");
        }
    }
}
