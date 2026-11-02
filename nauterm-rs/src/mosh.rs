//! Mosh bootstrap settings share SSH authentication; commands contain no credentials.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
pub mod network;
pub mod prediction;
pub const DEFAULT_COMMAND: &str = "mosh-server new -s -l LANG=en_US.UTF-8";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub command: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            command: DEFAULT_COMMAND.into(),
        }
    }
}
impl Config {
    pub fn new(command: Option<&str>) -> Result<Self> {
        ensure!(
            command.is_none_or(|value| !value.contains(['\r', '\n', '\0'])),
            "Mosh server command must be one nonempty line."
        );
        let config = Self {
            command: command
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(DEFAULT_COMMAND)
                .into(),
        };
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.command.trim().is_empty()
                && self.command.len() <= 65536
                && !self.command.contains(['\r', '\n', '\0']),
            "Mosh server command must be one nonempty line."
        );
        Ok(())
    }
    pub(crate) fn native(&self) -> nauterm_core::MoshServerCommand {
        nauterm_core::MoshServerCommand {
            command: self.command.clone(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_follow_the_shared_core_default_and_reject_invalid_shell_lines() -> Result<()> {
        assert_eq!(
            Config::default().native(),
            nauterm_core::MoshServerCommand::default()
        );
        assert_eq!(Config::new(Some("  "))?, Config::default());
        assert_eq!(
            Config::new(Some(" /opt/bin/mosh-server new -s "))?.command,
            "/opt/bin/mosh-server new -s"
        );
        for command in ["mosh-server\nother", "mosh-server\r", "mosh\0"] {
            assert!(Config::new(Some(command)).is_err());
        }
        Ok(())
    }
}
