//! Public references to shared Host/Group profiles. Secrets remain in the
//! original records and are resolved only for a connection or explicit edit.
use anyhow::{Context, Result, ensure};
use nauterm_core::{HostEntry, HostGroup, NautermDatabase};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Host,
    Group,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Ssh,
    Telnet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub kind: Kind,
    pub id: i64,
    pub uuid: String,
    pub protocol: Protocol,
}

pub fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

impl Reference {
    pub fn parse(value: &str) -> Option<Self> {
        let mut parts = value.split(':');
        if parts.next()? != "native" {
            return None;
        }
        let kind = match parts.next()? {
            "host" => Kind::Host,
            "group" => Kind::Group,
            _ => return None,
        };
        let id = parts.next()?.parse::<i64>().ok().filter(|id| *id > 0)?;
        let uuid = parts.next()?.to_owned();
        if !valid_uuid(&uuid) {
            return None;
        }
        let protocol = match parts.next() {
            None => Protocol::Ssh,
            Some("telnet") => Protocol::Telnet,
            _ => return None,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            kind,
            id,
            uuid,
            protocol,
        })
    }

    pub fn load(&self, database: &NautermDatabase) -> Result<Profile> {
        let profile = match self.kind {
            Kind::Host => Profile::Host(Box::new(
                database
                    .get_host(self.id)?
                    .context("The shared Host was deleted")?,
            )),
            Kind::Group => Profile::Group(Box::new(
                database
                    .get_group(self.id)?
                    .context("The shared Group was deleted")?,
            )),
        };
        ensure!(
            profile.uuid() == Some(self.uuid.as_str()),
            "The shared profile was replaced; select the Host or Group again."
        );
        Ok(profile)
    }
}

pub enum Profile {
    Host(Box<HostEntry>),
    Group(Box<HostGroup>),
}
impl Profile {
    fn uuid(&self) -> Option<&str> {
        match self {
            Self::Host(row) => row.uuid.as_deref(),
            Self::Group(row) => row.uuid.as_deref(),
        }
    }
    pub fn mosh_enabled(&self) -> bool {
        match self {
            Self::Host(row) => {
                row.host_type == nauterm_core::HostType::Remote && row.mosh_enabled.unwrap_or(false)
            }
            Self::Group(row) => row.mosh_enabled.unwrap_or(false),
        }
    }
    pub fn ssh_enabled(&self) -> bool {
        match self {
            Self::Host(row) => row.ssh_enabled,
            Self::Group(row) => row.ssh_enabled,
        }
        .unwrap_or(false)
    }
    pub fn auth_fallback(&self) -> (Option<String>, Option<i64>) {
        let (username, key) = match self {
            Self::Host(row) => (&row.username, row.key_id),
            Self::Group(row) => (&row.username, row.key_id),
        };
        (
            username
                .as_ref()
                .filter(|value| !value.trim().is_empty())
                .cloned(),
            key,
        )
    }
    pub fn password(&mut self, protocol: Protocol) -> Option<Zeroizing<String>> {
        match (self, protocol) {
            (Self::Host(row), Protocol::Ssh) => row.password.take(),
            (Self::Group(row), Protocol::Ssh) => row.password.take(),
            (Self::Host(row), Protocol::Telnet) => row.telnet_password.take(),
            (Self::Group(row), Protocol::Telnet) => row.telnet_password.take(),
        }
        .map(Zeroizing::new)
    }
    pub fn environment(&mut self) -> Vec<crate::host_session::Variable> {
        let entries = match self {
            Self::Host(row) => &mut row.environment_variables,
            Self::Group(row) => &mut row.environment_variables,
        };
        std::mem::take(entries)
            .into_iter()
            .map(|entry| crate::host_session::Variable {
                variable: entry.variable,
                value: entry.value,
            })
            .collect()
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let (password, telnet, environment) = match self {
            Self::Host(row) => (
                &mut row.password,
                &mut row.telnet_password,
                &mut row.environment_variables,
            ),
            Self::Group(row) => (
                &mut row.password,
                &mut row.telnet_password,
                &mut row.environment_variables,
            ),
        };
        password.zeroize();
        telnet.zeroize();
        for entry in environment {
            entry.value.zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mosh_bootstrap_availability_does_not_enable_interactive_ssh_or_local_hosts() -> Result<()> {
        let mut row: nauterm_core::HostEntry = serde_json::from_value(
            serde_json::json!({"name":"Synthetic Mosh", "type":"remote", "identity_id":null, "ssh_enabled":false, "mosh_enabled":true}),
        )?;
        let source = Profile::Host(Box::new(row.clone()));
        assert!(source.mosh_enabled());
        assert!(!source.ssh_enabled());
        row.host_type = nauterm_core::HostType::Local;
        assert!(!Profile::Host(Box::new(row)).mosh_enabled());
        Ok(())
    }
    #[test]
    fn references_keep_table_uuid_and_protocol_without_secret_values() {
        let value = "native:host:1:12345678-1234-1234-1234-123456789abc";
        let parsed = Reference::parse(value).unwrap();
        assert_eq!(parsed.kind, Kind::Host);
        assert_eq!(parsed.protocol, Protocol::Ssh);
        assert_eq!(
            Reference::parse(&format!("{value}:telnet"))
                .unwrap()
                .protocol,
            Protocol::Telnet
        );
        assert!(Reference::parse("native:host:0:12345678-1234-1234-1234-123456789abc").is_none());
        assert!(Reference::parse(&format!("{value}:telnet:extra")).is_none());
        assert!(Reference::parse("native:host:1:invalid").is_none());
    }
}
