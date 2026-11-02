use super::*;
use crate::fido2::editor::{Event, Wizard};

pub(super) struct Draft {
    pub view: Entity<Wizard>,
    _events: Subscription,
    _updates: Subscription,
}
impl Keychain {
    pub(super) fn create_fido2(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let Some(vault) = self.vault.clone() else {
            return;
        };
        self.create(Kind::Generate, window, cx);
        let view = cx.new(|cx| Wizard::new(vault, self.dark, window, cx));
        let events = cx.subscribe_in(&view, window, |this, view, event: &Event, window, cx| {
            if !this
                .editor
                .as_ref()
                .and_then(|editor| editor.fido.as_ref())
                .is_some_and(|draft| draft.view == *view)
            {
                return;
            }
            match event {
                Event::Close => this.close_editor(window, cx),
                Event::Saved(id) => {
                    this.finish_save(*id, false, window, cx);
                    this.refresh(cx);
                }
            }
            cx.notify();
        });
        let updates = cx.observe(&view, |_, _, cx| cx.notify());
        if let Some(editor) = &mut self.editor {
            editor.fido = Some(Draft {
                view,
                _events: events,
                _updates: updates,
            });
        }
        cx.notify();
    }
}
