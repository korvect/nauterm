//! Serial configuration for saved Hosts and temporary connections.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

pub mod dialog;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub path: String,
    pub baud: u32,
    pub data_bits: u32,
    pub parity: Parity,
    pub stop_bits: u32,
    pub flow: Flow,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u32)]
pub enum Parity {
    #[default]
    None = 0,
    Even = 1,
    Odd = 2,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u32)]
pub enum Flow {
    #[default]
    None = 0,
    Software = 1,
    Hardware = 2,
}
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Port {
    pub path: String,
    pub display_name: String,
    pub description: Option<String>,
}
impl Port {
    pub fn label(&self) -> String {
        match self.description.as_deref().filter(|text| !text.is_empty()) {
            Some(description) => format!("{} ({description})", self.display_name),
            None => self.display_name.clone(),
        }
    }
}
pub fn default_port() -> &'static str {
    if cfg!(windows) {
        "COM1"
    } else if cfg!(target_os = "macos") {
        "/dev/cu.usbserial"
    } else {
        "/dev/ttyUSB0"
    }
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct StoredFraming {
    data_bits: Option<u32>,
    parity: Parity,
    stop_bits: Option<u32>,
    flow_control: Flow,
}

impl Config {
    pub(crate) fn from_host(host: &nauterm_core::HostEntry) -> Result<Option<Self>> {
        if host.host_type != nauterm_core::HostType::Serial {
            return Ok(None);
        }
        let (Some(path), Some(baud)) = (&host.serial_port, host.serial_baud_rate) else {
            return Ok(None);
        };
        let framing: StoredFraming = match host.config.get("serial") {
            Some(value) => {
                serde_json::from_value(value.clone()).context("Invalid serial framing")?
            }
            None => Default::default(),
        };
        let config = Self {
            path: path.clone(),
            baud: baud.try_into().context("Invalid serial baud rate")?,
            data_bits: framing.data_bits.unwrap_or(8),
            parity: framing.parity,
            stop_bits: framing.stop_bits.unwrap_or(1),
            flow: framing.flow_control,
        };
        config.validate()?;
        Ok(Some(config))
    }

    pub(crate) fn apply_to_host(&self, host: &mut nauterm_core::HostEntry) {
        host.serial_port = Some(self.path.clone());
        host.serial_baud_rate = Some(i64::from(self.baud));
        let options = host
            .config
            .entry("serial")
            .or_insert_with(|| serde_json::json!({}));
        if !options.is_object() {
            *options = serde_json::json!({});
        }
        options["data_bits"] = self.data_bits.into();
        options["parity"] = serde_json::json!(self.parity);
        options["stop_bits"] = self.stop_bits.into();
        options["flow_control"] = serde_json::json!(self.flow);
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(!self.path.trim().is_empty(), "Serial port is required.");
        ensure!(
            !self.path.contains('\0'),
            "Serial port contains a NUL character."
        );
        ensure!(self.baud > 0, "Baud rate must be a positive number.");
        ensure!(
            (5..=8).contains(&self.data_bits),
            "Data bits must be between 5 and 8."
        );
        ensure!(matches!(self.stop_bits, 1 | 2), "Stop bits must be 1 or 2.");
        Ok(())
    }
    pub fn summary(&self) -> String {
        let parity = match self.parity {
            Parity::None => 'N',
            Parity::Even => 'E',
            Parity::Odd => 'O',
        };
        let flow = match self.flow {
            Flow::None => "none",
            Flow::Software => "software",
            Flow::Hardware => "hardware",
        };
        format!(
            "{} baud, {}{parity}{}, flow {flow}",
            self.baud, self.data_bits, self.stop_bits
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_serial_profiles_share_flutter_config() {
        let mut host: nauterm_core::HostEntry = serde_json::from_value(serde_json::json!({
            "type": "serial", "name": "Console", "serial_port": "COM8", "serial_baud_rate": 57600,
            "config": {
                "serial": {"data_bits": 7, "parity": "even", "stop_bits": 2, "flow_control": "hardware", "extra": true},
                "icon": "cisco"
            }
        })).unwrap();
        let mut serial = Config::from_host(&host).unwrap().unwrap();
        assert_eq!(serial.parity, Parity::Even);
        assert_eq!(serial.flow, Flow::Hardware);
        serial.baud = 115200;
        serial.apply_to_host(&mut host);
        assert_eq!(Config::from_host(&host).unwrap(), Some(serial));
        assert_eq!(host.config["serial"]["extra"], true);
        assert_eq!(host.config["icon"], "cisco");
    }
    #[test]
    fn serial_parameters_allow_manual_paths_and_custom_positive_baud() {
        let mut config = Config {
            path: "/dev/serial/by-id/合成-test".into(),
            baud: 12345,
            data_bits: 8,
            parity: Parity::None,
            stop_bits: 1,
            flow: Flow::None,
        };
        config.validate().unwrap();
        assert_eq!(config.summary(), "12345 baud, 8N1, flow none");
        config.baud = 0;
        assert!(config.validate().is_err());
        config.baud = 115200;
        config.path = "\0device".into();
        assert!(config.validate().is_err());
        config.path = "COM42".into();
        config.data_bits = 9;
        assert!(config.validate().is_err());
        config.data_bits = 7;
        config.stop_bits = 3;
        assert!(config.validate().is_err());
        config.stop_bits = 2;
        config.parity = Parity::Even;
        config.flow = Flow::Hardware;
        config.validate().unwrap();
        assert_eq!(config.summary(), "115200 baud, 7E2, flow hardware");
    }
}
