use super::Tag;
use crate::{
    components::{Button, ButtonCustomVariant, Field, Input, ScrollbarsExt},
    palette::Palette,
    typography::FontWeights,
};
use gpui_kit::{
    assets::IconName,
    base::input::{InputEvent, InputState},
    component::{ActiveTheme, Disableable, Icon, Sizable},
    prelude::*,
    *,
};
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Filter,
    Field,
}
pub enum Event {
    Selection(BTreeSet<String>),
    Create(String),
    Rename(Tag),
    Delete(Tag),
}
#[derive(Clone)]
enum Choice {
    Tag(Tag),
    Create(String),
    Clear,
}
pub struct Picker {
    mode: Mode,
    manage: bool,
    tags: Vec<Tag>,
    selected: BTreeSet<String>,
    input: Entity<InputState>,
    _events: Subscription,
    _keyboard: Subscription,
    open: bool,
    disabled: bool,
    active: usize,
    keyboard: bool,
    hovered: Option<String>,
    trigger_hovered: bool,
    width: Pixels,
    scroll: UniformListScrollHandle,
    error: Option<String>,
}
impl EventEmitter<Event> for Picker {}
impl Picker {
    pub fn new(
        mode: Mode,
        tags: Vec<Tag>,
        selected: BTreeSet<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(if mode == Mode::Filter {
                "Search tags"
            } else {
                ""
            })
        });
        let events = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Focus | InputEvent::Blur) {
                cx.notify();
            }
            if matches!(event, InputEvent::Blur)
                && this.mode == Mode::Field
                && !this.input.read(cx).has_selection_focus(window, cx)
            {
                this.set_open(false, window, cx);
            }
            if matches!(event, InputEvent::Change) && !this.disabled {
                this.open = this.mode == Mode::Filter || !this.choices(cx).is_empty();
                this.active = 0;
                this.keyboard = false;
                this.error = None;
                this.scroll.scroll_to_item(0, ScrollStrategy::Nearest);
                cx.notify();
            }
        });
        let keyboard_owner = cx.weak_entity();
        let keyboard = cx.intercept_keystrokes(move |event, window, cx| {
            let Some(owner) = keyboard_owner.upgrade() else {
                return;
            };
            if owner
                .read(cx)
                .input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
                && owner.update(cx, |this, cx| this.key(&event.keystroke, window, cx))
            {
                window.prevent_default();
                cx.stop_propagation();
            }
        });
        Self {
            mode,
            manage: true,
            tags,
            selected,
            input,
            _events: events,
            _keyboard: keyboard,
            open: false,
            disabled: false,
            active: 0,
            keyboard: false,
            hovered: None,
            trigger_hovered: false,
            width: px(298.),
            scroll: UniformListScrollHandle::new(),
            error: None,
        }
    }
    pub fn selected(&self) -> &BTreeSet<String> {
        &self.selected
    }
    /// Host-selection flows share filtering without editing the tag catalog.
    pub fn filter_only(mut self) -> Self {
        self.manage = false;
        self
    }
    fn key(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.disabled {
            return false;
        }
        let composing = self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        });
        if composing {
            return false;
        }
        if key.key == "tab"
            && !key.modifiers.control
            && !key.modifiers.alt
            && !key.modifiers.platform
        {
            self.set_open(false, window, cx);
            let action = cx
                .build_action(
                    if key.modifiers.shift {
                        "root::TabPrev"
                    } else {
                        "root::Tab"
                    },
                    None,
                )
                .expect("base Root traversal registered");
            window.dispatch_action(action, cx);
            return true;
        }
        if key.modifiers.modified() {
            return false;
        }
        match key.key.as_str() {
            "up" => self.navigate(-1, window, cx),
            "down" => self.navigate(1, window, cx),
            "pageup" => self.navigate(-8, window, cx),
            "pagedown" => self.navigate(8, window, cx),
            "enter" if self.open => {
                if let Some(choice) = self.choices(cx).get(self.active).cloned() {
                    self.activate(choice, window, cx);
                }
            }
            "escape" if self.open => self.set_open(false, window, cx),
            _ => return false,
        }
        true
    }
    pub fn set_catalog(&mut self, tags: &[Tag], disabled: bool, cx: &mut Context<Self>) {
        if self.tags != tags || self.disabled != disabled {
            self.tags = tags.to_vec();
            self.disabled = disabled;
            self.selected
                .retain(|id| tags.iter().any(|tag| &tag.id == id));
            if disabled {
                self.open = false;
            }
            self.active = 0;
            cx.notify();
        }
    }
    pub fn created(&mut self, tag: &Tag, window: &mut Window, cx: &mut Context<Self>) {
        self.selected.insert(tag.id.clone());
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.error = None;
        cx.emit(Event::Selection(self.selected.clone()));
        cx.notify();
    }
    pub fn failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.error = Some(error);
        cx.notify();
    }
    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let open = open && (self.mode == Mode::Filter || !self.choices(cx).is_empty());
        if self.disabled || self.open == open {
            return;
        }
        self.open = open;
        self.active = 0;
        self.keyboard = false;
        self.hovered = None;
        self.error = None;
        // Dismissing the overlay must preserve the editable query, including
        // while its Input context menu is being used.
        if open && self.mode == Mode::Filter {
            self.input
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        if open {
            self.scroll.scroll_to_item(0, ScrollStrategy::Nearest);
        }
        cx.notify();
    }
    fn choices(&self, cx: &App) -> Vec<Choice> {
        let query = self.input.read(cx).value().trim().to_owned();
        let lower = query.to_lowercase();
        let mut choices = vec![];
        if self.mode == Mode::Field
            && !query.is_empty()
            && !self.tags.iter().any(|tag| tag.name.to_lowercase() == lower)
        {
            choices.push(Choice::Create(query));
        }
        choices.extend(
            self.tags
                .iter()
                .filter(|tag| tag.name.to_lowercase().contains(&lower))
                .cloned()
                .map(Choice::Tag),
        );
        if self.mode == Mode::Filter {
            choices.push(Choice::Clear);
        }
        choices
    }
    fn activate(&mut self, choice: Choice, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        match choice {
            Choice::Tag(tag) => {
                if !self.tags.iter().any(|entry| entry.id == tag.id) {
                    return;
                }
                if !self.selected.insert(tag.id.clone()) {
                    self.selected.remove(&tag.id);
                }
                if self.mode == Mode::Field {
                    self.input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                }
                cx.emit(Event::Selection(self.selected.clone()));
            }
            Choice::Create(name) => cx.emit(Event::Create(name)),
            Choice::Clear => {
                if self.selected.is_empty() {
                    return;
                }
                self.selected.clear();
                cx.emit(Event::Selection(self.selected.clone()));
            }
        }
        cx.notify();
    }
    fn navigate(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        if !self.open {
            self.set_open(true, window, cx);
        } else {
            let enabled: Vec<_> = self
                .choices(cx)
                .iter()
                .enumerate()
                .filter(|(_, choice)| !matches!(choice, Choice::Clear) || !self.selected.is_empty())
                .map(|(index, _)| index)
                .collect();
            if !enabled.is_empty() {
                self.active = enabled
                    .iter()
                    .position(|index| *index == self.active)
                    .map(|current| {
                        enabled
                            [(current as isize + delta).rem_euclid(enabled.len() as isize) as usize]
                    })
                    .unwrap_or_else(|| {
                        if delta < 0 {
                            *enabled.last().unwrap()
                        } else {
                            enabled[0]
                        }
                    });
            }
        }
        self.keyboard = true;
        let last = self
            .choices(cx)
            .len()
            .saturating_sub(if self.mode == Mode::Filter { 2 } else { 1 });
        self.scroll
            .scroll_to_item(self.active.min(last), ScrollStrategy::Nearest);
        cx.notify();
    }
    fn row(&self, choice: Choice, index: usize, owner: &Entity<Self>, cx: &App) -> AnyElement {
        let mut p = Palette::new(cx.theme().is_dark());
        let field = self.mode == Mode::Field;
        p.hover = if field {
            rgb(if cx.theme().is_dark() {
                0x3e3e3e
            } else {
                0xf0f4f5
            })
        } else {
            p.surface.blend(if cx.theme().is_dark() {
                rgb(0xffffff).alpha(0.07)
            } else {
                rgb(0x000000).alpha(0.045)
            })
        };
        let pick = owner.downgrade();
        let (label, selected, disabled) = match &choice {
            Choice::Tag(tag) => (tag.name.clone(), self.selected.contains(&tag.id), false),
            Choice::Create(name) => (
                crate::localization::text(cx, &format!("Create tag {name}")).to_string(),
                false,
                false,
            ),
            Choice::Clear => (
                crate::localization::text(cx, "Clear selection").to_string(),
                false,
                self.selected.is_empty(),
            ),
        };
        let mut button = Button::new(("tag-choice", index))
            .ghost()
            .w_full()
            .h(px(32.))
            .px(px(if field { 12. } else { 8. }))
            .rounded(px(if field { 8. } else { 6. }))
            .content_start()
            .text_size(px(if field { 14. } else { 12. }))
            .font_weight(if field {
                FontWeights::REGULAR
            } else {
                FontWeights::MEDIUM
            })
            .content_gap(px(if field { 6. } else { 10. }))
            .focusable(false)
            .tab_stop(false)
            .disabled(self.disabled || disabled)
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(
                        if self.keyboard && self.active == index {
                            p.hover
                        } else {
                            rgba(0)
                        }
                        .into(),
                    )
                    .foreground(p.text.into())
                    .hover(if self.keyboard { rgba(0) } else { p.hover }.into())
                    .highlight(
                        if field {
                            p.border
                        } else {
                            p.accent.alpha(0.08)
                        }
                        .into(),
                    )
                    .active(
                        if field {
                            p.border
                        } else {
                            p.accent.alpha(0.16)
                        }
                        .into(),
                    ),
            )
            .accessibility_label(label.clone());
        if matches!(choice, Choice::Tag(_)) {
            button = button.role(Role::CheckBox).checked(selected);
            if !field {
                button = button.icon(
                    Icon::new(if selected {
                        IconName::CircleCheck
                    } else {
                        IconName::Circle
                    })
                    .with_size(px(16.))
                    .text_color(if selected { p.accent } else { p.muted }),
                );
            }
        } else {
            button = button.icon(
                crate::assets::icon(if matches!(choice, Choice::Clear) {
                    IconName::X
                } else {
                    IconName::Plus
                })
                .size(px(16.))
                .text_color(p.muted),
            );
        }
        button = button.child(
            div()
                .flex_1()
                .min_w_0()
                .h(px(if field { 21. } else { 18. }))
                .line_height(px(if field { 21. } else { 18. }))
                .truncate()
                .child(label),
        );
        if let Choice::Tag(tag) = &choice
            && self.mode == Mode::Filter
            && self.manage
        {
            let id = tag.id.clone();
            let hover = owner.downgrade();
            button = button.on_hover(move |over, _, cx| {
                let _ = hover.update(cx, |this, cx| {
                    if *over {
                        this.hovered = Some(id.clone());
                        this.keyboard = false;
                    } else if this.hovered.as_ref() == Some(&id) {
                        this.hovered = None;
                    }
                    cx.notify();
                });
            });
            let rename = owner.downgrade();
            let delete = owner.downgrade();
            let rename_tag = tag.clone();
            let delete_tag = tag.clone();
            button = button.child(div().w(px(42.)).flex().justify_end().when(
                self.hovered.as_ref() == Some(&tag.id),
                |view| {
                    view.child(
                        Button::new(("rename-tag", index))
                            .ghost()
                            .size(px(20.))
                            .icon_size(px(13.))
                            .icon(IconName::Pencil)
                            .tooltip("Rename tag")
                            .on_click(move |_, window, cx| {
                                let _ = rename.update(cx, |this, cx| {
                                    this.set_open(false, window, cx);
                                    cx.emit(Event::Rename(rename_tag.clone()));
                                });
                            }),
                    )
                    .child(
                        Button::new(("delete-tag", index))
                            .ghost()
                            .size(px(20.))
                            .icon_size(px(13.))
                            .icon(IconName::Trash)
                            .tooltip("Delete tag")
                            .on_click(move |_, _, cx| {
                                let _ = delete
                                    .update(cx, |_, cx| cx.emit(Event::Delete(delete_tag.clone())));
                            }),
                    )
                },
            ));
        }
        if field {
            button = button.child(
                div()
                    .w(px(16.))
                    .h(px(18.))
                    .ml_auto()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(selected, |affix| {
                        affix.child(
                            Icon::new(IconName::Check)
                                .with_size(px(16.))
                                .text_color(p.accent),
                        )
                    }),
            );
        }
        let pointer = owner.downgrade();
        let hover_key = match &choice {
            Choice::Tag(tag) => tag.id.clone(),
            _ => format!("action-{index}"),
        };
        div()
            .id(SharedString::from(format!("tag-row-{hover_key}")))
            .h(px(if field { 34. } else { 32. }))
            .px(px(if field { 7. } else { 0. }))
            .py(px(if field { 1. } else { 0. }))
            .on_hover({
                let hover = owner.downgrade();
                let hover_key = hover_key.clone();
                move |over, _, cx| {
                    let _ = hover.update(cx, |this, cx| {
                        if !*over && this.hovered.as_ref() == Some(&hover_key) {
                            this.hovered = None;
                            cx.notify();
                        }
                    });
                }
            })
            .on_mouse_move(move |_, _, cx| {
                let _ = pointer.update(cx, |this, cx| {
                    if this.keyboard
                        || this.active != index
                        || this.hovered.as_ref() != Some(&hover_key)
                    {
                        this.keyboard = false;
                        this.active = index;
                        this.hovered = Some(hover_key.clone());
                        cx.notify();
                    }
                });
            })
            .child(button.on_click(move |_, window, cx| {
                let _ = pick.update(cx, |this, cx| this.activate(choice.clone(), window, cx));
            }))
            .into_any_element()
    }
    fn menu(&self, owner: &Entity<Self>, window: &Window, cx: &App) -> AnyElement {
        let choices = self.choices(cx);
        let count = choices.len();
        let filter = self.mode == Mode::Filter;
        let rows = count.saturating_sub(usize::from(filter));
        let weak = owner.downgrade();
        let p = Palette::new(cx.theme().is_dark());
        let mut search_palette = p;
        search_palette.surface = p.surface.blend(if cx.theme().is_dark() {
            rgb(0xffffff).alpha(0.07)
        } else {
            rgb(0x000000).alpha(0.045)
        });
        div()
            .id(("tag-options", owner.entity_id()))
            .rounded(px(10.))
            .overflow_hidden()
            .role(Role::Group)
            .aria_label(crate::localization::text(cx, "Tags"))
            .w(if self.mode == Mode::Filter {
                px(208.)
            } else {
                self.width - px(2.)
            })
            .px(px(if filter { 6. } else { 0. }))
            .py(px(if filter { 6. } else { 5. }))
            .text_color(p.text)
            .bg(p.surface)
            .when(self.mode == Mode::Filter, |view| {
                view.child(
                    div().pb(px(6.)).child(
                        Input::new(&self.input)
                            .small()
                            .h(px(32.))
                            .px(px(10.))
                            .py_0()
                            .gap(px(10.))
                            .rounded(px(7.))
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .font_weight(FontWeights::MEDIUM)
                            .palette(search_palette)
                            .bordered(false)
                            .aria_label(crate::localization::text(cx, "Search tags"))
                            .prefix(Icon::new(IconName::Search).with_size(px(16.))),
                    ),
                )
            })
            .when(count == 0 && !filter, |view| {
                view.child(
                    div()
                        .p(px(10.))
                        .text_size(px(12.))
                        .text_color(p.muted)
                        .child(crate::localization::text(cx, "rs.ui.copy.noMatchingTags")),
                )
            })
            .when(rows != 0, |view| {
                view.child(
                    crate::components::uniform_scrollbar(
                        "tag-scrollbar",
                        uniform_list("tag-choices", rows, move |range, _, cx| {
                            let Some(owner) = weak.upgrade() else {
                                return vec![];
                            };
                            let this = owner.read(cx);
                            let choices = this.choices(cx);
                            range
                                .filter_map(|index| {
                                    choices
                                        .get(index)
                                        .cloned()
                                        .map(|choice| this.row(choice, index, &owner, cx))
                                })
                                .collect()
                        })
                        .track_scroll(&self.scroll)
                        .w_full()
                        .h(px((rows as f32
                            * if filter { 32. } else { 34. })
                        .min(if filter { 249. } else { 256. })
                        .min(
                            (f32::from(window.viewport_size().height)
                                - if filter { 139. } else { 60. })
                            .max(32.),
                        ))),
                        &self.scroll,
                    )
                    .hover_only(),
                )
            })
            .when(filter, |view| {
                view.when(rows != 0, |view| {
                    view.child(
                        div()
                            .h(px(9.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .child(div().w_full().h(px(1.)).bg(p.border)),
                    )
                })
                .child(self.row(
                    Choice::Clear,
                    count.saturating_sub(1),
                    owner,
                    cx,
                ))
            })
            .into_any_element()
    }
}
impl Render for Picker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.input.read(cx).focus_handle(cx);
        let state = cx.entity();
        let content = state.downgrade();
        let p = Palette::new(cx.theme().is_dark());
        let mut overlay = crate::components::menu::dropdown_surface(
            ("tags-overlay", cx.entity_id()),
            p,
            cx.theme().is_dark(),
        )
        .open(self.open)
        .on_open_change(cx.listener(|this, open, window, cx| this.set_open(*open, window, cx)))
        .content(move |window, cx| {
            content
                .upgrade()
                .map(|state| state.read(cx).menu(&state, window, cx))
                .unwrap_or_else(|| div().into_any_element())
        });
        if self.mode == Mode::Filter {
            overlay = overlay.content_focus(&focus).trigger(
                Button::new("host-tag-filter")
                    .toolbar()
                    .ghost()
                    .icon(IconName::Tag)
                    .tooltip("Filter by tags"),
            );
        } else {
            let toggle = cx.weak_entity();
            let clear = cx.weak_entity();
            let chips = div()
                .id("selected-tag-chips")
                .max_w(px(195.))
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(4.))
                .overflow_x_scroll()
                .scrollbars()
                .children(
                    self.tags
                        .iter()
                        .filter(|tag| self.selected.contains(&tag.id))
                        .map(|tag| {
                            let remove = cx.weak_entity();
                            let id = tag.id.clone();
                            div()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .gap(px(3.))
                                .px(px(6.))
                                .py(px(2.))
                                .rounded(px(4.))
                                .bg(p.hover)
                                .text_size(px(12.))
                                .child(tag.name.clone())
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "remove-tag-{}",
                                        tag.id
                                    )))
                                    .ghost()
                                    .size(px(14.))
                                    .icon_size(px(11.))
                                    .icon(crate::assets::icon(IconName::X))
                                    .focusable(false)
                                    .tab_stop(false)
                                    .disabled(self.disabled)
                                    .tooltip("Remove tag")
                                    .on_click(
                                        move |_, _, cx| {
                                            let _ = remove.update(cx, |this, cx| {
                                                this.selected.remove(&id);
                                                cx.emit(Event::Selection(this.selected.clone()));
                                                cx.notify();
                                            });
                                        },
                                    ),
                                )
                        }),
                );
            let show_clear = !self.disabled
                && (self.trigger_hovered || focus.is_focused(window))
                && (!self.selected.is_empty() || self.input.read(cx).text().len() != 0);
            let amount =
                crate::components::affix::clear_transition(cx.entity_id(), show_clear, window, cx);
            let arrow = crate::components::affix::rotating_arrow(
                cx.entity_id(),
                self.open,
                p.muted,
                16.,
                window,
                cx,
            );
            let suffix = div()
                .relative()
                .flex_shrink_0()
                .size(px(16.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Button::new("tag-options")
                        .custom(
                            ButtonCustomVariant::new(cx)
                                .foreground(p.muted.into())
                                .highlight(rgba(0).into()),
                        )
                        .ink_ripple(false)
                        .size(px(16.))
                        .p_0()
                        .opacity(1. - amount)
                        .child(arrow)
                        .focusable(false)
                        .tab_stop(false)
                        .disabled(self.disabled || show_clear)
                        .localized_accessibility_label("Show tag options")
                        .on_click(move |_, window, cx| {
                            let _ =
                                toggle.update(cx, |this, cx| this.set_open(!this.open, window, cx));
                        }),
                )
                .when(amount > 0., |v| {
                    v.child(
                        crate::components::affix::clear_button::<false>(
                            "clear-host-tags",
                            p,
                            false,
                            cx,
                        )
                        .absolute()
                        .top_0()
                        .right_0()
                        .opacity(amount)
                        .disabled(self.disabled || !show_clear)
                        .tooltip("Clear selection")
                        .on_click(move |_, window, cx| {
                            let _ = clear.update(cx, |this, cx| {
                                this.selected.clear();
                                this.input
                                    .update(cx, |input, cx| input.set_value("", window, cx));
                                this.set_open(false, window, cx);
                                this.input.update(cx, |input, cx| input.focus(window, cx));
                                cx.emit(Event::Selection(this.selected.clone()));
                                cx.notify();
                            });
                        }),
                    )
                });
            let input = Input::new(&self.input)
                .large()
                .aria_label(crate::localization::text(cx, "Tags"))
                .cursor_pointer()
                .disabled(self.disabled)
                .when(!self.selected.is_empty(), |input| input.prefix(chips))
                .suffix(suffix);
            let down = cx.weak_entity();
            let hover = cx.weak_entity();
            let measure = cx.weak_entity();
            let trigger = div()
                .id("tag-select-trigger")
                .on_hover(move |over, _, cx| {
                    let _ = hover.update(cx, |this, cx| {
                        this.trigger_hovered = *over;
                        cx.notify();
                    });
                })
                .w_full()
                .relative()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    let _ = down.update(cx, |this, cx| this.set_open(true, window, cx));
                })
                .child(input)
                .child(
                    canvas(
                        move |bounds, _, cx| {
                            let _ = measure.update(cx, |this, cx| {
                                let width = (bounds.size.width - px(2.)).max(px(1.));
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
            overlay = overlay.inline_focus(&focus).trigger_element(trigger);
        }
        let root = div().id(("tag-picker", cx.entity_id())).child(overlay);
        if self.mode == Mode::Field {
            Field::control(
                root.w_full(),
                "Tags",
                focus,
                !self.selected.is_empty() || self.input.read(cx).text().len() > 0,
            )
            .disabled(self.disabled)
            .into_any_element()
        } else {
            root.into_any_element()
        }
    }
}
