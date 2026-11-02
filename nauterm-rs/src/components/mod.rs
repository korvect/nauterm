//! Product-owned controls. Workspace/settings skins are distinct; remaining
//! modal/tooltip coordination is ongoing work. Reuse the
//! unstyled gpui-base behavior/IME primitives rather than rebuilding them.
pub(crate) mod affix;
mod breadcrumb;
mod button;
mod card;
mod choice;
mod color;
mod cursor;
mod dialog;
pub mod drag;
mod field;
mod file_conflict;
mod ink;
mod input;
mod markdown;
pub use markdown::MarkdownDocument;
mod fitting_label;
pub use fitting_label::FittingLabel;
pub(crate) mod management;
pub mod menu;
pub mod message;
mod modal;
pub use card::{card_edit_button, card_grid, card_text, item_card};
pub use management::{ButtonGroup, DrawerFrame, EmptyState};
pub(crate) mod motion;
pub mod overlay;
pub(crate) mod panel;
mod scrollbar;
mod segmented;
mod select;
mod selectable_text;
pub use selectable_text::selectable_text;
mod settings_field;
mod spinner;
mod tab;
pub use scrollbar::{ScrollbarsExt, uniform_scrollbar};
pub use tab::Tab;
mod tooltip;
mod tracked_text;
pub(crate) use tracked_text::TrackedText;

pub use breadcrumb::Breadcrumb;
pub use button::ButtonCustomVariant;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use button::button_snapshot;
pub use spinner::Spinner;
pub type Button = button::Button<false>;
pub mod settings {
    pub use super::settings_field::Field;
    pub type Button = super::button::Button<true>;
    pub type Input = super::input::Input<true>;
}
pub use choice::{Checkbox, RadioGroup, Switch};
pub type WorkspaceSwitch = choice::Toggle<false>;
pub use color::{RgbChange, RgbPicker, RgbState};
pub use dialog::{Dialog, DialogColors, DialogFrame};
pub use field::{Field, terminal_field, validation_message};
pub use file_conflict::FileConflictDialog;
pub use modal::Modal;
pub use segmented::Segmented;
pub type Input = input::Input<false>;
pub use select::{
    SelectAction, SelectCreate, SelectEdited, SelectEvent, SelectModel, SelectOption,
};

use gpui_kit::component::Size;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ControlMetrics {
    pub height: f32,
    pub font: f32,
    pub line_height: f32,
    pub padding: f32,
    pub vertical_padding: f32,
}

impl ControlMetrics {
    pub fn settings_button() -> Self {
        Self {
            height: 29.,
            font: 12.,
            line_height: 18.,
            padding: 13.,
            vertical_padding: 0.,
        }
    }
    pub fn settings_input(size: Size) -> Self {
        Self {
            height: if matches!(size, Size::Small | Size::XSmall) {
                31.
            } else {
                34.
            },
            font: 14.,
            line_height: 18.,
            padding: 10.,
            vertical_padding: 4.,
        }
    }
    pub fn button(size: Size) -> Self {
        // workspace_form_controls.dart::_WorkspaceControlSize.
        let (height, font, padding) = match size {
            Size::XSmall => (24., 12., 8.),
            Size::Small => (24., 14., 9.),
            Size::Medium => (32., 14., 12.),
            Size::Large => (40., 14., 14.),
            Size::Size(height) => (height.into(), 14., 12.),
        };
        Self {
            height,
            font,
            line_height: font + 8.,
            padding,
            vertical_padding: 0.,
        }
    }
    pub fn input(size: Size) -> Self {
        let mut metrics = Self::button(size);
        metrics.line_height = metrics.font * 1.5;
        if size == Size::XSmall {
            metrics.height = 16.;
        }
        metrics.padding = if matches!(size, Size::XSmall | Size::Small) {
            7.
        } else {
            11.
        };
        metrics.vertical_padding = match size {
            Size::XSmall | Size::Small => 0.,
            Size::Large => 7.,
            _ => 4.,
        };
        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_metrics_do_not_reuse_workspace_control_sizes() {
        assert_eq!(ControlMetrics::settings_button().height, 29.);
        assert_eq!(ControlMetrics::settings_button().font, 12.);
        assert_eq!(ControlMetrics::settings_input(Size::Medium).height, 34.);
        assert_eq!(ControlMetrics::settings_input(Size::Small).height, 31.);
        assert_eq!(
            ControlMetrics::settings_input(Size::Medium).line_height,
            18.
        );
        assert_eq!(ControlMetrics::input(Size::Medium).line_height, 21.);
        assert_ne!(
            ControlMetrics::settings_button(),
            ControlMetrics::button(Size::Medium)
        );
    }
    #[test]
    fn reference_control_sizes_are_shared_not_page_constants() {
        for (size, button, input, font, padding) in [
            (Size::XSmall, 24., 16., 12., 8.),
            (Size::Small, 24., 24., 14., 9.),
            (Size::Medium, 32., 32., 14., 12.),
            (Size::Large, 40., 40., 14., 14.),
        ] {
            let metrics = ControlMetrics::button(size);
            assert_eq!(
                (metrics.height, metrics.font, metrics.padding),
                (button, font, padding)
            );
            assert_eq!(ControlMetrics::input(size).height, input);
        }
    }
    #[test]
    fn input_insets_follow_the_reference_instead_of_kit_defaults() {
        for (size, horizontal, vertical) in [
            (Size::XSmall, 7., 0.),
            (Size::Small, 7., 0.),
            (Size::Medium, 11., 4.),
            (Size::Large, 11., 7.),
        ] {
            let metrics = ControlMetrics::input(size);
            assert_eq!(
                (metrics.padding, metrics.vertical_padding),
                (horizontal, vertical)
            );
        }
    }

    fn imports_raw_control(source: &str) -> bool {
        source.split(';').any(|statement| {
            let Some((_, import)) = statement.rsplit_once("use ") else {
                return false;
            };
            let import = import.trim();
            ((import.starts_with("gpui_kit::") && !import.starts_with("gpui_kit::base::"))
                || import.starts_with("gpui_component::"))
                && import
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .any(|token| {
                        matches!(
                            token,
                            "Button"
                                | "Input"
                                | "Textarea"
                                | "Select"
                                | "Checkbox"
                                | "Radio"
                                | "RadioGroup"
                                | "Switch"
                                | "Spinner"
                                | "Popover"
                                | "PopupMenu"
                                | "PopupMenuItem"
                                | "DropdownMenu"
                                | "ContextMenuExt"
                                | "Dialog"
                                | "AlertDialog"
                        )
                    })
        })
    }

    #[test]
    fn import_guard_checks_exact_names_and_all_import_styles() {
        for source in [
            "use gpui_kit::component::button::Button;",
            "use gpui_kit::{component::{button::{Button as RawButton, ButtonVariants}}};",
            "use gpui_component::input::{InputState, Input, Textarea};",
            "use gpui_kit::component::select::{Select as RawSelect, SelectState};",
            "use gpui_kit::component::radio::{Radio as RawRadio, RadioGroup};",
            "use gpui_kit::component::checkbox::Checkbox;",
            "use gpui_kit::component::switch::Switch;",
            "use gpui_kit::component::spinner::Spinner;",
            "use gpui_kit::component::popover::Popover as RawPopover;",
            "use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};",
            "use gpui_kit::component::dialog::{Dialog, DialogHeader};",
        ] {
            assert!(imports_raw_control(source));
        }
        for source in [
            "use crate::components::{Button, Input, Field};",
            "use gpui_kit::base::Button as BaseButton;",
            "use gpui_kit::base::input::{Input, InputState};",
            "use gpui_kit::component::{button::ButtonVariants, input::{InputEvent, InputState, TextareaState}};",
        ] {
            assert!(!imports_raw_control(source));
        }
    }
    #[test]
    fn migrated_controls_are_not_wrappers_around_library_widgets() {
        for (name, source) in [
            ("button", include_str!("button.rs")),
            ("ink", include_str!("ink.rs")),
            ("input", include_str!("input.rs")),
            ("field", include_str!("field.rs")),
            ("settings_field", include_str!("settings_field.rs")),
            ("select", include_str!("select.rs")),
            ("menu", include_str!("menu.rs")),
            ("modal", include_str!("modal.rs")),
            ("dialog", include_str!("dialog.rs")),
            ("choice", include_str!("choice.rs")),
            ("overlay", include_str!("overlay.rs")),
            ("spinner", include_str!("spinner.rs")),
            ("tooltip", include_str!("tooltip.rs")),
        ] {
            assert!(
                !imports_raw_control(source),
                "{name} must own its implementation"
            );
            assert!(
                !source.contains("popover::Popover"),
                "{name} must use our window-owned overlay policy"
            );
            assert!(
                !source.contains("component::native_menu"),
                "{name} must use our product Menu, not a styled native-menu bridge"
            );
        }
    }
    #[test]
    fn application_views_only_construct_shared_basic_controls() {
        // Check source-level component reuse without mounting a UI.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .find(|directory| directory.join("src/components/mod.rs").is_file())
            .expect("Nauterm component source root")
            .join("src");
        let mut directories = vec![root.clone()];
        while let Some(directory) = directories.pop() {
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path == root.join("components") {
                    continue;
                }
                if path.is_dir() {
                    directories.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let source = std::fs::read_to_string(&path).unwrap();
                    assert!(
                        !imports_raw_control(&source),
                        "{} must use the shared basic controls in components",
                        path.display()
                    );
                    assert!(
                        !source.contains("FontWeight::MEDIUM"),
                        "{} must use typography::FontWeights::MEDIUM so macOS selects the real Medium face",
                        path.display()
                    );
                }
            }
        }
    }
}
