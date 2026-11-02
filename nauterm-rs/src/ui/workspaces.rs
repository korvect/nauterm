use super::*;
use crate::components::menu::MenuItem;
use crate::{
    components::Field,
    quick_connect::{QuickConnect, QuickConnectEvent, Target},
    workspace::monitor_columns,
};

pub(super) struct QuickDialog {
    view: Entity<QuickConnect>,
    pending: Option<QuickRequest>,
    _events: Subscription,
}
impl QuickDialog {
    pub(super) fn cancel_pending(&mut self) {
        self.pending = None;
    }
}
struct QuickRequest {
    target: Box<Target>,
    destination: Option<Id>,
    overview: bool,
}
pub(super) struct RenameWorkspace {
    id: Id,
    input: Entity<InputState>,
    pub(super) error: Option<SharedString>,
    validation_error: Option<SharedString>,
    _input: Subscription,
}

impl Nauterm {
    pub(super) fn refresh_workspace_views(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.data.general.workspace_enabled && self.page == Page::Sessions {
            self.overview = false;
            if self.active_terminal().is_none() {
                self.switch_drawer_page(Page::Hosts);
            }
        }
        let terminals = &self.terminals;
        self.bell_tabs
            .retain(|id| terminals.iter().any(|tab| tab.id == *id));
        self.split_bounds.retain(|id, _| {
            terminals.iter().any(|tab| {
                tab.tree
                    .root()
                    .is_some_and(|root| root.split(*id).is_some())
            })
        });
        self.pane_bounds.retain(|id, _| {
            terminals
                .iter()
                .any(|tab| tab.tree.panes().iter().any(|pane| pane.id == *id))
        });
        if let Some(dialog) = &self.quick_connect {
            dialog
                .view
                .update(cx, |view, cx| view.set_data(self.data.clone(), cx));
        }
        let active = self.active_terminal();
        let session_page = self.page == Page::Sessions && !self.overview;
        if session_page && let Some(id) = active {
            self.bell_tabs.remove(&id);
        }
        for tab in self.terminals.iter() {
            for pane in tab.tree.panes() {
                for session in &pane.sessions {
                    match &session.content {
                        TabContent::Connection(view) => {
                            let allow = self.session_auto_focus(session.id);
                            view.update(cx, |view, cx| view.allow_auto_focus(allow, cx));
                        }
                        TabContent::Terminal(view) => {
                            let allow = self.session_auto_focus(session.id);
                            let visible = session_page
                                && Some(tab.id) == active
                                && !tab.files_visible()
                                && session.id == pane.active().id;
                            let history_visible = session_page
                                && Some(tab.id) == active
                                && !tab.show_sftp
                                && tab.tools.history_visible()
                                && session.id == tab.active().id;
                            view.update(cx, |view, cx| {
                                view.set_focus_scope(visible, window, cx);
                                view.set_reconnect_focus(allow);
                                view.set_history_tool_visible(history_visible, cx);
                                view.set_composer_visible(pane.composer_visible, cx);
                            });
                        }
                        TabContent::Preparing(_) => {}
                    }
                }
            }
        }
    }
    pub(super) fn commit_workspace_metadata(
        &mut self,
        mut candidate: AppState,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.resources_ready(),
            "Resolve the workspace metadata error before changing workspaces."
        );
        self.store.save(&mut candidate)?;
        self.data = candidate;
        Ok(())
    }
    pub(super) fn session_auto_focus(&self, id: u64) -> bool {
        self.page == Page::Sessions
            && !self.overview
            && self.quick_connect.is_none()
            && self.serial_connect.is_none()
            && self.rename_workspace.is_none()
            && self.close_pending.is_none()
            && self.recovery_pending.is_none()
            && self.terminals.iter().any(|tab| {
                Some(tab.id) == self.active_terminal()
                    && !tab.files_visible()
                    && tab.active().id == id
                    && tab.workspace == self.data.selected_workspace
            })
    }
    pub(super) fn show_session(
        &mut self,
        id: u64,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(owner) = self
            .terminals
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.workspace)
        else {
            return;
        };
        if !self
            .data
            .workspaces
            .iter()
            .any(|workspace| workspace.id == owner)
        {
            return;
        }
        if self.data.selected_workspace != owner {
            let mut candidate = self.data.clone();
            candidate.selected_workspace = owner;
            if let Err(error) = self.commit_workspace_metadata(candidate) {
                self.notice = Some(error.to_string());
                cx.notify();
                return;
            }
        }
        self.terminals.select(id);
        self.bell_tabs.remove(&id);
        self.overview = overview && self.data.general.workspace_enabled;
        self.navigate(Page::Sessions, window, cx);
    }
    fn select_workspace(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        if !self
            .data
            .workspaces
            .iter()
            .any(|workspace| workspace.id == id)
        {
            return;
        }
        if self.data.selected_workspace != id {
            let mut candidate = self.data.clone();
            candidate.selected_workspace = id;
            if let Err(error) = self.commit_workspace_metadata(candidate) {
                self.notice = Some(error.to_string());
                cx.notify();
                return;
            }
        }
        self.overview = self.data.general.workspace_enabled;
        self.navigate(Page::Sessions, window, cx);
    }
    pub(crate) fn open_quick_connect(
        &mut self,
        new_workspace: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let destination = (!new_workspace).then_some(self.data.selected_workspace);
        let overview = self.workspace_overview_visible();
        let view = cx.new(|cx| QuickConnect::new(self.data.clone(), window, cx));
        let events = cx.subscribe_in(
            &view,
            window,
            move |this, source, event: &QuickConnectEvent, window, cx| {
                if this.close_pending.is_some()
                    || !this
                        .quick_connect
                        .as_ref()
                        .is_some_and(|dialog| dialog.view == *source && dialog.pending.is_none())
                {
                    return;
                }
                match event {
                    QuickConnectEvent::Dismiss => {
                        crate::components::Modal::dismiss(window, cx);
                    }
                    QuickConnectEvent::Connect(target) => {
                        this.quick_connect.as_mut().unwrap().pending = Some(QuickRequest {
                            target: target.clone(),
                            destination,
                            overview,
                        });
                        crate::components::Modal::dismiss(window, cx);
                    }
                }
                cx.notify();
            },
        );
        self.rename_workspace = None;
        self.serial_connect = None;
        self.quick_connect = Some(QuickDialog {
            view,
            pending: None,
            _events: events,
        });
        cx.notify();
    }
    fn connect_quick_target(
        &mut self,
        request: QuickRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exiting.is_some() || self.close_pending.is_some() {
            return;
        }
        let QuickRequest {
            target,
            destination,
            overview,
        } = request;
        let workspace = if let Some(id) = destination {
            if !self
                .data
                .workspaces
                .iter()
                .any(|workspace| workspace.id == id)
            {
                self.notice = Some("The selected workspace is no longer available.".into());
                cx.notify();
                return;
            }
            id
        } else {
            let mut candidate = self.data.clone();
            let result = candidate
                .add_workspace(target.label())
                .and_then(|id| self.commit_workspace_metadata(candidate).map(|_| id));
            match result {
                Ok(id) => id,
                Err(error) => {
                    self.notice = Some(error.to_string());
                    self.dismiss_workspace_dialog(window, cx);
                    return;
                }
            }
        };
        self.overview = overview;
        if self.data.selected_workspace != workspace {
            self.select_workspace(workspace, window, cx);
        } else {
            self.navigate(Page::Sessions, window, cx);
        }
        match target.as_ref().clone() {
            Target::Shell(shell) => {
                self.open_local_in(Some(shell), workspace, overview, window, cx)
            }
            Target::Host(host) => self.open_ssh_in(host, workspace, overview, window, cx),
            Target::SavedSsh(host) => {
                self.open_remote_in(host, workspace, overview, RemoteMode::Ssh, window, cx)
            }
            Target::SavedMosh(host) => {
                self.open_remote_in(host, workspace, overview, RemoteMode::Mosh, window, cx)
            }
            Target::SavedTelnet(host) => {
                self.open_saved_telnet_in(host, workspace, overview, window, cx)
            }
            Target::Sftp(host) => {
                self.open_remote_in(host, workspace, overview, RemoteMode::Sftp, window, cx)
            }
            Target::Telnet(options) => {
                self.open_telnet_in(options, workspace, overview, window, cx)
            }
        }
    }
    fn finish_workspace_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pending = self
            .quick_connect
            .as_mut()
            .and_then(|dialog| dialog.pending.take());
        self.dismiss_workspace_dialog(window, cx);
        if let Some(request) = pending {
            // Let the old modal release its focus before focusing the new tab.
            let owner = cx.weak_entity();
            window.on_next_frame(move |window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.connect_quick_target(request, window, cx)
                });
            });
        }
    }
    pub(super) fn dismiss_workspace_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let open = self.quick_connect.is_some()
            || self.rename_workspace.is_some()
            || self.serial_connect.is_some();
        if !open {
            return false;
        }
        self.quick_connect = None;
        self.rename_workspace = None;
        self.serial_connect = None;
        self.focus.focus(window, cx);
        if self.page == Page::Sessions
            && let Some(tab) = self
                .terminals
                .iter()
                .find(|tab| Some(tab.id) == self.active_terminal())
        {
            tab.focus(window, cx);
        }
        cx.notify();
        true
    }
    fn rename_workspace(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self
            .data
            .workspaces
            .iter()
            .find(|workspace| workspace.id == id)
            .map(|workspace| workspace.name.clone())
        else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let events = cx.subscribe_in(
            &input,
            window,
            |this, source, event: &InputEvent, window, cx| {
                if this.close_pending.is_none()
                    && matches!(event, InputEvent::PressEnter { .. })
                    && this
                        .rename_workspace
                        .as_ref()
                        .is_some_and(|dialog| dialog.input == *source)
                {
                    this.save_workspace_name(window, cx);
                }
            },
        );
        self.quick_connect = None;
        self.serial_connect = None;
        self.rename_workspace = Some(RenameWorkspace {
            id,
            input: input.clone(),
            error: None,
            validation_error: None,
            _input: events,
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }
    fn save_workspace_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.rename_workspace else {
            return;
        };
        let name = dialog.input.read(cx).value();
        let mut candidate = self.data.clone();
        if let Err(error) = candidate.rename_workspace(dialog.id, &name) {
            self.rename_workspace.as_mut().unwrap().validation_error =
                Some(error.to_string().into());
            cx.notify();
            return;
        }
        let result = self.commit_workspace_metadata(candidate);
        match result {
            Ok(()) => {
                crate::components::Modal::dismiss(window, cx);
            }
            Err(error) => {
                if let Some(dialog) = &mut self.rename_workspace {
                    dialog.error = Some(error.to_string().into());
                }
            }
        }
        cx.notify();
    }
    pub(super) fn confirm_workspace_close(
        &mut self,
        id: Id,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active = self.data.selected_workspace == id;
        let mut candidate = self.data.clone();
        if let Err(error) = candidate
            .close_workspace(id)
            .and_then(|_| self.commit_workspace_metadata(candidate))
        {
            self.notice = Some(error.to_string());
            return;
        }
        // Only after durable metadata succeeds, drop this workspace's live
        // native sessions/requests. Other workspace/session entities stay alive.
        self.terminals.close_workspace(id);
        if active {
            self.overview = true;
            self.navigate(Page::Sessions, window, cx);
        }
    }
    fn workspace_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.data.dark);
        div()
            .w(px(crate::workspace::SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .bg(p.sidebar)
            .border_r_1()
            .border_color(p.border)
            .pt(px(14.))
            .pb(px(12.))
            .flex()
            .flex_col()
            .child(
                Button::new("new-workspace")
                    .xsmall()
                    .secondary()
                    .mx(px(12.))
                    .h(px(32.))
                    .label("New workspace")
                    .icon(crate::assets::icon(IconName::Plus))
                    .on_click(
                        cx.listener(|this, _, window, cx| {
                            this.open_quick_connect(true, window, cx)
                        }),
                    ),
            )
            .child(
                div()
                    .id("workspace-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .scrollbars()
                    .px(px(12.))
                    .mt(px(14.))
                    .children(self.data.workspaces.iter().map(|workspace| {
                        let id = workspace.id;
                        let selected = self.data.selected_workspace == id;
                        let owner = cx.entity().downgrade();
                        let rename = owner.clone();
                        let count = self
                            .terminals
                            .in_workspace(id)
                            .map(|tab| tab.tree.sessions().count())
                            .sum::<usize>();
                        let group: SharedString = format!("workspace-card-{id}").into();
                        Button::new(("workspace", id))
                            .large()
                            .h(px(56.))
                            .w_full()
                            .px(px(10.))
                            .mb(px(6.))
                            .rounded(px(8.))
                            .content_start()
                            .ghost()
                            .group(group.clone())
                            .border_1()
                            .border_color(if selected { rgb(0x3d7eff) } else { p.sidebar })
                            .bg(if selected {
                                if self.data.dark {
                                    rgb(0x163044)
                                } else {
                                    rgb(0xedf7ff)
                                }
                            } else {
                                p.sidebar
                            })
                            .localized_accessibility_label(format!(
                                "Workspace {} — {count} {}",
                                workspace.name,
                                if count == 1 { "session" } else { "sessions" }
                            ))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_workspace(id, window, cx)
                            }))
                            .child(
                                div()
                                    .size(px(36.))
                                    .flex_shrink_0()
                                    .rounded(px(7.))
                                    .bg(if selected {
                                        rgb(workspace.color)
                                    } else {
                                        p.hover
                                    })
                                    .text_color(if selected { rgb(0xffffff) } else { p.text })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Icon::new(IconName::LayoutGrid).with_size(px(21.))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(13.))
                                            .font_weight(FontWeights::MEDIUM)
                                            .child(workspace.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeights::MEDIUM)
                                            .text_color(p.muted)
                                            .child(format!(
                                                "{count} {}",
                                                if count == 1 { "session" } else { "sessions" }
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .invisible()
                                    .group_hover(group, |view| view.visible())
                                    .child(
                                        Button::new(("rename-workspace", id))
                                            .xsmall()
                                            .ghost()
                                            .icon(IconName::Pencil)
                                            .icon_size(px(15.))
                                            .tooltip("Rename workspace")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                cx.stop_propagation();
                                                this.rename_workspace(id, window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new(("close-workspace", id))
                                            .xsmall()
                                            .ghost()
                                            .icon(crate::assets::icon(IconName::X))
                                            .icon_size(px(15.))
                                            .tooltip("Close workspace")
                                            .text_color(rgb(0xef4444))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                cx.stop_propagation();
                                                this.close_pending =
                                                    Some(CloseTarget::Workspace(id));
                                                this.focus.focus(window, cx);
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .context_menu(move |menu, _, _| {
                                let close = owner.clone();
                                let rename = rename.clone();
                                menu.item(MenuItem::localized("Rename workspace").on_click(
                                    move |_, window, cx| {
                                        let _ = rename.update(cx, |this, cx| {
                                            this.rename_workspace(id, window, cx)
                                        });
                                    },
                                ))
                                .item(
                                    MenuItem::localized("Close workspace").on_click(
                                        move |_, window, cx| {
                                            let _ = close.update(cx, |this, cx| {
                                                this.close_pending =
                                                    Some(CloseTarget::Workspace(id));
                                                this.focus.focus(window, cx);
                                                cx.notify();
                                            });
                                        },
                                    ),
                                )
                            })
                    })),
            )
            .into_any_element()
    }
    pub(super) fn workspace_page(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.data.dark);
        let workspace = self.data.selected_workspace;
        let tabs: Vec<_> = self.terminals.in_workspace(workspace).collect();
        let width = (f32::from(window.viewport_size().width)
            - crate::workspace::SIDEBAR_WIDTH
            - 24.
            - self.workspace_assistant_inset(window))
        .max(1.);
        let height = (f32::from(window.viewport_size().height) - 44. - 24.).max(1.);
        let count = tabs.len() + 1;
        let columns = monitor_columns(count, width, height);
        let rows = count.div_ceil(columns);
        let tile_width = ((width - 10. * (columns - 1) as f32) / columns as f32).max(1.);
        let minimum_height = tabs
            .iter()
            .filter_map(|tab| tab.tree.root())
            .map(|root| root.min_height(280.) + 34.)
            .fold(314., f32::max);
        let tile_height = ((height - 10. * (rows - 1) as f32) / rows as f32).max(minimum_height);
        let mut tiles = vec![];
        for tab in &tabs {
            let id = tab.id;
            let selected = self.active_terminal() == Some(id);
            let phase = tab.terminal_phase(cx);
            let status = if tab.active().replay.is_some() {
                "Replay"
            } else {
                phase.monitor_label()
            };
            let status_color = match phase {
                crate::connection_state::Phase::Failed => rgb(0xca3f37),
                crate::connection_state::Phase::Exited => p.muted,
                crate::connection_state::Phase::Completing => {
                    rgb(if self.data.dark { 0x30d158 } else { 0x35d394 })
                }
                _ => p.accent,
            };
            let status_background = if phase == crate::connection_state::Phase::Exited {
                p.hover
            } else {
                status_color.opacity(if self.data.dark { 0.18 } else { 0.10 })
            };
            let title = tab.title(cx);
            tiles.push(
                div()
                    .id(("workspace-session", id))
                    .w(px(tile_width))
                    .h(px(tile_height))
                    .flex_shrink_0()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border)
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.terminals.select(id);
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .h(px(34.))
                            .flex_shrink_0()
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .bg(p.surface)
                            .border_b_1()
                            .border_color(p.border)
                            .child(
                                Button::new(("workspace-session-title", id))
                                    .xsmall()
                                    .ghost()
                                    .text_color(if selected { p.text } else { p.muted })
                                    .content_start()
                                    .flex_1()
                                    .min_w_0()
                                    .icon(IconName::Terminal)
                                    .icon_size(px(16.))
                                    .raw_label(title)
                                    .on_click(cx.listener(
                                        move |this, event: &ClickEvent, window, cx| {
                                            if event.click_count() >= 2
                                                && let Some(tab) = this
                                                    .terminals
                                                    .iter_mut()
                                                    .find(|tab| tab.id == id)
                                            {
                                                tab.show_sftp = false;
                                            }
                                            this.show_session(
                                                id,
                                                event.click_count() < 2,
                                                window,
                                                cx,
                                            )
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .id(("workspace-session-status", id))
                                    .role(Role::Label)
                                    .aria_label(format!("Session status: {status}"))
                                    .h(px(20.))
                                    .px(px(7.))
                                    .rounded(px(999.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .bg(status_background)
                                    .text_color(status_color)
                                    .text_size(px(12.))
                                    .font_weight(FontWeights::MEDIUM)
                                    .child(crate::localization::message(cx, status)),
                            )
                            .child(
                                Button::new(("workspace-session-open", id))
                                    .xsmall()
                                    .ghost()
                                    .text_color(if selected { p.text } else { p.muted })
                                    .icon(IconName::Maximize2)
                                    .tooltip("Open terminal")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Some(tab) =
                                            this.terminals.iter_mut().find(|tab| tab.id == id)
                                        {
                                            tab.show_sftp = false;
                                        }
                                        this.show_session(id, false, window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .child(self.terminal_tab_element(tab, cx)),
                    )
                    .into_any_element(),
            );
        }
        tiles.push(
            Button::new("workspace-new-connect")
                .large()
                .ghost()
                .w(px(tile_width))
                .h(px(tile_height))
                .rounded(px(8.))
                .border_1()
                .border_color(p.border)
                .child(
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(12.))
                        .child(
                            div()
                                .size(px(42.))
                                .rounded(px(10.))
                                .bg(p.hover)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    crate::assets::icon(IconName::Plus)
                                        .with_size(px(28.))
                                        .text_color(p.accent),
                                ),
                        )
                        .child(
                            div()
                                .font_weight(FontWeights::MEDIUM)
                                .child(crate::localization::text(cx, "workspace.label.newConnect")),
                        ),
                )
                .on_click(
                    cx.listener(|this, _, window, cx| this.open_quick_connect(false, window, cx)),
                )
                .into_any_element(),
        );
        let content = if tabs.is_empty() {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .bg(p.surface)
                .child(
                    div()
                        .size(px(56.))
                        .rounded(px(12.))
                        .bg(p.hover)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(IconName::Terminal).with_size(px(28.))),
                )
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeights::MEDIUM)
                        .child(crate::localization::text(
                            cx,
                            "workspace.label.noTerminalSessions",
                        )),
                )
                .child(
                    div()
                        .text_color(p.muted)
                        .font_weight(FontWeights::MEDIUM)
                        .child(crate::localization::text(
                            cx,
                            "workspace.description.createAConnectFromAHost",
                        )),
                )
                .child(
                    Button::new("workspace-empty-connect")
                        .primary()
                        .label("New Connect")
                        .icon(crate::assets::icon(IconName::Plus))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_quick_connect(false, window, cx)
                        })),
                )
                .into_any_element()
        } else {
            div()
                .id("workspace-monitors")
                .flex_1()
                .min_w_0()
                .min_h_0()
                .bg(p.surface)
                .overflow_y_scroll()
                .scrollbars()
                .p(px(12.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .children(
                    tiles
                        .into_iter()
                        .enumerate()
                        .fold(Vec::<Vec<AnyElement>>::new(), |mut rows, (i, tile)| {
                            if i % columns == 0 {
                                rows.push(vec![]);
                            }
                            rows.last_mut().unwrap().push(tile);
                            rows
                        })
                        .into_iter()
                        .map(|row| div().flex().flex_shrink_0().gap(px(10.)).children(row)),
                )
                .into_any_element()
        };
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .child(self.workspace_list(cx))
            .child(content)
            .into_any_element()
    }
    pub(super) fn workspace_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (panel, initial, title) = if let Some(dialog) = &self.quick_connect {
            (
                dialog.view.clone().into_any_element(),
                dialog.view.read(cx).focus_handle(cx),
                "Quick Connect",
            )
        } else if let Some(dialog) = &self.rename_workspace {
            (
                crate::components::DialogFrame::new("rename-workspace-dialog", "Rename workspace")
                    .width(360.)
                    .colors(crate::components::DialogColors::workspace(self.data.dark))
                    .content(
                        Field::input(&dialog.input, "Name").error(dialog.validation_error.clone()),
                    )
                    .action(
                        Button::new("cancel-workspace-rename")
                            .ghost()
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                crate::components::Modal::dismiss(window, cx)
                            }),
                    )
                    .action(
                        Button::new("confirm-workspace-rename")
                            .primary()
                            .label("Rename")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_workspace_name(window, cx)
                            })),
                    )
                    .into_any_element(),
                dialog.input.read(cx).focus_handle(cx),
                "Rename workspace",
            )
        } else {
            return None;
        };
        Some(
            crate::components::Modal::new("workspace-modal", title)
                .initial_focus(&initial)
                .return_focus(&self.focus)
                .on_dismiss(cx.listener(|this, _, window, cx| {
                    this.finish_workspace_dialog(window, cx);
                }))
                .content(panel)
                .into_any_element(),
        )
    }
    pub(super) fn close_title(&self, target: CloseTarget) -> &'static str {
        match target {
            CloseTarget::Tab(_) => "Close terminal?",
            CloseTarget::Session(_) => "Close terminal?",
            CloseTarget::Quit => "Quit Nauterm RS?",
            CloseTarget::Window => "Close window?",
            CloseTarget::Restart => "Restart Nauterm RS?",
            CloseTarget::Workspace(_) => "Close Workspace?",
        }
    }
    pub(super) fn close_message(&self, target: CloseTarget, cx: &App) -> String {
        let base = self.close_base_message(target, cx);
        let editing = |session: &SessionView| {
            [&session.files, &session.tool_files]
                .into_iter()
                .flatten()
                .any(|files| files.read(cx).has_external_edits(cx))
        };
        let has_edits = match target {
            CloseTarget::Quit | CloseTarget::Window | CloseTarget::Restart => {
                self.sftp
                    .as_ref()
                    .is_some_and(|files| files.read(cx).has_external_edits(cx))
                    || self
                        .terminals
                        .iter()
                        .any(|tab| tab.tree.sessions().any(editing))
            }
            CloseTarget::Session(id) => self.session(id).is_some_and(editing),
            CloseTarget::Tab(id) => self
                .terminals
                .iter()
                .find(|tab| tab.id == id)
                .is_some_and(|tab| tab.tree.sessions().any(editing)),
            CloseTarget::Workspace(id) => self
                .terminals
                .in_workspace(id)
                .any(|tab| tab.tree.sessions().any(editing)),
        };
        if has_edits {
            format!(
                "{base}\n\nExternal edit sessions will close. Stable pending saves are uploaded only when safe; unsent edits are retained in private local folders."
            )
        } else {
            base
        }
    }
    fn close_base_message(&self, target: CloseTarget, cx: &App) -> String {
        if matches!(target, CloseTarget::Restart) && !self.has_open_sessions(cx) {
            return "Restart Nauterm RS after saving the current workspace?".into();
        }
        if matches!(
            target,
            CloseTarget::Quit | CloseTarget::Window | CloseTarget::Restart
        ) && self.active_forward_count() > 0
        {
            return format!(
                "{} active port forward(s) will stop. Running terminal sessions and SFTP work will also be disconnected.",
                self.active_forward_count()
            );
        }
        if let CloseTarget::Workspace(id) = target {
            let name = self
                .data
                .workspaces
                .iter()
                .find(|workspace| workspace.id == id)
                .map(|workspace| workspace.name.as_str())
                .unwrap_or("Unavailable workspace");
            let count = self
                .terminals
                .in_workspace(id)
                .map(|tab| tab.tree.sessions().count())
                .sum::<usize>();
            return if count == 0 {
                format!("Close \"{name}\"?")
            } else {
                format!(
                    "Close \"{name}\" and its {count} {}? Running sessions will be disconnected.",
                    if count == 1 { "session" } else { "sessions" }
                )
            };
        }
        if matches!(
            target,
            CloseTarget::Quit | CloseTarget::Window | CloseTarget::Restart
        ) && self
            .sftp
            .as_ref()
            .is_some_and(|view| view.read(cx).has_request(cx))
        {
            "SFTP work will stop. Paused transfers are not restored on the next launch, and remote partial data may remain. Running sessions will be disconnected.".into()
        } else if matches!(
            target,
            CloseTarget::Quit | CloseTarget::Window | CloseTarget::Restart
        ) && self
            .keychain
            .as_ref()
            .is_some_and(|view| view.read(cx).has_connection(cx))
        {
            "Running sessions will be disconnected. A key export may have partially completed; check the remote file before retrying.".into()
        } else {
            "Running sessions will be disconnected. Their processes may still be running remotely."
                .into()
        }
    }
}
