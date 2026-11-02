//! Transient Telnet connection and bounded, one-shot login-prompt handling.
//! Saved Host protocol profiles are resolved into this runtime configuration.
use anyhow::{Result, ensure};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub port: u16,
    pub username: String,
    pub identity_id: Option<i64>,
    pub encoding: crate::host_session::Encoding,
    pub theme: Option<crate::terminal_appearance::ThemeId>,
    pub source: String,
    pub password_id: Option<String>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            port: 23,
            username: String::new(),
            identity_id: None,
            encoding: Default::default(),
            theme: None,
            source: String::new(),
            password_id: None,
        }
    }
}
impl Profile {
    pub fn project(
        source: String,
        port: Option<i64>,
        username: Option<&str>,
        identity_id: Option<i64>,
        theme: Option<&str>,
        encoding: Option<&str>,
    ) -> Result<Self> {
        let profile = Self {
            source,
            port: u16::try_from(port.unwrap_or(23))?,
            username: username.unwrap_or_default().into(),
            identity_id,
            password_id: None,
            theme: theme.and_then(crate::terminal_appearance::ThemeId::from_id),
            encoding: encoding
                .and_then(|value| {
                    crate::host_session::Encoding::ALL
                        .into_iter()
                        .find(|encoding| encoding.label().eq_ignore_ascii_case(value))
                })
                .unwrap_or_default(),
        };
        profile.validate()?;
        Ok(profile)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.port != 0, "Telnet port must be between 1 and 65535.");
        ensure!(
            self.identity_id.is_none_or(|id| id > 0),
            "Invalid Telnet identity."
        );
        ensure!(
            self.source.is_empty()
                || crate::resource_reference::Reference::parse(&self.source).is_some_and(
                    |reference| {
                        reference.protocol == crate::resource_reference::Protocol::Telnet
                    }
                ),
            "Invalid shared Telnet profile reference."
        );
        ensure!(
            self.password_id
                .as_deref()
                .is_none_or(|value| crate::host_password::valid_reference(value)
                    && crate::resource_reference::Reference::parse(value)
                        .is_none_or(|reference| reference.protocol
                            == crate::resource_reference::Protocol::Telnet)),
            "Invalid Telnet password reference."
        );
        Ok(())
    }
}

pub fn inherit_defaults(
    profile: &mut Profile,
    parent: &Profile,
    flags: &mut crate::host_session::Inheritance,
) {
    if !flags.port_configured && matches!(profile.port, 0 | 23) {
        profile.port = parent.port;
        flags.port_configured = true;
    }
    if profile.identity_id.is_none()
        && profile.username.trim().is_empty()
        && profile.password_id.is_none()
        && !flags.password_configured
    {
        profile.identity_id = parent.identity_id;
        if profile.identity_id.is_none() {
            profile.username = parent.username.clone();
            profile.password_id = parent.password_id.clone();
        }
    }
    if !flags.encoding_configured {
        profile.encoding = parent.encoding;
        flags.encoding_configured = true;
    }
    profile.theme = profile.theme.or(parent.theme);
}

pub struct Resolved {
    pub title: String,
    pub options: Options,
}

fn resolve_host(
    row: &mut nauterm_core::HostEntry,
    identity: Option<&mut crate::vault::IdentityRecord>,
    source: String,
) -> Result<Resolved> {
    ensure!(
        row.host_type == nauterm_core::HostType::Remote && row.telnet_enabled.unwrap_or(false),
        "Telnet is not enabled for this Host."
    );
    let profile = Profile::project(
        source,
        row.telnet_port,
        row.telnet_username.as_deref(),
        row.telnet_identity_id,
        row.telnet_theme_id.as_deref(),
        row.telnet_encoding.as_deref(),
    )?;
    let (identity_user, identity_password) = identity
        .map(|identity| {
            (
                identity.username.clone(),
                identity.password.take().map(Zeroizing::new),
            )
        })
        .unwrap_or_default();
    let options = Options {
        host: row.host.clone().unwrap_or_default(),
        port: profile.port,
        username: identity_user
            .filter(|value| !value.trim().is_empty())
            .or_else(|| row.telnet_username.clone())
            .unwrap_or_default(),
        password: identity_password
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| Zeroizing::new(row.telnet_password.take().unwrap_or_default())),
        encoding: profile.encoding,
    };
    options.validate()?;
    Ok(Resolved {
        title: row.name.clone(),
        options,
    })
}

impl crate::vault::Vault {
    pub async fn saved_telnet_host(&self, source: String) -> Result<Resolved> {
        let reference = crate::resource_reference::Reference::parse(&source)
            .ok_or_else(|| anyhow::anyhow!("Invalid saved Telnet Host reference."))?;
        ensure!(
            reference.kind == crate::resource_reference::Kind::Host
                && reference.protocol == crate::resource_reference::Protocol::Telnet,
            "Select a saved Telnet Host."
        );
        self.dispatch(move |db| {
            db.inner.atomic(|database| {
                let mut profile = reference.load(database)?;
                let crate::resource_reference::Profile::Host(row) = &mut profile else {
                    anyhow::bail!("Select a saved Telnet Host.");
                };
                let mut identity = row
                    .telnet_identity_id
                    .map(|id| database.get_identity(id))
                    .transpose()?
                    .flatten()
                    .map(|mut identity| crate::vault::IdentityRecord {
                        id: identity.id,
                        name: std::mem::take(&mut identity.name),
                        username: identity.username.take(),
                        password: identity.password.take(),
                        key_id: identity.key_id,
                    });
                resolve_host(row, identity.as_mut(), source)
            })
        })
        .await
    }
}

#[derive(Clone)]
pub struct Options {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Zeroizing<String>,
    pub encoding: crate::host_session::Encoding,
}
impl Options {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.host.trim().is_empty()
                && !self.host.chars().any(char::is_control)
                && self.port != 0,
            "A Telnet host and port are required."
        );
        ensure!(
            !self.username.contains(['\r', '\n', '\0'])
                && !self.password.contains(['\r', '\n', '\0']),
            "Telnet login values cannot contain line breaks or NUL characters."
        );
        Ok(())
    }
    pub fn endpoint(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
    /// Quick Connect accepts an endpoint only. Credentials come from a saved
    /// profile or the interactive remote prompt, never a password-bearing URL.
    pub fn from_uri(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.chars().any(char::is_control) {
            return None;
        }
        let uri = url::Url::parse(value).ok()?;
        if uri.scheme() != "telnet"
            || !uri.username().is_empty()
            || uri.password().is_some()
            || !matches!(uri.path(), "" | "/")
            || uri.query().is_some()
            || uri.fragment().is_some()
        {
            return None;
        }
        let host = match uri.host()? {
            url::Host::Domain(host) => host.to_owned(),
            url::Host::Ipv4(host) => host.to_string(),
            url::Host::Ipv6(host) => host.to_string(),
        };
        let options = Self {
            host,
            port: uri.port().unwrap_or(23),
            username: String::new(),
            password: Zeroizing::new(String::new()),
            encoding: Default::default(),
        };
        options.validate().ok()?;
        Some(options)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Credential {
    Username,
    Password,
}
#[derive(Default)]
pub(crate) struct Login {
    tail: Zeroizing<Vec<u8>>,
    sent_username: bool,
    sent_password: bool,
    done: bool,
}
impl Login {
    /// Native capture is UTF-8 output after Telnet negotiation and
    /// configured character decoding. Only the nine-byte prompt suffix lives
    /// here, never a recording/pre-authentication raw-output buffer.
    pub(crate) fn feed_bytes(&mut self, bytes: &[u8], options: &Options) -> Vec<Credential> {
        if self.done {
            return vec![];
        }
        if options.username.trim().is_empty() && options.password.is_empty() {
            self.done = true;
            return vec![];
        }
        let mut credentials = vec![];
        for byte in bytes {
            if self.done {
                break;
            }
            self.tail.push(byte.to_ascii_lowercase());
            if self.tail.len() > 9 {
                self.tail.remove(0);
            }
            if !self.sent_username
                && !options.username.trim().is_empty()
                && (self.tail.ends_with(b"login:") || self.tail.ends_with(b"username:"))
            {
                self.sent_username = true;
                credentials.push(Credential::Username);
                self.tail.zeroize();
                self.done = options.password.is_empty();
            } else if !self.sent_password
                && !options.password.is_empty()
                && self.tail.ends_with(b"password:")
            {
                self.sent_password = true;
                self.done = true;
                credentials.push(Credential::Password);
                self.tail.zeroize();
            }
        }
        credentials
    }
}

pub(crate) struct Session {
    pub options: Options,
    pub login: Login,
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = "native:host:1:12345678-1234-1234-1234-123456789abc:telnet";
    fn saved_host() -> nauterm_core::HostEntry {
        serde_json::from_value(serde_json::json!({
            "id":1,"uuid":"12345678-1234-1234-1234-123456789abc","name":"Saved Telnet",
            "type":"remote","host":"example.test","identity_id":null,
            "ssh_enabled":true,"port":2022,"username":"ssh-user","password":"SYNTHETIC_SSH",
            "telnet_enabled":true,"telnet_port":2323,"telnet_username":"telnet-user",
            "telnet_password":"SYNTHETIC_TELNET","telnet_encoding":"GBK","telnet_identity_id":7
        }))
        .unwrap()
    }
    #[test]
    fn saved_profiles_use_telnet_fields_and_identity_fallbacks() -> Result<()> {
        let mut row = saved_host();
        let mut identity = crate::vault::IdentityRecord {
            id: Some(7),
            name: "Synthetic identity".into(),
            username: Some("identity-user".into()),
            password: Some("SYNTHETIC_IDENTITY".into()),
            key_id: Some(99),
        };
        let resolved = resolve_host(&mut row, Some(&mut identity), SOURCE.into())?;
        assert_eq!(resolved.options.host, "example.test");
        assert_eq!(resolved.options.port, 2323);
        assert_eq!(
            resolved.options.encoding,
            crate::host_session::Encoding::Gbk
        );
        assert_eq!(resolved.options.username, "identity-user");
        assert_eq!(resolved.options.password.as_str(), "SYNTHETIC_IDENTITY");
        assert!(identity.password.is_none());
        identity.username = Some(String::new());
        identity.password = Some(String::new());
        let resolved = resolve_host(&mut row, Some(&mut identity), SOURCE.into())?;
        assert_eq!(resolved.options.username, "telnet-user");
        assert_eq!(resolved.options.password.as_str(), "SYNTHETIC_TELNET");
        assert_eq!(row.password.as_deref(), Some("SYNTHETIC_SSH"));
        row.telnet_enabled = Some(false);
        assert!(resolve_host(&mut row, None, SOURCE.into()).is_err());
        Ok(())
    }
    #[test]
    fn protocol_projection_keeps_telnet_separate_from_ssh() -> Result<()> {
        let profile = Profile::project(SOURCE.into(), None, None, None, None, None)?;
        assert_eq!(profile.port, 23);
        let host = crate::model::Host {
            connections: crate::model::ConnectionProfiles {
                mosh_command: crate::mosh::DEFAULT_COMMAND.into(),
                ssh: false,
                mosh: false,
                telnet: Some(profile),
            },
            ..Default::default()
        };
        assert!(!host.supports_ssh());
        assert_eq!(host.protocol_summary(), "telnet");
        assert!(crate::model::AppState::default().host_matches(&host, "protocol:telnet"));
        assert!(Profile::project(SOURCE.into(), Some(0), None, None, None, None).is_err());
        assert!(
            Profile::project(SOURCE.replace(":telnet", ""), None, None, None, None, None).is_err()
        );
        let restored: crate::recovery::Target = serde_json::from_str(&serde_json::to_string(
            &crate::recovery::Target::TelnetHost { id: 1, port: None },
        )?)?;
        assert_eq!(
            restored,
            crate::recovery::Target::TelnetHost { id: 1, port: None }
        );
        Ok(())
    }
    #[test]
    fn group_defaults_fill_empty_telnet_values_without_replacing_explicit_choices() -> Result<()> {
        let parent = Profile {
            port: 2323,
            username: "group-user".into(),
            password_id: Some(SOURCE.replace(":host:", ":group:")),
            encoding: crate::host_session::Encoding::Big5,
            theme: Some(crate::terminal_appearance::ThemeId::NysaDark),
            ..Default::default()
        };
        let mut child = Profile::default();
        let mut flags = crate::host_session::Inheritance::default();
        inherit_defaults(&mut child, &parent, &mut flags);
        assert_eq!(child.port, 2323);
        assert_eq!(child.username, "group-user");
        assert_eq!(child.password_id, parent.password_id);
        assert_eq!(child.encoding, parent.encoding);
        assert_eq!(child.theme, parent.theme);
        child.validate()?;
        let another = Profile {
            port: 9923,
            username: "another".into(),
            encoding: crate::host_session::Encoding::Gbk,
            ..Default::default()
        };
        inherit_defaults(&mut child, &another, &mut flags);
        assert_eq!(child.port, 2323);
        assert_eq!(child.username, "group-user");
        assert_eq!(child.encoding, parent.encoding);
        let mut manual = Profile {
            port: 23,
            username: "typed-user".into(),
            encoding: crate::host_session::Encoding::Utf8,
            ..Default::default()
        };
        let mut explicit = crate::host_session::Inheritance {
            port_configured: true,
            encoding_configured: true,
            password_configured: true,
        };
        inherit_defaults(&mut manual, &parent, &mut explicit);
        assert_eq!(manual.port, 23);
        assert_eq!(manual.username, "typed-user");
        assert!(manual.password_id.is_none());
        assert_eq!(manual.encoding, crate::host_session::Encoding::Utf8);
        let mut identity = Profile::default();
        inherit_defaults(
            &mut identity,
            &Profile {
                identity_id: Some(7),
                ..parent
            },
            &mut Default::default(),
        );
        assert_eq!(identity.identity_id, Some(7));
        assert!(identity.username.is_empty());
        assert!(identity.password_id.is_none());
        let invalid = Profile {
            password_id: Some(SOURCE.replace(":telnet", "")),
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
        Ok(())
    }
    #[test]
    fn explicit_endpoint_uris_validate_ports_ipv6_and_exclude_credentials() {
        let options = Options::from_uri("telnet://[::1]:2323").unwrap();
        assert_eq!(options.host, "::1");
        assert_eq!(options.endpoint(), "[::1]:2323");
        assert_eq!(Options::from_uri("telnet://localhost").unwrap().port, 23);
        for value in [
            "telnet://local\nhost",
            "ssh://localhost",
            "telnet://user@localhost",
            "telnet://user:password@localhost",
            "telnet://localhost:0",
            "telnet://localhost/path",
            "telnet://localhost?query",
            "telnet://localhost#fragment",
        ] {
            assert!(Options::from_uri(value).is_none());
        }
    }
    #[test]
    fn login_prompts_cross_chunks_are_bounded_and_each_credential_is_sent_once() {
        let mut options = Options::from_uri("telnet://localhost").unwrap();
        options.username = " test-user ".into();
        options.password = Zeroizing::new("test-password".into());
        let mut login = Login::default();
        assert!(login.feed_bytes(b"Lo", &options).is_empty());
        assert_eq!(
            login.feed_bytes(b"GiN:", &options),
            vec![Credential::Username]
        );
        assert!(login.feed_bytes(b"login: PAss", &options).is_empty());
        assert_eq!(
            login.feed_bytes(b"WOrd:", &options),
            vec![Credential::Password]
        );
        assert!(login.feed_bytes(b"login: password:", &options).is_empty());
        assert!(login.tail.is_empty());
        let mut login = Login::default();
        assert!(login.feed_bytes(&[b'x'; 10000], &options).is_empty());
        assert!(login.tail.len() <= 9);
        let mut login = Login::default();
        let mut large = b"login:".to_vec();
        large.extend_from_slice(&[b'x'; 10000]);
        assert_eq!(
            login.feed_bytes(&large, &options),
            vec![Credential::Username]
        );
        assert!(login.tail.len() <= 9);
        options.username = "injected\nline".into();
        assert!(options.validate().is_err());
    }
}
