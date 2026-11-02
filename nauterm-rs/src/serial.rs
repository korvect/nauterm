//! Serial connection parameters are transient, never workspace recovery targets.
use anyhow::{Result, ensure};

pub mod dialog;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub path: String,
    pub baud: u32,
    pub data_bits: u32,
    pub parity: Parity,
    pub stop_bits: u32,
    pub flow: Flow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parity {
    None = 0,
    Even = 1,
    Odd = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Flow {
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
impl Config {
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
