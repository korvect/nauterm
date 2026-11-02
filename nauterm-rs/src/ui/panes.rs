use super::*;
use crate::pane_tree::{Node, Pane, SplitAxis};
use gpui_kit::base::ElementExt;

pub(super) struct SplitDrag {
    tab: u64,
    split: u64,
    boundary: usize,
    last: Point<Pixels>,
}

impl Nauterm {
    pub(super) fn terminal_subscriptions(
        &self,
        terminal: &Entity<Terminal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        vec![
            cx.subscribe_in(
                terminal,
                window,
                |_, source, _: &crate::terminal::bell::Observed, window, cx| {
                    let owner = source.downgrade();
                    // Coalesce native wakeups after the source releases its borrow.
                    // Defer instead of waiting for paint: hidden windows still ring.
                    window.defer(cx, move |window, cx| {
                        let _ = owner.update(cx, |terminal, cx| terminal.deliver_bell(window, cx));
                    });
                },
            ),
            cx.subscribe(
                terminal,
                |this, source, _: &crate::terminal::bell::Indicator, cx| {
                    let id = this.terminals.iter().find(|tab| {
                        tab.tree.sessions().any(|session| {
                            matches!(&session.content, TabContent::Terminal(current) if current.entity_id() == source.entity_id())
                        })
                    }).map(|tab| tab.id);
                    if let Some(id) = id {
                        let selected = this.page == Page::Sessions
                            && !this.overview
                            && this.active_terminal() == Some(id);
                        if !selected && this.bell_tabs.insert(id) {
                            cx.notify();
                        }
                    }
                },
            ),
            cx.subscribe_in(terminal, window, |this, source, _: &crate::terminal::reconnect::ExitRequested, window, cx| {
                let id = this.terminals.iter().flat_map(|tab| tab.tree.sessions())
                    .find(|session| matches!(&session.content, TabContent::Terminal(current) if current == source))
                    .map(|session| session.id);
                if let Some(id) = id { this.request_session_close(id, window, cx); }
            }),
            cx.subscribe_in(terminal, window, |this, source, _: &crate::terminal::reconnect::Requested, window, cx| {
                this.reconnect_terminal(source, window, cx);
            }),
            cx.subscribe(terminal, |this, source, _: &crate::terminal::TerminalConnected, cx| {
                let id = this.terminals.iter().flat_map(|tab| tab.tree.sessions())
                    .find(|session| matches!(&session.content, TabContent::Terminal(terminal) if terminal.entity_id() == source.entity_id()))
                    .map(|session| session.id);
                if let Some(id) = id {
                    let options = source.read(cx).reconnect_profile().map(Box::new);
                    if let Some(session) = this.session_mut(id) { session.ssh = options; }
                    this.detect_terminal_platform(id, cx);
                }
            }),
            cx.subscribe_in(terminal, window, |this, source, command: &crate::terminal::TerminalCommand, window, cx| {
                let target = this.terminals.iter().find_map(|tab| {
                    tab.tree.sessions().find_map(|session| {
                        matches!(&session.content, TabContent::Terminal(terminal) if terminal.entity_id() == source.entity_id())
                            .then_some((tab.id, session.id))
                    })
                });
                let Some((tab, session)) = target else { return; };
                if this.page != Page::Sessions || crate::components::overlay::is_modal_open(window, cx) { return; }
                this.select_pane_session(tab, session, window, cx);
                use crate::terminal::TerminalCommand as Command;
                match command {
                    Command::SplitRight => this.duplicate_selected(Some(SplitAxis::Horizontal), window, cx),
                    Command::SplitDown => this.duplicate_selected(Some(SplitAxis::Vertical), window, cx),
                    Command::NewTab => this.duplicate_selected(None, window, cx),
                    Command::Settings => this.open_settings(window, cx),
                    Command::Close => this.close_selected_session(window, cx),
                }
            }),
            cx.subscribe(terminal, |_, _, _: &TerminalMetadataChanged, cx| {
                cx.notify()
            }),
            cx.subscribe(
                terminal,
                |this, _, error: &crate::terminal::TerminalCompletionError, cx| {
                    this.notice = Some(error.0.clone());
                    cx.notify();
                },
            ),
        ]
    }
    fn reconnect_terminal(
        &mut self,
        terminal: &Entity<Terminal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exiting.is_some() {
            return;
        }
        let Some(session) = self.terminals.iter().flat_map(|tab| tab.tree.sessions())
            .find(|session| matches!(&session.content, TabContent::Terminal(current) if current == terminal)) else { return; };
        if self.close_pending.is_some() {
            terminal.update(cx, |terminal, cx| terminal.defer_reconnect(cx));
            return;
        }
        if let Some(login) = terminal.read(cx).reconnect_login() {
            login.update(cx, |connection, cx| connection.retry_attached(window, cx));
            return;
        }
        let Some(core) = self.core.clone() else {
            terminal.update(cx, |terminal, cx| {
                terminal.reconnect_failed("The terminal core is unavailable.".into(), cx)
            });
            return;
        };
        let Some(options) = terminal.read(cx).reconnect_profile() else {
            return;
        };
        let host = session
            .host_id()
            .and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
            .cloned();
        let presentation = crate::connection::TerminalPresentation {
            title: session.title.clone(),
            dark: self.data.dark,
            palette: session.appearance(&self.data).palette(),
            recording: None,
        };
        let mosh = terminal.read(cx).reconnect_mosh();
        let source = terminal.downgrade();
        let connection = cx.new(|cx| {
            Connection::new_reconnect(core, options, source, mosh, presentation, window, cx)
        });
        let keychain = self.ensure_keychain(cx);
        let vault = keychain.read(cx).vault();
        connection.update(cx, |connection, cx| {
            connection.configure_authentication(
                vault,
                keychain.downgrade(),
                host.as_ref()
                    .and_then(crate::vault::authentication::Binding::from_host),
                cx,
            );
            if let Some(host) = &host {
                connection.set_host_platform(host.platform.clone(), cx);
            }
        });
        terminal.update(cx, |terminal, cx| {
            terminal.install_reconnect(connection, window, cx)
        });
    }
    pub(super) fn session(&self, id: u64) -> Option<&SessionView> {
        self.terminals.iter().find_map(|tab| tab.tree.session(id))
    }
    pub(super) fn session_mut(&mut self, id: u64) -> Option<&mut SessionView> {
        self.terminals
            .iter_mut()
            .find_map(|tab| tab.tree.session_mut(id))
    }
    pub(super) fn close_pane_session(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self
            .terminals
            .iter()
            .find(|tab| tab.tree.session(id).is_some())
            .map(|tab| tab.id)
        else {
            return;
        };
        let was_active = self.active_terminal() == Some(tab_id);
        let tab = self
            .terminals
            .iter_mut()
            .find(|tab| tab.id == tab_id)
            .unwrap();
        tab.tree.remove_session(id);
        if tab.tree.root().is_none() {
            self.terminals.remove(tab_id);
            if was_active && self.active_terminal().is_none() {
                self.overview = false;
                if self.page == Page::Sessions {
                    self.navigate(Page::Hosts, window, cx);
                }
            }
        }
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
    }
    fn activate_pane(&mut self, tab: u64, pane: u64, cx: &mut Context<Self>) {
        self.bell_tabs.remove(&tab);
        if let Some(tab) = self.terminals.iter_mut().find(|item| item.id == tab) {
            tab.tree.select_pane(pane);
        }
        self.terminals.select(tab);
        cx.notify();
    }
    fn select_pane_session(
        &mut self,
        tab: u64,
        session: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.bell_tabs.remove(&tab);
        if let Some(tab) = self.terminals.iter_mut().find(|item| item.id == tab) {
            tab.tree.select_session(session);
        }
        self.terminals.select(tab);
        if let Some(session) = self.session(session) {
            session.content.focus(window, cx);
        }
        cx.notify();
    }
    pub(super) fn duplicate_selected(
        &mut self,
        axis: Option<SplitAxis>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page != Page::Sessions
            || self.quick_connect.is_some()
            || self.serial_connect.is_some()
            || self.rename_workspace.is_some()
            || self.close_pending.is_some()
        {
            return;
        }
        let target = self
            .terminals
            .iter()
            .find(|tab| Some(tab.id) == self.active_terminal())
            .map(|tab| (tab.id, tab.tree.selected_pane()));
        if let Some((tab, pane)) = target {
            self.duplicate_pane(tab, pane, axis, window, cx);
        }
    }
    fn duplicate_pane(
        &mut self,
        tab_id: u64,
        pane_id: u64,
        axis: Option<SplitAxis>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if axis.is_none() && !self.data.terminal.multiple_tabs {
            return;
        }
        let Some(tab) = self.terminals.iter().find(|tab| tab.id == tab_id) else {
            return;
        };
        let Some(pane) = tab.tree.panes().into_iter().find(|pane| pane.id == pane_id) else {
            return;
        };
        if !matches!(pane.active().content, TabContent::Terminal(_)) {
            return;
        }
        let ssh = pane.active().ssh.clone();
        let serial = match &pane.active().content {
            TabContent::Terminal(terminal) => terminal.read(cx).serial_config().cloned(),
            _ => None,
        };
        let telnet = match &pane.active().content {
            TabContent::Terminal(terminal) => terminal.read(cx).telnet_options().cloned(),
            _ => None,
        };
        let restore_target = if ssh.is_some() || serial.is_some() || telnet.is_some() {
            pane.active().restore_target.clone()
        } else {
            pane.active()
                .restore_target
                .clone()
                .or(Some(RestoreTarget::Local { shell: None }))
        };
        let source = pane.active();
        if source.replay.is_some() {
            return;
        }
        let mut local_config = if let TabContent::Terminal(terminal) = &source.content {
            (
                terminal.read(cx).local_shell(),
                terminal
                    .read(cx)
                    .working_directory()
                    .or_else(|| source.restore_directory.clone()),
            )
        } else {
            (None, None)
        };
        if let Some(RestoreTarget::Host { id, .. }) = restore_target.as_ref()
            && let Some(host) = self
                .data
                .hosts
                .iter()
                .find(|host| host.id == *id && !host.is_remote())
        {
            local_config.0 = local_config.0.or_else(|| host.shell_path.clone());
            local_config.1 = local_config.1.or_else(|| host.work_dir.clone());
        }
        let appearance = source.appearance(&self.data);
        let host_id = source.host_id();
        let title = if ssh.is_some()
            || serial.is_some()
            || telnet.is_some()
            || matches!(restore_target, Some(RestoreTarget::Host { .. }))
        {
            pane.active().title.clone()
        } else if axis.is_some() {
            "Local Terminal".into()
        } else {
            format!("Local Terminal {}", pane.sessions.len() + 1)
        };
        let result = (|| -> anyhow::Result<_> {
            let core = self
                .core
                .as_ref()
                .cloned()
                .map(Ok)
                .unwrap_or_else(Core::discover)?;
            let (content, metadata) = if ssh.is_some() || serial.is_some() || telnet.is_some() {
                (TabContent::Preparing(None), vec![])
            } else {
                let directory = crate::shells::expand_directory(local_config.1.as_deref())?;
                let mut session = core.restore_local(
                    local_config.0.as_deref(),
                    directory.as_deref(),
                    appearance.palette(),
                )?;
                self.record_session(&mut session, &title, &appearance, host_id, cx);
                let terminal =
                    cx.new(|cx| Terminal::new(session, vec![], appearance, None, window, cx));
                let metadata = self.terminal_subscriptions(&terminal, window, cx);
                (TabContent::Terminal(terminal), metadata)
            };
            Ok((core, content, metadata))
        })();
        let (core, content, metadata) = match result {
            Ok(result) => result,
            Err(error) => {
                self.notice = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let id = self.next_tab;
        self.next_tab += 1;
        let session = SessionView {
            id,
            title,
            content,
            ssh: ssh.clone(),
            restore_target,
            restore_directory: None,
            files: None,
            tool_files: None,
            _file_events: vec![],
            replay: None,
            _metadata: metadata,
            preparation_generation: 0,
        };
        let Some(tab) = self.terminals.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        let inserted = if let Some(axis) = axis {
            let split = self.next_split;
            self.next_split += 1;
            tab.tree.insert_split(pane_id, id, session, axis, split)
        } else {
            tab.tree.insert_session(pane_id, session)
        };
        if inserted.is_err() {
            return;
        }
        self.terminals.select(tab_id);
        self.core = Some(core.clone());
        if let Some(options) = ssh {
            self.install_connection(id, core, *options, window, cx);
        } else if let Some(config) = serial {
            self.install_serial_connection(id, core, config, window, cx);
        } else if let Some(options) = telnet {
            self.install_telnet_connection(id, core, options, window, cx);
        }
        if let Some(session) = self.session(id) {
            session.content.focus(window, cx);
        }
        self.notice = None;
        cx.notify();
    }
    pub(super) fn close_selected_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != Page::Sessions
            || self.quick_connect.is_some()
            || self.serial_connect.is_some()
            || self.rename_workspace.is_some()
            || self.close_pending.is_some()
        {
            return;
        }
        let session = self
            .terminals
            .iter()
            .find(|tab| Some(tab.id) == self.active_terminal())
            .map(|tab| tab.active().id);
        if let Some(session) = session {
            self.request_session_close(session, window, cx);
        }
    }
    pub(super) fn request_session_close(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session(id) else {
            return;
        };
        if self.session_needs_close_confirmation(session, cx) {
            self.close_pending = Some(CloseTarget::Session(id));
            self.focus.focus(window, cx);
        } else {
            self.close_pane_session(id, window, cx);
        }
        cx.notify();
    }
    fn session_needs_close_confirmation(&self, session: &SessionView, cx: &App) -> bool {
        self.data.general.confirm_on_close
            && session.replay.is_none()
            && (session.content.phase(cx) == crate::connection_state::Phase::Completing
                || [&session.files, &session.tool_files]
                    .into_iter()
                    .flatten()
                    .any(|files| {
                        let files = files.read(cx);
                        files.phase(cx) == crate::connection_state::Phase::Completing
                            || files.has_request(cx)
                    }))
    }
    pub(super) fn request_tab_close(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.terminals.iter().find(|tab| tab.id == id) else {
            return;
        };
        if tab
            .tree
            .sessions()
            .any(|session| self.session_needs_close_confirmation(session, cx))
        {
            self.close_pending = Some(CloseTarget::Tab(id));
            self.focus.focus(window, cx);
        } else {
            self.confirm_close(CloseTarget::Tab(id), window, cx);
        }
        cx.notify();
    }
    pub(super) fn terminal_tab_element(
        &self,
        tab: &TerminalTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        tab.tree
            .root()
            .map(|node| self.pane_node(tab, node, cx))
            .unwrap_or_else(|| div().into_any_element())
    }
    fn pane_node(
        &self,
        tab: &TerminalTab,
        node: &Node<SessionView>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node {
            Node::Pane(pane) => self.pane_element(tab, pane, cx),
            Node::Split(split) => {
                let id = split.id;
                let tab_id = tab.id;
                let horizontal = split.axis == SplitAxis::Horizontal;
                let p = Palette::terminal(tab.active().appearance(&self.data).palette());
                let mut children = vec![];
                for (index, child) in split.children.iter().enumerate() {
                    children.push(
                        div()
                            .flex_basis(px(0.))
                            .flex_grow(split.fractions[index])
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .when(horizontal, |view| view.h_full())
                            .when(!horizontal, |view| view.w_full())
                            .child(self.pane_node(tab, child, cx))
                            .into_any_element(),
                    );
                    if index + 1 < split.children.len() {
                        children.push(
                            div()
                                .id(("split-divider", id * 1024 + index as u64))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(p.background)
                                .when(horizontal, |view| {
                                    view.w(px(5.)).h_full().cursor(CursorStyle::ResizeLeftRight)
                                })
                                .when(!horizontal, |view| {
                                    view.h(px(5.)).w_full().cursor(CursorStyle::ResizeUpDown)
                                })
                                .hover(|view| view.bg(p.accent.opacity(0.10)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                        this.split_drag = Some(SplitDrag {
                                            tab: tab_id,
                                            split: id,
                                            boundary: index,
                                            last: event.position,
                                        });
                                        cx.stop_propagation();
                                    }),
                                )
                                .child(
                                    div()
                                        .bg(p.text.alpha(0.08))
                                        .when(horizontal, |view| view.w(px(1.)).h_full())
                                        .when(!horizontal, |view| view.h(px(1.)).w_full()),
                                )
                                .into_any_element(),
                        );
                    }
                }
                div()
                    .size_full()
                    .relative()
                    .flex()
                    .min_w_0()
                    .min_h_0()
                    .when(!horizontal, |view| view.flex_col())
                    .children(children)
                    .on_prepaint({
                        let view = cx.entity().downgrade();
                        move |bounds, _, cx| {
                            let _ = view.update(cx, |this, _| {
                                this.split_bounds.insert(id, bounds);
                            });
                        }
                    })
                    .into_any_element()
            }
        }
    }
    fn pane_element(
        &self,
        tab: &TerminalTab,
        pane: &Pane<SessionView>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id;
        let pane_id = pane.id;
        let session = pane.active();
        let terminal = match &session.content {
            TabContent::Terminal(view) => Some(view.clone()),
            _ => None,
        };
        let p = if terminal.is_some() {
            Palette::terminal(session.appearance(&self.data).palette())
        } else {
            Palette::new(self.data.dark)
        };
        let selected =
            self.active_terminal() == Some(tab_id) && tab.tree.selected_pane() == pane_id;
        let title_color = if selected { p.text } else { p.muted };
        let chrome = ButtonCustomVariant::for_surface(p)
            .foreground(if selected { p.text } else { p.muted }.into());
        let compact = self
            .pane_bounds
            .get(&pane_id)
            .is_some_and(|bounds| bounds.size.width < px(260.));
        let show_tabs = self.data.terminal.multiple_tabs || pane.sessions.len() > 1;
        div()
            .id(("terminal-pane", pane_id))
            .role(Role::Group)
            .aria_label(format!("Terminal pane {pane_id}"))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .on_prepaint({
                let view = cx.entity().downgrade();
                move |bounds, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        let old = this.pane_bounds.insert(pane_id, bounds);
                        let was_compact = old.is_some_and(|old| old.size.width < px(260.));
                        if was_compact != (bounds.size.width < px(260.)) {
                            cx.notify();
                        }
                    });
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| this.activate_pane(tab_id, pane_id, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| this.activate_pane(tab_id, pane_id, cx)),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(move |this, _, _, cx| this.activate_pane(tab_id, pane_id, cx)),
            )
            .when(terminal.is_some() && session.replay.is_none(), |view| {
                let composer = pane.composer_visible;
                view.child(
                    div()
                        .h(px(34.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .bg(p.surface)
                        .child(
                            div()
                                .id(("pane-tabs", pane_id))
                                .flex_1()
                                .min_w_0()
                                .when(show_tabs, |view| view.overflow_x_scroll())
                                .when(!show_tabs, |view| view.overflow_hidden())
                                .scrollbars_hidden()
                                .flex()
                                .items_center()
                                .px(px(6.))
                                .gap(px(4.))
                                .when(show_tabs, |view| {
                                    view.children(pane.sessions.iter().map(|item| {
                                        let id = item.id;
                                        let active = item.id == session.id;
                                        div()
                                            .id(("pane-session", id))
                                            .h(px(28.))
                                            .max_w(px(260.))
                                            .flex_shrink_0()
                                            .flex()
                                            .items_center()
                                            .rounded(px(6.))
                                            .when(selected && active, |view| view.bg(p.hover))
                                            .child(
                                                Button::new(("select-pane-session", id))
                                                    .ink_ripple(false)
                                                    .xsmall()
                                                    .ghost()
                                                    .custom(chrome)
                                                    .content_start()
                                                    .min_w_0()
                                                    .text_size(px(12.))
                                                    .raw_label(item.title(cx))
                                                    .text_color(if active {
                                                        title_color
                                                    } else {
                                                        p.muted
                                                    })
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            cx.stop_propagation();
                                                            this.select_pane_session(
                                                                tab_id, id, window, cx,
                                                            );
                                                        },
                                                    )),
                                            )
                                            .child(
                                                Button::new(("close-pane-session", id))
                                                    .xsmall()
                                                    .ghost()
                                                    .custom(chrome)
                                                    .icon(crate::assets::icon(IconName::X))
                                                    .icon_size(px(13.))
                                                    .tooltip("Close terminal")
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            cx.stop_propagation();
                                                            this.request_session_close(
                                                                id, window, cx,
                                                            );
                                                        },
                                                    )),
                                            )
                                    }))
                                })
                                .when(!show_tabs, |view| {
                                    view.child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_size(px(12.))
                                            .font_weight(crate::typography::FontWeights::MEDIUM)
                                            .text_color(title_color)
                                            .text_ellipsis()
                                            .child(session.title(cx)),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .gap(px(2.))
                                .pr(px(4.))
                                .child(
                                    Button::new(("pane-composer", pane_id))
                                        .xsmall()
                                        .ghost()
                                        .custom(chrome)
                                        .icon(IconName::Keyboard)
                                        .text_color(if composer { p.accent } else { p.muted })
                                        .tooltip(if composer {
                                            "Hide Composer"
                                        } else {
                                            "Show Composer"
                                        })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            if let Some(tab) = this
                                                .terminals
                                                .iter_mut()
                                                .find(|tab| tab.id == tab_id)
                                            {
                                                tab.tree.toggle_composer(pane_id);
                                                if let Some(pane) = tab
                                                    .tree
                                                    .panes()
                                                    .into_iter()
                                                    .find(|pane| pane.id == pane_id)
                                                {
                                                    // Hiding a focused input must not leave an
                                                    // invisible composer receiving keystrokes.
                                                    pane.active().content.focus(window, cx);
                                                }
                                            }
                                            cx.notify();
                                        })),
                                )
                                .when(!compact, |view| {
                                    view.child(
                                        Button::new(("pane-sftp", pane_id))
                                            .xsmall()
                                            .ghost()
                                            .custom(chrome)
                                            .icon(IconName::Folder)
                                            .disabled(session.ssh.is_none())
                                            .tooltip("Open SFTP")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                cx.stop_propagation();
                                                this.activate_pane(tab_id, pane_id, cx);
                                                this.toggle_terminal_sftp(window, cx);
                                            })),
                                    )
                                })
                                .when(!compact && self.data.terminal.multiple_tabs, |view| {
                                    view.child(
                                        Button::new(("pane-new-tab", pane_id))
                                            .xsmall()
                                            .ghost()
                                            .custom(chrome)
                                            .icon(IconName::SquarePlus)
                                            .tooltip("New Tab")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                cx.stop_propagation();
                                                this.duplicate_pane(
                                                    tab_id, pane_id, None, window, cx,
                                                );
                                            })),
                                    )
                                })
                                .child(
                                    Button::new(("pane-split-right", pane_id))
                                        .xsmall()
                                        .ghost()
                                        .custom(chrome)
                                        .icon(IconName::SquareSplitHorizontal)
                                        .tooltip("Split right")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            this.duplicate_pane(
                                                tab_id,
                                                pane_id,
                                                Some(SplitAxis::Horizontal),
                                                window,
                                                cx,
                                            );
                                        })),
                                )
                                .child(
                                    Button::new(("pane-split-down", pane_id))
                                        .xsmall()
                                        .ghost()
                                        .custom(chrome)
                                        .icon(IconName::SquareSplitVertical)
                                        .tooltip("Split down")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            this.duplicate_pane(
                                                tab_id,
                                                pane_id,
                                                Some(SplitAxis::Vertical),
                                                window,
                                                cx,
                                            );
                                        })),
                                ),
                        ),
                )
            })
            .child(
                div().flex_1().min_w_0().min_h_0().child(
                    session.content.element(
                        session
                            .replay
                            .as_ref()
                            .map(|_| session.appearance(&self.data).palette()),
                    ),
                ),
            )
            .into_any_element()
    }
    pub(super) fn resize_split(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.split_drag = None;
            return;
        }
        let Some(drag) = &mut self.split_drag else {
            return;
        };
        let Some(bounds) = self.split_bounds.get(&drag.split) else {
            return;
        };
        let Some(tab) = self.terminals.iter_mut().find(|tab| tab.id == drag.tab) else {
            self.split_drag = None;
            return;
        };
        let Some(split) = tab.tree.root().and_then(|root| root.split(drag.split)) else {
            self.split_drag = None;
            return;
        };
        let (delta, extent) = if split.axis == SplitAxis::Horizontal {
            (event.position.x - drag.last.x, bounds.size.width)
        } else {
            (event.position.y - drag.last.y, bounds.size.height)
        };
        let extent = f32::from(extent) - 5. * split.children.len().saturating_sub(1) as f32;
        drag.last = event.position;
        if tab
            .tree
            .resize(drag.split, drag.boundary, f32::from(delta), extent)
        {
            cx.notify();
        }
        cx.stop_propagation();
    }
}
