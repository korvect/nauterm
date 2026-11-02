//! Product composer suggestions over the existing multiline Input and Overlay.
//! Filling changes only the editor. Native command submission stays in Terminal.
use crate::{
    components::{Input, overlay::Overlay},
    composer_completion::{Completion, History},
};
use gpui_kit::{
    base::{
        Placement,
        input::{InputEvent, TextareaState},
    },
    prelude::*,
    *,
};

pub struct Suggestions {
    pub input: Entity<TextareaState>,
    sources: Vec<String>,
    snippets: Vec<String>,
    history: Vec<String>,
    resolved: Option<(String, Vec<String>)>,
    observed: Option<(String, bool)>,
    navigation: History,
    model: Completion,
    enabled: bool,
    sensitive: bool,
    visible: bool,
    limit: usize,
    colors: (u32, u32, u32),
    width: Pixels,
    scroll: UniformListScrollHandle,
    _events: Subscription,
    _keyboard: Subscription,
}
pub struct Changed;
pub struct Dismissed;
impl EventEmitter<Changed> for Suggestions {}
impl EventEmitter<Dismissed> for Suggestions {}
impl Suggestions {
    pub fn new(input: Entity<TextareaState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let events = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(
                event,
                InputEvent::Change | InputEvent::Focus | InputEvent::Blur
            ) {
                this.sync_input(cx);
            }
        });
        let owner = cx.weak_entity();
        let keyboard = cx.intercept_keystrokes(move |event, window, cx| {
            let Some(owner) = owner.upgrade() else {
                return;
            };
            if !owner
                .read(cx)
                .input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            {
                return;
            }
            if owner.update(cx, |this, cx| this.key(&event.keystroke, window, cx)) {
                window.prevent_default();
                cx.stop_propagation();
            }
        });
        Self {
            input,
            sources: vec![],
            snippets: vec![],
            history: vec![],
            resolved: None,
            observed: None,
            navigation: History::default(),
            model: Completion::default(),
            enabled: false,
            sensitive: true,
            visible: true,
            limit: 999,
            colors: (0xffffff, 0x151927, 0x168df2),
            width: px(240.),
            scroll: UniformListScrollHandle::new(),
            _events: events,
            _keyboard: keyboard,
        }
    }
    pub fn query_input(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<String> {
        if !self.visible
            || !self.enabled
            || self.sensitive
            || !self.input.read(cx).focus_handle(cx).is_focused(window)
            || self.input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            })
        {
            return None;
        }
        Some(self.input.read(cx).value().to_string())
    }
    pub fn set_resolved(&mut self, input: &str, values: Vec<String>, cx: &mut Context<Self>) {
        if self.input.read(cx).value().as_ref() != input
            || self
                .resolved
                .as_ref()
                .is_some_and(|(old, items)| old == input && *items == values)
        {
            return;
        }
        self.resolved = Some((input.into(), values));
        self.sync_input(cx);
    }
    pub fn clear_resolved(&mut self, cx: &mut Context<Self>) {
        if self.resolved.take().is_some() {
            self.sync_input(cx);
        }
    }
    fn refresh_model(&mut self, cx: &mut Context<Self>) {
        let input = self.input.read(cx).value().to_string();
        if let Some((_, resolved)) = self
            .resolved
            .as_ref()
            .filter(|(snapshot, _)| *snapshot == input)
        {
            self.model.refresh_resolved(
                &input,
                resolved,
                &self.sources,
                self.enabled,
                self.sensitive,
                self.limit,
            );
        } else {
            self.model.refresh(
                &input,
                &self.sources,
                self.enabled,
                self.sensitive,
                self.limit,
            );
        }
    }
    pub fn set_sources(&mut self, sources: Vec<String>, cx: &mut Context<Self>) {
        if self.snippets == sources {
            return;
        }
        self.snippets = sources;
        self.rebuild_sources(cx);
    }
    pub fn set_history(&mut self, history: Vec<String>, cx: &mut Context<Self>) {
        if self.history == history {
            return;
        }
        let mut seen = std::collections::HashSet::new();
        self.history = history
            .iter()
            .map(|command| command.trim())
            .filter(|command| !command.is_empty() && seen.insert(*command))
            .map(str::to_owned)
            .collect();
        self.rebuild_sources(cx);
    }
    fn rebuild_sources(&mut self, cx: &mut Context<Self>) {
        self.sources = self.history.iter().chain(&self.snippets).cloned().collect();
        self.sync_input(cx);
    }
    pub fn policy(
        &mut self,
        enabled: bool,
        sensitive: bool,
        visible: bool,
        limit: usize,
        colors: (u32, u32, u32),
        cx: &mut Context<Self>,
    ) {
        if (
            self.enabled,
            self.sensitive,
            self.visible,
            self.limit,
            self.colors,
        ) == (enabled, sensitive, visible, limit, colors)
        {
            return;
        }
        self.enabled = enabled;
        self.sensitive = sensitive;
        self.visible = visible;
        self.limit = limit;
        self.colors = colors;
        self.sync_input(cx);
    }
    pub fn sync_input(&mut self, cx: &mut Context<Self>) {
        self.navigation
            .observe(self.input.read(cx).value().as_ref());
        self.refresh_model(cx);
        cx.emit(Changed);
        cx.notify();
    }
    fn fill(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.navigation.reset();
        self.replace_text(text, window, cx);
    }
    fn replace_text(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        // Use the editing engine's ordinary replacement so Undo can recover the
        // original prefix. set_value would reset its native undo history.
        self.input.update(cx, |input, cx| {
            let end = text.len();
            input.replace_all(text, window, cx);
            input.set_selected_range(end..end, cx);
            input.focus(window, cx);
        });
        self.sync_input(cx);
    }
    fn key(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.visible || self.sensitive || key.modifiers.modified() {
            return false;
        }
        if self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return false;
        }
        self.sync_input(cx);
        match key.key.as_str() {
            "escape" => {
                cx.emit(Dismissed);
                self.model.dismiss()
            }
            "up" | "down" => {
                if self.model.step(if key.key == "up" { -1 } else { 1 }) {
                    return true;
                }
                let input = self.input.read(cx).value().to_string();
                let next = if key.key == "up" {
                    self.navigation.previous(&input, &self.history)
                } else {
                    self.navigation.next(&input, &self.history)
                };
                if let Some(text) = next {
                    self.replace_text(text, window, cx);
                    true
                } else {
                    false
                }
            }
            "tab" | "right" => {
                if !self.enabled {
                    return false;
                }
                let input = self.input.read(cx);
                let selection = input.selected_range();
                if !selection.is_empty()
                    || selection.end != input.text().len()
                    || self.model.items().is_empty()
                {
                    return false;
                }
                if let Some(text) = self.model.activate() {
                    self.fill(text, window, cx);
                }
                true
            }
            _ => false,
        }
    }
    fn menu(&self, entity: &Entity<Self>, window: &Window, cx: &App) -> AnyElement {
        let colors = self.colors;
        let owner = entity.downgrade();
        let selected = self.model.selected().filter(|_| self.model.armed());
        let count = self.model.items().len();
        let snapshot = std::sync::Arc::<str>::from(self.input.read(cx).value().to_string());
        div()
            .id(("composer-suggestions", entity.entity_id()))
            .role(Role::ListBox)
            .aria_label(crate::localization::text(cx, "Command suggestions"))
            .w(self
                .width
                .min((window.viewport_size().width - px(18.)).max(px(1.))))
            .h(px((count as f32 * 28. + 8.).min(176.)))
            .py(px(4.))
            .bg(rgb(colors.0))
            .text_color(rgb(colors.1))
            .child(
                uniform_list(
                    ("composer-list", entity.entity_id()),
                    count,
                    move |range, _, cx| {
                        let Some(entity) = owner.upgrade() else {
                            return vec![];
                        };
                        range
                            .filter_map(|index| {
                                let text = entity.read(cx).model.items().get(index)?.clone();
                                let pick = entity.downgrade();
                                let expected = snapshot.clone();
                                Some(
                                    div()
                                        .id(("composer-option", index))
                                        .role(Role::ListBoxOption)
                                        .aria_label(text.clone())
                                        .aria_selected(selected == Some(index))
                                        .h(px(28.))
                                        .px(px(10.))
                                        .flex()
                                        .items_center()
                                        .when(selected == Some(index), |view| {
                                            view.bg(rgba(0x8ca0a633))
                                        })
                                        .hover(|view| view.bg(rgba(0x8ca0a622)))
                                        .child(div().min_w_0().text_ellipsis().child(text.clone()))
                                        .on_click(move |_, window, cx| {
                                            let _ = pick.update(cx, |this, cx| {
                                                if this.visible
                                                    && this.enabled
                                                    && !this.sensitive
                                                    && this.input.read(cx).value().as_ref()
                                                        == expected.as_ref()
                                                    && this.model.items().contains(&text)
                                                {
                                                    this.fill(text.clone(), window, cx)
                                                }
                                            });
                                        }),
                                )
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .track_scroll(&self.scroll)
                .size_full(),
            )
            .into_any_element()
    }
}
impl Render for Suggestions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Programmatic input replacement can notify without an InputEvent.
        self.navigation
            .observe(self.input.read(cx).value().as_ref());
        self.refresh_model(cx);
        let query = self.query_input(window, cx);
        let observed = (self.input.read(cx).value().to_string(), query.is_some());
        if self.observed.as_ref() != Some(&observed) {
            self.observed = Some(observed);
            cx.emit(Changed);
        }
        if let Some(index) = self.model.selected() {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        }
        let focus = self.input.read(cx).focus_handle(cx);
        let composing = self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        });
        let open = self.visible
            && self.enabled
            && !self.sensitive
            && !composing
            && !self.model.items().is_empty()
            && focus.is_focused(window);
        let measure = cx.weak_entity();
        let menu = cx.weak_entity();
        let close = cx.weak_entity();
        let mut editor_palette = crate::palette::Palette::new(false);
        editor_palette.surface = rgb(self.colors.0);
        editor_palette.text = rgb(self.colors.1);
        editor_palette.muted =
            crate::components::motion::mix(editor_palette.surface, editor_palette.text, 0.55);
        editor_palette.accent = rgb(self.colors.2);
        let trigger = div()
            .relative()
            .min_w_0()
            .w_full()
            .child(
                Input::multiline(&self.input)
                    .palette(editor_palette)
                    .appearance(false)
                    .bordered(false)
                    .aria_label(crate::localization::text(cx, "Command")),
            )
            .child(
                canvas(
                    move |bounds, _, cx| {
                        let _ = measure.update(cx, |this, cx| {
                            let width = bounds.size.width.max(px(1.));
                            if this.width != width {
                                this.width = width;
                                cx.notify();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        Overlay::new(("composer-overlay", cx.entity_id()))
            .open(open)
            .inline_focus(&focus)
            .side(Placement::Top)
            .p_0()
            .border_color(rgb(self.colors.2))
            .bg(rgb(self.colors.0))
            .trigger_element(trigger)
            .on_open_change(move |open, window, cx| {
                if !open {
                    let _ = close.update(cx, |this, cx| {
                        if this.input.read(cx).focus_handle(cx).is_focused(window) {
                            this.model.dismiss();
                        }
                        cx.notify();
                    });
                }
            })
            .content(move |window, cx| {
                menu.upgrade()
                    .map(|entity| entity.read(cx).menu(&entity, window, cx))
                    .unwrap_or_else(|| div().into_any_element())
            })
    }
}
