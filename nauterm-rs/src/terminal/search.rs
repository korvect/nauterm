use super::*;
use crate::typography::FontWeights;
use crate::{
    components::Input,
    terminal_search::{self, Direction, NativeResult},
};
use gpui_kit::base::input::{Enter, Escape, InputState};
use gpui_kit::component::Disableable;

pub(super) struct Search {
    pub visible: bool,
    input: Entity<InputState>,
    result: Option<NativeResult>,
    error: Option<String>,
    _events: Subscription,
}
impl Search {
    pub fn new(window: &mut Window, cx: &mut Context<Terminal>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search terminal"));
        let events = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
            if this.search.visible && matches!(event, InputEvent::Change) {
                this.run_search(Direction::Next, false, cx);
            }
        });
        Self {
            visible: false,
            input,
            result: None,
            error: None,
            _events: events,
        }
    }
}
impl Terminal {
    pub(super) fn show_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.visible = true;
        self.search.input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        self.run_search(Direction::Next, false, cx);
    }
    fn hide_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.visible = false;
        self.search.result = None;
        self.search.error = None;
        self.clear_selection();
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn run_search(
        &mut self,
        direction: Direction,
        from_selection: bool,
        cx: &mut Context<Self>,
    ) {
        self.search.error = None;
        self.search.result = None;
        self.command_block = None;
        let query = self.search.input.read(cx).value().to_string();
        if query.is_empty() {
            self.clear_selection();
            cx.notify();
            return;
        }
        let mut selection = from_selection.then_some(self.selection).flatten();
        if let Some((lines, wrap)) = terminal_search::boundary_step(
            self.frame.columns,
            self.frame.rows,
            self.frame.history_lines,
            self.frame.display_offset,
            selection,
            direction,
        ) {
            self.session.scroll(lines);
            self.refresh(cx);
            if wrap {
                selection = None;
            }
        }
        let origin = terminal_search::origin(
            self.frame.columns,
            self.frame.rows,
            self.frame.history_lines,
            self.frame.display_offset,
            selection,
            direction,
        );
        let result = self.session.search(&query, direction, origin);
        // Native search scrolls the actual viewport; use its new snapshot for
        // grid coordinates and painting, not a frontend copy of terminal text.
        self.refresh(cx);
        self.clear_selection();
        match result.and_then(|result| {
            let selection = result.selection(
                self.frame.columns,
                self.frame.rows,
                self.frame.history_lines,
                self.frame.display_offset,
            )?;
            Ok((result, selection))
        }) {
            Ok((result, selection)) => {
                self.selection = selection;
                self.search.error.clone_from(&result.error);
                self.search.result = Some(result);
            }
            Err(error) => self.search.error = Some(error.to_string()),
        }
        cx.notify();
    }
    pub(super) fn search_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let has_query = !self.search.input.read(cx).value().is_empty();
        let has_match = self.search.error.is_none()
            && self
                .search
                .result
                .as_ref()
                .is_some_and(|result| result.found);
        let status = if self.search.error.is_some() {
            "Error".into()
        } else {
            self.search
                .result
                .as_ref()
                .map(NativeResult::status)
                .unwrap_or_default()
        };
        let palette = self.appearance.palette();
        let foreground = palette.base.foreground;
        let dark = palette.dark;
        let (accent, error) = (palette.accent, palette.ansi[1]);
        let background = rgb(crate::terminal_appearance::blend(
            palette.base.background,
            foreground,
            if dark { 0.045 } else { 0.025 },
        ));
        div()
            .absolute()
            .top(px(10.))
            .left(px(10.))
            .right(px(10.))
            .flex()
            .justify_end()
            .child(
                div()
                    .id("terminal-search")
                    .occlude()
                    .role(Role::Group)
                    .aria_label(crate::localization::text(cx, "Terminal search"))
                    .w_full()
                    .max_w(px(356.))
                    .min_w_0()
                    .px(px(8.))
                    .py(px(7.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .bg(background)
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgba((foreground << 8) | if dark { 46 } else { 36 }))
                    .shadow_md()
                    .text_color(rgb(foreground))
                    .capture_action(cx.listener(|this, event: &Enter, _, cx| {
                        this.run_search(
                            if event.shift {
                                Direction::Previous
                            } else {
                                Direction::Next
                            },
                            true,
                            cx,
                        );
                        cx.stop_propagation();
                    }))
                    .on_action(cx.listener(|this, _: &Escape, window, cx| {
                        this.hide_search(window, cx);
                        cx.stop_propagation();
                    }))
                    .child(
                        gpui_kit::component::Icon::new(IconName::Search)
                            .with_size(px(18.))
                            .text_color(rgba((foreground << 8) | 148)),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.search.input)
                                .terminal_shortcuts()
                                .small()
                                .h(px(28.))
                                .px(px(3.))
                                .text_size(px(13.))
                                .appearance(false)
                                .aria_label(crate::localization::text(cx, "Search terminal")),
                        ),
                    )
                    .when(has_query && !status.is_empty(), |view| {
                        view.child(
                            div()
                                .id("terminal-search-status")
                                .role(Role::Label)
                                .aria_label(self.search.error.as_ref().map_or_else(
                                    || {
                                        crate::localization::message(
                                            cx,
                                            &format!("Search results: {status}"),
                                        )
                                    },
                                    |error| {
                                        crate::localization::message(
                                            cx,
                                            &format!("Search error: {error}"),
                                        )
                                    },
                                ))
                                .max_w(px(74.))
                                .flex_shrink_0()
                                .overflow_hidden()
                                .text_ellipsis()
                                .text_size(px(11.))
                                .font_weight(FontWeights::SEMIBOLD)
                                .text_color(rgb(if has_match { accent } else { error }))
                                .child(crate::localization::message(cx, &status)),
                        )
                    })
                    .child(
                        Button::new("terminal-search-previous")
                            .xsmall()
                            .ghost()
                            .icon(IconName::ChevronUp)
                            .tooltip("Previous match · Shift+Enter")
                            .disabled(!has_match)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run_search(Direction::Previous, true, cx)
                            })),
                    )
                    .child(
                        Button::new("terminal-search-next")
                            .xsmall()
                            .ghost()
                            .icon(IconName::ChevronDown)
                            .tooltip("Next match · Enter")
                            .disabled(!has_match)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run_search(Direction::Next, true, cx)
                            })),
                    )
                    .child(
                        Button::new("terminal-search-close")
                            .xsmall()
                            .ghost()
                            .icon(crate::assets::icon(IconName::X))
                            .tooltip("Close search · Escape")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.hide_search(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn find_from_shortcut(
        &mut self,
        _: &Find,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_search(window, cx);
        cx.stop_propagation();
    }
    pub(super) fn close_search_action(
        &mut self,
        _: &crate::ui::CloseEditor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.search.visible {
            self.hide_search(window, cx);
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }
}
