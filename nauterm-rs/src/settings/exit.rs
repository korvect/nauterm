use super::*;
use crate::components::{Dialog, DialogColors, DialogFrame, Spinner};
impl Settings {
    pub(super) fn exit_guard(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.exit_pending {
            return None;
        }
        let p = Palette::settings(self.values.dark);
        Some(
            Dialog::new("settings-exit-pending", "Finishing settings")
                .focus_scope(&self.exit_focus)
                .return_focus(&self.focus)
                .dismissible(false)
                .content(
                    DialogFrame::new("settings-exit-pending-frame", "Finishing settings")
                        .colors(DialogColors {
                            surface: p.surface,
                            header: p.sidebar,
                            border: p.border,
                            foreground: p.text,
                            dark: self.values.dark,
                        })
                        .content(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(Spinner::new())
                                .child(crate::localization::text(
                                    cx,
                                    "rs.ui.copy.continueMainWindow",
                                )),
                        ),
                )
                .into_any_element(),
        )
    }
}
