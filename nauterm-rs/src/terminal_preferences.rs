//! Preferences resolved once when a terminal or SSH transport is created.
use anyhow::{Result, ensure};
use nauterm_core::terminal::{TerminalEmulatorBackend, TerminalOptions, TerminalType};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    #[default]
    Ghostty,
    Alacritty,
}
impl Engine {
    pub const ALL: [Self; 2] = [Self::Ghostty, Self::Alacritty];
    pub fn label(self) -> &'static str {
        match self {
            Self::Ghostty => "Ghostty",
            Self::Alacritty => "Alacritty",
        }
    }
    fn native(self) -> TerminalEmulatorBackend {
        match self {
            Self::Ghostty => TerminalEmulatorBackend::Ghostty,
            Self::Alacritty => TerminalEmulatorBackend::Alacritty,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Emulation {
    #[default]
    #[serde(rename = "xterm-256color")]
    Xterm256Color,
    #[serde(rename = "xterm-16color")]
    Xterm16Color,
    #[serde(rename = "xterm")]
    Xterm,
}
impl Emulation {
    pub const ALL: [Self; 3] = [Self::Xterm256Color, Self::Xterm16Color, Self::Xterm];
    pub fn label(self) -> &'static str {
        self.native().term()
    }
    fn native(self) -> TerminalType {
        match self {
            Self::Xterm256Color => TerminalType::Xterm256Color,
            Self::Xterm16Color => TerminalType::Xterm16Color,
            Self::Xterm => TerminalType::Xterm,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub shell_path: Option<String>,
    pub engine: Engine,
    pub emulation: Emulation,
    pub ssh_keepalive_seconds: u32,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            shell_path: None,
            engine: Engine::default(),
            emulation: Emulation::default(),
            ssh_keepalive_seconds: 20,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.shell_path
                .as_deref()
                .is_none_or(|path| std::path::Path::new(path).is_absolute()
                    && !path.chars().any(char::is_control)),
            "Shell must be an absolute executable path."
        );
        Ok(())
    }
    pub fn options(&self) -> TerminalOptions {
        // A local shell path must not influence remote terminal behavior.
        TerminalOptions {
            emulator_backend: self.engine.native(),
            terminal_type: self.emulation.native(),
            ..TerminalOptions::default()
        }
    }
    pub fn keepalive_millis(&self) -> u64 {
        u64::from(self.ssh_keepalive_seconds) * 1000
    }
    pub fn set_keepalive_millis(&mut self, millis: u64) {
        self.ssh_keepalive_seconds = (millis / 1000).min(u64::from(u32::MAX)) as u32;
    }
    pub fn parse_keepalive(value: &str) -> Result<u32> {
        value.parse().map_err(|_| {
            anyhow::anyhow!(
                "SSH keepalive must be a whole number of seconds between 0 and {}.",
                u32::MAX
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_preferences_map_to_native_options_without_opening_a_session() -> Result<()> {
        let mut preferences = Preferences::default();
        preferences.validate()?;
        assert_eq!(preferences.engine, Engine::Ghostty);
        assert_eq!(preferences.keepalive_millis(), 20_000);
        for engine in Engine::ALL {
            preferences.engine = engine;
            for emulation in Emulation::ALL {
                preferences.emulation = emulation;
                let options = preferences.options();
                assert_eq!(options.emulator_backend, engine.native());
                assert_eq!(options.terminal_type.term(), emulation.label());
            }
        }
        preferences.shell_path = Some("relative-shell".into());
        assert!(preferences.validate().is_err());
        Ok(())
    }
    #[test]
    fn keepalive_config_uses_milliseconds_without_overflow() -> Result<()> {
        let mut preferences = Preferences::default();
        for (millis, seconds) in [
            (0, 0),
            (999, 0),
            (20_999, 20),
            (u64::from(u32::MAX) * 1000, u32::MAX),
            (u64::MAX, u32::MAX),
        ] {
            preferences.set_keepalive_millis(millis);
            assert_eq!(preferences.ssh_keepalive_seconds, seconds);
            assert_eq!(Preferences::parse_keepalive(&seconds.to_string())?, seconds);
            assert_eq!(preferences.keepalive_millis(), u64::from(seconds) * 1000);
        }
        for invalid in ["", "-1", "1.5", "4294967296"] {
            assert!(Preferences::parse_keepalive(invalid).is_err());
        }
        Ok(())
    }
}
