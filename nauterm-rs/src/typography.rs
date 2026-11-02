//! Product text roles, including platform font-selection compatibility.
use gpui_kit::{FontWeight, Styled, px};

pub struct FontWeights;
impl FontWeights {
    // These are product roles from lib/ui/nauterm_typography.dart, not CSS
    // weight names: Flutter deliberately maps both medium and semibold to 500,
    // and bold to 600. Explicit reference weights (e.g. 700) stay explicit.
    pub const REGULAR: FontWeight = ui_weight(400, cfg!(target_os = "macos"));
    pub const MEDIUM: FontWeight = ui_weight(500, cfg!(target_os = "macos"));
    pub const SEMIBOLD: FontWeight = ui_weight(500, cfg!(target_os = "macos"));
    pub const BOLD: FontWeight = ui_weight(600, cfg!(target_os = "macos"));
}

const fn ui_weight(reference: u16, macos: bool) -> FontWeight {
    // zed-font-kit's CoreText loader reports .SFNS-Medium as 530. Its CSS
    // matching rules otherwise select Regular for a request of 500. This UI
    // adapter does not reinterpret terminal/user-selected font-weight numbers.
    FontWeight(if macos && reference == 500 {
        530.
    } else {
        reference as f32
    })
}

/// Settings has its own reference text scale. Apply it after a control's size
/// preset so form text does not depend on control or parent defaults.
#[derive(Clone, Copy)]
pub enum SettingsText {
    Body,
    PageTitle,
    Intro,
    Label,
    Description,
    Header,
    Section,
    Caption,
}
impl SettingsText {
    pub fn apply<T: Styled>(self, element: T) -> T {
        let (size, line, weight) = match self {
            Self::Body => (14., 20., FontWeights::REGULAR),
            Self::PageTitle => (20., 28., FontWeights::SEMIBOLD),
            Self::Intro => (12., 17.2, FontWeights::REGULAR),
            Self::Label => (14., 19.6, FontWeights::MEDIUM),
            Self::Description => (12., 16.2, FontWeights::REGULAR),
            Self::Header => (14., 20., FontWeights::SEMIBOLD),
            Self::Section => (11., 15.95, FontWeight::BOLD),
            Self::Caption => (13., 18.6, FontWeights::MEDIUM),
        };
        element
            .text_size(px(size))
            .line_height(px(line))
            .font_weight(weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_roles_and_platform_face_mapping_are_separate() {
        assert_eq!(FontWeights::SEMIBOLD, FontWeights::MEDIUM);
        assert_eq!(FontWeights::REGULAR, FontWeight(400.));
        assert_eq!(FontWeights::BOLD, FontWeight(600.));
        for value in [300, 400, 600, 700, 800, 900] {
            assert_eq!(ui_weight(value, true), FontWeight(value as f32));
            assert_eq!(ui_weight(value, false), FontWeight(value as f32));
        }
        assert_eq!(ui_weight(500, true), FontWeight(530.));
        assert_eq!(ui_weight(500, false), FontWeight(500.));
        assert_eq!(
            FontWeights::MEDIUM,
            ui_weight(500, cfg!(target_os = "macos"))
        );
    }
}
