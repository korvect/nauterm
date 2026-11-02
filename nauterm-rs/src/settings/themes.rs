use super::*;
use crate::terminal_appearance::Palette as TerminalPalette;
use crate::theme_preview::ThemePreview;
use gpui_kit::component::Selectable;

const COLOR_NAMES: [&str; 23] = [
    "Accent",
    "Foreground",
    "Background",
    "Color",
    "Text",
    "Background",
    "Text",
    "Black",
    "Red",
    "Green",
    "Yellow",
    "Blue",
    "Magenta",
    "Cyan",
    "White",
    "Black",
    "Red",
    "Green",
    "Yellow",
    "Blue",
    "Magenta",
    "Cyan",
    "White",
];

// Flutter uses FittedBox.scaleDown for these names. Shape the complete name,
// then reduce its font size only when the measured cell actually needs it.
fn color_name(name: &'static str, color: Rgba) -> AnyElement {
    div()
        .w_full()
        .h(px(13.))
        .flex_shrink_0()
        .text_size(px(9.))
        .font_weight(FontWeights::MEDIUM)
        .text_color(color)
        .child(
            canvas(
                move |bounds, window, cx| {
                    if bounds.size.width <= px(0.) {
                        return None;
                    }
                    let style = window.text_style();
                    let font_size = style.font_size.to_pixels(window.rem_size());
                    let text = crate::localization::text(cx, name);
                    let run = style.to_run(text.len());
                    let line = window.text_system().shape_line(
                        text.clone(),
                        font_size,
                        std::slice::from_ref(&run),
                        None,
                    );
                    Some(if line.width > bounds.size.width {
                        window.text_system().shape_line(
                            text,
                            font_size * (bounds.size.width / line.width),
                            &[run],
                            None,
                        )
                    } else {
                        line
                    })
                },
                |bounds, line, window, cx| {
                    if let Some(line) = line {
                        let _ = line.paint(
                            bounds.origin,
                            bounds.size.height,
                            TextAlign::Center,
                            Some(bounds.size.width),
                            window,
                            cx,
                        );
                    }
                },
            )
            .size_full(),
        )
        .into_any_element()
}

impl Settings {
    pub(super) fn init_theme_colors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (index, color) in self
            .values
            .terminal
            .palette()
            .editor_colors()
            .into_iter()
            .enumerate()
        {
            let state = cx.new(|cx| RgbState::new(color, window, cx));
            let events = cx.subscribe_in(
                &state,
                window,
                move |this, state, event: &RgbChange, window, cx| {
                    // A overlay dismissed while selecting another theme cannot later
                    // apply its old draft to the new selection.
                    if state.read(cx).revision != event.revision {
                        return;
                    }
                    if let Err(error) = this.values.terminal.change_color(index, event.color) {
                        this.error = Some(error.to_string());
                        cx.notify();
                        return;
                    }
                    this.changed(window, cx);
                },
            );
            self.colors.push(ColorControl {
                state,
                _events: events,
            });
        }
    }
    fn select_theme(&mut self, id: ThemeId, window: &mut Window, cx: &mut Context<Self>) {
        self.external_theme(id, window, cx);
        self.changed(window, cx);
    }
    pub fn external_theme(&mut self, id: ThemeId, window: &mut Window, cx: &mut Context<Self>) {
        self.values.terminal.select_theme(id);
        let values = self.values.terminal.palette().editor_colors();
        for (control, color) in self.colors.iter().zip(values) {
            control
                .state
                .update(cx, |state, cx| state.set_value(color, window, cx));
        }
        cx.notify();
    }
    fn color_row(&self, range: std::ops::Range<usize>, ansi: bool) -> AnyElement {
        div()
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_start()
            .gap(px(if ansi { 0. } else { 8. }))
            .children(range.map(|index| {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(3.))
                    .when(ansi, |view| view.flex_1().min_w_0())
                    .when(!ansi, |view| view.w(px(56.)))
                    .child(color_name(
                        COLOR_NAMES[index],
                        Palette::settings(self.values.dark).muted,
                    ))
                    .child(RgbPicker::new(
                        &self.colors[index].state,
                        format!(
                            "{} {} color",
                            match index {
                                0..=2 => "Primary",
                                3..=4 => "Cursor",
                                5..=6 => "Selection",
                                7..=14 => "Normal",
                                _ => "Bright",
                            },
                            COLOR_NAMES[index]
                        ),
                    ))
            }))
            .into_any_element()
    }
    fn inspector(&self) -> AnyElement {
        let p = Palette::settings(self.values.dark);
        let label = |text: &'static str| {
            SettingsText::Section
                .apply(div().id(SharedString::from(format!("theme-color-section-{text}"))))
                .role(Role::Heading)
                .aria_label(text)
                .text_color(rgb(0x6b7280))
                .child(TrackedText::new(text, px(0.8)))
        };
        let group = |title, gap, content: AnyElement| {
            div()
                .w_full()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(gap))
                .child(label(title))
                .child(content)
        };
        let divider = || {
            div()
                .w_full()
                .h(px(1.))
                .flex_shrink_0()
                .my(px(16.))
                .bg(p.border)
        };
        let ansi = |title, range| {
            div()
                .w_full()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .font_weight(FontWeights::MEDIUM)
                        .child(crate::localization::view(title)),
                )
                .child(self.color_row(range, true))
        };
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.sidebar)
            .p(px(14.))
            .flex()
            .flex_col()
            .child(group("PRIMARY", 10., self.color_row(0..3, false)))
            .child(divider())
            .child(group("CURSOR", 10., self.color_row(3..5, false)))
            .child(divider())
            .child(group("SELECTION", 10., self.color_row(5..7, false)))
            .child(divider())
            .child(group(
                "ANSI COLORS",
                12.,
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(ansi("Normal", 7..15))
                    .child(ansi("Bright", 15..23))
                    .into_any_element(),
            ))
            .into_any_element()
    }
    pub(super) fn theme_section(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::settings(self.values.dark);
        let compact = window.viewport_size().width < px(720.);
        let usable = (f32::from(window.viewport_size().width) - 252.).min(1000.);
        let gallery_width = if compact {
            usable - 20.
        } else {
            (usable - 24.) / 2. - 20.
        };
        let columns = if gallery_width < 500. { 2 } else { 3 };
        let selected =
            (self.values.terminal.custom_palette.is_none()).then_some(self.values.terminal.theme);
        let title = if selected.is_some() {
            self.values.terminal.theme.label()
        } else {
            "Custom".into()
        };
        let ready = self.theme_gallery_ready;
        let row_count = if ready {
            self.theme_matches.len().div_ceil(columns)
        } else {
            0
        };
        div().w_full().flex().gap(px(24.)).when(compact, |view| view.flex_col())
            .child(div().flex_1().min_w_0().h(px(620.)).flex().flex_col().gap(px(14.))
                .child(div().w_full().h(px(ThemePreview::COMPACT_HEIGHT)).flex_shrink_0().child(ThemePreview::new(title, self.values.terminal.palette()).compact()))
                .child(self.inspector()))
            .child(div().flex_1().min_w_0().h(px(620.)).rounded(px(10.)).border_1().border_color(p.border).bg(p.sidebar).p(px(10.)).flex().flex_col().gap(px(10.))
                .child(Input::new(&self.theme_search).small().aria_label(crate::localization::text(cx, "Search themes")).cleanable(true).prefix(Icon::new(IconName::Search).with_size(px(14.))))
                .child(div().flex_1().min_h_0().overflow_hidden()
                    .when(!ready, |view| view.flex().items_center().justify_center().child(crate::components::Spinner::new()))
                    .when(ready && row_count == 0, |view| view.flex().items_center().justify_center().text_color(p.muted).child(crate::localization::text(cx, "rs.ui.copy.noMatchingThemes")))
                    .when(row_count > 0, |view| view.child(crate::components::uniform_scrollbar("theme-grid-scrollbar", uniform_list("terminal-theme-grid", row_count,
                        cx.processor(move |this, rows: std::ops::Range<usize>, _, cx| {
                            rows.map(|row| {
                                let start = row * columns;
                                let end = (start + columns).min(this.theme_matches.len());
                                div().w_full().h(px(142.)).pb(px(10.)).flex().gap(px(10.))
                                    .children(this.theme_matches[start..end].iter().copied().map(|id| {
                                        let colors = TerminalPalette::new(id);
                                        Button::new(SharedString::from(format!("theme-{}", id.id()))).ghost().selected(selected == Some(id))
                                            .flex_1().min_w_0().h(px(132.)).p_0().rounded(px(8.)).accessibility_label(id.label())
                                            .child(ThemePreview::new(id.label(), colors).selected(selected == Some(id)))
                                            .on_click(cx.listener(move |this, _, window, cx| this.select_theme(id, window, cx)))
                                    }))
                                    .children((end - start..columns).map(|_| div().flex_1().min_w_0()))
                            }).collect()
                        })).track_scroll(&self.theme_scroll).size_full(), &self.theme_scroll)))))
            .into_any_element()
    }
}
