//! The library's Shell History is a list drawer, not a resource editor or modal.
use super::*;
use crate::{
    components::{DialogColors, DialogFrame, Modal},
    shell_history::store::{Entry, Service},
};

pub(super) enum Action {
    Close,
    Save(String),
}
pub(super) struct History {
    service: Service,
    entries: std::rc::Rc<Vec<Entry>>,
    scroll: UniformListScrollHandle,
    dark: bool,
    can_save: bool,
    loading: bool,
    refresh_pending: bool,
    busy: bool,
    clear_pending: bool,
    hovered: Option<usize>,
    error: Option<String>,
    focus: FocusHandle,
    _updates: Task<()>,
}
impl EventEmitter<Action> for History {}
impl Nauterm {
    pub(super) fn open_shell_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_snippets(cx);
        self.retire_editor();
        self.selection.clear();
        self.selection_kind = None;
        if let Some(view) = &self.history {
            view.update(cx, |view, cx| {
                view.refresh(cx);
                view.focus.focus(window, cx);
            });
            return;
        }
        let view =
            cx.new(|cx| History::new(self.shell_history.clone(), self.data.dark, window, cx));
        self.history_events =
            Some(
                cx.subscribe_in(&view, window, |this, _, action, window, cx| {
                    match action {
                        Action::Close => {
                            this.retire_history();
                            this.focus.focus(window, cx);
                        }
                        Action::Save(command) => this.snippet_from_history(command, window, cx),
                    }
                    cx.notify();
                }),
            );
        self.history = Some(view);
        cx.notify();
    }
}
impl History {
    fn new(service: Service, dark: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        let notifications = service.subscribe();
        let updates = cx.spawn(async move |view, cx| {
            while notifications.recv().await.is_ok() {
                if view.update(cx, |view, cx| view.refresh(cx)).is_err() {
                    break;
                }
            }
        });
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut view = Self {
            service,
            dark,
            focus,
            _updates: updates,
            entries: std::rc::Rc::default(),
            scroll: UniformListScrollHandle::new(),
            can_save: false,
            loading: false,
            refresh_pending: false,
            busy: false,
            clear_pending: false,
            hovered: None,
            error: None,
        };
        view.refresh(cx);
        view
    }
    pub(super) fn configure(&mut self, dark: bool, can_save: bool, cx: &mut Context<Self>) {
        if self.dark != dark || self.can_save != can_save {
            self.dark = dark;
            self.can_save = can_save;
            cx.notify();
        }
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.busy {
            self.refresh_pending = true;
            return;
        }
        self.loading = true;
        self.refresh_pending = false;
        let service = self.service.clone();
        cx.spawn(async move |view, cx| {
            let result = service.entries().await;
            let _ = view.update(cx, |view, cx| {
                view.loading = false;
                match result {
                    Ok(entries) => {
                        view.entries = std::rc::Rc::new(entries);
                        if view
                            .hovered
                            .is_some_and(|index| index >= view.entries.len())
                        {
                            view.hovered = None;
                        }
                        view.error = None;
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                if view.refresh_pending {
                    view.refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn clear(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        let service = self.service.clone();
        cx.spawn(async move |view, cx| {
            let result = service.clear().await;
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(()) => {
                        view.clear_pending = false;
                        view.refresh(cx);
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
impl Render for History {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.dark);
        let owner = cx.weak_entity();
        let entries = self.entries.clone();
        let save_owner = owner.clone();
        let can_save = self.can_save && !self.busy;
        let hovered = self.hovered;
        let hover_color = rgb(if self.dark { 0x27313c } else { 0xedf4f5 });
        div()
            .w(px(layout::DRAWER_WIDTH))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(p.surface)
            .border_l_1()
            .border_color(p.border)
            .track_focus(&self.focus)
            .child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .pl(px(20.))
                    .pr(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(14.))
                            .font_weight(FontWeights::MEDIUM)
                            .child(crate::localization::text(
                                cx,
                                "workspace.label.shellHistory",
                            )),
                    )
                    .when(self.error.is_some(), |view| {
                        view.child(
                            Button::new("reload-shell-history")
                                .toolbar()
                                .ghost()
                                .icon(IconName::RefreshCw)
                                .tooltip("Reload shell history")
                                .disabled(self.loading || self.busy)
                                .on_click(cx.listener(|view, _, _, cx| view.refresh(cx))),
                        )
                    })
                    .child(
                        Button::new("shell-history-more")
                            .toolbar()
                            .ghost()
                            .icon(IconName::Ellipsis)
                            .tooltip("More")
                            .disabled(self.loading || self.busy)
                            .dropdown_menu(move |menu, _, _| {
                                let owner = owner.clone();
                                menu.width(170.).item(
                                    MenuItem::localized("Clear all")
                                        .icon(IconName::Trash)
                                        .destructive(true)
                                        .on_click(move |_, _, cx| {
                                            let _ = owner.update(cx, |view, cx| {
                                                view.clear_pending = true;
                                                view.error = None;
                                                cx.notify();
                                            });
                                        }),
                                )
                            }),
                    )
                    .child(
                        Button::new("close-shell-history")
                            .toolbar()
                            .ghost()
                            .icon(IconName::ArrowRightToLine)
                            .tooltip("Close")
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(Action::Close))),
                    ),
            )
            .when(self.entries.is_empty(), |view| {
                view.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p(px(20.))
                        .text_color(p.muted)
                        .child(crate::localization::view(if self.loading {
                            "Loading shell history…"
                        } else {
                            "No shell history recorded yet."
                        })),
                )
            })
            .when(!self.entries.is_empty(), |view| {
                view.child(
                    div().flex_1().min_h_0().py(px(10.)).child(
                        crate::components::uniform_scrollbar(
                            "shell-history-scrollbar",
                            uniform_list(
                                "shell-history-list",
                                entries.len(),
                                move |range, _, _| {
                                    range
                                        .map(|index| {
                                            let command = entries[index].command.clone();
                                            let owner = save_owner.clone();
                                            let hover_owner = save_owner.clone();
                                            div()
                                                .id(("shell-history-row", index))
                                                .relative()
                                                .w_full()
                                                .min_w_0()
                                                .h(px(48.))
                                                .overflow_hidden()
                                                .px(px(20.))
                                                .py(px(6.))
                                                .flex()
                                                .items_center()
                                                .bg(if hovered == Some(index) {
                                                    hover_color
                                                } else {
                                                    p.surface
                                                })
                                                .on_hover(move |over, _, cx| {
                                                    let _ = hover_owner.update(cx, |view, cx| {
                                                        let next = if *over {
                                                            Some(index)
                                                        } else if view.hovered == Some(index) {
                                                            None
                                                        } else {
                                                            view.hovered
                                                        };
                                                        if view.hovered != next {
                                                            view.hovered = next;
                                                            cx.notify();
                                                        }
                                                    });
                                                })
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .whitespace_nowrap()
                                                        .text_ellipsis()
                                                        .text_size(px(13.))
                                                        .line_height(px(19.))
                                                        .font_weight(FontWeights::MEDIUM)
                                                        .child(command.clone()),
                                                )
                                                .when(hovered == Some(index), |row| {
                                                    row.child(
                                                        div()
                                                            .absolute()
                                                            .right(px(20.))
                                                            .top(px(11.))
                                                            .pl(px(8.))
                                                            .bg(hover_color)
                                                            .child(
                                                                Button::new((
                                                                    "save-history-snippet",
                                                                    index,
                                                                ))
                                                                .h(px(26.))
                                                                .min_w(px(58.))
                                                                .xsmall()
                                                                .primary()
                                                                .label("SAVE")
                                                                .disabled(!can_save)
                                                                .on_click(move |_, _, cx| {
                                                                    let _ = owner.update(
                                                                        cx,
                                                                        |view, cx| {
                                                                            if view.can_save
                                                                                && !view.busy
                                                                            {
                                                                                cx.emit(
                                                                                    Action::Save(
                                                                                        command
                                                                                            .clone(
                                                                                            ),
                                                                                    ),
                                                                                );
                                                                            }
                                                                        },
                                                                    );
                                                                }),
                                                            ),
                                                    )
                                                })
                                        })
                                        .collect()
                                },
                            )
                            .track_scroll(&self.scroll)
                            .size_full(),
                            &self.scroll,
                        ),
                    ),
                )
            })
            .when(self.clear_pending, |view| {
                view.child(
                    crate::components::Dialog::new("clear-shell-history", "Clear shell history?")
                        .return_focus(&self.focus)
                        .dismissible(!self.busy)
                        .on_dismiss(cx.listener(|view, _, _, cx| {
                            view.clear_pending = false;
                            view.error = None;
                            cx.notify();
                        }))
                        .content(
                            DialogFrame::new("clear-shell-history-frame", "Clear shell history?")
                                .width(400.)
                                .colors(DialogColors::workspace(self.dark))
                                .content(div().flex().flex_col().gap(px(10.)).child(
                                    crate::localization::text(
                                        cx,
                                        "rs.ui.copy.clearShellHistoryDescription",
                                    ),
                                ))
                                .action(
                                    Button::new("cancel-clear-history")
                                        .ghost()
                                        .label("Cancel")
                                        .disabled(self.busy)
                                        .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                                )
                                .action(
                                    Button::new("confirm-clear-history")
                                        .danger()
                                        .label("Clear all")
                                        .disabled(self.busy)
                                        .on_click(cx.listener(|view, _, _, cx| view.clear(cx))),
                                ),
                        ),
                )
            })
    }
}
