//! Terminal-bound browser chrome. Its endpoint is fixed by the SSH session;
//! task/favorite overlays belong to this pane rather than a global popup.
use super::*;
use crate::components::ButtonCustomVariant;
use crate::typography::FontWeights;

impl FilePane {
    pub(super) fn tool_button(
        &self,
        id: &'static str,
        icon: IconName,
        label: &'static str,
        active: bool,
    ) -> Button {
        let p = self.palette();
        Button::new(id)
            .toolbar()
            .ghost()
            .size(px(29.))
            .p_0()
            .custom(
                ButtonCustomVariant::for_surface(p)
                    .color(if active { p.control } else { rgba(0) }.into())
                    .foreground(if active { p.accent } else { p.muted }.into())
                    .hover(p.control.into())
                    .highlight(p.accent.alpha(0.10).into())
                    .active(p.accent.alpha(0.16).into()),
            )
            .icon(Icon::new(icon).size(px(15.)))
            .tooltip(label)
    }
    pub(super) fn tool_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let owner = cx.weak_entity();
        let sort_owner = owner.clone();
        let filtering = self.filter_open || !self.filter.read(cx).value().is_empty();
        div()
            .w_full()
            .h(px(40.))
            .flex_shrink_0()
            .pl(px(9.))
            .pr(px(6.))
            .bg(p.surface)
            .flex()
            .items_center()
            .gap(px(1.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .when(filtering, |view| {
                        view.child(
                            Input::new(&self.filter)
                                .palette(p)
                                .small()
                                .appearance(false)
                                .aria_label(crate::localization::text(cx, "Filter files"))
                                .cleanable(true),
                        )
                    })
                    .when(!filtering, |view| {
                        view.flex()
                            .items_center()
                            .gap(px(7.))
                            .child(
                                Icon::new(IconName::Folder)
                                    .size(px(15.))
                                    .text_color(p.accent),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .text_size(px(12.))
                                    .line_height(px(18.))
                                    .font_weight(FontWeights::SEMIBOLD)
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(self.title()),
                            )
                    }),
            )
            .when(!filtering, |view| {
                view.child(
                    self.tool_button("tool-sftp-filter", IconName::Search, "Filter files", false)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.filter_open = true;
                            this.filter
                                .update(cx, |filter, cx| filter.focus(window, cx));
                            cx.notify();
                        })),
                )
            })
            .child(
                self.tool_button("tool-sftp-refresh", IconName::RefreshCw, "Refresh", false)
                    .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx))),
            )
            .child(
                self.tool_button("tool-sftp-sort", IconName::ArrowDownAZ, "Sort files", false)
                    .dropdown_menu(move |menu, _, cx| {
                        let p = sort_owner
                            .upgrade()
                            .map(|pane| pane.read(cx).tool_menu_palette())
                            .unwrap_or(p);
                        let mut menu = menu.palette(p);
                        for (column, name) in [
                            (Sort::Name, "Name"),
                            (Sort::Modified, "Modified"),
                            (Sort::Size, "Size"),
                            (Sort::Kind, "Kind"),
                        ] {
                            for ascending in [true, false] {
                                let owner = sort_owner.clone();
                                let checked = owner.upgrade().is_some_and(|pane| {
                                    let pane = pane.read(cx);
                                    pane.sort == column && pane.ascending == ascending
                                });
                                menu = menu.item(
                                    MenuItem::new(format!(
                                        "{name} {}",
                                        if ascending { "↑" } else { "↓" }
                                    ))
                                    .checked(checked)
                                    .on_click(
                                        move |_, _, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.sort = column;
                                                this.ascending = ascending;
                                                cx.notify();
                                            });
                                        },
                                    ),
                                );
                            }
                        }
                        menu
                    }),
            )
            .child(
                self.tool_button("tool-sftp-tasks", IconName::List, "Tasks", self.tasks_open)
                    .when(self.tasks.read(cx).active() > 0, |button| {
                        button.child({
                            let count = self.tasks.read(cx).active();
                            div()
                                .absolute()
                                .top(px(1.))
                                .right(px(1.))
                                .min_w(px(12.))
                                .h(px(12.))
                                .px(px(2.))
                                .rounded_full()
                                .bg(p.accent)
                                .text_color(p.background)
                                .text_size(px(7.5))
                                .line_height(px(12.))
                                .text_center()
                                .child(if count > 9 {
                                    "9+".into()
                                } else {
                                    count.to_string()
                                })
                        })
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tasks_open = !this.tasks_open;
                        if this.tasks_open {
                            this.tasks.read(cx).reload_history();
                        }
                        this.favorites.open = false;
                        cx.notify();
                    })),
            )
            .child(
                self.tool_button("tool-sftp-actions", IconName::Ellipsis, "Actions", false)
                    .dropdown_menu(move |menu, _, cx| {
                        actions::browser_menu(menu, owner.clone(), cx)
                    }),
            )
            .into_any_element()
    }
    pub(super) fn reconnect_bound(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(remote) = self.remote.as_ref() else {
            return;
        };
        let setup = terminal::RemoteSetup {
            options: remote.options.for_new_connection(),
            title: remote.title.clone(),
            host_id: remote.host_id,
            path: if self.history.path.is_empty() {
                "~".into()
            } else {
                self.history.path.clone()
            },
        };
        self.reset_load();
        self.error = None;
        self.stage = Stage::Connecting;
        match Core::discover() {
            Ok(core) => self.install_remote(core, setup, window, cx),
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    pub(super) fn tool_closed(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.tool_header(cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.))
                    .text_color(p.muted)
                    .text_size(px(11.5))
                    .line_height(px(18.))
                    .child(crate::localization::text(
                        cx,
                        "rs.ui.copy.sftpConnectionClosed",
                    ))
                    .child(
                        Button::new("tool-sftp-reconnect")
                            .small()
                            .ghost()
                            .label("Reconnect")
                            .custom(ButtonCustomVariant::for_surface(p))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.reconnect_bound(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn tool_menu_palette(&self) -> Palette {
        let mut p = self.palette();
        p.hover = p.control;
        p
    }
    pub(super) fn tool_path_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        div()
            .w_full()
            .h(px(36.))
            .flex_shrink_0()
            .px(px(6.))
            .bg(p.surface)
            .border_b_1()
            .border_color(p.border)
            .flex()
            .items_center()
            .child(
                self.tool_button("tool-sftp-home", IconName::House, "Home", false)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate(this.home(), Navigation::Visit, window, cx)
                    })),
            )
            .when_some(self.directory_buttons(cx), |view, buttons| {
                view.child(buttons)
            })
            .child(
                self.tool_button("tool-sftp-back", IconName::ChevronLeft, "Back", false)
                    .disabled(self.history.back.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(path) = this.history.back.last().cloned() {
                            this.navigate(path, Navigation::Back, window, cx);
                        }
                    })),
            )
            .child(
                self.tool_button(
                    "tool-sftp-forward",
                    IconName::ChevronRight,
                    "Forward",
                    false,
                )
                .disabled(self.history.forward.is_empty())
                .on_click(cx.listener(|this, _, window, cx| {
                    if let Some(path) = this.history.forward.last().cloned() {
                        this.navigate(path, Navigation::Forward, window, cx);
                    }
                })),
            )
            .child(
                Button::new("tool-sftp-path")
                    .ghost()
                    .flex_1()
                    .min_w_0()
                    .h(px(29.))
                    .px(px(2.))
                    .custom(ButtonCustomVariant::for_surface(p))
                    .content_start()
                    .child(
                        div()
                            .min_w_0()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .font_family("monospace")
                            .text_size(px(11.5))
                            .line_height(px(18.))
                            .child(self.history.path.clone()),
                    )
                    .raw_tooltip(self.history.path.clone())
                    .on_click(cx.listener(|this, _, window, cx| this.edit_path(window, cx))),
            )
            .children(self.favorite_controls(cx))
            .into_any_element()
    }
    pub(super) fn tool_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .id("tool-sftp-overlay-shield")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.tasks_open = false;
                            this.favorites.open = false;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .id("tool-sftp-panel")
                    .absolute()
                    .left(px(8.))
                    .right(px(8.))
                    .top(px(46.))
                    .occlude()
                    .rounded(px(9.))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border)
                    .shadow(vec![gpui_kit::base::box_shadow(
                        px(0.),
                        px(4.),
                        px(12.),
                        px(-5.),
                        rgba(0x0000001f).into(),
                    )])
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(if self.tasks_open {
                        crate::sftp_tasks::TaskPanel::new(self.tasks.clone(), p).into_any_element()
                    } else {
                        self.favorite_panel(cx)
                    }),
            )
            .into_any_element()
    }
}
