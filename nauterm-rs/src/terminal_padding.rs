//! The reference comma-separated CSS shorthand, stored as four config leaves.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
impl Padding {
    pub fn validate(self) -> Result<()> {
        ensure!(
            [self.top, self.right, self.bottom, self.left]
                .into_iter()
                .all(|value| value.is_finite() && value >= 0.),
            "Padding must contain finite, non-negative values."
        );
        Ok(())
    }
    pub fn parse(text: &str) -> Result<Self> {
        let values = text
            .split(',')
            .take(5)
            .map(|part| part.trim().parse::<f32>())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let padding = match values.as_slice() {
            [all] => Self {
                top: *all,
                right: *all,
                bottom: *all,
                left: *all,
            },
            [vertical, horizontal] => Self {
                top: *vertical,
                right: *horizontal,
                bottom: *vertical,
                left: *horizontal,
            },
            [top, horizontal, bottom] => Self {
                top: *top,
                right: *horizontal,
                bottom: *bottom,
                left: *horizontal,
            },
            [top, right, bottom, left] => Self {
                top: *top,
                right: *right,
                bottom: *bottom,
                left: *left,
            },
            _ => anyhow::bail!("Padding requires 1 to 4 comma-separated values."),
        };
        padding.validate()?;
        Ok(padding)
    }
    pub fn format(self) -> String {
        let values = if self.left == self.top && self.top == self.right && self.right == self.bottom
        {
            vec![self.top]
        } else if self.top == self.bottom && self.left == self.right {
            vec![self.top, self.left]
        } else if self.left == self.right {
            vec![self.top, self.right, self.bottom]
        } else {
            vec![self.top, self.right, self.bottom, self.left]
        };
        values
            .into_iter()
            .map(|value| {
                if value == 0. {
                    "0".into()
                } else {
                    value.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
    /// Keep a tiny pane and excessively large preferences geometrically valid.
    pub fn fitted(self, width: f32, height: f32) -> Self {
        let width = width.max(0.);
        let height = height.max(0.);
        let left = self.left.min(width);
        let top = self.top.min(height);
        Self {
            left,
            top,
            right: self.right.min(width - left),
            bottom: self.bottom.min(height - top),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padding_shorthand_roundtrips_without_rounding_fractional_values() -> Result<()> {
        for (text, sides, formatted) in [
            ("0", [0., 0., 0., 0.], "0"),
            (" 4.5 ", [4.5, 4.5, 4.5, 4.5], "4.5"),
            (" 4.0, 4,4.00, 4 ", [4., 4., 4., 4.], "4"),
            ("2,8.0, 2.00,8", [2., 8., 2., 8.], "2, 8"),
            ("2, 8", [2., 8., 2., 8.], "2, 8"),
            ("2, 8, 4", [2., 8., 4., 8.], "2, 8, 4"),
            ("1, 2, 3, 4", [1., 2., 3., 4.], "1, 2, 3, 4"),
        ] {
            let padding = Padding::parse(text)?;
            assert_eq!(
                [padding.top, padding.right, padding.bottom, padding.left],
                sides
            );
            assert_eq!(padding.format(), formatted);
            assert_eq!(Padding::parse(&padding.format())?, padding);
        }
        for invalid in ["", "1,", "1,2,3,4,5", "-1", "NaN", "inf", "1, -2"] {
            assert!(Padding::parse(invalid).is_err());
        }
        Ok(())
    }
}
