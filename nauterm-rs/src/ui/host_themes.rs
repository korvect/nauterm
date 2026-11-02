use super::host_credentials::Target;
use super::*;
use crate::{
    terminal_appearance::{Palette as TerminalPalette, ThemeId},
    theme_preview::ThemePreview,
};

pub(super) struct Draft {
    pub selected: Option<ThemeId>,
    pub gallery: bool,
    search: Entity<InputState>,
    scroll: UniformListScrollHandle,
    _search_events: Subscription,
}
impl Draft {
    pub fn new(selected: Option<ThemeId>, window: &mut Window, cx: &mut Context<Nauterm>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search themes"));
        let scroll = UniformListScrollHandle::new();
        let reset = scroll.clone();
        let events = cx.subscribe(&search, move |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                reset.scroll_to_item(0, ScrollStrategy::Nearest);
                cx.notify();
            }
        });
        Self {
            selected,
            gallery: false,
            search,
            scroll,
            _search_events: events,
        }
    }
}
impl Editor {
    fn theme_for(&self, target: Target) -> Option<&Draft> {
        match target {
            Target::Ssh => self.theme.as_ref(),
            Target::Telnet => self.host.as_ref().map(|draft| &draft.telnet.theme),
        }
    }
    fn theme_for_mut(&mut self, target: Target) -> Option<&mut Draft> {
        match target {
            Target::Ssh => self.theme.as_mut(),
            Target::Telnet => self.host.as_mut().map(|draft| &mut draft.telnet.theme),
        }
    }
    pub(super) fn theme_gallery_target(&self) -> Option<Target> {
        [Target::Ssh, Target::Telnet]
            .into_iter()
            .find(|target| self.theme_for(*target).is_some_and(|draft| draft.gallery))
    }
}
impl Nauterm {
    pub(super) fn host_theme_section(&self, editor: &Editor, cx: &mut Context<Self>) -> AnyElement {
        self.protocol_theme_section(editor, Target::Ssh, cx)
    }
    pub(super) fn protocol_theme_section(
        &self,
        editor: &Editor,
        target: Target,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let draft = editor.theme_for(target).expect("host theme draft");
        let colors = draft
            .selected
            .map(TerminalPalette::new)
            .unwrap_or_else(|| self.data.terminal.palette());
        let title = draft
            .selected
            .map(ThemeId::label)
            .unwrap_or_else(|| "Default".into());
        Button::new(SharedString::from(format!("host-{}-theme", target.id())))
            .ghost()
            .disabled(editor.host.as_ref().is_some_and(|draft| draft.busy))
            .w_full()
            .h(px(ThemePreview::COMPACT_HEIGHT))
            .p_0()
            .rounded(px(8.))
            .tooltip("Select terminal theme")
            .child(ThemePreview::new(title, colors).compact().selected(true))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.refresh_theme_catalog(cx);
                if let Some(draft) = this.editor.as_mut().and_then(|e| e.theme_for_mut(target)) {
                    draft.gallery = true;
                    draft.search.update(cx, |input, cx| input.focus(window, cx));
                    cx.notify();
                }
            }))
            .into_any_element()
    }
    pub(super) fn leave_host_theme_gallery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.editor {
            for target in [Target::Ssh, Target::Telnet] {
                if let Some(draft) = editor.theme_for_mut(target) {
                    draft.gallery = false;
                }
            }
            if let Some((_, field)) = editor.fields.first() {
                field.update(cx, |input, cx| input.focus(window, cx));
            }
        }
        cx.notify();
    }
    pub(super) fn host_theme_gallery(&self, cx: &mut Context<Self>) -> AnyElement {
        let editor = self.editor.as_ref().expect("host editor");
        let target = editor.theme_gallery_target().expect("host theme gallery");
        let draft = editor.theme_for(target).unwrap();
        let p = Palette::new(self.data.dark);
        let query = draft.search.read(cx).value().trim().to_lowercase();
        let entries = std::rc::Rc::new(
            std::iter::once(None)
                .filter(|_| query.is_empty() || "default".contains(&query))
                .chain(crate::theme_catalog::filter(&query).into_iter().map(Some))
                .collect::<Vec<_>>(),
        );
        let count = entries.len().div_ceil(2);
        let selected = draft.selected;
        let default = self.data.terminal.palette();
        let owner = cx.weak_entity();
        div()
            .w(px(layout::DRAWER_WIDTH))
            .h_full()
            .flex_shrink_0()
            .min_h_0()
            .bg(p.background)
            .border_l_1()
            .border_color(p.border)
            .flex()
            .flex_col()
            .on_action(cx.listener(|this, _: &CloseEditor, window, cx| {
                this.leave_host_theme_gallery(window, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .px(px(10.))
                    .bg(p.surface)
                    .border_b_1()
                    .border_color(p.border)
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("back-host-theme")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowLeft)
                            .tooltip("Back")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.leave_host_theme_gallery(window, cx)
                            })),
                    )
                    .child(
                        div()
                            .font_weight(FontWeights::MEDIUM)
                            .child(crate::localization::text(cx, "rs.ui.copy.selectTheme")),
                    ),
            )
            .child(
                div().p(px(12.)).flex_shrink_0().child(
                    Input::new(&draft.search)
                        .prefix(icon(IconName::Search))
                        .aria_label(crate::localization::text(cx, "Search host themes")),
                ),
            )
            .when(count == 0, |v| {
                v.child(
                    div()
                        .p(px(16.))
                        .text_color(p.muted)
                        .child(crate::localization::text(cx, "rs.ui.copy.noMatchingThemes")),
                )
            })
            .when(count > 0, |v| {
                v.child(crate::components::uniform_scrollbar(
                    "host-theme-scrollbar",
                    uniform_list("host-theme-gallery", count, move |range, _, _| {
                        range
                            .map(|row| {
                                div()
                                    .w_full()
                                    .h(px(142.))
                                    .px(px(10.))
                                    .pb(px(10.))
                                    .flex()
                                    .gap(px(10.))
                                    .children(entries.iter().skip(row * 2).take(2).copied().map(
                                        |theme| {
                                            let colors =
                                                theme.map(TerminalPalette::new).unwrap_or(default);
                                            let title = theme
                                                .map(ThemeId::label)
                                                .unwrap_or_else(|| "Default".into());
                                            let owner = owner.clone();
                                            Button::new(SharedString::from(format!(
                                                "host-theme-{}",
                                                theme.map(ThemeId::id).unwrap_or("default")
                                            )))
                                            .ghost()
                                            .p_0()
                                            .h_full()
                                            .flex_1()
                                            .min_w_0()
                                            .rounded(px(8.))
                                            .raw_tooltip(title.clone())
                                            .child(
                                                ThemePreview::new(title, colors)
                                                    .selected(selected == theme),
                                            )
                                            .on_click(move |_, window, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    if let Some(draft) = this
                                                        .editor
                                                        .as_mut()
                                                        .and_then(|e| e.theme_for_mut(target))
                                                    {
                                                        draft.selected = theme;
                                                    }
                                                    this.leave_host_theme_gallery(window, cx);
                                                });
                                            })
                                        },
                                    ))
                                    .when(row * 2 + 1 == entries.len(), |v| {
                                        v.child(div().flex_1().min_w_0())
                                    })
                            })
                            .collect()
                    })
                    .track_scroll(&draft.scroll)
                    .size_full(),
                    &draft.scroll,
                ))
            })
            .into_any_element()
    }
}
