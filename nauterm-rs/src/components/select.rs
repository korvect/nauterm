//! Product-owned typed selection over base Input/Select and our Overlay.
//! Search/highlight are drafts; only an enabled option or Clear commits an ID.
//! Editable mode keeps manual text in its owning InputState independently.
use super::{Field, overlay::Overlay, settings};
use crate::{palette::Palette, typography::FontWeights};
use gpui_kit::base::{
    Placement, Select as BaseSelect,
    input::{InputEvent, InputState},
};
use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme, Disableable, Icon, Selectable, Sizable},
    prelude::*,
    *,
};
use zeroize::Zeroizing;

#[derive(Clone, Debug, PartialEq)]
pub struct SelectOption<V: Clone + PartialEq> {
    value: Option<V>,
    title: SharedString,
    source_title: Option<SharedString>,
    unavailable: bool,
    icon: Option<IconName>,
    image: Option<std::sync::Arc<Image>>,
    font_family: Option<SharedString>,
    font_sample: Option<(SharedString, SharedString)>,
}
impl<V: Clone + PartialEq> SelectOption<V> {
    pub fn new(value: V, title: impl Into<SharedString>) -> Self {
        Self {
            value: Some(value),
            title: title.into(),
            source_title: None,
            unavailable: false,
            icon: None,
            image: None,
            font_family: None,
            font_sample: None,
        }
    }
    /// Product choices opt into translation; resource names and font families stay literal.
    pub fn localized(value: V, title: impl Into<SharedString>) -> Self {
        let mut option = Self::new(value, title);
        option.source_title = Some(option.title.clone());
        option
    }
    fn translate(&mut self, language: crate::localization::Language) {
        if let Some(source) = &self.source_title {
            self.title = crate::localization::text_for(language, source);
        }
    }
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn image(mut self, image: std::sync::Arc<Image>) -> Self {
        self.image = Some(image);
        self
    }
    fn leading(&self) -> Option<AnyElement> {
        self.image
            .clone()
            .map(|image| {
                img(image)
                    .size(px(18.))
                    .object_fit(ObjectFit::Contain)
                    .rounded(px(4.))
                    .flex_shrink_0()
                    .into_any_element()
            })
            .or_else(|| {
                self.icon
                    .map(|icon| crate::assets::icon(icon).size(px(18.)).into_any_element())
            })
    }
    pub fn font_preview(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }
    /// Show a separate sample while keeping the option's name in the UI font.
    /// The caller must verify glyph coverage before supplying a sample.
    pub fn font_sample(
        mut self,
        family: impl Into<SharedString>,
        sample: impl Into<SharedString>,
    ) -> Self {
        self.font_sample = Some((family.into(), sample.into()));
        self
    }
    pub fn value(&self) -> &Option<V> {
        &self.value
    }
    pub fn matches(&self, query: &str) -> bool {
        let query = Zeroizing::new(query.trim().to_lowercase());
        self.title.to_lowercase().contains(query.as_str())
            || self.source_title.as_ref().is_some_and(|source| {
                crate::localization::text_for(crate::localization::Language::English, source)
                    .to_lowercase()
                    .contains(query.as_str())
            })
    }
}
fn with_missing<V: Clone + PartialEq>(
    mut options: Vec<SelectOption<V>>,
    selected: &Option<V>,
) -> Vec<SelectOption<V>> {
    if let Some(value) = selected
        && !options.iter().any(|o| o.value.as_ref() == Some(value))
    {
        options.push(SelectOption {
            value: Some(value.clone()),
            title: "Unavailable item".into(),
            source_title: Some("Unavailable item".into()),
            unavailable: true,
            icon: None,
            image: None,
            font_family: None,
            font_sample: None,
        });
    }
    options
}
/// Existing form event contract. Clear is no selection, not a synthetic option.
pub enum SelectEvent<V: Clone + PartialEq> {
    Confirm(Option<Option<V>>),
}

/// A related-entry action does not change the selected value. The owning form
/// decides when a successfully saved entry can become the selection.
#[derive(Clone, PartialEq)]
pub struct SelectAction {
    pub id: &'static str,
    label: SharedString,
    icon: IconName,
    from_query: bool,
}
impl SelectAction {
    pub fn new(id: &'static str, label: impl Into<SharedString>, icon: IconName) -> Self {
        Self {
            id,
            label: label.into(),
            icon,
            from_query: true,
        }
    }
    /// Create a named entry directly from a nonempty, nonconflicting query.
    pub fn create_from_query(mut self) -> Self {
        self.from_query = true;
        self
    }
    fn available<V: Clone + PartialEq>(
        &self,
        query: &str,
        masked: bool,
        options: &[SelectOption<V>],
    ) -> bool {
        if !self.from_query {
            return true;
        }
        if masked {
            return false;
        }
        let query = query.trim().to_lowercase();
        !query.is_empty()
            && !options.iter().any(|option| {
                option.value.is_some()
                    && !option.unavailable
                    && option.title.trim().to_lowercase() == query
            })
    }
}
pub struct SelectCreate {
    pub action: &'static str,
    pub text: String,
}
/// A selected ID was detached by a real text edit. Text stays in its owning
/// InputState; secret values are never copied into selection event payloads.
pub struct SelectEdited;

#[derive(Clone, Default, PartialEq)]
struct Presentation {
    label: SharedString,
    terminal_palette: Option<Palette>,
    terminal_form: bool,
    digits_only: bool,
    settings: bool,
    disabled: bool,
    cleanable: bool,
    filterable: bool,
    actions: Vec<SelectAction>,
    error: Option<SharedString>,
}
pub struct SelectState<V: Clone + PartialEq + 'static> {
    options: Vec<SelectOption<V>>,
    language: crate::localization::Language,
    committed: Option<Option<V>>,
    input: Entity<InputState>,
    editable: bool,
    retain_selection_on_search: bool,
    custom_value: Option<fn(&str) -> V>,
    visible: Vec<usize>,
    active: Option<usize>, // inventory cursor, never the persisted value
    hovered: Option<usize>,
    pointer_mode: bool,
    open: bool,
    open_requested: bool,
    trigger_hovered: bool,
    presentation: Presentation,
    width: Pixels,
    terminal_menu_width: Option<Pixels>,
    trigger: Bounds<Pixels>,
    // Chosen on open and locked so filtering or scrolling cannot flip it;
    // kept through exit. `room` is the panel height that side allowed then.
    placement: Option<Placement>,
    room: f32,
    placement_locked: bool,
    scroll: UniformListScrollHandle,
    _input_events: Subscription,
    _keyboard: Subscription,
    native_composition_key: bool,
}
impl<V: Clone + PartialEq + 'static> EventEmitter<SelectEvent<V>> for SelectState<V> {}
impl<V: Clone + PartialEq + 'static> EventEmitter<SelectCreate> for SelectState<V> {}
impl<V: Clone + PartialEq + 'static> EventEmitter<SelectEdited> for SelectState<V> {}
impl<V: Clone + PartialEq + 'static> Focusable for SelectState<V> {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search_input().read(cx).focus_handle(cx)
    }
}
impl<V: Clone + PartialEq + 'static> SelectState<V> {
    fn action_indices(&self, cx: &App) -> Vec<usize> {
        let input = self.input.read(cx);
        self.presentation
            .actions
            .iter()
            .enumerate()
            .filter_map(|(index, action)| {
                action
                    .available(
                        &input.value(),
                        input.presentation().is_masked(),
                        &self.options,
                    )
                    .then_some(index)
            })
            .collect()
    }
    fn enabled_indices(&self, cx: &App) -> Vec<usize> {
        let actions = self.action_indices(cx);
        actions
            .iter()
            .copied()
            .filter(|index| self.presentation.actions[*index].from_query)
            .map(|index| self.options.len() + index)
            .chain(
                self.visible
                    .iter()
                    .copied()
                    .filter(|index| !self.options[*index].unavailable),
            )
            .chain(
                actions
                    .iter()
                    .copied()
                    .filter(|index| !self.presentation.actions[*index].from_query)
                    .map(|index| self.options.len() + index),
            )
            .collect()
    }
    fn new(
        mut options: Vec<SelectOption<V>>,
        selected: Option<V>,
        input: Option<Entity<InputState>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let language = crate::localization::language(cx);
        for option in &mut options {
            option.translate(language);
        }
        let committed = selected.map(Some);
        let title = committed
            .as_ref()
            .and_then(|v| options.iter().find(|o| &o.value == v))
            .map(|o| o.title.clone())
            .unwrap_or_default();
        let editable = input.is_some();
        let input = input.unwrap_or_else(|| {
            cx.new(|cx| InputState::new(window, cx).default_value(title.clone()))
        });
        if editable && committed.is_some() {
            input.update(cx, |input, cx| input.set_value(title, window, cx));
        }
        let events = cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
            InputEvent::Change if !this.presentation.disabled && this.is_editable() => {
                if this.editable
                    && !this.retain_selection_on_search
                    && this.committed.is_some()
                    && this.input.read(cx).value() != this.title()
                {
                    this.committed = None;
                    cx.emit(SelectEdited);
                }
                this.open = !(this.is_editable()
                    && this.options.is_empty()
                    && this.input.read(cx).value().trim().is_empty());
                this.filter(true, cx);
                cx.notify();
            }
            InputEvent::Blur if !this.input.read(cx).has_selection_focus(window, cx) => {
                this.set_open(false, window, cx);
            }
            InputEvent::Focus => cx.notify(),
            _ => {}
        });
        // Scope pre-action interception to this exact editor focus. Raw key
        // listeners run after bindings in GPUI and would handle Enter twice.
        let keyboard_owner = cx.weak_entity();
        let keyboard = cx.intercept_keystrokes(move |event, window, cx| {
            let Some(owner) = keyboard_owner.upgrade() else {
                return;
            };
            if !owner.read(cx).focus_handle(cx).is_focused(window) {
                return;
            }
            if owner.update(cx, |this, cx| this.key(&event.keystroke, window, cx)) {
                window.prevent_default();
                cx.stop_propagation();
            }
        });
        let mut state = Self {
            options,
            language,
            committed,
            input,
            editable,
            retain_selection_on_search: false,
            custom_value: None,
            visible: vec![],
            active: None,
            hovered: None,
            pointer_mode: false,
            open: false,
            open_requested: false,
            trigger_hovered: false,
            presentation: Presentation::default(),
            width: px(240.),
            terminal_menu_width: None,
            trigger: Bounds::default(),
            placement: None,
            room: 0.,
            placement_locked: false,
            scroll: UniformListScrollHandle::new(),
            _input_events: events,
            _keyboard: keyboard,
            native_composition_key: false,
        };
        state.filter(false, cx);
        state
    }
    pub fn selected_value(&self) -> Option<&Option<V>> {
        self.committed.as_ref()
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn search_input(&self) -> &Entity<InputState> {
        &self.input
    }
    fn palette(&self, cx: &App) -> Palette {
        self.presentation.terminal_palette.unwrap_or_else(|| {
            if self.presentation.settings {
                Palette::settings(cx.theme().is_dark())
            } else {
                Palette::new(cx.theme().is_dark())
            }
        })
    }
    fn compact_terminal(&self) -> bool {
        self.presentation.terminal_palette.is_some() && !self.presentation.terminal_form
    }
    fn dark(&self, cx: &App) -> bool {
        self.presentation
            .terminal_palette
            .map(|p| {
                p.background.r * 0.2126 + p.background.g * 0.7152 + p.background.b * 0.0722 < 0.5
            })
            .unwrap_or_else(|| cx.theme().is_dark())
    }
    fn is_editable(&self) -> bool {
        self.editable || self.presentation.filterable || self.custom_value.is_some()
    }
    fn is_clearable(&self) -> bool {
        self.presentation.cleanable && self.is_editable()
    }
    fn title(&self) -> SharedString {
        self.selected_value()
            .and_then(|v| self.options.iter().find(|o| &o.value == v))
            .map(|o| o.title.clone())
            .unwrap_or_default()
    }
    fn refresh_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = crate::localization::language(cx);
        if self.language == language {
            return;
        }
        let showing_committed = self.input.read(cx).value() == self.title();
        self.language = language;
        for option in &mut self.options {
            option.translate(language);
        }
        self.terminal_menu_width = None;
        if showing_committed {
            self.restore_text(window, cx);
        }
        self.filter(false, cx);
    }
    fn restore_text(&self, window: &mut Window, cx: &mut Context<Self>) {
        if (self.editable || self.custom_value.is_some())
            && self.committed.as_ref().is_none_or(|value| value.is_none())
        {
            return;
        }
        let title = self.title();
        if self.input.read(cx).value() == title {
            return;
        }
        self.input
            .update(cx, |input, cx| input.set_value(title, window, cx));
    }
    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if (open && self.presentation.disabled) || self.open == open {
            return;
        }
        if open
            && self.options.is_empty()
            && (self.is_editable() || self.presentation.actions.is_empty())
            && self.input.read(cx).value().trim().is_empty()
        {
            self.input.update(cx, |input, cx| input.focus(window, cx));
            return;
        }
        self.open = open;
        self.hovered = None;
        self.pointer_mode = false;
        self.active = None;
        self.restore_text(window, cx);
        self.filter(false, cx);
        if open {
            let select_text =
                !self.editable && (self.presentation.filterable || self.custom_value.is_some());
            self.input.update(cx, |input, cx| {
                input.focus(window, cx);
                if select_text {
                    input.select_all(window, cx);
                }
            });
        }
        cx.notify();
    }
    fn filter(&mut self, reset: bool, cx: &App) {
        if reset {
            self.pointer_mode = false;
            self.hovered = None;
        }
        let query = Zeroizing::new(self.input.read(cx).value().trim().to_lowercase());
        // The committed title initially fills the editable trigger. Like the
        // reference, show all options until that text is actually changed.
        let all = !(self.presentation.filterable || self.custom_value.is_some())
            || query.is_empty()
            || query.as_str() == self.title().trim().to_lowercase();
        self.visible = self
            .options
            .iter()
            .enumerate()
            .filter(|(_, o)| all || o.matches(&query))
            .map(|(i, _)| i)
            .collect();
        let selected = self.committed.as_ref().and_then(|v| {
            self.visible
                .iter()
                .copied()
                .find(|i| &self.options[*i].value == v && !self.options[*i].unavailable)
        });
        let enabled = self.enabled_indices(cx);
        self.active = if reset {
            selected
        } else {
            self.active.filter(|i| enabled.contains(i)).or(selected)
        };
        self.scroll_to_active();
    }
    fn scroll_to_active(&self) {
        if let Some(row) = self
            .active
            .and_then(|i| self.visible.iter().position(|v| *v == i))
        {
            self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        }
    }
    fn navigate(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.pointer_mode = false;
        self.hovered = None;
        let enabled = self.enabled_indices(cx);
        self.active = next_enabled(&enabled, self.active, delta);
        self.scroll_to_active();
        cx.notify();
    }
    fn commit(&mut self, value: Option<V>, window: &mut Window, cx: &mut Context<Self>) {
        if self.presentation.disabled
            || !self.open
            || !self
                .visible
                .iter()
                .any(|i| self.options[*i].value == value && !self.options[*i].unavailable)
        {
            return;
        }
        let clear_input = self.is_editable() && value.is_none();
        self.committed = Some(value);
        self.open = false;
        if clear_input {
            self.input
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.restore_text(window, cx);
        cx.emit(SelectEvent::Confirm(self.committed.clone()));
        cx.notify();
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.presentation.disabled {
            return;
        }
        if !self.open {
            self.set_open(true, window, cx);
            return;
        }
        if self.active.is_none()
            && let Some(parse) = self.custom_value
        {
            let text = self.input.read(cx).value().trim().to_owned();
            if !text.is_empty() {
                let existing = self.options.iter().find(|option| {
                    !option.unavailable
                        && (option.title.eq_ignore_ascii_case(&text)
                            || option
                                .source_title
                                .as_ref()
                                .is_some_and(|source| source.eq_ignore_ascii_case(&text)))
                });
                let value = if let Some(option) = existing {
                    option.value.clone()
                } else {
                    let value = parse(&text);
                    self.options.push(SelectOption::new(value.clone(), text));
                    self.filter(false, cx);
                    Some(value)
                };
                self.commit(value, window, cx);
                return;
            }
        }
        if let Some(i) = self.active {
            if i < self.options.len() {
                self.commit(self.options[i].value.clone(), window, cx);
            } else {
                self.create(i - self.options.len(), window, cx);
            }
        }
    }
    fn create(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.presentation.disabled || !self.open {
            return;
        }
        let Some(action) = self.presentation.actions.get(index) else {
            return;
        };
        if !action.available(
            &self.input.read(cx).value(),
            self.input.read(cx).presentation().is_masked(),
            &self.options,
        ) {
            return;
        }
        let event = SelectCreate {
            action: action.id,
            text: if self.input.read(cx).presentation().is_masked()
                || self.input.read(cx).value().trim() == self.title().trim()
            {
                String::new()
            } else {
                self.input.read(cx).value().trim().to_owned()
            },
        };
        self.set_open(false, window, cx);
        cx.emit(event);
    }
    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.presentation.disabled || !self.is_clearable() {
            return;
        }
        if self.retain_selection_on_search {
            // Required values such as Font only clear their search draft. Keep
            // the applied selection until another option/custom value commits.
            self.input.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
            self.open = true;
            self.hovered = None;
            self.pointer_mode = false;
            self.active = None;
            self.filter(true, cx);
            cx.notify();
            return;
        }
        self.committed = None;
        self.open = false;
        if self.is_editable() {
            self.input
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.restore_text(window, cx);
        self.input.update(cx, |input, cx| input.focus(window, cx));
        cx.emit(SelectEvent::Confirm(None));
        cx.notify();
    }
    fn key(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.presentation.disabled {
            return false;
        }
        self.native_composition_key = self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        });
        let key = keystroke.key.as_str();
        if !matches!(
            key,
            "up" | "down" | "pageup" | "pagedown" | "enter" | "escape" | "tab"
        ) {
            return false;
        }
        // The native IME may unmark during action dispatch. Remember that this
        // keystroke began in composition so a fallback Confirm cannot commit.
        if self.native_composition_key {
            return false;
        }
        if key == "tab"
            && !keystroke.modifiers.control
            && !keystroke.modifiers.alt
            && !keystroke.modifiers.platform
        {
            self.set_open(false, window, cx);
            // Reuse base Root's traversal/focus-trap policy, not Input indent.
            // Base 0.7.3 registers these actions but keeps their Rust types
            // private; dispatch through GPUI's public action registry.
            let name = if keystroke.modifiers.shift {
                "root::TabPrev"
            } else {
                "root::Tab"
            };
            let action = cx
                .build_action(name, None)
                .expect("base Root traversal action registered");
            window.dispatch_action(action, cx);
            return true;
        }
        if keystroke.modifiers.modified() {
            return false;
        }
        match key {
            "escape" if self.open => self.set_open(false, window, cx),
            "enter" => self.confirm(window, cx),
            "up" | "down" | "pageup" | "pagedown" => {
                if !self.open {
                    self.set_open(true, window, cx);
                    if self.open && self.active.is_none() {
                        self.navigate(
                            if key == "up" || key == "pageup" {
                                -1
                            } else {
                                1
                            },
                            cx,
                        );
                    }
                } else {
                    self.navigate(
                        match key {
                            "up" => -1,
                            "down" => 1,
                            "pageup" => -8,
                            _ => 8,
                        },
                        cx,
                    );
                }
            }
            _ => return false,
        }
        true
    }
    fn reconcile(
        &mut self,
        mut options: Vec<SelectOption<V>>,
        selected: Option<V>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_language(window, cx);
        for option in &mut options {
            option.translate(self.language);
        }
        // Preserve an unselected field through ordinary form reconciliation.
        if self.options == options && self.committed.clone().flatten() == selected {
            return;
        }
        let active = self
            .active
            .and_then(|i| self.options.get(i))
            .map(|o| o.value.clone());
        let changed_selection = self.committed.clone().flatten() != selected;
        let showing_committed = self.input.read(cx).value() == self.title();
        self.options = options;
        self.terminal_menu_width = None;
        if changed_selection {
            self.committed = selected.map(Some);
        }
        self.active = active.and_then(|v| self.options.iter().position(|o| o.value == v));
        if !self.open || changed_selection || showing_committed {
            self.restore_text(window, cx);
        }
        self.filter(false, cx);
        cx.notify();
    }
    fn row<const S: bool>(&self, index: usize, state: &Entity<Self>, cx: &App) -> AnyElement {
        let option = &self.options[index];
        let p = self.palette(cx);
        let selected = self.committed.as_ref() == Some(&option.value);
        let highlighted = if self.pointer_mode {
            self.hovered == Some(index)
        } else {
            self.active == Some(index)
        };
        let value = option.value.clone();
        let pick = state.downgrade();
        let hover = state.downgrade();
        let dark = self.dark(cx);
        let hover_color = self
            .presentation
            .terminal_palette
            .map(|p| p.hover)
            .unwrap_or_else(|| rgb(if dark { 0x3e3e3e } else { 0xf0f4f5 }));
        let terminal = self.compact_terminal();
        let selected_color = if terminal {
            p.accent.alpha(0.10)
        } else if S {
            if dark {
                p.accent.opacity(0.18)
            } else {
                rgb(0xeaf3ff)
            }
        } else {
            hover_color
        };
        let option = super::button::Button::<S>::new(("select-option", index))
            .ghost()
            .content_start()
            .w_full()
            .h(px(if S { 34. } else { 32. }))
            .px(px(if self.compact_terminal() {
                9.
            } else if S {
                10.
            } else {
                12.
            }))
            .rounded(px(if terminal {
                6.
            } else if S {
                0.
            } else {
                8.
            }))
            .role(Role::ListBoxOption)
            .accessibility_label(option.title.clone())
            .selected(selected)
            .disabled(option.unavailable)
            .focusable(false)
            .tab_stop(false)
            .font_weight(if terminal {
                crate::typography::FontWeights::MEDIUM
            } else if S && selected {
                FontWeights::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .text_size(px(if self.compact_terminal() { 11.5 } else { 14. }))
            // Labels and font previews need space for ascent and descent,
            // independently of the fixed selectable row height.
            .line_height(relative(1.5))
            .custom(
                super::ButtonCustomVariant::new(cx)
                    .color(
                        if highlighted && terminal && (self.pointer_mode || !selected) {
                            p.control
                        } else if selected {
                            selected_color
                        } else if highlighted {
                            if S { p.hover } else { hover_color }
                        } else {
                            rgba(0)
                        }
                        .into(),
                    )
                    .foreground(
                        if (S || terminal) && selected {
                            p.accent
                        } else {
                            p.text
                        }
                        .into(),
                    )
                    .hover(
                        if terminal {
                            p.control
                        } else if S {
                            if dark {
                                rgb(0x3a3a3a)
                            } else if selected {
                                rgb(0xe3efff)
                            } else {
                                rgb(0xf3f6fa)
                            }
                        } else {
                            hover_color
                        }
                        .into(),
                    )
                    .highlight(
                        if terminal {
                            p.accent.alpha(0.08)
                        } else if S {
                            p.accent.opacity(0.18)
                        } else {
                            rgb(if dark { 0x484848 } else { 0xe9f1f3 })
                        }
                        .into(),
                    )
                    .active(
                        if terminal {
                            p.accent.alpha(0.14)
                        } else if S {
                            p.accent.opacity(0.22)
                        } else {
                            rgb(if dark { 0x484848 } else { 0xe9f1f3 })
                        }
                        .into(),
                    ),
            )
            .when(highlighted, |v| v.aria_active_descendant())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .w_full()
                    .when_some(option.leading(), |row, leading| row.child(leading))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .when_some(
                                option.font_family.clone().filter(|_| !terminal),
                                |v, family| v.font_family(family),
                            )
                            .child(option.title.clone()),
                    )
                    .when_some(option.font_sample.clone(), |row, (family, sample)| {
                        row.child(
                            div()
                                .w(px(64.))
                                .flex_shrink_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_right()
                                .font(font(family))
                                .text_size(px(13.))
                                .child(sample),
                        )
                    })
                    .when(S || (terminal && selected), |v| {
                        v.child(
                            div()
                                .w(px(if self.compact_terminal() { 15. } else { 20. }))
                                .flex_shrink_0()
                                .flex()
                                .justify_center()
                                .when(selected, |v| {
                                    v.child(
                                        Icon::new(IconName::Check)
                                            .size(px(16.))
                                            .text_color(p.accent),
                                    )
                                }),
                        )
                    }),
            )
            .on_hover(move |over, _, cx| {
                let _ = hover.update(cx, |this, cx| {
                    if *over {
                        if this.options.get(index).is_some_and(|o| !o.unavailable) {
                            this.pointer_mode = true;
                            this.hovered = Some(index);
                            this.active = Some(index);
                            cx.notify();
                        }
                    } else if this.hovered == Some(index) {
                        this.hovered = None;
                        cx.notify();
                    }
                });
            })
            .on_click(move |_, w, cx| {
                let _ = pick.update(cx, |this, cx| this.commit(value.clone(), w, cx));
            })
            .into_any_element();
        if S {
            option
        } else {
            div()
                .h(px(if terminal { 32. } else { 34. }))
                .px(px(if terminal { 6. } else { 7. }))
                .py(px(if terminal { 0. } else { 1. }))
                .child(option)
                .into_any_element()
        }
    }
    fn action_row_height(&self) -> f32 {
        if self.compact_terminal() { 32. } else { 34. }
    }
    fn action_rows(
        &self,
        indices: &[usize],
        state: &Entity<Self>,
        p: Palette,
        cx: &App,
    ) -> AnyElement {
        let terminal = self.compact_terminal();
        let settings = self.presentation.settings;
        let compact = self.compact_terminal();
        let hover_color = if terminal {
            p.control
        } else {
            rgb(if self.dark(cx) { 0x3e3e3e } else { 0xf0f4f5 })
        };
        let pressed = if terminal {
            p.border
        } else {
            rgb(if self.dark(cx) { 0x484848 } else { 0xe9f1f3 })
        };
        div()
            .children(indices.iter().copied().map(|index| {
                let action = &self.presentation.actions[index];
                let position = self.options.len() + index;
                let highlighted = if self.pointer_mode {
                    self.hovered == Some(position)
                } else {
                    self.active == Some(position)
                };
                let pick = state.downgrade();
                let hover = state.downgrade();
                let label = if action.from_query {
                    format!("{} {}", action.label, self.input.read(cx).value().trim()).into()
                } else {
                    action.label.clone()
                };
                let button = super::Button::new(("select-create", index))
                    .ghost()
                    .content_start()
                    .w_full()
                    .h(px(if settings { 34. } else { 32. }))
                    .px(px(if compact {
                        9.
                    } else if settings {
                        10.
                    } else {
                        12.
                    }))
                    .rounded(px(if settings {
                        0.
                    } else if compact {
                        6.
                    } else {
                        8.
                    }))
                    .focusable(false)
                    .tab_stop(false)
                    .role(Role::ListBoxOption)
                    .accessibility_label(label.clone())
                    .text_size(px(if compact { 11.5 } else { 14. }))
                    .line_height(relative(1.5))
                    .font_weight(if compact {
                        FontWeights::MEDIUM
                    } else {
                        FontWeights::REGULAR
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .w_full()
                            .gap(px(6.))
                            .child(
                                crate::assets::icon(if action.from_query {
                                    IconName::Plus
                                } else {
                                    action.icon
                                })
                                .size(px(16.))
                                .text_color(p.muted),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(label),
                            ),
                    )
                    .custom(
                        super::ButtonCustomVariant::new(cx)
                            .color(if highlighted { hover_color } else { rgba(0) }.into())
                            .foreground(p.text.into())
                            .hover(hover_color.into())
                            .highlight(pressed.into())
                            .active(pressed.into()),
                    )
                    .on_hover(move |over, _, cx| {
                        let _ = hover.update(cx, |this, cx| {
                            if *over {
                                this.pointer_mode = true;
                                this.hovered = Some(position);
                                this.active = Some(position);
                            } else if this.hovered == Some(position) {
                                this.hovered = None;
                            }
                            cx.notify();
                        });
                    })
                    .on_click(move |_, window, cx| {
                        let _ = pick.update(cx, |this, cx| this.create(index, window, cx));
                    });
                div()
                    .h(px(self.action_row_height()))
                    .px(px(if settings {
                        0.
                    } else if compact {
                        6.
                    } else {
                        7.
                    }))
                    .py(px(if settings || compact { 0. } else { 1. }))
                    .child(button)
            }))
            .into_any_element()
    }
    /// Desired option list height and the fixed chrome (padding, border,
    /// action rows) around it.
    fn list_and_chrome(&self, named: usize, related: usize) -> (f32, f32) {
        let row_height = if self.compact_terminal() {
            32.
        } else {
            SELECT_ROW
        };
        let rows = self.visible.len() as f32 * row_height;
        let list = rows.min(if self.compact_terminal() {
            224.
        } else if self.presentation.settings {
            204.
        } else {
            228.
        });
        let chrome = if self.compact_terminal() {
            12.
        } else if self.presentation.settings {
            0.
        } else {
            10.
        } + 2.
            + named as f32 * self.action_row_height()
            + if related > 0 {
                9. + related as f32 * self.action_row_height()
            } else {
                0.
            };
        (list, chrome)
    }
    /// Picks the side and the room there from the trigger's current position.
    /// Called once on open; the result stays fixed while the page scrolls.
    fn fit(&self, named: usize, related: usize, window: &Window) -> (Placement, f32) {
        let (list, chrome) = self.list_and_chrome(named, related);
        let (top, bottom) = frame_insets(window);
        fit_dropdown(
            f32::from(self.trigger.top()) - top,
            f32::from(window.viewport_size().height - self.trigger.bottom()) - bottom,
            chrome + list,
        )
    }
    /// The option list's height within the room locked on open. Chrome is
    /// reserved first, so a cramped side shortens the list instead of
    /// covering the trigger.
    fn list_height(&self, named: usize, related: usize) -> Pixels {
        let (list, chrome) = self.list_and_chrome(named, related);
        px(list.min(self.room - chrome).max(SELECT_ROW.min(list)))
    }
    fn menu(&self, state: &Entity<Self>, window: &Window, cx: &App) -> AnyElement {
        let p = self.palette(cx);
        let weak = state.downgrade();
        let (named, related): (Vec<_>, Vec<_>) = self
            .action_indices(cx)
            .into_iter()
            .partition(|index| self.presentation.actions[*index].from_query);
        let list_height = self.list_height(named.len(), related.len());
        div()
            .id("select-options")
            .role(Role::ListBox)
            .aria_label(self.presentation.label.clone())
            .w(self
                .width
                .max(self.terminal_menu_width.unwrap_or(px(0.)))
                .min((window.viewport_size().width - px(18.)).max(px(1.))))
            .text_color(p.text)
            .bg(
                if self.presentation.terminal_palette.is_none()
                    && !self.presentation.settings
                    && self.dark(cx)
                {
                    rgb(0x343434)
                } else {
                    p.surface
                },
            )
            .rounded(px(if self.compact_terminal() {
                10.
            } else if self.presentation.settings {
                7.
            } else {
                11.
            }))
            .overflow_hidden()
            .when(!named.is_empty(), |view| {
                view.child(self.action_rows(&named, state, p, cx))
            })
            .when(self.visible.is_empty() && named.is_empty(), |v| {
                v.child(
                    div()
                        .p(px(12.))
                        .text_size(px(12.))
                        .text_color(p.muted)
                        .child(crate::localization::text(cx, "rs.ui.copy.noMatchingItems")),
                )
            })
            .when(!self.visible.is_empty(), |v| {
                v.child(
                    super::uniform_scrollbar(
                        "select-scrollbar",
                        uniform_list("select-list", self.visible.len(), move |range, _, cx| {
                            let Some(state) = weak.upgrade() else {
                                return vec![];
                            };
                            let this = state.read(cx);
                            range
                                .filter_map(|row| {
                                    this.visible.get(row).map(|index| {
                                        if this.presentation.settings {
                                            this.row::<true>(*index, &state, cx)
                                        } else {
                                            this.row::<false>(*index, &state, cx)
                                        }
                                    })
                                })
                                .collect::<Vec<_>>()
                        })
                        .track_scroll(&self.scroll)
                        .w_full()
                        .h(list_height),
                        &self.scroll,
                    )
                    .hover_only(),
                )
            })
            .when(!related.is_empty(), |view| {
                view.child(
                    div()
                        .mt(px(4.))
                        .pt(px(4.))
                        .border_t_1()
                        .border_color(p.border)
                        .child(self.action_rows(&related, state, p, cx)),
                )
            })
            .into_any_element()
    }
    fn editor<const S: bool>(
        &self,
        state: &Entity<Self>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let palette = self.palette(cx);
        let clear_visible = self.is_clearable()
            && !self.presentation.disabled
            && (self.trigger_hovered || self.focus_handle(cx).is_focused(window))
            && self.input.read(cx).text().len() > 0;
        let amount = super::affix::clear_transition(state.entity_id(), clear_visible, window, cx);
        let clear = state.downgrade();
        let toggle = state.downgrade();
        let foreground = if S && (self.open || self.focus_handle(cx).is_focused(window)) {
            palette.accent
        } else {
            palette.muted
        };
        let arrow = super::affix::rotating_arrow(
            state.entity_id(),
            self.open,
            foreground,
            if S { 18. } else { 16. },
            window,
            cx,
        );
        let suffix = div()
            .relative()
            .flex_shrink_0()
            .size(px(if S { 18. } else { 16. }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                super::button::Button::<S>::new(("select-toggle", state.entity_id()))
                    .custom(
                        super::ButtonCustomVariant::new(cx)
                            .foreground(foreground.into())
                            .highlight(rgba(0).into()),
                    )
                    .ink_ripple(false)
                    .size(px(if S { 18. } else { 16. }))
                    .p_0()
                    .opacity(1. - amount)
                    .child(arrow)
                    .disabled(self.presentation.disabled || clear_visible)
                    .focusable(false)
                    .tab_stop(false)
                    .localized_accessibility_label("Show options")
                    .on_click(move |_, w, cx| {
                        let _ = toggle.update(cx, |this, cx| this.set_open(!this.open, w, cx));
                    }),
            )
            .when(amount > 0., |view| {
                view.child(
                    super::affix::clear_button::<S>(
                        ("select-clear", state.entity_id()),
                        palette,
                        self.presentation.terminal_palette.is_some(),
                        cx,
                    )
                    .absolute()
                    .top_0()
                    .right_0()
                    .opacity(amount)
                    .disabled(self.presentation.disabled || !clear_visible)
                    .tooltip(if self.retain_selection_on_search {
                        "Clear search"
                    } else {
                        "Clear selection"
                    })
                    .on_click(move |_, w, cx| {
                        let _ = clear.update(cx, |this, cx| this.clear(w, cx));
                    }),
                )
            });
        let missing = self
            .committed
            .as_ref()
            .is_some_and(|v| self.options.iter().any(|o| &o.value == v && o.unavailable));
        let leading = self
            .committed
            .as_ref()
            .and_then(|value| self.options.iter().find(|option| &option.value == value))
            .and_then(SelectOption::leading);
        super::input::Input::<S>::new(&self.input)
            .large()
            .when(
                self.presentation.digits_only && self.is_editable(),
                |input| input.digits_only(),
            )
            .aria_label(self.presentation.label.clone())
            .readonly(!self.is_editable())
            .editor_cursor(if self.is_editable() {
                CursorStyle::IBeam
            } else {
                CursorStyle::PointingHand
            })
            .cursor_pointer()
            // A fixed choice is focusable for the keyboard but shows no caret.
            .when(!self.is_editable(), |v| v.caret_color(rgba(0)))
            .focused(self.open)
            .disabled(self.presentation.disabled)
            .invalid(missing || self.presentation.error.is_some())
            .when_some(leading, |input, leading| input.prefix(leading).gap(px(7.)))
            .suffix(suffix)
            .when(S, |v| v.h(px(34.)).pr(px(8.)))
            .when_some(self.presentation.terminal_palette, |v, palette| {
                if self.presentation.terminal_form {
                    v.with_size(gpui_kit::component::Size::Medium)
                        .palette(palette)
                        .bg(palette.control)
                } else {
                    v.terminal_control(palette)
                }
            })
            .into_any_element()
    }
}
/// Gap between trigger and panel, and the panel's viewport margin, matching
/// the overlay positioner.
const DROPDOWN_GAP: f32 = 4.;
const DROPDOWN_MARGIN: f32 = 8.;
const SELECT_ROW: f32 = 34.;
/// Window frame kept clear above/below, as the positioner's clamp does.
fn frame_insets(window: &Window) -> (f32, f32) {
    let inset = f32::from(window.client_inset().unwrap_or(px(0.)));
    match window.window_decorations() {
        Decorations::Server => (0., 0.),
        Decorations::Client { tiling } => (
            if tiling.top { 0. } else { inset },
            if tiling.bottom { 0. } else { inset },
        ),
    }
}
/// Below when the panel fits, otherwise above when that fits, otherwise the
/// side with more room. `above`/`below` are the distances from the trigger to
/// the usable viewport edges. Returns the side and the panel height it allows.
fn fit_dropdown(above: f32, below: f32, desired: f32) -> (Placement, f32) {
    let room = |space: f32| (space - DROPDOWN_GAP - DROPDOWN_MARGIN).max(0.);
    let (above, below) = (room(above), room(below));
    if desired <= below || (desired > above && below >= above) {
        (Placement::Bottom, below)
    } else {
        (Placement::Top, above)
    }
}
fn next_enabled(enabled: &[usize], current: Option<usize>, delta: isize) -> Option<usize> {
    if enabled.is_empty() {
        return None;
    }
    let Some(current) = current.and_then(|i| enabled.iter().position(|v| *v == i)) else {
        return if delta < 0 {
            enabled.last()
        } else {
            enabled.first()
        }
        .copied();
    };
    Some(enabled[current.saturating_add_signed(delta).min(enabled.len() - 1)])
}
impl<V: Clone + PartialEq + 'static> Render for SelectState<V> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.entity();
        if self.compact_terminal() && self.terminal_menu_width.is_none() {
            let mut style = window.text_style();
            style.font_weight = crate::typography::FontWeights::MEDIUM;
            let longest = self
                .options
                .iter()
                .map(|option| {
                    let run = style.to_run(option.title.len());
                    window
                        .text_system()
                        .shape_line(option.title.clone(), px(11.5), &[run], None)
                        .width()
                })
                .fold(px(0.), |width, next| width.max(next));
            // Flutter's terminal dropdown measures labels and reserves 48px
            // for row padding, its checkmark and the panel's six-pixel inset.
            self.terminal_menu_width = Some(longest + px(48.));
        }
        let focus = self.focus_handle(cx);
        let p = self.palette(cx);
        let editor = if self.presentation.settings {
            self.editor::<true>(&state, window, cx)
        } else {
            self.editor::<false>(&state, window, cx)
        };
        let down = cx.weak_entity();
        let hover = cx.weak_entity();
        let measurement = cx.weak_entity();
        let trigger = div()
            .id(("select-trigger", state.entity_id()))
            .w_full()
            .on_hover(move |over, _, cx| {
                let _ = hover.update(cx, |this, cx| {
                    this.trigger_hovered = *over;
                    cx.notify();
                });
            })
            .on_mouse_down(MouseButton::Left, move |_, w, cx| {
                let _ = down.update(cx, |this, cx| {
                    let fixed = !this.is_editable();
                    this.set_open(!fixed || !this.open, w, cx);
                });
            })
            .child(editor)
            .child(
                canvas(
                    move |bounds, _, cx| {
                        let _ = measurement.update(cx, |this, cx| {
                            let width = (bounds.size.width - px(2.)).max(px(1.));
                            // Side and height are fixed on open; the overlay
                            // itself follows a scrolling trigger.
                            this.trigger = bounds;
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
            )
            .relative();
        let changed = cx.weak_entity();
        let menu = cx.weak_entity();
        if !self.open {
            self.placement_locked = false;
        } else if !self.placement_locked {
            let (named, related): (Vec<_>, Vec<_>) = self
                .action_indices(cx)
                .into_iter()
                .partition(|index| self.presentation.actions[*index].from_query);
            let (placement, room) = self.fit(named.len(), related.len(), window);
            self.placement = Some(placement);
            self.room = room;
            self.placement_locked = true;
        }
        let overlay = Overlay::new(("select-overlay", state.entity_id()))
            .open(self.is_open())
            .side(self.placement.unwrap_or(Placement::Bottom))
            // Side and size are chosen once on open; scrolling changes neither.
            .fixed_side()
            .inline_focus(&focus)
            .bg(
                if self.presentation.terminal_palette.is_none()
                    && !self.presentation.settings
                    && self.dark(cx)
                {
                    rgb(0x343434)
                } else {
                    p.surface
                },
            )
            .border_color(p.border)
            .p(px(0.))
            .py(px(if self.compact_terminal() {
                6.
            } else if self.presentation.settings {
                0.
            } else {
                5.
            }))
            .rounded(px(if self.compact_terminal() {
                10.
            } else if self.presentation.settings {
                7.
            } else {
                11.
            }))
            .shadow(vec![
                BoxShadow {
                    color: black().opacity(if self.dark(cx) {
                        if self.presentation.settings {
                            0.3
                        } else {
                            0.34
                        }
                    } else {
                        0.12
                    }),
                    offset: point(
                        px(0.),
                        px(if self.presentation.settings { 8. } else { 10. }),
                    ),
                    blur_radius: px(if self.presentation.settings { 18. } else { 22. }),
                    spread_radius: px(0.),
                    inset: false,
                },
                BoxShadow {
                    color: black().opacity(if self.dark(cx) {
                        if self.presentation.settings {
                            0.14
                        } else {
                            0.18
                        }
                    } else {
                        0.05
                    }),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(3.),
                    spread_radius: px(0.),
                    inset: false,
                },
            ])
            .when(self.compact_terminal(), |overlay| {
                overlay.shadow(vec![BoxShadow {
                    color: black().opacity(if self.dark(cx) { 0.34 } else { 0.14 }),
                    offset: point(px(0.), px(9.)),
                    blur_radius: px(7.),
                    spread_radius: px(0.),
                    inset: false,
                }])
            })
            .trigger_element(trigger)
            .on_open_change(move |open, w, cx| {
                let _ = changed.update(cx, |this, cx| this.set_open(*open, w, cx));
            })
            .content(move |window, cx| {
                menu.upgrade()
                    .map(|state| state.read(cx).menu(&state, window, cx))
                    .unwrap_or_else(|| div().into_any_element())
            });
        let opened = cx.weak_entity();
        let confirm = cx.weak_entity();
        let root = BaseSelect::new(("select", state.entity_id()))
            .open(self.open)
            .disabled(self.presentation.disabled)
            .content_focus_handle(&focus)
            .accessibility_label(self.presentation.label.clone())
            .accessibility_value(self.title())
            .on_open_change(move |open, w, cx| {
                let _ = opened.update(cx, |this, cx| {
                    if !this.native_composition_key {
                        this.set_open(open, w, cx);
                    }
                });
            })
            .on_confirm(move |w, cx| {
                let _ = confirm.update(cx, |this, cx| {
                    if !this.native_composition_key && this.open {
                        this.confirm(w, cx);
                    }
                });
            })
            .w_full()
            .min_w_0()
            .child(overlay);
        if self.presentation.settings || self.compact_terminal() {
            root.into_any_element()
        } else {
            let missing = self
                .committed
                .as_ref()
                .is_some_and(|v| self.options.iter().any(|o| &o.value == v && o.unavailable));
            Field::control(
                root,
                self.presentation.label.clone(),
                focus,
                self.input.read(cx).text().len() > 0,
            )
            .with_palette(self.presentation.terminal_palette)
            .when(self.presentation.terminal_form, |field| {
                field.with_size(gpui_kit::component::Size::Medium)
            })
            .disabled(self.presentation.disabled)
            .error(
                self.presentation.error.clone().or_else(|| {
                    missing.then(|| "The selected item is no longer available.".into())
                }),
            )
            .into_any_element()
        }
    }
}
/// Retained editor/inventory, owned by the form. Options use public titles/IDs;
/// editable input may be sensitive and retains its InputState's masking policy.
pub struct SelectModel<V: Clone + PartialEq + 'static> {
    pub state: Entity<SelectState<V>>,
}
impl<V: Clone + PartialEq + 'static> SelectModel<V> {
    /// Query edits/Clear keep the applied value; closing restores its label.
    /// Choosing or submitting a custom option still emits a confirmation.
    pub fn retain_selection_on_search(self, cx: &mut App) -> Self {
        self.state
            .update(cx, |state, _| state.retain_selection_on_search = true);
        self
    }
    /// Only controls that accept a custom value expose an editable settings trigger.
    pub fn allow_custom_values(self, parse: fn(&str) -> V, cx: &mut App) -> Self {
        self.state
            .update(cx, |state, _| state.custom_value = Some(parse));
        self
    }
    pub fn new(
        options: Vec<SelectOption<V>>,
        selected: Option<V>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let options = with_missing(options, &selected);
        Self {
            state: cx.new(|cx| SelectState::new(options, selected, None, window, cx)),
        }
    }
    /// Bind a free-form input to an optional typed selection. Blur/Escape keep
    /// manual text; only choosing an option changes its ID. Existing input
    /// masking and sensitive clipboard/accessibility policies stay in effect.
    pub fn editable(
        options: Vec<SelectOption<V>>,
        selected: Option<V>,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let options = with_missing(options, &selected);
        Self {
            state: cx
                .new(|cx| SelectState::new(options, selected, Some(input.clone()), window, cx)),
        }
    }
    pub fn reconcile(
        &mut self,
        options: Vec<SelectOption<V>>,
        selected: Option<V>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.state.update(cx, |state, cx| {
            state.reconcile(with_missing(options, &selected), selected, window, cx)
        });
    }
    /// Synchronize a controlled value without emitting a user confirmation.
    pub fn set_selected(&mut self, selected: Option<V>, window: &mut Window, cx: &mut App) {
        let options = self.state.read(cx).options.clone();
        self.reconcile(options, selected, window, cx);
    }
    pub fn field(&self, label: &'static str, disabled: bool) -> AnyElement {
        SelectView {
            state: self.state.clone(),
            presentation: Presentation {
                label: label.into(),
                disabled,
                cleanable: true,
                ..Default::default()
            },
        }
        .into_any_element()
    }
    /// A searchable trigger is also editable, even when its InputState is owned
    /// internally. Ordinary choice fields retain pointer/selection semantics.
    pub fn filterable_field(&self, label: &'static str, disabled: bool) -> AnyElement {
        self.field_with_actions(label, disabled, vec![])
    }
    /// Editable numeric choices retain the same input filtering as a port field.
    pub fn digits_field(&self, label: &'static str, disabled: bool) -> AnyElement {
        SelectView {
            state: self.state.clone(),
            presentation: Presentation {
                label: label.into(),
                disabled,
                cleanable: true,
                digits_only: true,
                ..Default::default()
            },
        }
        .into_any_element()
    }
    pub fn field_with_error(
        &self,
        label: &'static str,
        disabled: bool,
        error: Option<SharedString>,
    ) -> AnyElement {
        SelectView {
            state: self.state.clone(),
            presentation: Presentation {
                label: label.into(),
                disabled,
                cleanable: true,
                filterable: true,
                error,
                ..Default::default()
            },
        }
        .into_any_element()
    }
    /// Open after the next presentation has installed its actions/disabled state.
    pub fn open(&self, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            state.open_requested = true;
            cx.notify();
        });
    }
    pub fn field_with_actions(
        &self,
        label: &'static str,
        disabled: bool,
        actions: Vec<SelectAction>,
    ) -> AnyElement {
        SelectView {
            state: self.state.clone(),
            presentation: Presentation {
                label: label.into(),
                disabled,
                cleanable: true,
                filterable: true,
                actions,
                ..Default::default()
            },
        }
        .into_any_element()
    }
    /// A form field whose selection cannot be cleared independently.
    pub fn required_field(&self, label: &'static str, disabled: bool) -> AnyElement {
        SelectView {
            state: self.state.clone(),
            presentation: Presentation {
                label: label.into(),
                disabled,
                cleanable: false,
                ..Default::default()
            },
        }
        .into_any_element()
    }
    /// Standalone workspace control styled by its terminal, with an external label.
    pub fn terminal_control(
        &self,
        label: &'static str,
        palette: Palette,
        filterable: bool,
    ) -> AnyElement {
        SelectView {
            state: self.state.clone(),
            presentation: Presentation {
                label: label.into(),
                terminal_palette: Some(palette),
                filterable,
                cleanable: filterable,
                ..Default::default()
            },
        }
        .into_any_element()
    }
    /// Floating-label workspace form, projected into the terminal's palette.
    pub fn terminal_form_with_actions(
        &self,
        label: &'static str,
        disabled: bool,
        palette: Palette,
        actions: Vec<SelectAction>,
    ) -> AnyElement {
        SelectView {
            state: self.state.clone(),
            presentation: Presentation {
                label: label.into(),
                terminal_palette: Some(palette),
                terminal_form: true,
                disabled,
                filterable: true,
                cleanable: true,
                actions,
                ..Default::default()
            },
        }
        .into_any_element()
    }
    pub fn settings_field(&self, label: &'static str, cx: &App) -> AnyElement {
        settings::Field::new(
            label,
            self.control(label, cx),
            self.state.read(cx).focus_handle(cx),
        )
        .into_any_element()
    }
    pub fn control(&self, label: &'static str, _: &App) -> AnyElement {
        self.settings_control(label, false)
    }
    pub fn control_disabled(&self, label: &'static str, disabled: bool) -> AnyElement {
        SelectView {
            state: self.state.clone(),
            presentation: Presentation {
                label: label.into(),
                settings: true,
                disabled,
                ..Default::default()
            },
        }
        .into_any_element()
    }
    fn settings_control(&self, label: &'static str, filterable: bool) -> AnyElement {
        SelectView {
            state: self.state.clone(),
            presentation: Presentation {
                label: label.into(),
                settings: true,
                filterable,
                ..Default::default()
            },
        }
        .into_any_element()
    }
}
#[derive(IntoElement)]
struct SelectView<V: Clone + PartialEq + 'static> {
    state: Entity<SelectState<V>>,
    presentation: Presentation,
}
impl<V: Clone + PartialEq + 'static> RenderOnce for SelectView<V> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| {
            state.refresh_language(window, cx);
            state.presentation = self.presentation;
            if state.compact_terminal()
                && state.input.read(cx).presentation().placeholder() != &state.presentation.label
            {
                state.input.update(cx, |input, cx| {
                    input.set_placeholder(state.presentation.label.clone(), window, cx);
                });
            }
            if state.presentation.disabled {
                state.open_requested = false;
                state.set_open(false, window, cx);
            } else if state.open_requested {
                state.open_requested = false;
                state.set_open(true, window, cx);
            }
        });
        self.state
    }
}
#[cfg(test)]
mod tests {
    use super::{SelectAction, SelectOption, fit_dropdown, next_enabled, with_missing};
    use gpui_kit::{assets::IconName, base::Placement};
    #[test]
    fn localization_changes_product_choices_without_touching_ids_or_user_names() {
        use crate::localization::Language;
        let mut product = SelectOption::localized("system", "System Default");
        product.translate(Language::Chinese);
        assert_eq!(product.title, "跟随系统");
        assert!(product.matches("跟随"));
        assert!(product.matches("system"));
        assert_eq!(product.value(), &Some("system"));
        product.translate(Language::English);
        assert_eq!(product.title, "System Default");
        for name in ["System Default", "Light", "主机", "/tmp/{2}"] {
            let mut resource = SelectOption::new(42, name.to_owned());
            resource.translate(Language::Chinese);
            assert_eq!(resource.title, name);
            assert_eq!(resource.value(), &Some(42));
        }
        let mut weight = SelectOption::localized(300, "settings.terminal.fontWeight.light");
        weight.translate(Language::Chinese);
        assert_eq!(weight.title, "细体");
        assert!(weight.matches("Light"));
        weight.translate(Language::English);
        assert_eq!(weight.title, "Light");
    }
    #[test]
    fn dropdown_opens_below_then_flips_then_shrinks_on_the_larger_side() {
        // 12px of gap + margin is reserved on each side.
        assert_eq!(fit_dropdown(300., 300., 200.), (Placement::Bottom, 288.));
        assert_eq!(fit_dropdown(400., 100., 200.), (Placement::Top, 388.));
        assert_eq!(fit_dropdown(150., 120., 200.), (Placement::Top, 138.));
        assert_eq!(fit_dropdown(120., 150., 200.), (Placement::Bottom, 138.));
        assert_eq!(fit_dropdown(5., 5., 200.), (Placement::Bottom, 0.));
    }
    #[test]
    fn named_creation_requires_a_new_nonempty_unmasked_query() {
        let options = vec![SelectOption::new(7u64, " Prod 测试 ")];
        let action =
            SelectAction::new("package", "Create package", IconName::Package).create_from_query();
        assert!(!action.available("   ", false, &options));
        assert!(!action.available("prod 测试", false, &options));
        assert!(!action.available("new package", true, &options));
        assert!(action.available(" Prod 测试 copy ", false, &options));
        let related = SelectAction::new("identity", "Create identity", IconName::Plus);
        assert!(!related.available("", false, &options));
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum CredentialRef {
        Identity(u64),
        Key(u64),
    }
    #[test]
    fn selection_uses_typed_ids_not_display_names_or_filtered_indices() {
        let options = with_missing(
            vec![
                SelectOption::new(CredentialRef::Identity(1), "Same name"),
                SelectOption::new(CredentialRef::Key(1), "Same name"),
            ],
            &Some(CredentialRef::Key(1)),
        );
        assert_eq!(options.len(), 2);
        assert_eq!(options[1].value(), &Some(CredentialRef::Key(1)));
        assert!(options.iter().all(|o| !o.unavailable));
    }
    #[test]
    fn missing_values_are_not_silently_cleared_or_changed_to_the_first_item() {
        let options = with_missing(vec![SelectOption::new(2u64, "Renamed group")], &Some(1));
        assert_eq!(options.len(), 2);
        assert_eq!(options[1].value(), &Some(1));
        assert!(options[1].unavailable);
        assert_eq!(with_missing(options, &Some(1)).len(), 2);
        assert!(!with_missing(vec![SelectOption::new(1u64, "Restored")], &Some(1))[0].unavailable);
    }
    #[test]
    fn unselected_fields_and_search_keep_the_original_values() {
        let item = SelectOption::new(7u64, "ED25519 — 测试 KEY");
        assert!(item.matches(" key "));
        assert!(item.matches("测试"));
        assert!(!item.matches("RSA"));
        assert_eq!(item.value(), &Some(7));
        assert_eq!(with_missing(vec![item.clone()], &None), vec![item]);
        assert!(with_missing::<u64>(vec![], &None).is_empty());
    }
    #[test]
    fn keyboard_cursor_skips_unavailable_rows_and_clamps() {
        assert_eq!(next_enabled(&[], None, 1), None);
        assert_eq!(next_enabled(&[0, 2, 5], None, 1), Some(0));
        assert_eq!(next_enabled(&[0, 2, 5], None, -1), Some(5));
        assert_eq!(next_enabled(&[0, 2, 5], Some(0), 1), Some(2));
        assert_eq!(next_enabled(&[0, 2, 5], Some(5), 1), Some(5));
        assert_eq!(next_enabled(&[0, 2, 5], Some(0), -1), Some(0));
        assert_eq!(next_enabled(&[0, 2, 5], Some(2), -8), Some(0));
    }
}
