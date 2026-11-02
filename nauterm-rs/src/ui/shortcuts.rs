use super::*;
use crate::shortcuts::{Command, Scope};

impl Nauterm {
    pub(super) fn install_shortcuts(&mut self, window: &Window, cx: &mut Context<Self>) {
        cx.set_global(self.data.shortcuts.clone());
        crate::app_menu::update_shortcuts(&self.data.shortcuts, cx);
        let owner = cx.weak_entity();
        let window_id = window.window_handle().window_id();
        self.subscriptions
            .push(cx.intercept_keystrokes(move |event, window, cx| {
                if window.window_handle().window_id() != window_id
                    || !crate::shortcuts::accepts_event(event)
                {
                    return;
                }
                // Editing commands win in terminal scope if a user assigns a
                // workspace command to the same chord. Never also send it to PTY.
                let config = crate::shortcuts::current(cx);
                if let Some(command) = config.matching(Scope::Collection, &event.keystroke)
                    && owner
                        .update(cx, |this, cx| this.collection_shortcut(command, window, cx))
                        .unwrap_or(false)
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    return;
                }
                let terminal = event
                    .context_stack
                    .iter()
                    .any(|context| context.contains("TerminalView"));
                let input = event
                    .context_stack
                    .iter()
                    .any(|context| context.contains("Input"));
                if terminal
                    && let Some(command) = config.matching(Scope::Terminal, &event.keystroke)
                {
                    if !input {
                        return;
                    }
                    // Keep editing in the focused composer/search Input. A
                    // remapped editing chord must not fall through to a
                    // workspace command using the same chord.
                    use gpui_kit::base::input::{Copy, Paste, SelectAll};
                    let action: Box<dyn Action> = match command {
                        Command::Copy => Box::new(Copy),
                        Command::Paste => Box::new(Paste),
                        Command::SelectAll => Box::new(SelectAll),
                        // Use our terminal Find, not the base Input's Find.
                        Command::Search => Box::new(crate::terminal::Find),
                        _ => unreachable!(),
                    };
                    window.dispatch_action(action, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                    return;
                }
                let Some(command) = config.matching(Scope::Workspace, &event.keystroke) else {
                    return;
                };
                if owner
                    .update(cx, |this, cx| this.workspace_shortcut(command, window, cx))
                    .unwrap_or(false)
                {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }));
    }

    fn collection_shortcut(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if crate::components::overlay::is_modal_open(window, cx) {
            return false;
        }
        match self.page {
            Page::Keychain => self.keychain.clone().is_some_and(|view| {
                view.update(cx, |view, cx| view.collection_shortcut(command, window, cx))
            }),
            Page::Proxies => self.proxies.clone().is_some_and(|view| {
                view.update(cx, |view, cx| view.collection_shortcut(command, window, cx))
            }),
            _ => self.resource_shortcut(command, window, cx),
        }
    }

    pub(super) fn workspace_shortcut(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if crate::components::overlay::is_modal_open(window, cx)
            || self.quick_connect.is_some()
            || self.serial_connect.is_some()
            || self.rename_workspace.is_some()
            || self.close_pending.is_some()
            || self.recovery_pending.is_some()
        {
            return false;
        }
        match command {
            Command::QuickConnect | Command::Palette => self.open_quick_connect(false, window, cx),
            Command::OpenSettings => {
                let page = if self.page == Page::Sessions
                    && !self.overview
                    && self.active_terminal().is_some()
                {
                    crate::settings::Page::Terminal
                } else {
                    crate::settings::Page::General
                };
                self.open_settings_at(Some(page), window, cx);
            }
            Command::NewLocalTerminal => self.open_local(window, cx),
            Command::SplitRight => {
                self.duplicate_selected(Some(crate::pane_tree::SplitAxis::Horizontal), window, cx)
            }
            Command::SplitDown => {
                self.duplicate_selected(Some(crate::pane_tree::SplitAxis::Vertical), window, cx)
            }
            Command::CloseTab => self.close_selected_session(window, cx),
            Command::Tab(index) => self.select_shortcut_tab(index as usize, window, cx),
            Command::PreviousTab | Command::NextTab => {
                let tabs = crate::workspace::TopTabs {
                    sftp: self.data.sftp.show_tab,
                    workspace: self.data.general.workspace_enabled,
                };
                let fixed = tabs.fixed_count();
                let ids: Vec<_> = self
                    .terminals
                    .in_workspace(self.data.selected_workspace)
                    .map(|tab| tab.id)
                    .collect();
                let current = match self.page {
                    Page::Sftp => tabs.index(crate::workspace::TopTab::Sftp).unwrap_or(0),
                    Page::Sessions if self.overview => {
                        tabs.index(crate::workspace::TopTab::Workspace).unwrap_or(0)
                    }
                    Page::Sessions => ids
                        .iter()
                        .position(|id| Some(*id) == self.active_terminal())
                        .map_or(fixed - 1, |index| fixed + index),
                    _ => 0,
                };
                let length = fixed + ids.len();
                let next = if command == Command::NextTab {
                    (current + 1) % length
                } else {
                    (current + length - 1) % length
                };
                self.select_shortcut_tab(next, window, cx);
            }
            Command::Ssh | Command::Sftp => {
                let Some(id) = self.active_terminal() else {
                    return true;
                };
                self.show_session(id, false, window, cx);
                let sftp = self
                    .terminals
                    .iter()
                    .find(|tab| tab.id == id)
                    .is_some_and(|tab| tab.show_sftp);
                if sftp != (command == Command::Sftp) {
                    self.toggle_terminal_sftp(window, cx);
                }
            }
            _ => return false,
        }
        true
    }

    fn select_shortcut_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let tabs = crate::workspace::TopTabs {
            sftp: self.data.sftp.show_tab,
            workspace: self.data.general.workspace_enabled,
        };
        match tabs.target(index) {
            crate::workspace::TopTab::Vaults => self.navigate(Page::Hosts, window, cx),
            crate::workspace::TopTab::Sftp => self.navigate(Page::Sftp, window, cx),
            crate::workspace::TopTab::Workspace => {
                self.overview = true;
                self.navigate(Page::Sessions, window, cx);
            }
            crate::workspace::TopTab::Session(index) => {
                let id = self
                    .terminals
                    .in_workspace(self.data.selected_workspace)
                    .nth(index)
                    .map(|tab| tab.id);
                if let Some(id) = id {
                    self.show_session(id, false, window, cx);
                }
            }
        }
    }
}
