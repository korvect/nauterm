use super::*;
use crate::components::menu::{ContextMenuExt, MenuItem, MenuState};

/// Parent-owned actions are scoped to the terminal that opened the menu.
#[derive(Clone, Copy)]
pub enum Command {
    SplitRight,
    SplitDown,
    NewTab,
    Settings,
    Close,
}
impl EventEmitter<Command> for Terminal {}

impl Terminal {
    pub(super) fn context_menu(
        &self,
        surface: impl InteractiveElement + ParentElement + Styled + IntoElement + 'static,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let owner = cx.weak_entity();
        let enabled = owner.clone();
        surface
            .context_menu(move |menu, _, cx: &mut Context<MenuState>| {
                let Some(owner) = owner.upgrade() else {
                    return menu;
                };
                let this = owner.read(cx);
                let palette = crate::palette::Palette::terminal(this.appearance.palette());
                let selected = this
                    .effective_selection()
                    .is_some_and(|selection| selection.start < selection.end);
                let shortcuts = crate::shortcuts::current(cx);
                use crate::shortcuts::Command as Shortcut;
                let command = |label: &'static str, action: Command| {
                    let owner = owner.downgrade();
                    MenuItem::localized(label).on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |_, cx| cx.emit(action));
                    })
                };
                menu.width(214.)
                    .palette(palette)
                    .item(
                        shortcuts
                            .menu(
                                MenuItem::localized("Copy")
                                    .icon(IconName::Copy)
                                    .disabled(!selected),
                                Shortcut::Copy,
                            )
                            .on_click({
                                let owner = owner.downgrade();
                                move |_, _, cx| {
                                    let _ = owner.update(cx, |this, cx| this.copy_selection(cx));
                                }
                            }),
                    )
                    .item(
                        shortcuts
                            .menu(
                                MenuItem::localized("Paste")
                                    .icon(IconName::ClipboardPaste)
                                    .disabled(this.session.is_replay()),
                                Shortcut::Paste,
                            )
                            .on_click({
                                let owner = owner.downgrade();
                                move |_, _, cx| {
                                    if let Some(text) =
                                        cx.read_from_clipboard().and_then(|item| item.text())
                                    {
                                        let _ =
                                            owner.update(cx, |this, cx| this.paste_text(&text, cx));
                                    }
                                }
                            }),
                    )
                    .separator()
                    .item(
                        shortcuts.menu(
                            command("Split Right", Command::SplitRight)
                                .icon(IconName::Columns2)
                                .disabled(this.session.is_replay()),
                            Shortcut::SplitRight,
                        ),
                    )
                    .item(
                        shortcuts.menu(
                            command("Split Down", Command::SplitDown)
                                .icon(IconName::Rows2)
                                .disabled(this.session.is_replay()),
                            Shortcut::SplitDown,
                        ),
                    )
                    .separator()
                    .when(this.appearance.multiple_tabs, |menu| {
                        menu.item(
                            command("New Tab", Command::NewTab)
                                .icon(IconName::Plus)
                                .disabled(this.session.is_replay()),
                        )
                    })
                    .item(
                        shortcuts
                            .menu(
                                MenuItem::localized("Search").icon(IconName::Search),
                                Shortcut::Search,
                            )
                            .on_click({
                                let owner = owner.downgrade();
                                move |_, window, cx| {
                                    let _ =
                                        owner.update(cx, |this, cx| this.show_search(window, cx));
                                }
                            }),
                    )
                    .separator()
                    .item(
                        MenuItem::localized("Clear")
                            .icon(IconName::Eraser)
                            .disabled(this.session.is_replay())
                            .on_click({
                                let owner = owner.downgrade();
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |this, cx| {
                                        this.send(b"\x0c", cx);
                                        this.focus.focus(window, cx);
                                    });
                                }
                            }),
                    )
                    .item(
                        shortcuts
                            .menu(
                                MenuItem::localized("Select All").icon(IconName::ListChecks),
                                Shortcut::SelectAll,
                            )
                            .on_click({
                                let owner = owner.downgrade();
                                move |_, _, cx| {
                                    let _ = owner.update(cx, |this, cx| this.select_all(cx));
                                }
                            }),
                    )
                    .separator()
                    .item(shortcuts.menu(
                        command("Settings", Command::Settings).icon(IconName::Settings),
                        Shortcut::OpenSettings,
                    ))
                    .item(
                        shortcuts.menu(
                            command("Close", Command::Close)
                                .icon(IconName::X)
                                .destructive(true),
                            Shortcut::CloseTab,
                        ),
                    )
            })
            .enabled_when(move |_, cx| {
                enabled
                    .upgrade()
                    .is_some_and(|owner| !owner.read(cx).reports_mouse())
            })
    }
    pub(super) fn select_all(&mut self, cx: &mut Context<Self>) {
        self.clear_selection();
        self.selection = Some(Selection {
            start: 0,
            end: (i64::from(self.frame.history_lines) + i64::from(self.frame.rows))
                * i64::from(self.frame.columns),
        });
        cx.notify();
    }
}
