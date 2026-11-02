//! Settings adapters over the shared font inventory.
use super::*;
pub(super) use crate::fonts::{Options, options};

impl Settings {
    pub fn external_font(
        &mut self,
        family: &str,
        size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.values.terminal.family = family.into();
        self.values.terminal.size = size;
        let fonts = self.font_options.clone().unwrap_or_default();
        for control in &mut self.controls {
            let options = match control.field {
                Field::Family => crate::fonts::primary_options(&fonts, family),
                Field::Size => crate::fonts::size_options(size),
                _ => continue,
            };
            control.select.reconcile(
                options,
                Some(control.field.value(&self.values.terminal)),
                window,
                cx,
            );
        }
        cx.notify();
    }
}
