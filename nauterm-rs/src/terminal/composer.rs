//! The reference floating command surface, over the shared multiline Input.
//! Expanding never resizes the PTY; only one collapsed line is reserved below it.
use super::*;
use crate::components::{ButtonCustomVariant, Input, motion};

pub(super) struct State {
    expanded: bool,
    motion: motion::Transition,
    sensitive: Option<bool>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            expanded: false,
            motion: motion::Transition::ease_in_out(0.),
            sensitive: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Geometry {
    bounds: Bounds<Pixels>,
    input_right: Pixels,
    input_bottom: Pixels,
    expand_top: Pixels,
    expand_right: Pixels,
}
fn reserved_height(line_height: f32) -> f32 {
    (line_height + 22.).max(42.) + 8.
}
fn geometry(viewport: Size<Pixels>, line_height: f32, lines: usize, progress: f32) -> Geometry {
    let width = f32::from(viewport.width).max(0.);
    let height = f32::from(viewport.height).max(0.);
    let collapsed_width = (width - 16.).clamp(0., 960.);
    let collapsed_height = (line_height * lines.clamp(1, 3) as f32 + 22.)
        .max(42.)
        .min(height);
    let collapsed_left = (width - collapsed_width) / 2.;
    let collapsed_top = (height - collapsed_height - 8.).max(0.);
    let expanded_left = 12_f32.min(width / 2.);
    let expanded_top = 44_f32.min((height - 12.).max(0.));
    let expanded_width = (width - expanded_left * 2.).max(0.);
    let expanded_height = (height - expanded_top - 12.)
        .max(collapsed_height)
        .min(height - expanded_top);
    let t = progress.clamp(0., 1.);
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    let actual_height = lerp(collapsed_height, expanded_height);
    let content_height = (actual_height - 14.).max(0.);
    Geometry {
        bounds: Bounds::new(
            point(
                px(lerp(collapsed_left, expanded_left)),
                px(lerp(collapsed_top, expanded_top)),
            ),
            size(px(lerp(collapsed_width, expanded_width)), px(actual_height)),
        ),
        input_right: px(lerp(64., 34.)),
        input_bottom: px(34. * t),
        expand_top: px((content_height - 28.).max(0.) * (1. - t)),
        expand_right: px(30. * (1. - t)),
    }
}

impl Terminal {
    pub(super) fn composer_reserved_height(&self) -> Pixels {
        if self.composer_visible {
            px(reserved_height(self.appearance.cell_height()))
        } else {
            px(0.)
        }
    }
    fn toggle_composer_expanded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.composer_ui.expanded = !self.composer_ui.expanded;
        self.focus_composer(window, cx);
        cx.notify();
    }
    fn focus_composer(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer_sensitive() {
            self.secret_composer
                .update(cx, |input, cx| input.focus(window, cx));
        } else {
            self.composer
                .update(cx, |input, cx| input.focus(window, cx));
        }
    }
    /// Lay out after the terminal's flex/split layout, using this frame's bounds.
    /// A cached prepaint size makes the composer trail a moving divider by a frame.
    pub(super) fn composer_overlay(&self, cx: &Context<Self>) -> AnyElement {
        let owner = cx.entity();
        canvas(
            move |bounds, window, cx| {
                let surface = owner.update(cx, |this, cx| {
                    this.composer_surface(bounds.size, window, cx)
                });
                let mut element = div()
                    .size_full()
                    .relative()
                    .child(surface)
                    .into_any_element();
                element.layout_as_root(bounds.size.map(AvailableSpace::Definite), window, cx);
                element.prepaint_at(bounds.origin, window, cx);
                element
            },
            |_, mut element, window, cx| element.paint(window, cx),
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }
    fn composer_surface(
        &mut self,
        viewport: Size<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sensitive = self.composer_sensitive();
        if self.composer_ui.sensitive != Some(sensitive) {
            let was_sensitive = self.composer_ui.sensitive == Some(true);
            self.composer_ui.sensitive = Some(sensitive);
            let transfer_focus = if sensitive {
                self.composer.read(cx).focus_handle(cx).is_focused(window)
            } else {
                self.secret_composer
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            };
            // Secrets never share the multiline command/history editor. Clear
            // an unsent password when another input source leaves that prompt.
            if was_sensitive && !sensitive {
                self.secret_composer
                    .update(cx, |input, cx| input.set_value("", window, cx));
            }
            if transfer_focus {
                self.focus_composer(window, cx);
            }
        }
        let palette = self.appearance.palette();
        let mut p = crate::palette::Palette::terminal(palette);
        p.muted = p.text.alpha(0.48);
        let face = renderer::terminal_font(&self.appearance);
        let line_height = self.appearance.cell_height();
        let collapsed_width = (f32::from(viewport.width) - 16.).clamp(0., 960.);
        let lines = if sensitive {
            1
        } else {
            let text: SharedString = self.composer.read(cx).value().to_string().into();
            let run = TextRun {
                len: text.len(),
                font: face.clone(),
                color: p.text.into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_text(
                    text,
                    px(self.appearance.size),
                    &[run],
                    Some(px((collapsed_width - 116.).max(1.))),
                    Some(3),
                )
                .map(|lines| {
                    lines
                        .iter()
                        .map(|line| line.wrap_boundaries().len() + 1)
                        .sum::<usize>()
                        .clamp(1, 3)
                })
                .unwrap_or(1)
        };
        let expanded = self.composer_ui.expanded;
        let (progress, moving) = self.composer_ui.motion.set(
            f32::from(expanded),
            Instant::now(),
            if expanded {
                motion::COMPOSER_EXPAND
            } else {
                motion::COMPOSER_COLLAPSE
            },
            cx.reduce_motion(),
        );
        if moving {
            window.request_animation_frame();
        }
        let expanded_surface = expanded || progress > 0.;
        let g = geometry(viewport, line_height, lines, progress);
        let focused = if sensitive {
            self.secret_composer
                .read(cx)
                .has_selection_focus(window, cx)
        } else {
            self.composer.read(cx).has_selection_focus(window, cx)
        };
        let border = if focused {
            p.accent.alpha(0.78)
        } else {
            p.text.alpha(if palette.dark { 0.14 } else { 0.10 })
        };
        // Restore depth with a denser contact shadow and a soft ambient layer.
        // GPUI's blur is Gaussian sigma: keep both tails inside the 8–12px
        // gutters so a neighboring tool panel does not expose a clipped seam.
        // Interpolate with the surface instead of switching on the first frame.
        let shadow_alpha = if palette.dark {
            0.26 + 0.08 * progress
        } else {
            0.14 + 0.06 * progress
        };
        let input = if sensitive {
            Input::new(&self.secret_composer)
        } else {
            Input::multiline(&self.composer)
        }
        .terminal_shortcuts()
        .palette(p)
        .caret_color(rgb(palette.base.cursor))
        .appearance(false)
        .bordered(false)
        .font(face)
        .text_size(px(self.appearance.size))
        .line_height(px(line_height))
        .h_full()
        .aria_label(if sensitive { "Password" } else { "Command" });
        let expand = Button::new("expand-composer")
            .ghost()
            .custom(ButtonCustomVariant::for_surface(p).foreground(p.muted.into()))
            .size(px(28.))
            .icon_size(px(16.))
            .icon(if expanded {
                IconName::Minimize2
            } else {
                IconName::Maximize2
            })
            .tooltip(if expanded {
                "Collapse Composer"
            } else {
                "Expand Composer"
            })
            .on_click(cx.listener(|this, _, window, cx| this.toggle_composer_expanded(window, cx)));
        let submit = Button::new("submit-command")
            .ghost()
            .custom(ButtonCustomVariant::for_surface(p).foreground(p.muted.into()))
            .size(px(28.))
            .icon_size(px(17.))
            .icon(IconName::CornerDownLeft)
            .tooltip("Send · Shift+Enter for a new line")
            .on_click(cx.listener(move |this, _, window, cx| {
                if this.composer_sensitive() == sensitive {
                    this.submit_composer(window, cx);
                }
            }));
        div()
            .id("terminal-composer-surface")
            .absolute()
            .left(g.bounds.left())
            .top(g.bounds.top())
            .w(g.bounds.size.width)
            .h(g.bounds.size.height)
            .min_w_0()
            .min_h_0()
            .occlude()
            .rounded(px(10.))
            .bg(p.surface)
            .border_1()
            .border_color(border)
            .shadow(vec![
                gpui_kit::base::box_shadow(
                    px(0.),
                    px(1.5 + 1.5 * progress),
                    px(3. + 1.5 * progress),
                    px(-1. - progress),
                    rgb(0).alpha(shadow_alpha).into(),
                ),
                gpui_kit::base::box_shadow(
                    px(0.),
                    px(0.75),
                    px(1.),
                    px(-0.25),
                    rgb(0)
                        .alpha(if palette.dark {
                            0.10 + 0.02 * progress
                        } else {
                            0.05 + 0.01 * progress
                        })
                        .into(),
                ),
            ])
            .pl(px(11.))
            .pr(px(7.))
            .py(px(6.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_composer(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .relative()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right(g.input_right)
                            .bottom(g.input_bottom)
                            .min_w_0()
                            .min_h_0()
                            .flex()
                            .gap(px(8.))
                            .when(expanded_surface, |view| view.items_start())
                            .when(!expanded_surface, |view| view.items_center())
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_color(p.accent)
                                    .text_size(px(self.appearance.size))
                                    .font_family(self.appearance.resolved_family().to_owned())
                                    .line_height(px(line_height))
                                    .py(px(3.))
                                    .child(if sensitive {
                                        gpui_kit::component::Icon::new(IconName::Lock)
                                            .with_size(px(self.appearance.size))
                                            .into_any_element()
                                    } else {
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .child("$")
                                            .into_any_element()
                                    }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .py(px(3.))
                                    .when(sensitive || !expanded_surface, |view| {
                                        view.h(px(line_height * lines as f32 + 6.))
                                    })
                                    .when(!sensitive && expanded_surface, |view| view.h_full())
                                    .child(input),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(g.expand_top)
                            .right(g.expand_right)
                            .child(expand),
                    )
                    .child(div().absolute().bottom_0().right_0().child(submit)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{geometry, reserved_height};
    use gpui_kit::{Bounds, point, px, size};
    #[test]
    fn collapsed_composer_matches_reference_insets_and_three_line_limit() {
        let g = geometry(size(px(1600.), px(800.)), 14., 1, 0.);
        assert_eq!(
            g.bounds,
            Bounds::new(point(px(320.), px(750.)), size(px(960.), px(42.)))
        );
        assert_eq!(g.input_right, px(64.));
        assert_eq!(g.input_bottom, px(0.));
        assert_eq!(
            geometry(size(px(800.), px(600.)), 14., 9, 0.)
                .bounds
                .size
                .height,
            px(64.)
        );
        assert_eq!(reserved_height(14.), 50.);
    }
    #[test]
    fn expanded_composer_covers_the_terminal_without_changing_its_reservation() {
        let g = geometry(size(px(800.), px(600.)), 14., 1, 1.);
        assert_eq!(
            g.bounds,
            Bounds::new(point(px(12.), px(44.)), size(px(776.), px(544.)))
        );
        assert_eq!((g.expand_top, g.expand_right), (px(0.), px(0.)));
        assert_eq!((g.input_right, g.input_bottom), (px(34.), px(34.)));
    }
    #[test]
    fn composer_animation_remains_inside_a_short_split_pane() {
        for t in [0., 0.25, 0.5, 0.75, 1.] {
            let g = geometry(size(px(120.), px(32.)), 24., 3, t);
            assert!(g.bounds.left() >= px(0.) && g.bounds.right() <= px(120.));
            assert!(g.bounds.top() >= px(0.) && g.bounds.bottom() <= px(32.));
        }
    }
}
