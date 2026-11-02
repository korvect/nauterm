//! Explicit stable-release checks. Flutter assets never qualify as RS updates.
use anyhow::{Context, Result, ensure};
use semver::Version;
use serde::Deserialize;
use std::{io::Read, time::Duration};
use url::Url;

#[derive(Clone)]
pub struct Repository(String);
impl Repository {
    pub fn configured() -> Result<Self> {
        let value = option_env!("NAUTERM_UPDATE_REPOSITORY").unwrap_or_default();
        ensure!(
            !value.trim().is_empty(),
            "Update checks are unavailable in this build."
        );
        Self::parse(value)
    }
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        let parts: Vec<_> = value.split('/').collect();
        ensure!(
            parts.len() == 2
                && parts.iter().all(|part| !part.is_empty()
                    && part.len() <= 100
                    && !matches!(*part, "." | "..")
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric()
                            || matches!(byte, b'_' | b'-' | b'.'))),
            "The update repository setting is invalid."
        );
        Ok(Self(value.into()))
    }
    pub fn releases_url(&self) -> String {
        format!("https://github.com/{}/releases", self.0)
    }
    pub(super) fn download_url(&self, tag: &str, name: &str) -> String {
        format!(
            "https://github.com/{}/releases/download/{tag}/{name}",
            self.0
        )
    }
}
#[derive(Clone, Copy)]
pub struct Target {
    pub os: &'static str,
    pub arch: &'static str,
}
impl Target {
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        }
    }
    pub(super) fn asset_rank(self, version: &str, name: &str) -> Option<u8> {
        let arch = match self.arch {
            "aarch64" => "arm64",
            "x86_64" => "x86_64",
            _ => return None,
        };
        let names: Vec<String> = match self.os {
            "macos" => vec![
                format!("Nauterm-RS-{version}-macos-{arch}.dmg"),
                format!("Nauterm-RS-{version}-macos-{arch}.app.zip"),
            ],
            "windows" => vec![
                format!("Nauterm-RS-{version}-windows-{arch}-setup.exe"),
                format!("Nauterm-RS-{version}-windows-{arch}.zip"),
            ],
            "linux" => {
                let deb = if arch == "x86_64" { "amd64" } else { "arm64" };
                let rpm = if arch == "x86_64" {
                    "x86_64"
                } else {
                    "aarch64"
                };
                vec![
                    format!("Nauterm-RS-{version}-linux-{arch}.AppImage.tar.gz"),
                    format!("Nauterm-RS-{version}-linux-{arch}.AppImage"),
                    format!("nauterm-rs_{version}_{deb}.deb"),
                    format!("nauterm-rs-{version}-1.{rpm}.rpm"),
                ]
            }
            _ => return None,
        };
        names
            .iter()
            .position(|expected| expected == name)
            .map(|rank| rank as u8)
            .or_else(|| {
                if self.os != "linux" {
                    return None;
                }
                let build = name
                    .strip_prefix(&format!("nauterm-rs_{version}-"))
                    .and_then(|value| value.strip_suffix(&format!("_linux-{arch}.deb")))
                    .map(|value| (2, value))
                    .or_else(|| {
                        name.strip_prefix(&format!("nauterm-rs-{version}-"))
                            .and_then(|value| value.strip_suffix(&format!(".linux-{arch}.rpm")))
                            .map(|value| (3, value))
                    });
                build.and_then(|(rank, number)| {
                    (!number.is_empty()
                        && number.len() <= 5
                        && number.bytes().all(|byte| byte.is_ascii_digit())
                        && number.parse::<u16>().is_ok_and(|number| number > 0))
                    .then_some(rank)
                })
            })
    }
}
#[derive(Clone)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub(super) digest: Option<[u8; 32]>,
}
#[derive(Clone)]
pub struct Release {
    pub(super) repository: Repository,
    pub(super) tag: String,
    pub version: String,
    pub url: String,
    pub notes: String,
    pub asset: Option<Asset>,
    pub checksums: Option<Asset>,
    pub newer: bool,
}
#[derive(Deserialize)]
struct Payload {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<RemoteAsset>,
}
#[derive(Deserialize)]
struct RemoteAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    state: String,
    #[serde(default)]
    digest: Option<String>,
}
fn exact_url(value: &str, expected: &str) -> Result<String> {
    let parsed = Url::parse(value)?;
    ensure!(
        parsed.as_str() == Url::parse(expected)?.as_str(),
        "Release metadata contains an unexpected link."
    );
    Ok(parsed.to_string())
}
pub fn decode(
    repository: &Repository,
    current: &str,
    target: Target,
    bytes: &[u8],
) -> Result<Release> {
    ensure!(
        bytes.len() <= 2 * 1024 * 1024,
        "Release information is too large."
    );
    let payload: Payload =
        serde_json::from_slice(bytes).context("Release information is invalid.")?;
    ensure!(
        !payload.draft && !payload.prerelease,
        "The update feed did not return a stable release."
    );
    let version = Version::parse(
        payload
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&payload.tag_name),
    )
    .context("Release version is invalid.")?;
    ensure!(
        version.pre.is_empty(),
        "The update feed did not return a stable version."
    );
    let current = Version::parse(current)?;
    let url = exact_url(
        &payload.html_url,
        &format!("{}/tag/{}", repository.releases_url(), payload.tag_name),
    )?;
    ensure!(
        payload.assets.len() <= 256,
        "Release information contains too many packages."
    );
    let version_label = version.to_string();
    let mut assets = Vec::new();
    let mut checksums = None;
    for asset in payload.assets {
        let rank = target.asset_rank(&version_label, &asset.name);
        let manifest = asset.name == "SHA256SUMS.txt";
        if rank.is_none() && !manifest {
            continue;
        }
        ensure!(
            asset.state == "uploaded" && asset.size > 0,
            "The Nauterm RS package is not ready."
        );
        let url = exact_url(
            &asset.browser_download_url,
            &format!(
                "https://github.com/{}/releases/download/{}/{}",
                repository.0, payload.tag_name, asset.name
            ),
        )?;
        let digest = asset
            .digest
            .as_deref()
            .map(|value| {
                super::download::digest(
                    value
                        .strip_prefix("sha256:")
                        .context("Release metadata contains an unsupported digest.")?,
                )
            })
            .transpose()?;
        let asset = Asset {
            name: asset.name,
            url,
            size: asset.size,
            digest,
        };
        if manifest {
            ensure!(
                checksums.replace(asset).is_none(),
                "Release information contains duplicate checksum files."
            );
        } else if let Some(rank) = rank {
            assets.push((rank, asset));
        }
    }
    assets.sort_by_key(|(rank, _)| *rank);
    ensure!(
        assets.windows(2).all(|pair| pair[0].0 != pair[1].0),
        "Release information contains duplicate packages."
    );
    Ok(Release {
        repository: repository.clone(),
        tag: payload.tag_name,
        version: version_label,
        url,
        notes: payload
            .body
            .unwrap_or_default()
            .chars()
            .take(65536)
            .collect(),
        asset: assets.into_iter().next().map(|(_, asset)| asset),
        checksums,
        newer: version.cmp_precedence(&current).is_gt(),
    })
}
pub fn check(repository: Repository, current: &str, target: Target) -> Result<Release> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{}/releases/latest",
            repository.0
        ))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", format!("Nauterm-RS/{current}"))
        .send()
        .map_err(|error| {
            anyhow::anyhow!(if error.is_timeout() {
                "Update check timed out. Try again."
            } else {
                "Could not check for updates. Check your connection and try again."
            })
        })?;
    ensure!(
        response.status().is_success(),
        "Update check failed (HTTP {}). Try again later.",
        response.status().as_u16()
    );
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .context("Could not read release information.")?;
    decode(&repository, current, target, &bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn payload(name: &str) -> serde_json::Value {
        json!({"tag_name":"v0.4.0","html_url":"https://github.com/example/nauterm/releases/tag/v0.4.0", "body":"Synthetic notes", "draft":false,"prerelease":false,
            "assets":[{"name":name,"browser_download_url":format!("https://github.com/example/nauterm/releases/download/v0.4.0/{name}"),"size":1,"state":"uploaded"}]})
    }
    #[test]
    fn checks_never_offer_flutter_or_another_architecture_as_rs_updates() -> Result<()> {
        let repository = Repository::parse("example/nauterm")?;
        let target = Target {
            os: "macos",
            arch: "aarch64",
        };
        for name in [
            "Nauterm-0.4.0-macos-arm64.dmg",
            "Nauterm-RS-0.4.0-macos-x86_64.dmg",
        ] {
            assert!(
                decode(
                    &repository,
                    "0.3.9",
                    target,
                    &serde_json::to_vec(&payload(name))?
                )?
                .asset
                .is_none()
            );
        }
        let release = decode(
            &repository,
            "0.3.9",
            target,
            &serde_json::to_vec(&payload("Nauterm-RS-0.4.0-macos-arm64.dmg"))?,
        )?;
        assert!(release.newer);
        assert!(release.asset.is_some());
        Ok(())
    }
    #[test]
    fn stable_versions_and_download_destinations_are_bound_to_the_configured_repository()
    -> Result<()> {
        let repository = Repository::parse("example/nauterm")?;
        let target = Target {
            os: "windows",
            arch: "x86_64",
        };
        let source = payload("Nauterm-RS-0.4.0-windows-x86_64-setup.exe");
        let mut malformed = source.clone();
        malformed["assets"][0]["browser_download_url"] = json!("https://other.invalid/installer");
        assert!(
            decode(
                &repository,
                "0.3.9",
                target,
                &serde_json::to_vec(&malformed)?
            )
            .is_err()
        );
        malformed = source.clone();
        malformed["draft"] = json!(true);
        assert!(
            decode(
                &repository,
                "0.3.9",
                target,
                &serde_json::to_vec(&malformed)?
            )
            .is_err()
        );
        assert!(!decode(&repository, "1.0.0", target, &serde_json::to_vec(&source)?)?.newer);
        for repository in [
            "",
            "https://github.com/example/nauterm",
            "example/../nauterm",
            "example/nauterm?token=secret",
        ] {
            assert!(Repository::parse(repository).is_err());
        }
        Ok(())
    }
    #[test]
    fn supported_targets_use_rs_artifact_names_and_ignore_build_metadata_in_precedence()
    -> Result<()> {
        let repository = Repository::parse("example/nauterm")?;
        for (os, arch, name) in [
            ("macos", "aarch64", "Nauterm-RS-0.4.0-macos-arm64.app.zip"),
            ("macos", "x86_64", "Nauterm-RS-0.4.0-macos-x86_64.dmg"),
            (
                "windows",
                "aarch64",
                "Nauterm-RS-0.4.0-windows-arm64-setup.exe",
            ),
            ("windows", "x86_64", "Nauterm-RS-0.4.0-windows-x86_64.zip"),
            ("linux", "aarch64", "nauterm-rs_0.4.0_arm64.deb"),
            ("linux", "aarch64", "nauterm-rs_0.4.0-72_linux-arm64.deb"),
            ("linux", "x86_64", "nauterm-rs-0.4.0-72.linux-x86_64.rpm"),
            ("linux", "x86_64", "nauterm-rs-0.4.0-1.x86_64.rpm"),
        ] {
            let release = decode(
                &repository,
                "0.4.0+local",
                Target { os, arch },
                &serde_json::to_vec(&payload(name))?,
            )?;
            assert!(release.asset.is_some(), "{name}");
            assert!(!release.newer);
        }
        Ok(())
    }
}
