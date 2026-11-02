use super::*;
use crate::{
    components::{DialogColors, DialogFrame, Modal},
    tags::{
        self,
        picker::{Event, Mode, Picker},
    },
};
pub(super) struct Draft {
    pub view: Entity<Picker>,
    _events: Subscription,
}
pub(super) struct Rename {
    id: String,
    input: Entity<InputState>,
    pub(super) error: Option<String>,
    validation_error: Option<SharedString>,
}
impl Nauterm {
    pub(super) fn new_host_tags(
        &self,
        names: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Draft {
        let selected = tags::ids(&self.data.tags, names);
        let view =
            cx.new(|cx| Picker::new(Mode::Field, self.data.tags.clone(), selected, window, cx));
        let events = Self::tag_events(&view, window, cx);
        Draft {
            view,
            _events: events,
        }
    }
    pub(super) fn tag_events(
        view: &Entity<Picker>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(view, window, |this, view, event: &Event, window, cx| {
            if this.tag_filter != *view
                && !this.editor.as_ref().is_some_and(|editor| {
                    editor
                        .tag_pickers()
                        .iter()
                        .any(|picker| picker.entity_id() == view.entity_id())
                })
            {
                return;
            }
            match event {
                Event::Selection(selected) => {
                    if this.tag_filter.entity_id() == view.entity_id() {
                        this.selected_tags = selected.clone();
                    }
                }
                Event::Create(name) => {
                    let result = (|| {
                        anyhow::ensure!(
                            this.resources_ready(),
                            "Workspace storage is unavailable."
                        );
                        let (mut candidate, tag) = tags::create(&this.data, name)?;
                        this.store.save(&mut candidate)?;
                        this.data = candidate;
                        this.sync_tag_catalog(cx);
                        Ok::<_, anyhow::Error>(tag)
                    })();
                    view.update(cx, |picker, cx| match result {
                        Ok(tag) => picker.created(&tag, window, cx),
                        Err(error) => picker.failed(error.to_string(), cx),
                    });
                }
                Event::Rename(tag) => {
                    if let Some(tag) = this.data.tags.iter().find(|entry| entry.id == tag.id) {
                        let input = cx
                            .new(|cx| InputState::new(window, cx).default_value(tag.name.clone()));
                        this.rename_tag = Some(Rename {
                            id: tag.id.clone(),
                            input,
                            error: None,
                            validation_error: None,
                        });
                    }
                }
                Event::Delete(tag) => {
                    let result = (|| {
                        anyhow::ensure!(
                            this.resources_ready(),
                            "Workspace storage is unavailable."
                        );
                        let mut candidate = tags::delete(&this.data, &tag.id)?;
                        this.store.save(&mut candidate)?;
                        this.data = candidate;
                        this.sync_tag_catalog(cx);
                        anyhow::Ok(())
                    })();
                    if let Err(error) = result {
                        view.update(cx, |picker, cx| picker.failed(error.to_string(), cx));
                    }
                }
            }
            cx.notify();
        })
    }
    pub(super) fn sync_tag_catalog(&mut self, cx: &mut Context<Self>) {
        self.selected_tags
            .retain(|id| self.data.tags.iter().any(|tag| &tag.id == id));
        self.tag_filter.update(cx, |picker, cx| {
            picker.set_catalog(&self.data.tags, !self.resources_ready(), cx)
        });
        if let Some(editor) = &self.editor {
            for picker in editor.tag_pickers() {
                picker.update(cx, |picker, cx| {
                    picker.set_catalog(
                        &self.data.tags,
                        !self.resources_ready()
                            || editor.host.as_ref().is_some_and(|draft| draft.busy),
                        cx,
                    )
                });
            }
        }
    }
    pub(super) fn active_tag_filter(&self) -> tags::Filter {
        tags::Filter::new(&self.data.tags, &self.selected_tags)
    }
    fn save_renamed_tag(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = &self.rename_tag else {
            return;
        };
        let candidate = match tags::rename(&self.data, &draft.id, &draft.input.read(cx).value()) {
            Ok(candidate) => candidate,
            Err(error) => {
                self.rename_tag.as_mut().unwrap().validation_error = Some(error.to_string().into());
                cx.notify();
                return;
            }
        };
        let result = (|| {
            anyhow::ensure!(self.resources_ready(), "Workspace storage is unavailable.");
            let mut candidate = candidate;
            self.store.save(&mut candidate)?;
            Ok::<_, anyhow::Error>(candidate)
        })();
        match result {
            Ok(candidate) => {
                self.data = candidate;
                self.rename_tag = None;
                self.sync_tag_catalog(cx);
            }
            Err(error) => self.rename_tag.as_mut().unwrap().error = Some(error.to_string()),
        }
        cx.notify();
    }
    pub(super) fn rename_tag_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let draft = self.rename_tag.as_ref()?;
        let focus = draft.input.read(cx).focus_handle(cx);
        Some(
            Modal::new("rename-tag-modal", "Rename tag")
                .return_focus(&self.focus)
                .initial_focus(&focus)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.rename_tag = None;
                    cx.notify();
                }))
                .on_confirm(cx.listener(|this, _, _, cx| this.save_renamed_tag(cx)))
                .content(
                    DialogFrame::new("rename-tag-frame", "Rename tag")
                        .width(360.)
                        .colors(DialogColors::workspace(self.data.dark))
                        .content(
                            Field::input(&draft.input, "Name")
                                .error(draft.validation_error.clone()),
                        )
                        .action(
                            Button::new("cancel-rename-tag")
                                .ghost()
                                .label("Cancel")
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        )
                        .action(
                            Button::new("save-rename-tag")
                                .accent()
                                .label("Save")
                                .disabled(!self.resources_ready())
                                .on_click(cx.listener(|this, _, _, cx| this.save_renamed_tag(cx))),
                        ),
                )
                .into_any_element(),
        )
    }
}
