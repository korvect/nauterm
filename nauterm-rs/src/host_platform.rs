//! Opportunistic platform metadata belongs to one saved Host identity/endpoint.
use crate::{
    avatar::Platform,
    model::{Host, Id},
};
use anyhow::{Result, ensure};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    pub id: Id,
    pub uuid: String,
    pub address: String,
    pub port: u16,
}
impl Target {
    pub fn for_host(host: &Host) -> Option<Self> {
        let uuid = host
            .uuid
            .as_ref()
            .filter(|uuid| crate::resource_reference::valid_uuid(uuid))?;
        (host.id > 0 && host.is_remote() && (host.connections.ssh || host.connections.mosh)).then(
            || Self {
                id: host.id,
                uuid: uuid.clone(),
                address: host.address.clone(),
                port: host.port,
            },
        )
    }
    pub fn matches(&self, host: &Host) -> bool {
        Self::for_host(host).as_ref() == Some(self)
    }
}
pub fn detected(os: Option<String>, distro: Option<String>) -> Result<Platform> {
    let normalize = |value: Option<String>| {
        value
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let os = normalize(os);
    let distro = normalize(distro);
    ensure!(os.is_some(), "The host returned no operating system.");
    ensure!(
        os.iter()
            .chain(distro.iter())
            .all(|value| value.len() <= 512 && !value.chars().any(char::is_control)),
        "Invalid platform metadata."
    );
    Ok(Platform { os, distro })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platform_results_are_bound_to_saved_remote_identity_and_endpoint() {
        let mut host = Host {
            id: 7,
            uuid: Some("12345678-1234-4000-8000-123456789012".into()),
            address: "fixture.invalid".into(),
            port: 22,
            ..Default::default()
        };
        let target = Target::for_host(&host).unwrap();
        assert!(target.matches(&host));
        host.port = 2022;
        assert!(!target.matches(&host));
        host.port = 22;
        host.uuid = Some("12345678-1234-4000-8000-123456789013".into());
        assert!(!target.matches(&host));
        host.kind = crate::model::HostKind::Local;
        assert!(Target::for_host(&host).is_none());
        assert!(detected(None, Some("Ubuntu".into())).is_err());
        assert!(detected(Some("Linux\0".into()), None).is_err());
        assert_eq!(
            detected(Some(" Linux ".into()), Some(" Ubuntu ".into())).unwrap(),
            Platform {
                os: Some("Linux".into()),
                distro: Some("Ubuntu".into())
            }
        );
    }
}
