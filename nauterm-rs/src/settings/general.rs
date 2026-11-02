use super::*;
use crate::general_preferences::WindowSize;
use gpui_kit::base::ElementExt;

pub(super) struct LanguageControl {
    pub select: SelectModel<crate::localization::Language>,
    current: crate::localization::Language,
    _events: Subscription,
}
fn language_options() -> Vec<SelectOption<crate::localization::Language>> {
    crate::localization::Language::ALL
        .into_iter()
        .map(|value| {
            SelectOption::localized(value, value.label(crate::localization::Language::English))
        })
        .collect()
}

pub(super) struct WindowControls {
    width: Entity<InputState>,
    height: Entity<InputState>,
    pending: Option<Task<()>>,
    dirty: bool,
    available_width: Option<f32>,
    _events: Vec<Subscription>,
}
impl Settings {
    pub(super) fn language_control(
        language: crate::localization::Language,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> LanguageControl {
        let select = SelectModel::new(language_options(), Some(language), window, cx);
        let events = cx.subscribe_in(
            &select.state,
            window,
            |this, _, event: &SelectEvent<crate::localization::Language>, window, cx| {
                if let SelectEvent::Confirm(Some(Some(value))) = event {
                    this.values.general.language = *value;
                    this.changed(window, cx);
                }
            },
        );
        LanguageControl {
            select,
            current: language,
            _events: events,
        }
    }
    pub(super) fn sync_language_control(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.values.general.language;
        if self.language.current == language {
            return;
        }
        self.language.current = language;
        self.language
            .select
            .reconcile(language_options(), Some(language), window, cx);
        self.host_icons.reconcile(
            crate::avatar::Mode::ALL
                .into_iter()
                .map(|mode| SelectOption::localized(mode, mode.label()))
                .collect(),
            Some(self.values.host_icons),
            window,
            cx,
        );
        self.search.input.update(cx, |input, cx| {
            input.set_placeholder(
                crate::localization::text_for(language, "Search settings"),
                window,
                cx,
            )
        });
        self.search.relocalize(language);
        window.set_window_title(&format!("Nauterm RS {}", self.text("Settings")));
    }
    pub(super) fn text(&self, source: &str) -> SharedString {
        crate::localization::text_for(self.values.general.language, source)
    }
    pub(super) fn reset_window_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window_controls.pending = None;
        self.window_controls.dirty = false;
        let size = self.values.general.window;
        self.window_controls.width.update(cx, |input, cx| {
            input.set_value(size.width.round().to_string(), window, cx)
        });
        self.window_controls.height.update(cx, |input, cx| {
            input.set_value(size.height.round().to_string(), window, cx)
        });
    }

    pub(super) fn window_controls(
        size: WindowSize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> WindowControls {
        let width =
            cx.new(|cx| InputState::new(window, cx).default_value(size.width.round().to_string()));
        let height =
            cx.new(|cx| InputState::new(window, cx).default_value(size.height.round().to_string()));
        let events =
            [&width, &height]
                .into_iter()
                .map(|input| {
                    cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                        match event {
                            InputEvent::Change => {
                                this.window_controls.dirty = this.window_dimensions_dirty(cx);
                                if !this.window_controls.dirty {
                                    this.window_controls.pending = None;
                                    return;
                                }
                                this.window_controls.pending =
                                    Some(cx.spawn_in(window, async move |this, cx| {
                                        cx.background_executor()
                                            .timer(std::time::Duration::from_millis(300))
                                            .await;
                                        let _ = this.update_in(cx, |this, window, cx| {
                                            this.commit_window_size(false, window, cx)
                                        });
                                    }));
                            }
                            InputEvent::Blur | InputEvent::PressEnter { .. } => {
                                this.window_controls.pending = None;
                                this.commit_window_size(true, window, cx);
                            }
                            _ => {}
                        }
                    })
                })
                .collect();
        WindowControls {
            width,
            height,
            pending: None,
            dirty: false,
            available_width: None,
            _events: events,
        }
    }
    fn window_dimensions_dirty(&self, cx: &App) -> bool {
        self.window_controls.width.read(cx).value()
            != self.values.general.window.width.round().to_string()
            || self.window_controls.height.read(cx).value()
                != self.values.general.window.height.round().to_string()
    }
    fn parsed_window_size(&self, cx: &App) -> anyhow::Result<WindowSize> {
        WindowSize::parse(
            &self.window_controls.width.read(cx).value(),
            &self.window_controls.height.read(cx).value(),
        )
    }
    fn commit_window_size(
        &mut self,
        report_error: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.window_controls.dirty {
            return;
        }
        match self.parsed_window_size(cx) {
            Ok(size) => {
                self.validation_error = None;
                self.window_controls.dirty = false;
                if self.values.general.window != size {
                    self.values.general.window = size;
                    self.changed(window, cx);
                } else {
                    cx.notify();
                }
            }
            Err(error) if report_error => {
                self.validation_error = Some(error.to_string());
                cx.notify();
            }
            Err(_) => {}
        }
    }
    pub(super) fn finish_window_editing(&mut self, cx: &App) -> anyhow::Result<()> {
        self.window_controls.pending = None;
        if self.window_controls.dirty {
            self.values.general.window = self.parsed_window_size(cx)?;
            self.window_controls.dirty = false;
        }
        self.values.general.validate()
    }
    /// A screen clamp is acknowledged in this window so later unrelated
    /// settings changes do not repeatedly request the old oversized bounds.
    pub fn window_size_applied(
        &mut self,
        size: WindowSize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.values.general.window != size {
            self.values.general.window = size;
            self.window_controls.dirty = false;
            self.window_controls.width.update(cx, |input, cx| {
                input.set_value(size.width.round().to_string(), window, cx)
            });
            self.window_controls.height.update(cx, |input, cx| {
                input.set_value(size.height.round().to_string(), window, cx)
            });
        }
        cx.notify();
    }
    pub(super) fn window_settings(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let width = self
            .window_controls
            .available_width
            .unwrap_or(f32::from(window.viewport_size().width) - 252.);
        let paired = width >= 520.;
        let above = (if paired { (width - 16.) / 2. } else { width }) < 480.;
        let dimension =
            |title: &'static str, description: &'static str, input: &Entity<InputState>| {
                div()
                    .min_w_0()
                    .when(paired, |view| view.flex_1())
                    .flex()
                    .gap(px(if above { 12. } else { 16. }))
                    .when(above, |view| view.flex_col())
                    .when(!above, |view| view.items_center())
                    .child(
                        div()
                            .min_w_0()
                            .when(!above, |view| view.flex_1())
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(SettingsText::Label.apply(div()).child(self.text(title)))
                            .child(
                                SettingsText::Description
                                    .apply(div())
                                    .text_color(Palette::settings_faint(self.values.dark))
                                    .child(self.text(description)),
                            ),
                    )
                    .child(
                        div().min_w_0().when(!above, |view| view.flex_1()).child(
                            Input::new(input)
                                .digits_only()
                                .aria_label(format!("Window {title}")),
                        ),
                    )
            };
        let owner = cx.weak_entity();
        self.section(
            Section::Window,
            IconName::PanelsTopLeft,
            div()
                .w_full()
                .max_w(px(1000.))
                .flex()
                .gap(px(if paired { 16. } else { 18. }))
                .when(!paired, |view| view.flex_col())
                .on_prepaint(move |bounds, _, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        let next = f32::from(bounds.size.width);
                        let old = this.window_controls.available_width.unwrap_or(width);
                        let layout = |width: f32| {
                            let paired = width >= 520.;
                            (
                                paired,
                                (if paired { (width - 16.) / 2. } else { width }) < 480.,
                            )
                        };
                        this.window_controls.available_width = Some(next);
                        if layout(old) != layout(next) {
                            cx.notify();
                        }
                    });
                })
                .child(dimension(
                    "Width",
                    "Default window width in pixels.",
                    &self.window_controls.width,
                ))
                .child(dimension(
                    "Height",
                    "Default window height in pixels.",
                    &self.window_controls.height,
                )),
        )
    }
}
