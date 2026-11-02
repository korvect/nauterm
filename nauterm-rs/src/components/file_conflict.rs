//! Source-faithful remote edit decisions, using our Modal/DialogFrame/Button.
use super::{Button, DialogColors, DialogFrame, Modal};
use crate::file_conflict::Resolution;
use gpui_kit::{prelude::*, *};
use std::rc::Rc;
type Resolve = Rc<dyn Fn(Resolution, &mut Window, &mut App)>;
#[derive(IntoElement)]
pub struct FileConflictDialog {
    id: ElementId,
    host: String,
    path: String,
    missing: bool,
    dark: bool,
    focus: FocusHandle,
    resolve: Resolve,
}
impl FileConflictDialog {
    pub fn new(
        id: impl Into<ElementId>,
        host: String,
        path: String,
        missing: bool,
        dark: bool,
        focus: &FocusHandle,
        resolve: impl Fn(Resolution, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            host,
            path,
            missing,
            dark,
            focus: focus.clone(),
            resolve: Rc::new(resolve),
        }
    }
}
impl RenderOnce for FileConflictDialog {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut actions = vec![];
        for (label, resolution) in [
            ("Cancel", Resolution::Cancel),
            ("Keep Remote", Resolution::KeepRemote),
            ("Save a Copy", Resolution::SaveCopy),
            ("Overwrite Remote", Resolution::OverwriteRemote),
        ] {
            if self.missing && resolution == Resolution::KeepRemote {
                continue;
            }
            let resolve = self.resolve.clone();
            actions.push(
                Button::new((self.id.clone(), SharedString::from(label)))
                    .label(label)
                    .when(resolution == Resolution::Cancel, |button| button.ghost())
                    .when(resolution == Resolution::OverwriteRemote, |button| {
                        button.danger()
                    })
                    .on_click(move |_, window, cx| resolve(resolution, window, cx))
                    .into_any_element(),
            );
        }
        let cancel = self.resolve;
        Modal::new((self.id.clone(), "modal"), "Remote file changed")
            .return_focus(&self.focus)
            .dismissible(false)
            .on_dismiss(move |_, window, cx| cancel(Resolution::Cancel, window, cx))
            .content(
                DialogFrame::new((self.id, "frame"), "Remote file changed")
                    .width(540.)
                    .colors(DialogColors::workspace(self.dark))
                    .content(
                        div()
                            .w_full()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(crate::localization::view(if self.missing {
                                "The file was removed from the server after editing began."
                            } else {
                                "The file was changed on the server after editing began."
                            }))
                            .child(self.host)
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .max_h(px(58.))
                                    .overflow_hidden()
                                    .child(self.path),
                            )
                            .child(crate::localization::text(
                                cx,
                                "rs.ui.copy.localEditsRetained",
                            )),
                    )
                    .actions(actions),
            )
    }
}
