//! Public application preferences; recovery owns actual window geometry.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSize {
    pub width: f32,
    pub height: f32,
}
impl Default for WindowSize {
    fn default() -> Self {
        Self {
            width: 920.,
            height: 620.,
        }
    }
}
impl WindowSize {
    pub const MIN_WIDTH: f32 = 750.;
    pub const MIN_HEIGHT: f32 = 440.;
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.width.is_finite()
                && self.width >= Self::MIN_WIDTH
                && self.height.is_finite()
                && self.height >= Self::MIN_HEIGHT,
            "Window size must be at least 750 × 440 pixels."
        );
        Ok(())
    }
    pub fn parse(width: &str, height: &str) -> Result<Self> {
        let size = Self {
            width: width.parse()?,
            height: height.parse()?,
        };
        size.validate()?;
        Ok(size)
    }
    pub fn constrained(self, width: f32, height: f32) -> Self {
        Self {
            width: self
                .width
                .max(Self::MIN_WIDTH)
                .min(width.max(Self::MIN_WIDTH)),
            height: self
                .height
                .max(Self::MIN_HEIGHT)
                .min(height.max(Self::MIN_HEIGHT)),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub language: crate::localization::Language,
    pub window: WindowSize,
    pub confirm_on_close: bool,
    pub workspace_enabled: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: Default::default(),
            window: WindowSize::default(),
            confirm_on_close: true,
            workspace_enabled: true,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<()> {
        self.window.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_sizes_validate_and_fit_the_available_display() -> Result<()> {
        assert_eq!(WindowSize::parse("920", "620")?, WindowSize::default());
        for (width, height) in [
            ("749", "620"),
            ("920", "439"),
            ("NaN", "620"),
            ("920", "inf"),
            ("", "620"),
        ] {
            assert!(WindowSize::parse(width, height).is_err());
        }
        let requested = WindowSize {
            width: 2000.,
            height: 1500.,
        };
        assert_eq!(
            requested.constrained(1440., 900.),
            WindowSize {
                width: 1440.,
                height: 900.
            }
        );
        assert_eq!(
            requested.constrained(640., 360.),
            WindowSize {
                width: 750.,
                height: 440.
            }
        );
        assert_eq!(
            WindowSize::default().constrained(1440., 900.),
            WindowSize::default()
        );
        Ok(())
    }
}
