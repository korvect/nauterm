use super::*;
use crate::recovery::{
    DEFAULT_TOOLS_WIDTH, MAX_TOOLS_WIDTH, MIN_TOOLS_WIDTH, TerminalTool as ToolMode,
    TerminalToolsSnapshot,
};
#[path = "terminal_tool_content.rs"]
mod content;

struct ChatTool {
    view: Entity<crate::ai::chat::Chat>,
    _events: Subscription,
    _toolbar: Subscription,
}
pub(super) struct ToolsState {
    open: bool,
    width: f32,
    mode: ToolMode,
    pub(super) snippet_search: Option<Entity<InputState>>,
    pub(super) snippet_scroll: ScrollHandle,
    pub(super) snippet_sort: crate::management::SortOrder,
    pub(super) snippet_editor: Option<super::snippets::InlineSnippet>,
    _snippet_events: Option<Subscription>,
    history_search: Option<Entity<InputState>>,
    theme_search: Option<Entity<InputState>>,
    fonts: Option<content::FontControls>,
    history_scroll: UniformListScrollHandle,
    theme_scroll: UniformListScrollHandle,
    _tool_events: Vec<Subscription>,
    system: Option<(u64, Entity<crate::system_info::SystemTool>)>,
    chats: std::collections::BTreeMap<u64, ChatTool>,
}
impl Default for ToolsState {
    fn default() -> Self {
        Self {
            open: false,
            width: DEFAULT_TOOLS_WIDTH,
            mode: ToolMode::Sftp,
            snippet_search: None,
            snippet_scroll: ScrollHandle::new(),
            snippet_sort: Default::default(),
            snippet_editor: None,
            _snippet_events: None,
            history_search: None,
            theme_search: None,
            fonts: None,
            history_scroll: UniformListScrollHandle::new(),
            theme_scroll: UniformListScrollHandle::new(),
            _tool_events: vec![],
            system: None,
            chats: Default::default(),
        }
    }
}
impl ToolsState {
    pub(super) fn ai_open(&self, sftp: bool) -> bool {
        self.open && !sftp && self.mode == ToolMode::Ai
    }
    pub(super) fn toggle_ai(&mut self) {
        self.open = !(self.open && self.mode == ToolMode::Ai);
        self.mode = ToolMode::Ai;
    }
    pub(super) fn history_visible(&self) -> bool {
        self.open && self.mode == ToolMode::History
    }
    pub(super) fn refresh_theme(&self, colors: crate::terminal_appearance::Palette, cx: &mut App) {
        if let Some((_, tool)) = &self.system {
            tool.update(cx, |tool, cx| tool.set_theme(colors, cx));
        }
        for chat in self.chats.values() {
            chat.view.update(cx, |chat, cx| chat.set_theme(colors, cx));
        }
    }
    pub(super) fn snapshot(&self) -> Option<TerminalToolsSnapshot> {
        (self.open || self.width != DEFAULT_TOOLS_WIDTH || self.mode != ToolMode::Sftp).then_some(
            TerminalToolsSnapshot {
                open: self.open,
                width: self.width,
                mode: self.mode,
            },
        )
    }
    pub(super) fn restore(snapshot: Option<TerminalToolsSnapshot>) -> Self {
        match snapshot {
            Some(snapshot) => Self {
                open: snapshot.open,
                width: snapshot.width,
                mode: snapshot.mode,
                ..Default::default()
            },
            None => Self::default(),
        }
    }
}
fn tools_visible(open: bool, sftp_page: bool) -> bool {
    open && !sftp_page
}

impl Nauterm {
    pub(super) fn file_pages_active(&self, window: &Window) -> bool {
        window.is_window_active() && self.file_pages_visible()
    }
    pub(super) fn file_pages_visible(&self) -> bool {
        self.quick_connect.is_none()
            && self.rename_workspace.is_none()
            && self.close_pending.is_none()
            && self.recovery_pending.is_none()
    }
    fn set_terminal_tool(
        &mut self,
        tab_id: u64,
        mode: ToolMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if mode == ToolMode::Themes {
            self.refresh_theme_catalog(cx);
        }
        if let Some(tab) = self.terminals.iter_mut().find(|tab| tab.id == tab_id) {
            tab.tools.mode = mode;
        }
        self.prepare_terminal_tools(tab_id, window, cx);
        self.refresh_terminal_files(window, cx);
        cx.notify();
    }
    fn prepare_terminal_tools(&mut self, tab_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.terminals.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        if tab.tools.mode == ToolMode::Sftp
            && matches!(
                tab.active().restore_target.as_ref(),
                Some(RestoreTarget::Local { .. })
            )
        {
            tab.tools.mode = ToolMode::History;
        }
        Self::init_tool_search(&mut tab.tools, window, cx);
        let initialize_snippets =
            tab.tools.mode == ToolMode::Snippets && tab.tools.snippet_search.is_none();
        if initialize_snippets {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search snippets"));
            let scroll = tab.tools.snippet_scroll.clone();
            let events = cx.subscribe(&input, move |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }
            });
            tab.tools.snippet_search = Some(input);
            tab.tools._snippet_events = Some(events);
        }
        // Initialize once, not on every render: an unavailable vault must not
        // turn this into a notify/load loop. The normal Retry action is retained.
        if initialize_snippets {
            self.load_snippets(cx);
        }
    }
    pub(super) fn terminal_tools_open(&self) -> bool {
        self.terminals
            .iter()
            .find(|tab| Some(tab.id) == self.active_terminal())
            .is_some_and(|tab| tools_visible(tab.tools.open, tab.show_sftp))
    }
    fn selected_session(&self) -> Option<u64> {
        self.terminals
            .iter()
            .find(|tab| Some(tab.id) == self.active_terminal())
            .map(|tab| tab.active())
            .map(|session| session.id)
    }
    fn ensure_terminal_files(
        &mut self,
        id: u64,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session(id) else {
            return;
        };
        if session.replay.is_some() {
            return;
        }
        let host_id = match session.restore_target {
            Some(RestoreTarget::Host { id, .. } | RestoreTarget::MoshHost { id, .. }) => Some(id),
            _ => None,
        };
        let existing = if compact {
            &session.tool_files
        } else {
            &session.files
        };
        if let Some(files) = existing {
            if let (Some(options), TabContent::Terminal(terminal)) =
                (&session.ssh, &session.content)
            {
                let source = crate::directory_follow::Endpoint {
                    host: options.host.clone(),
                    port: options.port,
                    username: options.username.clone(),
                    host_id,
                };
                let terminal = terminal.clone();
                files.update(cx, |files, cx| {
                    files.bind_terminal_metadata(terminal, source, window, cx)
                });
            }
            return;
        }
        let binding = match (&session.ssh, &session.content) {
            (Some(options), TabContent::Terminal(terminal)) => Some(crate::sftp::TerminalBinding {
                terminal: terminal.clone(),
                options: options.for_new_connection(),
                title: session.title.clone(),
                host_id,
                compact,
            }),
            _ => None,
        };
        // Session tabs establish their SSH terminal first; only the separate
        // top-level SFTP page connects without a terminal binding.
        let Some(binding) = binding else {
            return;
        };
        let keychain = self.ensure_keychain(cx);
        let vault = keychain.read(cx).vault();
        let files = cx.new(|cx| {
            let files = crate::sftp::Sftp::for_terminal(
                self.data.clone(),
                self.store.paths.known_hosts(),
                vault,
                binding,
                window,
                cx,
            );
            files
                .with_keychain(keychain.downgrade(), cx)
                .with_terminal_navigation(!compact, cx)
        });
        let events = cx.subscribe_in(
            &files,
            window,
            move |this, source, _: &crate::sftp::ShowTerminal, window, cx| {
                let current = this.session(id).and_then(|session| {
                    if compact {
                        session.tool_files.as_ref()
                    } else {
                        session.files.as_ref()
                    }
                });
                if current.is_none_or(|current| current.entity_id() != source.entity_id())
                    || this.selected_session() != Some(id)
                {
                    return;
                }
                let active = this.active_terminal();
                if let Some(tab) = this.terminals.iter_mut().find(|tab| Some(tab.id) == active) {
                    tab.show_sftp = false;
                    tab.active().content.focus(window, cx);
                }
                this.refresh_terminal_files(window, cx);
                cx.notify();
            },
        );
        let status = cx.subscribe(&files, |_, _, _: &crate::sftp::StatusChanged, cx| {
            cx.notify()
        });
        let connected = cx.subscribe(
            &files,
            |this, _, event: &crate::sftp::RemoteConnected, cx| {
                this.detect_sftp_platform(event, cx);
            },
        );
        if let Some(session) = self.session_mut(id) {
            session._file_events.extend([events, status, connected]);
            if compact {
                session.tool_files = Some(files);
            } else {
                session.files = Some(files);
            }
        }
    }
    pub(super) fn refresh_terminal_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let files_visible =
            self.page == Page::Sessions && !self.overview && self.file_pages_visible();
        let visible =
            self.page == Page::Sessions && !self.overview && self.file_pages_active(window);
        let prepare = visible
            .then(|| {
                self.terminals
                    .iter()
                    .find(|tab| {
                        Some(tab.id) == self.active_terminal()
                            && tools_visible(tab.tools.open, tab.show_sftp)
                    })
                    .map(|tab| tab.id)
            })
            .flatten();
        if let Some(tab) = prepare {
            self.prepare_terminal_tools(tab, window, cx);
        }
        let selected = files_visible
            .then(|| {
                self.terminals
                    .iter()
                    .find(|tab| Some(tab.id) == self.active_terminal())
                    .map(|tab| tab.active().id)
            })
            .flatten();
        let tools_open = self.terminal_tools_open()
            && self
                .terminals
                .iter()
                .find(|tab| Some(tab.id) == self.active_terminal())
                .is_some_and(|tab| tab.tools.mode == ToolMode::Sftp);
        let full = self
            .terminals
            .iter()
            .find(|tab| Some(tab.id) == self.active_terminal())
            .is_some_and(TerminalTab::files_visible);
        if let Some(id) = selected.filter(|_| visible) {
            if full {
                self.ensure_terminal_files(id, false, window, cx);
            }
            if tools_open {
                self.ensure_terminal_files(id, true, window, cx);
            }
        }
        for session in self.terminals.iter().flat_map(|tab| tab.tree.sessions()) {
            for (view, active) in [
                (&session.files, selected == Some(session.id) && full),
                (
                    &session.tool_files,
                    selected == Some(session.id) && tools_open,
                ),
            ] {
                if let Some(view) = view {
                    view.update(cx, |view, cx| {
                        view.set_data(self.data.clone(), cx);
                        view.set_visible(active, window, cx);
                    });
                }
            }
        }
        self.refresh_system_tools(visible, cx);
        self.refresh_ai_tools(visible, window, cx);
        self.refresh_terminal_fonts(visible, window, cx);
    }
    fn refresh_ai_tools(&mut self, visible: bool, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.active_terminal();
        for tab in self.terminals.iter_mut() {
            let alive: std::collections::BTreeSet<_> =
                tab.tree.sessions().map(|session| session.id).collect();
            tab.tools.chats.retain(|id, _| alive.contains(id));
            let show = visible
                && selected == Some(tab.id)
                && tools_visible(tab.tools.open, tab.show_sftp)
                && tab.tools.mode == ToolMode::Ai;
            let id = tab.active().id;
            if show && !tab.tools.chats.contains_key(&id) {
                let source = Self::terminal_ai_source(tab.active(), &self.data);
                let service = self.store.ai_providers().map_err(|error| error.to_string());
                let view = cx.new(|cx| {
                    crate::ai::chat::Chat::new(service, source, self.data.ai.clone(), window, cx)
                });
                let events = cx.subscribe_in(
                    &view,
                    window,
                    |this, _, _: &crate::ai::chat::Configure, window, cx| {
                        this.open_settings_at(Some(crate::settings::Page::Ai), window, cx);
                    },
                );
                tab.tools.chats.insert(
                    id,
                    ChatTool {
                        _toolbar: crate::ai::chat::Chat::observe_toolbar(&view, cx),
                        view,
                        _events: events,
                    },
                );
            }
            for session in tab.tree.sessions() {
                if let Some(chat) = tab.tools.chats.get(&session.id) {
                    let source = Self::terminal_ai_source(session, &self.data);
                    chat.view.update(cx, |chat, cx| {
                        chat.sync(show && session.id == id, &self.data.ai, source, window, cx)
                    });
                }
            }
        }
    }
    fn refresh_system_tools(&mut self, visible: bool, cx: &mut Context<Self>) {
        let selected = self.active_terminal();
        for tab in self.terminals.iter_mut() {
            let show = visible
                && selected == Some(tab.id)
                && tools_visible(tab.tools.open, tab.show_sftp)
                && tab.tools.mode == ToolMode::SystemInfo;
            if tab
                .tools
                .system
                .as_ref()
                .is_some_and(|(id, _)| *id != tab.active().id)
            {
                tab.tools.system = None;
            }
            let source = match (&tab.active().content, &tab.active().ssh) {
                (TabContent::Terminal(terminal), Some(options))
                    if terminal.read(cx).phase() == crate::connection_state::Phase::Completing =>
                {
                    Some((terminal.clone(), options))
                }
                _ => None,
            };
            let active = show && source.is_some();
            let colors = tab.active().appearance(&self.data).palette();
            if active
                && let Some((terminal, options)) = source
                && tab
                    .tools
                    .system
                    .as_ref()
                    .is_none_or(|(_, tool)| !tool.read(cx).belongs_to(&terminal))
            {
                let options = options.for_new_connection();
                tab.tools.system = Some((
                    tab.active().id,
                    cx.new(|_| crate::system_info::SystemTool::new(&terminal, options, colors)),
                ));
            }
            if let Some((_, tool)) = &tab.tools.system {
                tool.update(cx, |tool, cx| tool.sync(active, colors, cx));
            }
        }
    }
    pub(super) fn toggle_terminal_sftp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.active_terminal();
        let Some(tab) = self
            .terminals
            .iter_mut()
            .find(|tab| Some(tab.id) == selected)
        else {
            return;
        };
        if !tab.show_sftp && tab.active().ssh.is_none() {
            return;
        }
        tab.show_sftp = !tab.show_sftp;
        self.terminal_tools_drag = None;
        let full = tab.files_visible();
        let id = tab.active().id;
        self.refresh_terminal_files(window, cx);
        if let Some(session) = self.session(id) {
            if full {
                if let Some(files) = &session.files {
                    files.update(cx, |view, cx| view.focus(window, cx));
                }
            } else {
                session.content.focus(window, cx);
            }
        }
        cx.notify();
    }
    pub(super) fn toggle_terminal_tools(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.active_terminal();
        let Some(tab) = self
            .terminals
            .iter_mut()
            .find(|tab| Some(tab.id) == selected)
        else {
            return;
        };
        if tab.show_sftp || tab.active().replay.is_some() {
            return;
        }
        tab.tools.open = !tab.tools.open;
        let open = tab.tools.open;
        self.refresh_terminal_files(window, cx);
        if !open
            && let Some(tab) = self
                .terminals
                .iter()
                .find(|tab| Some(tab.id) == self.active_terminal())
        {
            tab.focus(window, cx);
        }
        cx.notify();
    }
    fn terminal_tool_modes(&self, tab: &TerminalTab, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::terminal(tab.active().appearance(&self.data).palette());
        let tab_id = tab.id;
        let modes = [
            (
                ToolMode::Ai,
                "terminal-tools-ai",
                "AI Assistant",
                IconName::Sparkles,
            ),
            (
                ToolMode::Sftp,
                "terminal-tools-sftp-mode",
                "SFTP",
                IconName::Folder,
            ),
            (
                ToolMode::SystemInfo,
                "terminal-tools-system",
                "System information",
                IconName::Activity,
            ),
            (
                ToolMode::Snippets,
                "terminal-tools-snippets-mode",
                "Snippets",
                IconName::Code,
            ),
            (
                ToolMode::History,
                "terminal-tools-history",
                "Shell history",
                IconName::Clock,
            ),
            (
                ToolMode::Themes,
                "terminal-tools-themes",
                "Terminal settings",
                IconName::Palette,
            ),
        ];
        div()
            .id("terminal-tool-mode-strip")
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(2.))
            .overflow_x_scroll()
            .children(modes.map(|(mode, id, title, icon)| {
                Button::new(id)
                    .ghost()
                    .custom(crate::components::ButtonCustomVariant::for_surface(p))
                    .size(px(27.))
                    .flex_shrink_0()
                    .icon_size(px(16.))
                    .icon(icon)
                    .bg(if tab.tools.mode == mode {
                        p.control
                    } else {
                        rgba(0)
                    })
                    .text_color(if tab.tools.mode == mode {
                        p.accent
                    } else {
                        p.muted
                    })
                    .disabled(mode == ToolMode::Sftp && tab.active().ssh.is_none())
                    .tooltip(title)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_terminal_tool(tab_id, mode, window, cx)
                    }))
            }))
    }
    pub(super) fn terminal_area(
        &self,
        tab: &TerminalTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = Palette::terminal(tab.active().appearance(&self.data).palette());
        let session = tab.active();
        let full = tab.files_visible();
        let main = if full {
            session
                .files
                .as_ref()
                .map(|files| files.clone().into_any_element())
                .unwrap_or_else(|| {
                    session.content.element(
                        session
                            .replay
                            .as_ref()
                            .map(|_| session.appearance(&self.data).palette()),
                    )
                })
        } else {
            self.terminal_tab_element(tab, cx)
        };
        let width = panel_width(tab.tools.width, f32::from(window.viewport_size().width));
        let open = tools_visible(tab.tools.open, tab.show_sftp);
        let amount = crate::components::panel::width_transition(
            ("terminal-tools-motion", tab.id),
            f32::from(open),
            0.,
            if open {
                crate::components::panel::ENTER
            } else {
                crate::components::panel::EXIT
            },
            window,
            cx,
        );
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(p.background)
            .flex()
            .child(div().flex_1().min_w_0().min_h_0().child(main))
            .when(open || amount > 0., |view| {
                let tab_id = tab.id;
                let panel = div().size_full().flex().child(
                    div()
                        .id("terminal-tools-divider")
                        .w(px(8.))
                        .h_full()
                        .flex_shrink_0()
                        .cursor(CursorStyle::ResizeLeftRight)
                        .bg(p.background)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                this.terminal_tools_drag = Some((tab_id, event.position.x));
                                cx.notify();
                                cx.stop_propagation();
                            }),
                        ),
                )
                .child(
                    div()
                        .id("terminal-sftp-tools")
                        .role(Role::Group)
                        .aria_label(crate::localization::text(cx, "Terminal tools"))
                        .w(px(width))
                        .h_full()
                        .py(px(8.))
                        .pr(px(8.))
                        .flex_shrink_0()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        // The backing/insets match the terminal; only the
                        // rounded panel itself uses the synthesized surface.
                        .bg(p.background)
                        .child(
                            div()
                                .size_full()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .rounded(px(12.))
                                .border_1()
                                .border_color(if self.terminal_tools_drag.is_some_and(|(id, _)| id == tab_id) { p.accent } else { rgba(0) })
                                .overflow_hidden()
                                .bg(p.sidebar)
                                .child(
                                    div()
                                        .h(px(44.))
                                        .flex_shrink_0()
                                        .px(px(8.))
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(self.terminal_tool_modes(tab, cx))
                                        .when(tab.tools.mode == ToolMode::Ai, |bar| {
                                            bar.children(tab.tools.chats.get(&session.id).map(|chat| {
                                                crate::ai::chat::Chat::toolbar_actions(&chat.view, cx)
                                            }))
                                        }),
                                )
                                .child(
                                    div().flex_1().min_h_0().pb(px(if tab.tools.mode == ToolMode::Ai || (tab.tools.mode == ToolMode::Snippets && tab.tools.snippet_editor.is_some()) { 0. } else { 10. })).child(match tab.tools.mode {
                                        ToolMode::Ai => tab.tools.chats.get(&session.id).map(|chat| chat.view.clone().into_any_element()).unwrap_or_else(|| div().p(px(20.)).text_color(p.muted).child(crate::localization::text(cx, "rs.ui.copy.aiAvailableAfterTerminal")).into_any_element()),
                                        ToolMode::Snippets => self.snippet_library(tab, cx),
                                        ToolMode::History => self.history_tool(tab, cx),
                                        ToolMode::Themes => self.themes_tool(tab, width, cx),
                                        ToolMode::SystemInfo => tab.tools.system.as_ref().map(|(_, tool)| tool.clone().into_any_element()).unwrap_or_else(|| {
                                            div().size_full().p(px(20.)).flex().flex_col().items_center().justify_center().gap(px(8.))
                                                .child(icon(IconName::Activity)).child(crate::localization::text(cx, "workspace.label.noSshSession"))
                                                .child(div().text_color(p.muted).text_center().child(crate::localization::text(cx, "rs.ui.copy.systemInformationAfterConnect")))
                                                .into_any_element()
                                        }),
                                        ToolMode::Sftp => session
                                            .tool_files
                                            .as_ref()
                                            .map(|files| files.clone().into_any_element())
                                            .unwrap_or_else(|| {
                                                div()
                                            .size_full()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_center()
                                            .gap(px(8.))
                                            .p(px(20.))
                                            .child(icon(IconName::Folder))
                                            .child(crate::localization::text(cx, "workspace.label.noSshSession"))
                                            .child(div().text_color(p.muted).text_center().child(
                                                crate::localization::text(cx, "SFTP is available after connecting with SSH."),
                                            ))
                                            .into_any_element()
                                            }),
                                    }),
                                ),
                        ),
                );
                view.child(crate::components::panel::slide(("terminal-tools-slide", tab.id), width + 8., amount, open, panel))
            })
            .into_any_element()
    }
    pub(super) fn resize_terminal_tools(
        &mut self,
        event: &MouseMoveEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if event.pressed_button != Some(MouseButton::Left) {
            if self.terminal_tools_drag.take().is_some() {
                cx.notify();
            }
            return;
        }
        if let Some((id, previous)) = self.terminal_tools_drag
            && let Some(tab) = self
                .terminals
                .iter_mut()
                .find(|tab| tab.id == id && tab.tools.open)
        {
            self.terminal_tools_drag = Some((id, event.position.x));
            tab.tools.width = panel_width(
                tab.tools.width + f32::from(previous - event.position.x),
                f32::from(window.viewport_size().width),
            );
            cx.notify();
            cx.stop_propagation();
        } else {
            self.terminal_tools_drag = None;
        }
    }
}
fn panel_width(requested: f32, viewport: f32) -> f32 {
    let maximum = (viewport - 480.).clamp(MIN_TOOLS_WIDTH, MAX_TOOLS_WIDTH);
    requested.clamp(maximum.min(280.), maximum)
}
#[cfg(test)]
mod tests {
    #[test]
    fn sftp_page_never_opens_a_second_sftp_panel() {
        assert!(!super::tools_visible(true, true));
        assert!(!super::tools_visible(false, true));
        assert!(super::tools_visible(true, false));
        assert!(!super::tools_visible(false, false));
    }
    #[test]
    fn tool_visibility_and_width_belong_to_each_terminal_tab() {
        let first = super::ToolsState {
            open: true,
            width: 510.,
            ..Default::default()
        };
        let second = super::ToolsState::default();
        assert!(first.open);
        assert_eq!(first.width, 510.);
        assert!(!second.open);
        assert_eq!(second.width, 350.);
    }
    #[test]
    fn saved_tools_restore_structure_without_search_or_editor_state() {
        let state = super::ToolsState {
            open: true,
            width: 510.,
            mode: super::ToolMode::Themes,
            ..Default::default()
        };
        let restored = super::ToolsState::restore(state.snapshot());
        assert_eq!(restored.snapshot(), state.snapshot());
        assert!(restored.history_search.is_none());
        assert!(restored.theme_search.is_none());
        assert!(restored.snippet_search.is_none());
        assert!(restored.snippet_editor.is_none());
        assert!(restored.system.is_none());
        assert!(super::ToolsState::restore(None).snapshot().is_none());
    }
    #[test]
    fn tool_panel_width_preserves_reference_terminal_space_and_limits() {
        assert_eq!(super::panel_width(390., 1180.), 390.);
        assert_eq!(super::panel_width(390., 750.), 270.);
        assert_eq!(super::panel_width(1000., 1920.), 640.);
        assert_eq!(super::panel_width(0., 1180.), 280.);
    }
}
