use crate::components::menu::{ContextMenuExt, DropdownMenu, MenuItem};
use crate::{
    components::{
        Breadcrumb, Button, ButtonCustomVariant, Field, Input, ScrollbarsExt, SelectEvent,
        SelectModel, SelectOption, Tab,
    },
    connection::{Connection, ConnectionEvent},
    layout,
    model::*,
    native::{Core, SshOptions},
    palette::Palette,
    pane_tree::{PaneTree, SessionId},
    recovery::{BeginRun, Checkpoint, Journal, Snapshot, Target as RestoreTarget},
    store::Store,
    terminal::{Terminal, TerminalMetadataChanged},
    typography::FontWeights,
    workspace::{ScopedSession, Sessions},
};
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::{
    assets::IconName,
    component::{Disableable, Icon, Sizable},
    prelude::*,
    *,
};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
mod assistant;
mod card_menu;
mod dialogs;
mod drawer_motion;
mod exit;
mod forwards;
mod group_config;
mod group_from_protocol;
mod history;
mod host_config;
mod host_connection;
mod host_credentials;
mod host_environment;
mod host_import;
mod host_name;
mod host_platform;
mod host_tags;
mod host_themes;
mod mosh_config;
mod navigation;
mod panes;
#[cfg(test)]
mod preparation_tests;
mod related;
mod replay;
mod resources;
mod restoration;
mod serial;
mod settings;
mod shortcuts;
mod snippets;
mod startup;
mod sync;
mod telnet;
mod telnet_config;
mod terminal_files;
mod themes;
pub use restoration::initial_window_bounds;
mod workspaces;
use panes::SplitDrag;
use workspaces::{QuickDialog, RenameWorkspace};

gpui_kit::actions!(
    nauterm,
    [
        NewItem,
        CloseEditor,
        SelectCards,
        DeleteCards,
        OpenSettings,
        SplitRight,
        SplitDown,
        ClosePaneSession,
        NewPaneSession,
        Quit
    ]
);

#[derive(Clone, Copy, PartialEq, Eq)]
enum RemoteMode {
    Ssh,
    Sftp,
    Mosh,
}

struct Editor {
    scroll: ScrollHandle,
    kind: ItemKind,
    id: Option<Id>,
    group: Option<Id>,
    credential: Option<CredentialRef>,
    proxy_id: Option<i64>,
    fields: Vec<(&'static str, Entity<InputState>)>,
    error: Option<String>,
    validation_error: Option<String>,
    selects: Option<HostSelects>,
    forward: Option<forwards::ForwardSelects>,
    snippet: Option<snippets::SnippetDraft>,
    theme: Option<host_themes::Draft>,
    host: Option<host_config::Draft>,
    related_credential: Option<host_credentials::Related>,
    parent: Option<related::Parent>,
    related_proxy: Option<related::ProxyEditor>,
    tags: Option<host_tags::Draft>,
}

struct HostSelects {
    groups: SelectModel<Id>,
    auth: host_credentials::Authentication,
    proxies: SelectModel<i64>,
    _events: Vec<Subscription>,
}
fn group_options(data: &AppState, allowed: impl Fn(Id) -> bool) -> Vec<SelectOption<Id>> {
    data.groups
        .iter()
        .filter(|group| allowed(group.id))
        .map(|group| {
            SelectOption::new(
                group.id,
                data.group_path(group.id)
                    .iter()
                    .map(|group| group.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" / "),
            )
        })
        .collect()
}
struct LocalTarget {
    label: String,
    shell: Option<String>,
    directory: Option<String>,
    restore: RestoreTarget,
    appearance: crate::terminal_appearance::Appearance,
}
struct TerminalTab {
    id: u64,
    workspace: Id,
    tree: PaneTree<SessionView>,
    show_sftp: bool,
    tools: terminal_files::ToolsState,
}
struct SessionView {
    replay: Option<replay::State>,
    id: u64,
    title: String,
    content: TabContent,
    /// In-memory, zeroizing authentication snapshot used for explicit duplicate
    /// connections. Never persisted and never reused as accept-any-host trust.
    ssh: Option<Box<SshOptions>>,
    restore_target: Option<RestoreTarget>,
    restore_directory: Option<String>,
    files: Option<Entity<crate::sftp::Sftp>>,
    tool_files: Option<Entity<crate::sftp::Sftp>>,
    /// File-page callbacks outlive the preparing/connection/terminal content.
    _file_events: Vec<Subscription>,
    _metadata: Vec<Subscription>,
    preparation_generation: u64,
}
impl SessionId for SessionView {
    fn session_id(&self) -> u64 {
        self.id
    }
}
impl TerminalTab {
    fn owns_preparation(&self, (id, workspace): (u64, Id), generation: u64) -> bool {
        self.workspace == workspace
            && self.tree.session(id).is_some_and(|session| {
                session.preparation_generation == generation
                    && matches!(session.content, TabContent::Preparing(None))
            })
    }
    fn active(&self) -> &SessionView {
        self.tree
            .active()
            .expect("empty tabs are removed immediately")
    }
    fn focus(&self, window: &mut Window, cx: &mut App) {
        if self.files_visible()
            && let Some(files) = &self.active().files
        {
            files.update(cx, |files, cx| files.focus(window, cx));
        } else {
            self.active().content.focus(window, cx);
        }
    }
    fn title(&self, cx: &App) -> String {
        self.active().title(cx)
    }
    fn files_visible(&self) -> bool {
        self.show_sftp && matches!(self.active().content, TabContent::Terminal(_))
    }
    fn terminal_phase(&self, cx: &App) -> crate::connection_state::Phase {
        use crate::connection_state::Phase;
        let phases: Vec<_> = self
            .tree
            .sessions()
            .map(|session| session.content.phase(cx))
            .collect();
        if phases.contains(&Phase::Failed) {
            Phase::Failed
        } else if phases.contains(&Phase::Exited) {
            Phase::Exited
        } else if phases.iter().any(|phase| *phase != Phase::Completing) {
            Phase::Connecting
        } else {
            Phase::Completing
        }
    }
}
impl SessionView {
    fn host_id(&self) -> Option<Id> {
        match self.restore_target {
            Some(
                RestoreTarget::Host { id, .. }
                | RestoreTarget::TelnetHost { id, .. }
                | RestoreTarget::MoshHost { id, .. },
            ) => Some(id),
            _ => None,
        }
    }
    fn appearance(&self, data: &AppState) -> crate::terminal_appearance::Appearance {
        let mut appearance = data.terminal_for_host(self.host_id());
        if let Some(RestoreTarget::TelnetHost { id, .. }) = self.restore_target
            && let Some(theme) = data
                .hosts
                .iter()
                .find(|host| host.id == id)
                .and_then(|host| host.connections.telnet.as_ref())
                .and_then(|profile| profile.theme)
        {
            appearance = data.terminal.clone();
            appearance.select_theme(theme);
        } else if matches!(self.restore_target, Some(RestoreTarget::TelnetHost { .. })) {
            appearance = data.terminal.clone();
        }
        if let Some(theme) = self.replay.as_ref().and_then(|replay| replay.theme) {
            appearance.select_theme(theme);
        }
        appearance
    }
    fn title(&self, cx: &App) -> String {
        if self.ssh.is_some()
            || self.replay.is_some()
            || matches!(self.restore_target, Some(RestoreTarget::TelnetHost { .. }))
        {
            return self.title.clone();
        }
        let title = self.content.title(cx);
        if title.is_empty() {
            self.title.clone()
        } else {
            title
        }
    }
}
impl ScopedSession for TerminalTab {
    fn id(&self) -> u64 {
        self.id
    }
    fn workspace(&self) -> Id {
        self.workspace
    }
}
enum TabContent {
    Terminal(Entity<Terminal>),
    Connection(Entity<Connection>),
    Preparing(Option<String>),
}
impl TabContent {
    fn focus(&self, window: &mut Window, cx: &mut App) {
        match self {
            Self::Terminal(entity) => entity.update(cx, |view, cx| view.focus(window, cx)),
            Self::Connection(entity) => entity.update(cx, |view, cx| view.focus(window, cx)),
            Self::Preparing(_) => {}
        }
    }
    fn title(&self, cx: &App) -> String {
        match self {
            Self::Terminal(entity) => entity.read(cx).title().to_owned(),
            Self::Connection(_) => String::new(),
            Self::Preparing(_) => String::new(),
        }
    }
    fn phase(&self, cx: &App) -> crate::connection_state::Phase {
        use crate::connection_state::Phase;
        match self {
            Self::Terminal(entity) => entity.read(cx).phase(),
            Self::Connection(entity) => entity.read(cx).phase(),
            Self::Preparing(Some(_)) => Phase::Failed,
            Self::Preparing(None) => Phase::Connecting,
        }
    }
    fn element(&self, replay: Option<crate::terminal_appearance::Palette>) -> AnyElement {
        match self {
            Self::Terminal(entity) => entity.clone().into_any_element(),
            Self::Connection(entity) => entity.clone().into_any_element(),
            Self::Preparing(error) => div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .p(px(24.))
                .when_some(replay, |view, colors| {
                    view.bg(rgb(colors.base.background))
                        .text_color(rgb(colors.base.foreground))
                })
                .when(error.is_none(), |view| {
                    view.child(crate::components::Spinner::new())
                })
                .child(crate::localization::message_view(
                    error.clone().unwrap_or_else(|| {
                        if replay.is_some() {
                            "Processing terminal replay…".into()
                        } else {
                            "Loading saved credentials…".into()
                        }
                    }),
                ))
                .when(error.is_some(), |view| {
                    view.child(crate::localization::view(if replay.is_some() {
                        "Close this tab and retry from Logs after resolving the recording error."
                    } else {
                        "Close this tab and retry from Hosts after resolving the Keychain error."
                    }))
                })
                .into_any_element(),
        }
    }
}
#[derive(Clone, Copy)]
enum CloseTarget {
    Tab(u64),
    Session(u64),
    Quit,
    Window,
    Restart,
    Workspace(Id),
}

enum RetiredDrawer {
    Editor(Box<Editor>),
    History(Entity<history::History>),
}

pub struct Nauterm {
    window_id: WindowId,
    data: AppState,
    store: Arc<Store>,
    storage_loading: bool,
    startup_task: Option<Task<()>>,
    storage_error: Option<String>,
    page: Page,
    group: Option<Id>,
    searches: HashMap<Page, Entity<InputState>>,
    tag_filter: Entity<crate::tags::picker::Picker>,
    selected_tags: BTreeSet<String>,
    rename_tag: Option<host_tags::Rename>,
    subscriptions: Vec<Subscription>,
    focus: FocusHandle,
    selection: BTreeSet<Id>,
    selection_kind: Option<ItemKind>,
    card_navigation: crate::management::navigation::Navigation<(ItemKind, Id)>,
    editor: Option<Editor>,
    saved_drawers: HashMap<Page, drawer_motion::PageDrawer>,
    drawer_motion: crate::components::panel::Presence<RetiredDrawer>,
    sidebar_width: f32,
    child_drawer_open: bool,
    delete_pending: bool,
    collection_views: HashMap<Page, crate::management::CollectionView>,
    notice: Option<String>,
    core: Option<Arc<Core>>,
    theme_refreshing: bool,
    theme_refresh_pending: bool,
    font_options: Option<crate::fonts::Options>,
    font_loading: Option<Task<()>>,
    terminals: Sessions<TerminalTab>,
    overview: bool,
    quick_connect: Option<QuickDialog>,
    serial_connect: Option<serial::SerialDialog>,
    host_import: Option<host_import::Import>,
    host_exporting: bool,
    rename_workspace: Option<RenameWorkspace>,
    next_tab: u64,
    bell_tabs: BTreeSet<u64>,
    next_split: u64,
    split_bounds: HashMap<u64, Bounds<Pixels>>,
    pane_bounds: HashMap<u64, Bounds<Pixels>>,
    split_drag: Option<SplitDrag>,
    close_pending: Option<CloseTarget>,
    exiting: Option<crate::settings::ExitAction>,
    exit_task: Option<Task<()>>,
    menu_subscription: Option<Subscription>,
    keychain: Option<Entity<crate::keychain::Keychain>>,
    known_hosts: Option<Entity<crate::known_hosts::KnownHosts>>,
    recordings: Option<crate::recordings::Service>,
    shell_history: crate::shell_history::store::Service,
    history: Option<Entity<history::History>>,
    history_events: Option<Subscription>,
    logs: Option<Entity<crate::recordings::Logs>>,
    proxies: Option<Entity<crate::proxies::Proxies>>,
    deleted_proxies: BTreeSet<i64>,
    sftp: Option<Entity<crate::sftp::Sftp>>,
    journal: Option<Arc<Journal>>,
    recovery_pending: Option<Checkpoint>,
    recovery_focus: FocusHandle,
    recovery_always: bool,
    recovery_error: Option<String>,
    last_snapshot: Option<(Snapshot, RestoreBehavior)>,
    checkpoint_task: Option<Task<()>>,
    restore_task: Option<Task<()>>,
    terminal_tools_drag: Option<(u64, Pixels)>,
    workspace_assistants: HashMap<Id, assistant::WorkspaceAssistant>,
    workspace_assistant_drag: Option<(Id, Pixels, f32)>,
    settings_window: Option<settings::SettingsWindow>,
    pending_window_resize: Option<crate::general_preferences::WindowSize>,
    forward_service: Option<crate::port_forward::Service>,
    forward_states: HashMap<Id, forwards::Runtime>,
    forward_mutations: BTreeSet<Id>,
    forward_delete_busy: bool,
    next_forward_generation: u64,
    forward_events: Option<Task<()>>,
    host_os_requests: BTreeSet<crate::host_platform::Target>,
    snippet_state: snippets::SnippetState,
    automatic_sync: sync::Automatic,
    secure_preload: Option<Task<()>>,
}

fn icon(name: IconName) -> Icon {
    crate::assets::icon(name).with_size(px(16.)).flex_shrink_0()
}

fn page_icon(page: Page) -> IconName {
    match page {
        Page::Hosts => IconName::Server,
        Page::Keychain => IconName::KeyRound,
        Page::Proxies => IconName::Network,
        Page::PortForwards => IconName::ArrowLeftRight,
        Page::Snippets => IconName::Code,
        Page::KnownHosts => IconName::FingerprintPattern,
        Page::Logs => IconName::Clock,
        Page::Sftp => IconName::Folder,
        Page::Sessions => IconName::Terminal,
    }
}

fn item_icon(kind: ItemKind) -> IconName {
    match kind {
        ItemKind::Host => IconName::Server,
        ItemKind::Group => crate::assets::GROUP_ICON,
        ItemKind::Snippet => IconName::Code,
        ItemKind::SnippetPackage => IconName::Package,
        ItemKind::Forward => IconName::ArrowLeftRight,
        ItemKind::Proxy => IconName::Network,
    }
}

fn search_hint(page: Page, language: crate::localization::Language) -> SharedString {
    crate::localization::text_for(
        language,
        if page == Page::Hosts {
            "Search host:, group:, username:, tag:, or user@host"
        } else {
            "Search"
        },
    )
}

#[derive(PartialEq, Eq, Hash)]
enum ManagementSource {
    Context(&'static str),
    Editor(EntityId, &'static str),
    Snippet(u64),
}

impl Nauterm {
    /// Observe operation failures independently; field validation stays in forms.
    fn management_feedback(&self) -> Vec<(ManagementSource, crate::components::message::Message)> {
        let mut feedback = vec![];
        let mut push = |source, error: Option<String>| {
            if let Some(error) = error {
                feedback.push((source, crate::components::message::Message::error(error)));
            }
        };
        push(
            ManagementSource::Context("host-import"),
            self.host_import
                .as_ref()
                .and_then(|draft| draft.error.clone()),
        );
        push(
            ManagementSource::Context("rename-tag"),
            self.rename_tag
                .as_ref()
                .and_then(|draft| draft.error.clone()),
        );
        push(
            ManagementSource::Context("rename-workspace"),
            self.rename_workspace
                .as_ref()
                .and_then(|draft| draft.error.as_ref().map(ToString::to_string)),
        );
        for editor in self.editor.iter().chain(
            self.saved_drawers
                .values()
                .filter_map(|saved| saved.editor.as_ref()),
        ) {
            let id = editor.fields[0].1.entity_id();
            if let Some(draft) = &editor.host {
                push(
                    ManagementSource::Editor(id, "environment"),
                    draft
                        .environment_page
                        .as_ref()
                        .and_then(|page| page.error.clone()),
                );
                push(
                    ManagementSource::Editor(id, "inheritance"),
                    draft.inheritance_error.clone(),
                );
            }
            for (target, source) in [
                (host_credentials::Target::Ssh, "ssh-credentials"),
                (host_credentials::Target::Telnet, "telnet-credentials"),
            ] {
                push(
                    ManagementSource::Editor(id, source),
                    editor.auth(target).and_then(|auth| auth.error.clone()),
                );
            }
            push(ManagementSource::Editor(id, "save"), editor.error.clone());
        }
        for tab in self.terminals.iter() {
            push(
                ManagementSource::Snippet(tab.id),
                tab.tools
                    .snippet_editor
                    .as_ref()
                    .and_then(|editor| editor.error.clone()),
            );
        }
        if self.page == Page::Snippets {
            push(
                ManagementSource::Context("snippets"),
                self.snippet_state.error.clone(),
            );
        }
        feedback
    }

    pub fn new(
        store: Arc<Store>,
        recovery: Option<BeginRun>,
        recovery_error: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (mut data, storage_error) = match store.initial_state() {
            Ok(data) => (data, None),
            Err(e) => (AppState::default(), Some(e.to_string())),
        };
        crate::components::message::watch(window, cx, |view: &Self| {
            view.storage_error
                .clone()
                .map(crate::components::message::Message::error)
        });
        crate::components::message::watch(window, cx, |view: &Self| {
            view.notice
                .clone()
                .map(crate::components::message::Message::error)
        });
        crate::components::message::watch_many(window, cx, Self::management_feedback);
        data.dark = data.app_theme().dark(window.appearance());
        crate::localization::install(data.general.language, cx);
        crate::tags::materialize(&mut data);
        let tag_filter = cx.new(|cx| {
            crate::tags::picker::Picker::new(
                crate::tags::picker::Mode::Filter,
                data.tags.clone(),
                BTreeSet::new(),
                window,
                cx,
            )
        });
        crate::palette::apply_theme(data.dark, window, cx);
        cx.set_global(data.host_icons);
        crate::native::configure_terminal_defaults(&data.terminal, &data.terminal_preferences);
        let mut searches = HashMap::new();
        let mut subscriptions = vec![];
        for page in Page::SIDEBAR {
            let input = cx.new(|cx| {
                InputState::new(window, cx).placeholder(search_hint(page, data.general.language))
            });
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, _, event: &InputEvent, window, cx| {
                    if page == Page::Hosts && matches!(event, InputEvent::PressEnter { .. }) {
                        this.connect_search(window, cx);
                    } else {
                        cx.notify();
                    }
                },
            ));
            searches.insert(page, input);
        }
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let owner = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            owner
                .update(cx, |this, cx| {
                    this.request_exit(crate::settings::ExitAction::Window, window, cx);
                    false
                })
                .unwrap_or(true)
        });
        let previous = recovery
            .as_ref()
            .and_then(|recovery| recovery.previous.clone());
        let shell_history = crate::shell_history::store::Service::new(store.paths.shell_history());
        let overview = data.general.workspace_enabled;
        let mut this = Self {
            window_id: window.window_handle().window_id(),
            data,
            store,
            storage_loading: true,
            startup_task: None,
            storage_error,
            page: Page::Hosts,
            group: None,
            searches,
            tag_filter: tag_filter.clone(),
            selected_tags: BTreeSet::new(),
            rename_tag: None,
            subscriptions,
            focus,
            selection: BTreeSet::new(),
            selection_kind: None,
            card_navigation: Default::default(),
            editor: None,
            saved_drawers: HashMap::new(),
            drawer_motion: Default::default(),
            sidebar_width: layout::SIDEBAR_WIDTH,
            child_drawer_open: false,
            delete_pending: false,
            collection_views: HashMap::new(),
            notice: None,
            core: None,
            theme_refreshing: false,
            theme_refresh_pending: false,
            font_options: None,
            font_loading: None,
            terminals: Sessions::default(),
            overview,
            quick_connect: None,
            serial_connect: None,
            host_import: None,
            host_exporting: false,
            rename_workspace: None,
            next_tab: 1,
            bell_tabs: BTreeSet::new(),
            next_split: 1,
            split_bounds: HashMap::new(),
            pane_bounds: HashMap::new(),
            split_drag: None,
            close_pending: None,
            exiting: None,
            exit_task: None,
            menu_subscription: None,
            keychain: None,
            known_hosts: None,
            recordings: None,
            shell_history,
            history: None,
            history_events: None,
            logs: None,
            proxies: None,
            deleted_proxies: BTreeSet::new(),
            sftp: None,
            journal: recovery.map(|recovery| recovery.journal),
            recovery_pending: None,
            recovery_focus: cx.focus_handle(),
            recovery_always: false,
            recovery_error: None,
            last_snapshot: None,
            checkpoint_task: None,
            restore_task: None,
            terminal_tools_drag: None,
            workspace_assistants: HashMap::new(),
            workspace_assistant_drag: None,
            settings_window: None,
            pending_window_resize: None,
            forward_service: None,
            forward_states: HashMap::new(),
            forward_mutations: BTreeSet::new(),
            forward_delete_busy: false,
            next_forward_generation: 1,
            forward_events: None,
            host_os_requests: BTreeSet::new(),
            snippet_state: Default::default(),
            automatic_sync: Default::default(),
            secure_preload: None,
        };
        this.notice = recovery_error;
        this.install_shortcuts(window, cx);
        this.subscriptions
            .push(Self::tag_events(&tag_filter, window, cx));
        this.subscriptions
            .push(cx.observe_window_bounds(window, |this, window, cx| {
                if !window.is_fullscreen()
                    && let Some(size) = this.pending_window_resize.take()
                {
                    this.apply_window_size(size, window);
                }
                this.stage_recovery(window, cx);
                cx.notify();
            }));
        this.subscriptions
            .push(cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.terminal_tools_drag = None;
                    this.workspace_assistant_drag = None;
                } else {
                    this.refresh_theme_catalog(cx);
                }
                this.refresh_terminal_files(window, cx);
                cx.notify();
            }));
        this.subscriptions
            .push(cx.observe_window_appearance(window, |this, window, cx| {
                let dark = this.data.app_theme().dark(window.appearance());
                if this.data.dark != dark {
                    this.data.dark = dark;
                    this.sync_app_theme(window, cx);
                }
            }));
        this.load_initial_resources(previous, window, cx);
        this
    }

    fn active_terminal(&self) -> Option<u64> {
        self.terminals.selected(self.data.selected_workspace)
    }
    pub(crate) fn open_local(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_local_in(None, self.data.selected_workspace, false, window, cx);
    }
    fn open_local_in(
        &mut self,
        shell: Option<crate::shells::Shell>,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_local_terminal_in(shell, workspace, overview, window, cx);
    }
    fn open_local_terminal_in(
        &mut self,
        shell: Option<crate::shells::Shell>,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<Terminal>> {
        let label = shell
            .as_ref()
            .map(|shell| shell.label.clone())
            .unwrap_or_else(|| "Local terminal".into());
        let shell = shell.map(|shell| shell.path);
        let target = LocalTarget {
            label,
            appearance: self.data.terminal.clone(),
            shell: shell.clone(),
            directory: None,
            restore: RestoreTarget::Local {
                shell: shell.clone(),
            },
        };
        self.open_local_target(target, workspace, overview, window, cx)
    }
    fn open_local_host_in(
        &mut self,
        host: Host,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = match crate::shells::expand_directory(host.work_dir.as_deref()) {
            Ok(directory) => directory,
            Err(error) => {
                self.notice = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let target = LocalTarget {
            label: host.name,
            shell: host.shell_path,
            directory,
            appearance: self.data.terminal_for_host(Some(host.id)),
            restore: RestoreTarget::Host {
                id: host.id,
                port: None,
            },
        };
        self.open_local_target(target, workspace, overview, window, cx);
    }
    fn open_local_target(
        &mut self,
        target: LocalTarget,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<Terminal>> {
        if !self.data.workspaces.iter().any(|item| item.id == workspace) {
            return None;
        }
        let result = (|| {
            let core = self.core.clone().map(Ok).unwrap_or_else(Core::discover)?;
            self.core = Some(core.clone());
            if target.directory.is_none() && matches!(&target.restore, RestoreTarget::Local { .. })
            {
                match target.shell.as_deref() {
                    Some(shell) => core.local_shell(shell, target.appearance.palette()),
                    None => core.local(target.appearance.palette()),
                }
            } else {
                core.restore_local(
                    target.shell.as_deref(),
                    target.directory.as_deref(),
                    target.appearance.palette(),
                )
            }
        })();
        let mut session = match result {
            Ok(session) => session,
            Err(error) => {
                self.notice = Some(error.to_string());
                cx.notify();
                return None;
            }
        };
        let host_id = match &target.restore {
            RestoreTarget::Host { id, .. } => Some(*id),
            _ => None,
        };
        self.record_session(&mut session, &target.label, &target.appearance, host_id, cx);
        let terminal =
            cx.new(|cx| Terminal::new(session, vec![], target.appearance, None, window, cx));
        let metadata = self.terminal_subscriptions(&terminal, window, cx);
        let id = self.next_tab;
        self.next_tab += 1;
        self.terminals.push(TerminalTab {
            id,
            workspace,
            show_sftp: false,
            tools: Default::default(),
            tree: PaneTree::new_with_composer(
                id,
                SessionView {
                    id,
                    title: target.label,
                    content: TabContent::Terminal(terminal.clone()),
                    ssh: None,
                    restore_target: Some(target.restore),
                    restore_directory: None,
                    files: None,
                    tool_files: None,
                    _file_events: vec![],
                    replay: None,
                    _metadata: metadata,
                    preparation_generation: 0,
                },
                self.data.terminal.composer_enabled,
            ),
        });
        self.show_session(id, overview, window, cx);
        self.notice = None;
        cx.notify();
        Some(terminal)
    }
    fn open_ssh(&mut self, host: Host, window: &mut Window, cx: &mut Context<Self>) {
        self.open_ssh_in(host, self.data.selected_workspace, false, window, cx);
    }
    fn open_ssh_in(
        &mut self,
        host: Host,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if host.id > 0
            && host.is_remote()
            && [
                host.connections.ssh,
                host.connections.mosh,
                host.connections.telnet.is_some(),
            ]
            .into_iter()
            .filter(|value| *value)
            .count()
                > 1
        {
            self.open_protocol_choice(host, workspace, overview, window, cx);
            return;
        }
        if host.is_remote() && !host.connections.ssh {
            if host.connections.mosh {
                self.open_remote_in(host, workspace, overview, RemoteMode::Mosh, window, cx);
            } else if host.connections.telnet.is_some() {
                self.open_saved_telnet_in(host, workspace, overview, window, cx);
            } else {
                self.notice = Some("This Host has no enabled SSH or Telnet connection.".into());
                cx.notify();
            }
            return;
        }
        self.open_remote_in(host, workspace, overview, RemoteMode::Ssh, window, cx);
    }
    fn open_sftp_in(
        &mut self,
        host: Host,
        workspace: Id,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !host.supports_ssh() {
            self.notice = Some("SFTP requires a Host with SSH enabled.".into());
            cx.notify();
            return;
        }
        self.open_remote_in(host, workspace, false, RemoteMode::Sftp, window, cx);
    }
    fn open_remote_in(
        &mut self,
        host: Host,
        workspace: Id,
        overview: bool,
        mode: RemoteMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sftp_page = mode == RemoteMode::Sftp;
        if !host.is_remote() {
            if sftp_page {
                self.notice = Some("A local host is not an SSH/SFTP endpoint.".into());
                cx.notify();
            } else {
                self.open_local_host_in(host, workspace, overview, window, cx);
            }
            return;
        }
        if !(if mode == RemoteMode::Mosh {
            host.connections.mosh
        } else {
            host.supports_ssh()
        }) {
            self.notice = Some("The selected protocol is not enabled for this Host.".into());
            cx.notify();
            return;
        }
        if !self.data.workspaces.iter().any(|item| item.id == workspace) {
            return;
        }
        let core = match self
            .core
            .as_ref()
            .cloned()
            .map(Ok)
            .unwrap_or_else(Core::discover)
        {
            Ok(core) => core,
            Err(error) => {
                self.notice = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        self.core = Some(core.clone());
        let options = SshOptions::new(
            host.address,
            host.port,
            host.username,
            self.store.paths.known_hosts(),
        );
        let id = self.next_tab;
        self.next_tab += 1;
        self.terminals.push(TerminalTab {
            id,
            workspace,
            show_sftp: sftp_page,
            tools: Default::default(),
            tree: PaneTree::new_with_composer(
                id,
                SessionView {
                    id,
                    title: host.name,
                    content: TabContent::Preparing(None),
                    ssh: None,
                    restore_target: (host.id > 0).then_some(if mode == RemoteMode::Mosh {
                        RestoreTarget::MoshHost {
                            id: host.id,
                            port: None,
                            command: None,
                        }
                    } else {
                        RestoreTarget::Host {
                            id: host.id,
                            port: None,
                        }
                    }),
                    restore_directory: None,
                    files: None,
                    tool_files: None,
                    _file_events: vec![],
                    replay: None,
                    _metadata: vec![],
                    preparation_generation: 0,
                },
                self.data.terminal.composer_enabled,
            ),
        });
        self.show_session(id, overview, window, cx);
        self.notice = None;
        self.load_host_options(
            (id, workspace),
            core,
            options,
            (host.credential, host.proxy_id, host.ssh),
            window,
            cx,
        );
        cx.notify();
    }
    fn load_host_options(
        &mut self,
        target: (u64, Id),
        core: Arc<Core>,
        mut options: SshOptions,
        references: (
            Option<CredentialRef>,
            Option<i64>,
            crate::host_session::Setup,
        ),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = target.0;
        let (credential, proxy_id, setup) = references;
        let mosh = self.session(id).is_some_and(|session| {
            matches!(session.restore_target, Some(RestoreTarget::MoshHost { .. }))
        });
        let Some(session) = self
            .session_mut(id)
            .filter(|session| matches!(session.content, TabContent::Preparing(_)))
        else {
            return;
        };
        session.preparation_generation = session.preparation_generation.wrapping_add(1);
        let generation = session.preparation_generation;
        session._metadata.clear();
        session.content = TabContent::Preparing(None);
        let proxy_id = proxy_id.filter(|id| !self.deleted_proxies.contains(id));
        if proxy_id.is_some() {
            let proxies = self.ensure_proxies(cx);
            match proxies.read(cx).connection_readiness() {
                Ok(true) => {}
                Err(error) => {
                    self.session_mut(id).unwrap().content = TabContent::Preparing(Some(error));
                    cx.notify();
                    return;
                }
                Ok(false) => {
                    let subscription = cx.subscribe_in(
                        &proxies,
                        window,
                        move |this, proxies, _: &crate::proxies::Changed, window, cx| {
                            if !this.owns_preparation(target, generation) {
                                return;
                            }
                            let ready = proxies.read(cx).connection_readiness();
                            if matches!(ready, Ok(false)) {
                                return;
                            }
                            this.session_mut(id).unwrap()._metadata.clear();
                            match ready {
                                Ok(true) => this.load_host_options(
                                    target,
                                    core.clone(),
                                    options.clone(),
                                    (credential, proxy_id, setup.clone()),
                                    window,
                                    cx,
                                ),
                                Err(error) => {
                                    this.session_mut(id).unwrap().content =
                                        TabContent::Preparing(Some(error));
                                    cx.notify();
                                }
                                Ok(false) => unreachable!(),
                            }
                        },
                    );
                    self.session_mut(id).unwrap()._metadata.push(subscription);
                    cx.notify();
                    return;
                }
            }
        }
        options.encoding = setup.encoding;
        if mosh
            || credential.is_some()
            || proxy_id.is_some()
            || setup.environment_id.is_some()
            || setup.password_id.is_some()
            || setup.startup_snippet.is_some()
        {
            let keychain = self.ensure_keychain(cx);
            let Some(vault) = keychain.read(cx).vault() else {
                if let Some(tab) = self.session_mut(id) {
                    tab.content = TabContent::Preparing(Some("The saved connection settings cannot be loaded. Resolve the Keychain storage error and try again.".into()));
                }
                cx.notify();
                return;
            };
            let retry = proxy_id.map(|_| (options.clone(), setup.clone()));
            cx.spawn_in(window, async move |this, cx| {
                let result = async {
                    let options = vault
                        .apply_remote_connection(credential, proxy_id, setup.clone(), options, mosh)
                        .await?;
                    vault.apply_shell_setup(setup, options).await
                }
                .await;
                let _ = this.update_in(cx, |this, window, cx| {
                    // The placeholder owns its immutable workspace before async IO.
                    // Closing a workspace/tab removes it and cancels publication.
                    if !this.owns_preparation(target, generation) {
                        return;
                    }
                    let removed = proxy_id.is_some_and(|id| this.deleted_proxies.contains(&id));
                    match result {
                        Ok(mut options) => {
                            if removed {
                                options.proxy = None;
                            }
                            this.install_connection(id, core, options, window, cx);
                        }
                        Err(_) if removed => {
                            let (options, setup) =
                                retry.expect("removed proxy has a retry snapshot");
                            this.load_host_options(
                                target,
                                core,
                                options,
                                (credential, None, setup),
                                window,
                                cx,
                            );
                        }
                        Err(error) => {
                            if let Some(tab) = this.session_mut(id) {
                                tab.content = TabContent::Preparing(Some(error.to_string()));
                            }
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        } else {
            self.install_connection(id, core, options, window, cx);
        }
        cx.notify();
    }
    fn owns_preparation(&self, (id, workspace): (u64, Id), generation: u64) -> bool {
        self.exiting.is_none()
            && self
                .terminals
                .iter()
                .any(|tab| tab.owns_preparation((id, workspace), generation))
    }
    fn install_connection(
        &mut self,
        id: u64,
        core: Arc<Core>,
        options: SshOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exiting.is_some() {
            return;
        }
        let Some(tab) = self.session(id) else {
            return;
        };
        let label = tab.title.clone();
        let appearance = tab.appearance(&self.data);
        let host_id = tab.host_id();
        let mosh = match &tab.restore_target {
            Some(RestoreTarget::MoshHost {
                id: host_id,
                command,
                ..
            }) => {
                let config = self
                    .data
                    .hosts
                    .iter()
                    .find(|host| host.id == *host_id && host.connections.mosh)
                    .ok_or_else(|| anyhow::anyhow!("Mosh is no longer enabled for this Host."))
                    .and_then(|host| {
                        crate::mosh::Config::new(
                            command.as_deref().or(Some(&host.connections.mosh_command)),
                        )
                    });
                match config {
                    Ok(config) => Some(config),
                    Err(error) => {
                        self.session_mut(id).unwrap().content =
                            TabContent::Preparing(Some(error.to_string()));
                        cx.notify();
                        return;
                    }
                }
            }
            _ => None,
        };
        let recording = self.prepare_connection_recording(&label, &appearance, host_id, cx);
        let connection = cx.new(|cx| {
            let presentation = crate::connection::TerminalPresentation {
                title: label,
                dark: self.data.dark,
                palette: appearance.palette(),
                recording,
            };
            if let Some(config) = mosh {
                Connection::new_mosh_unfocused(core, options, config, presentation, window, cx)
            } else {
                Connection::new_unfocused(core, options, presentation, window, cx)
            }
        });
        self.watch_connection(id, connection, window, cx);
    }
    fn watch_connection(
        &mut self,
        id: u64,
        connection: Entity<Connection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = self.session(id).and_then(|session| match &session.content {
            TabContent::Connection(current) if current != &connection => {
                Some(current.read(cx).previous_attempt())
            }
            _ => None,
        });
        if let Some(previous) = previous {
            connection.update(cx, |view, _| view.inherit_previous_attempt(previous));
        }
        let host = self
            .session(id)
            .and_then(SessionView::host_id)
            .and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
            .cloned();
        if let Some(host) = &host {
            connection.update(cx, |view, cx| {
                view.set_host_platform(host.platform.clone(), cx)
            });
        }
        if connection.read(cx).uses_ssh_authentication() {
            let keychain = self.ensure_keychain(cx);
            let vault = keychain.read(cx).vault();
            connection.update(cx, |view, cx| {
                view.configure_authentication(
                    vault,
                    keychain.downgrade(),
                    host.as_ref()
                        .and_then(crate::vault::authentication::Binding::from_host),
                    cx,
                );
            });
        }
        let source = host.as_ref().and_then(|host| {
            host.uuid
                .clone()
                .filter(|uuid| crate::resource_reference::valid_uuid(uuid))
                .map(|uuid| (host.id, uuid))
        });
        connection.update(cx, |view, cx| view.allow_host_actions(source.is_some(), cx));
        let host_actions = source.map(|(host_id, uuid)| cx.subscribe_in(&connection, window,
            move |this, connection, action: &crate::connection::HostAction, window, cx| {
                if this.exiting.is_some() || !this.session(id).is_some_and(|session| matches!(&session.content, TabContent::Connection(current) if current == connection)) { return; }
                this.connection_host_action(id, (host_id, &uuid), *action, connection, window, cx);
            }));
        let events = cx.subscribe_in(
            &connection,
            window,
            move |this, connection, event: &ConnectionEvent, window, cx| match event {
                _ if this.exiting.is_some() => {}
                ConnectionEvent::Ready => {
                    if this.session(id).is_none() {
                        return;
                    }
                    let ssh = connection.read(cx).duplicate_options();
                    let Some((session, logs)) =
                        connection.update(cx, |view, _| view.take_session())
                    else {
                        return;
                    };
                    if ssh.is_some()
                        && let Some(directory) = this
                            .session(id)
                            .and_then(|session| session.restore_directory.as_deref())
                        && let Some(command) = crate::recovery::change_directory_command(directory)
                        && let Err(error) = session.send(command.as_bytes())
                    {
                        this.notice = Some(format!("Could not restore the SSH directory: {error}"));
                    }
                    let appearance = this
                        .session(id)
                        .expect("checked session")
                        .appearance(&this.data);
                    let terminal = cx.new(|cx| {
                        Terminal::new(session, logs, appearance, ssh.clone(), window, cx)
                    });
                    if this.session_auto_focus(id) {
                        terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
                    }
                    let metadata = this.terminal_subscriptions(&terminal, window, cx);
                    if let Some(tab) = this.session_mut(id) {
                        tab.content = TabContent::Terminal(terminal);
                        tab.ssh = ssh.map(Box::new);
                        tab._metadata = metadata;
                    }
                    this.detect_terminal_platform(id, cx);
                    cx.notify();
                }
                ConnectionEvent::Close => this.confirm_close(CloseTarget::Session(id), window, cx),
            },
        );
        let status = cx.observe(&connection, |_, _, cx| cx.notify());
        if let Some(tab) = self.session_mut(id) {
            tab.content = TabContent::Connection(connection.clone());
            tab._metadata = vec![events, status];
            tab._metadata.extend(host_actions);
        }
        if self.session_auto_focus(id) {
            connection.update(cx, |connection, cx| connection.focus(window, cx));
        }
        cx.notify();
    }

    fn search_target(&self, cx: &App) -> Option<Host> {
        let query = self
            .searches
            .get(&Page::Hosts)?
            .read(cx)
            .value()
            .to_string();
        if let Some(host) = self
            .data
            .hosts
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(query.trim()))
        {
            return Some(host.clone());
        }
        let address = crate::ssh_address::SshAddress::parse(&query).ok()?;
        Some(Host {
            connections: Default::default(),
            uuid: None,
            platform: Default::default(),
            kind: Default::default(),
            shell_path: None,
            work_dir: None,
            ssh: Default::default(),
            id: 0,
            name: address.host.clone(),
            address: address.host,
            port: address.port,
            username: address.username,
            group_id: None,
            tags: vec![],
            credential: None,
            proxy_id: None,
            theme: None,
        })
    }

    fn connect_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(host) = self.search_target(cx) {
            self.open_ssh(host, window, cx);
        }
    }

    fn confirm_close(&mut self, target: CloseTarget, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            CloseTarget::Quit | CloseTarget::Window | CloseTarget::Restart => {
                self.begin_exit(target, window, cx);
                return;
            }
            CloseTarget::Workspace(id) => self.confirm_workspace_close(id, window, cx),
            CloseTarget::Session(id) => self.close_pane_session(id, window, cx),
            CloseTarget::Tab(id) => {
                let active = self.active_terminal() == Some(id);
                self.terminals.remove(id);
                if active {
                    if self.active_terminal().is_none() {
                        self.overview = false;
                        if self.page == Page::Sessions {
                            self.navigate(Page::Hosts, window, cx);
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
                }
            }
        }
        self.close_pending = None;
        cx.notify();
    }
    fn has_open_sessions(&self, cx: &App) -> bool {
        self.active_forward_count() > 0
            || self
                .terminals
                .iter()
                .any(|tab| tab.tree.sessions().any(|session| session.replay.is_none()))
            || self
                .sftp
                .as_ref()
                .is_some_and(|view| view.read(cx).has_request(cx))
            || self
                .keychain
                .as_ref()
                .is_some_and(|view| view.read(cx).has_connection(cx))
    }

    fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        let page = if page == Page::Sessions && !self.data.general.workspace_enabled {
            self.overview = false;
            if self.active_terminal().is_some() {
                Page::Sessions
            } else {
                Page::Hosts
            }
        } else {
            page
        };
        if page == Page::Hosts {
            self.refresh_theme_catalog(cx);
        }
        if page == Page::Sftp && !self.data.sftp.show_tab {
            return;
        }
        self.switch_drawer_page(page);
        if matches!(page, Page::Hosts | Page::PortForwards | Page::Logs)
            && self.resources_ready()
            && let Err(error) = self.store.refresh_resources(&mut self.data)
        {
            self.notice = Some(error.to_string());
        }
        if page == Page::Snippets {
            if !self.snippet_state.busy {
                self.snippet_state.ready = false;
            }
            self.load_snippets(cx);
        }
        if page == Page::Logs {
            if let Some(view) = &self.logs {
                view.update(cx, |view, cx| view.refresh(cx));
            } else if let Some(service) = self.ensure_recordings(cx) {
                let view = cx.new(|cx| {
                    crate::components::message::watch_window_many(
                        self.window_id,
                        cx,
                        crate::recordings::Logs::feedback,
                    );
                    crate::recordings::Logs::new(
                        service,
                        self.searches[&Page::Logs].clone(),
                        self.data.dark,
                        cx,
                    )
                });
                self.subscriptions.push(cx.subscribe_in(
                    &view,
                    window,
                    |this, _, event: &crate::recordings::ReplayRequested, window, cx| {
                        this.open_recording_replay(event.0.clone(), window, cx);
                    },
                ));
                self.logs = Some(view);
            }
        }
        if page == Page::Sftp {
            self.ensure_sftp(window, cx);
        }
        if page == Page::Proxies {
            if let Some(view) = &self.proxies {
                view.update(cx, |view, cx| view.refresh(cx));
            } else {
                self.ensure_proxies(cx);
            }
        }
        if page == Page::KnownHosts {
            if let Some(view) = &self.known_hosts {
                view.update(cx, |view, cx| view.refresh(cx));
            } else {
                let view = cx.new(|cx| {
                    crate::components::message::watch_window_many(
                        self.window_id,
                        cx,
                        crate::known_hosts::KnownHosts::feedback,
                    );
                    crate::known_hosts::KnownHosts::new(
                        self.store.paths.known_hosts(),
                        self.searches[&Page::KnownHosts].clone(),
                        self.data.dark,
                        cx,
                    )
                });
                self.subscriptions.push(cx.subscribe(
                    &view,
                    |this, _, event: &crate::known_hosts::ConvertToHost, cx| {
                        if this
                            .data
                            .hosts
                            .iter()
                            .any(|host| host.address == event.0 && host.port == event.1)
                        {
                            this.notice =
                                Some("A saved host already uses this address and port.".into());
                        } else {
                            let mut candidate = this.data.clone();
                            let id = candidate.allocate_id();
                            candidate.hosts.push(Host {
                                connections: Default::default(),
                                uuid: None,
                                platform: Default::default(),
                                kind: Default::default(),
                                shell_path: None,
                                work_dir: None,
                                ssh: Default::default(),
                                id,
                                name: event.0.clone(),
                                address: event.0.clone(),
                                port: event.1,
                                username: String::new(),
                                group_id: None,
                                tags: vec![],
                                credential: None,
                                proxy_id: None,
                                theme: None,
                            });
                            let result = this.storage_error.as_ref().map_or_else(
                                || this.store.save(&mut candidate),
                                |error| Err(anyhow::anyhow!(error.clone())),
                            );
                            match result {
                                Ok(()) => {
                                    this.data = candidate;
                                    this.notice =
                                        Some("Known host converted to a saved host.".into());
                                }
                                Err(error) => {
                                    this.notice = Some(format!("Could not save host: {error}"))
                                }
                            }
                        }
                        cx.notify();
                    },
                ));
                self.known_hosts = Some(view);
            }
        }
        if page == Page::Keychain {
            self.ensure_keychain(cx);
        }
        if let Some(sftp) = &self.sftp {
            sftp.update(cx, |view, cx| {
                view.set_visible(page == Page::Sftp && self.file_pages_visible(), window, cx)
            });
        }
        self.focus.focus(window, cx);
        if page == Page::Sftp
            && let Some(sftp) = &self.sftp
        {
            sftp.update(cx, |view, cx| view.focus(window, cx));
        }
        if page == Page::Sessions
            && !self.overview
            && let Some(tab) = self
                .terminals
                .iter()
                .find(|t| Some(t.id) == self.active_terminal())
        {
            tab.focus(window, cx);
        }
        cx.notify();
    }

    fn ensure_sftp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sftp.is_some() {
            return;
        }
        let keychain = self.ensure_keychain(cx);
        let vault = keychain.read(cx).vault();
        let sftp = cx.new(|cx| {
            crate::sftp::Sftp::new(
                self.data.clone(),
                self.store.paths.known_hosts(),
                vault,
                window,
                cx,
            )
            .with_keychain(keychain.downgrade(), cx)
        });
        self.subscriptions.push(cx.subscribe(
            &sftp,
            |this, _, event: &crate::sftp::RemoteConnected, cx| {
                this.detect_sftp_platform(event, cx);
            },
        ));
        self.sftp = Some(sftp);
    }

    fn ensure_keychain(&mut self, cx: &mut Context<Self>) -> Entity<crate::keychain::Keychain> {
        if let Some(view) = &self.keychain {
            view.update(cx, |view, cx| view.set_resources(self.data.clone(), cx));
            return view.clone();
        }
        let search = self.searches[&Page::Keychain].clone();
        let known_hosts_path = self.store.paths.known_hosts();
        let database_path = self.store.database_path();
        let view = cx.new(|cx| {
            crate::components::message::watch_window_many(
                self.window_id,
                cx,
                crate::keychain::Keychain::feedback,
            );
            crate::keychain::Keychain::new(
                known_hosts_path,
                database_path,
                search,
                self.data.dark,
                cx,
            )
        });
        view.update(cx, |view, cx| view.set_resources(self.data.clone(), cx));
        self.subscriptions.push(cx.observe(&view, |this, view, cx| {
            if this.page == Page::Keychain && this.child_drawer_open != view.read(cx).drawer_open()
            {
                cx.notify();
            }
        }));
        self.subscriptions.push(cx.subscribe(
            &view,
            |this, _, message: &crate::keychain::Message, cx| {
                crate::components::message::show_for_window(this.window_id, message.0.clone(), cx);
            },
        ));
        self.subscriptions.push(cx.subscribe(
            &view,
            |this, view, _: &crate::keychain::Changed, cx| {
                let inventory = view.read(cx).inventory();
                if this.resources_ready()
                    && !this.editor.as_ref().is_some_and(|editor| {
                        matches!(
                            editor.kind,
                            ItemKind::Host | ItemKind::Group | ItemKind::Forward
                        )
                    })
                    && let Err(error) = this.store.refresh_resources(&mut this.data)
                {
                    this.notice = Some(error.to_string());
                }
                if let Some(proxies) = &this.proxies {
                    proxies.update(cx, |view, cx| view.set_inventory(inventory, cx));
                }
                cx.notify();
            },
        ));
        self.keychain = Some(view.clone());
        view
    }

    fn new_kind(&self) -> Option<ItemKind> {
        match self.page {
            Page::Hosts => Some(ItemKind::Host),
            Page::Snippets => Some(ItemKind::Snippet),
            Page::PortForwards => Some(ItemKind::Forward),
            _ => None,
        }
    }

    fn collection_view(&self) -> crate::management::CollectionView {
        self.collection_views
            .get(&self.page)
            .copied()
            .unwrap_or_default()
    }

    fn select_cards(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        let query = self
            .searches
            .get(&self.page)
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_default();
        let matches = |text: &str| text.to_lowercase().contains(&query.to_lowercase());
        let tag_filter = self.active_tag_filter();
        self.selection = match (self.page, self.selection_kind) {
            (Page::Hosts, Some(ItemKind::Host)) => self
                .data
                .hosts
                .iter()
                .filter(|h| {
                    (self.group.is_none() || h.group_id == self.group)
                        && self.data.host_matches(h, &query)
                        && tag_filter.matches(h)
                })
                .map(|h| h.id)
                .collect(),
            (Page::Hosts, Some(ItemKind::Group)) => self
                .data
                .groups
                .iter()
                .filter(|g| g.parent == self.group && matches(&g.name))
                .map(|g| g.id)
                .collect(),
            (Page::Snippets, Some(ItemKind::Snippet)) => self
                .data
                .snippets
                .iter()
                .filter(|s| {
                    s.matches_query(&query)
                        && self
                            .snippet_state
                            .package
                            .is_none_or(|id| s.package_id == Some(id))
                })
                .map(|s| s.id)
                .collect(),
            (Page::Snippets, Some(ItemKind::SnippetPackage)) => self
                .snippet_state
                .catalog
                .packages
                .iter()
                .filter(|p| self.snippet_state.package.is_none() && matches(&p.name))
                .map(|p| p.id)
                .collect(),
            (Page::Proxies, Some(ItemKind::Proxy)) => self
                .data
                .proxies
                .iter()
                .filter(|p| matches(&format!("{} {}:{}", p.name, p.address, p.port)))
                .map(|p| p.id)
                .collect(),
            (Page::PortForwards, Some(ItemKind::Forward)) => self
                .data
                .forwards
                .iter()
                .filter(|f| self.forward_matches(f, &query))
                .map(|f| f.id)
                .collect(),
            _ => return,
        };
        cx.notify();
    }

    fn open_editor(
        &mut self,
        kind: ItemKind,
        id: Option<Id>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_editor_in(kind, id, self.group, self.snippet_state.package, window, cx);
    }
    fn open_editor_in(
        &mut self,
        kind: ItemKind,
        id: Option<Id>,
        initial_group: Option<Id>,
        initial_package: Option<Id>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.resources_ready() {
            return;
        }
        if kind == ItemKind::Group
            && id.is_none()
            && initial_group.is_some_and(|parent| !self.data.group_parent_allowed(None, parent))
        {
            self.notice = Some(GROUP_DEPTH_ERROR.into());
            cx.notify();
            return;
        }

        if kind == ItemKind::Forward && id.is_some_and(|id| self.forward_mutations.contains(&id)) {
            return;
        }
        if matches!(kind, ItemKind::Snippet | ItemKind::SnippetPackage)
            && (!self.snippet_state.ready || self.snippet_state.busy)
        {
            self.load_snippets(cx);
            return;
        }
        self.retire_history();
        if matches!(kind, ItemKind::Host | ItemKind::Group) {
            self.ensure_keychain(cx);
            self.ensure_proxies(cx);
            self.load_snippets(cx);
        }
        let credential = if kind == ItemKind::Host {
            id.and_then(|id| self.data.hosts.iter().find(|h| h.id == id))
                .and_then(|h| h.credential)
        } else if kind == ItemKind::Group {
            id.and_then(|id| self.data.groups.iter().find(|group| group.id == id))
                .and_then(|group| group.profile())
                .and_then(|profile| profile.credential)
        } else {
            None
        };
        let mut group = initial_group;
        let mut proxy_id = None;
        let values: Vec<(&'static str, String)> = match kind {
            ItemKind::Host => {
                let host = id.and_then(|id| self.data.hosts.iter().find(|h| h.id == id));
                if let Some(host) = host {
                    group = host.group_id;
                    proxy_id = host.proxy_id;
                }
                vec![
                    ("Name", host.map(|h| h.name.clone()).unwrap_or_default()),
                    (
                        "Address",
                        host.map(|h| h.address.clone()).unwrap_or_default(),
                    ),
                    (
                        "SSH Port",
                        host.map(|h| h.port.to_string()).unwrap_or("22".into()),
                    ),
                    (
                        "Username",
                        host.map(|h| h.username.clone()).unwrap_or_default(),
                    ),
                    ("Tags", host.map(|h| h.tags.join(", ")).unwrap_or_default()),
                ]
            }
            ItemKind::Group => {
                let existing = id.and_then(|id| self.data.groups.iter().find(|g| g.id == id));
                if let Some(existing) = existing {
                    group = existing.parent;
                    proxy_id = existing.proxy_id;
                }
                let profile = existing.and_then(Group::profile).unwrap_or_default();
                vec![
                    ("Name", existing.map(|g| g.name.clone()).unwrap_or_default()),
                    ("Address", String::new()),
                    ("SSH Port", profile.port.to_string()),
                    ("Username", profile.username),
                    ("Tags", String::new()),
                ]
            }
            ItemKind::Snippet => {
                let item = id.and_then(|id| self.data.snippets.iter().find(|s| s.id == id));
                vec![
                    ("Name", item.map(|s| s.name.clone()).unwrap_or_default()),
                    (
                        "Command",
                        item.map(|s| s.command.clone()).unwrap_or_default(),
                    ),
                    (
                        "Description",
                        item.map(|s| s.description.clone()).unwrap_or_default(),
                    ),
                ]
            }
            ItemKind::SnippetPackage => vec![(
                "Name",
                id.and_then(|id| {
                    self.snippet_state
                        .catalog
                        .packages
                        .iter()
                        .find(|p| p.id == id)
                })
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            )],
            ItemKind::Proxy => {
                let item = id.and_then(|id| self.data.proxies.iter().find(|s| s.id == id));
                vec![
                    ("Name", item.map(|s| s.name.clone()).unwrap_or_default()),
                    (
                        "Address",
                        item.map(|s| s.address.clone()).unwrap_or_default(),
                    ),
                    (
                        "Port",
                        item.map(|s| s.port.to_string()).unwrap_or("1080".into()),
                    ),
                ]
            }
            ItemKind::Forward => {
                let item = id.and_then(|id| self.data.forwards.iter().find(|s| s.id == id));
                let destination =
                    item.and_then(|f| crate::port_forward::parse_destination(&f.destination).ok());
                vec![
                    ("Name", item.map(|s| s.name.clone()).unwrap_or_default()),
                    (
                        "Bind Address",
                        item.map(|s| s.bind_address.clone())
                            .unwrap_or("127.0.0.1".into()),
                    ),
                    (
                        "Bind Port",
                        item.map(|s| s.local_port.to_string()).unwrap_or_default(),
                    ),
                    (
                        "Destination Address",
                        destination
                            .as_ref()
                            .map(|(host, _)| host.clone())
                            .unwrap_or_default(),
                    ),
                    (
                        "Destination Port",
                        destination
                            .map(|(_, port)| port.to_string())
                            .unwrap_or_default(),
                    ),
                ]
            }
        };
        if matches!(kind, ItemKind::Host | ItemKind::Group) && id.is_none() && proxy_id.is_none() {
            proxy_id = group
                .and_then(|id| self.data.groups.iter().find(|group| group.id == id))
                .and_then(|group| group.proxy_id);
        }
        let fields: Vec<(&'static str, Entity<InputState>)> = values
            .into_iter()
            .map(|(label, value)| {
                (
                    label,
                    cx.new(|cx| InputState::new(window, cx).default_value(value)),
                )
            })
            .collect();
        if let Some(id) = id {
            self.selection = BTreeSet::from([id]);
            self.selection_kind = Some(kind);
        } else {
            self.selection.clear();
            self.selection_kind = None;
        }
        let selects = matches!(kind, ItemKind::Host | ItemKind::Group).then(|| {
            let inventory = self
                .keychain
                .as_ref()
                .map(|view| view.read(cx).inventory())
                .unwrap_or_default();
            let groups = SelectModel::new(
                group_options(&self.data, |parent| {
                    kind != ItemKind::Group || self.data.group_parent_allowed(id, parent)
                }),
                group,
                window,
                cx,
            );
            let auth = host_credentials::Authentication::new(
                credential,
                &fields[3].1,
                &inventory,
                window,
                cx,
            );
            let proxies = SelectModel::new(self.proxy_options(cx), proxy_id, window, cx);
            let group_event = cx.subscribe_in(
                &groups.state,
                window,
                |this, state, event: &SelectEvent<Id>, window, cx| {
                    if let Some(editor) = &mut this.editor
                        && editor
                            .selects
                            .as_ref()
                            .is_some_and(|selects| selects.groups.state == *state)
                    {
                        let SelectEvent::Confirm(value) = event;
                        if value
                            .flatten()
                            .is_some_and(|parent| !editor.parent_group_allowed(&this.data, parent))
                        {
                            editor.validation_error = Some(GROUP_DEPTH_ERROR.into());
                            cx.notify();
                            return;
                        }
                        editor.group = value.flatten();
                        this.inherit_editor_group(window, cx);
                        cx.notify();
                    }
                },
            );
            let group_create = cx.subscribe_in(
                &groups.state,
                window,
                |this, state, event: &crate::components::SelectCreate, window, cx| {
                    if event.action == "group"
                        && this
                            .editor
                            .as_ref()
                            .and_then(|editor| editor.selects.as_ref())
                            .is_some_and(|selects| selects.groups.state == *state)
                    {
                        let focus = state.read(cx).focus_handle(cx);
                        this.open_related_resource(
                            related::Target::ParentGroup,
                            &event.text,
                            focus,
                            window,
                            cx,
                        );
                    }
                },
            );
            let proxy_event = cx.subscribe_in(
                &proxies.state,
                window,
                |this, state, event: &SelectEvent<i64>, _, cx| {
                    if let Some(editor) = &mut this.editor
                        && editor
                            .selects
                            .as_ref()
                            .is_some_and(|selects| selects.proxies.state == *state)
                    {
                        let SelectEvent::Confirm(value) = event;
                        editor.proxy_id = value.flatten();
                        cx.notify();
                    }
                },
            );
            let proxy_create = cx.subscribe_in(
                &proxies.state,
                window,
                |this, state, event: &crate::components::SelectCreate, window, cx| {
                    if event.action == "proxy"
                        && this
                            .editor
                            .as_ref()
                            .and_then(|editor| editor.selects.as_ref())
                            .is_some_and(|selects| selects.proxies.state == *state)
                    {
                        this.open_related_proxy(&event.text, window, cx);
                    }
                },
            );
            HostSelects {
                groups,
                auth,
                proxies,
                _events: vec![group_event, group_create, proxy_event, proxy_create],
            }
        });
        let forward =
            (kind == ItemKind::Forward).then(|| self.forward_selects(id, &fields, window, cx));
        let snippet = (kind == ItemKind::Snippet).then(|| {
            snippets::SnippetDraft::new(
                id.and_then(|id| self.data.snippets.iter().find(|s| s.id == id)),
                &self.snippet_state.catalog.packages,
                initial_package,
                window,
                cx,
            )
        });
        let theme = matches!(kind, ItemKind::Host | ItemKind::Group).then(|| {
            host_themes::Draft::new(
                if kind == ItemKind::Host {
                    id.and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
                        .and_then(|host| host.theme)
                } else {
                    id.and_then(|id| self.data.groups.iter().find(|group| group.id == id))
                        .and_then(Group::profile)
                        .and_then(|profile| profile.theme)
                },
                window,
                cx,
            )
        });
        let host = matches!(kind, ItemKind::Host | ItemKind::Group).then(|| {
            let group = (kind == ItemKind::Group)
                .then(|| id.and_then(|id| self.data.groups.iter().find(|group| group.id == id)))
                .flatten();
            let group_host = group.map(Group::editor_host);
            host_config::Draft::new(
                if kind == ItemKind::Host {
                    id.and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
                } else {
                    group_host.as_ref()
                },
                &self.data.snippets,
                &self
                    .keychain
                    .as_ref()
                    .map(|view| view.read(cx).inventory())
                    .unwrap_or_default(),
                window,
                cx,
            )
        });
        let tags = (kind == ItemKind::Host).then(|| {
            self.new_host_tags(
                id.and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
                    .map(|host| host.tags.as_slice())
                    .unwrap_or_default(),
                window,
                cx,
            )
        });
        self.editor = Some(Editor {
            scroll: ScrollHandle::new(),
            kind,
            id,
            group,
            credential,
            proxy_id,
            fields,
            error: None,
            validation_error: None,
            selects,
            forward,
            snippet,
            theme,
            host,
            tags,
            related_credential: None,
            parent: None,
            related_proxy: None,
        });
        if matches!(kind, ItemKind::Host | ItemKind::Group) {
            let editor = self.editor.as_mut().unwrap();
            let draft = editor.host.as_mut().unwrap();
            if kind == ItemKind::Group {
                draft.ssh_enabled = id
                    .and_then(|id| self.data.groups.iter().find(|group| group.id == id))
                    .and_then(Group::profile)
                    .is_some();
                draft.inheritance.port_configured = draft.ssh_enabled;
                draft.inheritance.encoding_configured = draft.ssh_enabled;
                draft.ssh_configured = id.is_some();
            }
            draft.watch_port(&editor.fields[2].1, cx);
            if id.is_none() {
                self.inherit_editor_group(window, cx);
            }
            self.load_editor_password(window, cx);
        }
        if kind == ItemKind::Host {
            let automatic = host_name::Draft::new(self.editor.as_ref().unwrap(), cx);
            self.editor
                .as_mut()
                .unwrap()
                .host
                .as_mut()
                .unwrap()
                .automatic_name = Some(automatic);
            self.refresh_host_name(false, window, cx);
        }
        if let Some((_, field)) = self.editor.as_ref().and_then(|editor| {
            let remote = editor.kind == ItemKind::Host
                && editor
                    .host
                    .as_ref()
                    .is_some_and(|host| host.kind == HostKind::Remote);
            editor.fields.get(usize::from(remote))
        }) {
            field.update(cx, |field, cx| field.focus(window, cx));
        }
        cx.notify();
    }

    fn save_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.editor {
            editor.validation_error = None;
        }
        self.refresh_host_name(true, window, cx);
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.kind == ItemKind::SnippetPackage)
        {
            self.save_snippet_package(window, cx);
            return;
        }
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.kind == ItemKind::Snippet)
        {
            self.save_snippet_editor(window, cx);
            return;
        }
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.kind == ItemKind::Forward)
        {
            self.save_forward_editor(window, cx);
            return;
        }
        let Some(editor) = &self.editor else {
            return;
        };
        if editor.host.as_ref().is_some_and(|draft| draft.busy) {
            return;
        }
        if editor.kind == ItemKind::Group
            && editor
                .group
                .is_some_and(|parent| !editor.parent_group_allowed(&self.data, parent))
        {
            self.editor.as_mut().unwrap().validation_error = Some(GROUP_DEPTH_ERROR.into());
            cx.notify();
            return;
        }
        if editor
            .host
            .as_ref()
            .is_some_and(|draft| draft.kind == HostKind::Remote && draft.ssh_enabled)
            && let Some(error) = editor
                .selects
                .as_ref()
                .and_then(|selects| selects.auth.validate(cx).err())
        {
            self.editor.as_mut().unwrap().validation_error = Some(error.to_string());
            cx.notify();
            return;
        }
        let values: Vec<String> = editor
            .fields
            .iter()
            .map(|(_, field)| field.read(cx).value().to_string().trim().to_string())
            .collect();
        let local = editor
            .host
            .as_ref()
            .is_some_and(|draft| draft.kind == HostKind::Local);
        let group_without_ssh = matches!(editor.kind, ItemKind::Host | ItemKind::Group)
            && editor.host.as_ref().is_some_and(|draft| !draft.ssh_enabled);
        let error = if values[0].is_empty() {
            Some("Name is required.")
        } else if matches!(
            editor.kind,
            ItemKind::Host | ItemKind::Proxy | ItemKind::Forward
        ) && !local
            && values[1].is_empty()
        {
            Some("Address is required.")
        } else if (matches!(editor.kind, ItemKind::Proxy | ItemKind::Forward)
            || (matches!(editor.kind, ItemKind::Host | ItemKind::Group) && !group_without_ssh))
            && !local
            && values[2].parse::<u16>().ok().filter(|p| *p > 0).is_none()
        {
            Some("Port must be between 1 and 65535.")
        } else if editor.kind == ItemKind::Snippet && values[1].is_empty() {
            Some("Command is required.")
        } else if let Some(proxy_id) = editor.proxy_id.filter(|_| !local && !group_without_ssh) {
            if self
                .proxies
                .as_ref()
                .is_none_or(|view| !view.read(cx).available())
            {
                Some(
                    "Proxy storage is not ready. Open Proxies to resolve its loading or storage error.",
                )
            } else if !self
                .proxies
                .as_ref()
                .unwrap()
                .read(cx)
                .summaries()
                .iter()
                .any(|proxy| proxy.id == proxy_id)
            {
                Some("The selected proxy is no longer available. Choose another proxy or None.")
            } else {
                None
            }
        } else {
            None
        };
        if let Some(error) = error {
            self.editor.as_mut().unwrap().validation_error = Some(error.into());
            cx.notify();
            return;
        }
        if let Some(error) = editor
            .host
            .as_ref()
            .and_then(|draft| draft.validate(cx).err())
        {
            self.editor.as_mut().unwrap().validation_error = Some(error.to_string());
            cx.notify();
            return;
        }
        let (host_kind, shell_path, work_dir) = editor
            .host
            .as_ref()
            .map(|draft| draft.values(cx))
            .unwrap_or((HostKind::Remote, None, None));
        let (kind, existing, group, credential, proxy_id, theme) = (
            editor.kind,
            editor.id,
            editor.group,
            editor.credential,
            editor.proxy_id,
            editor.theme.as_ref().and_then(|draft| draft.selected),
        );
        let previous = self.data.clone();
        let id = existing.unwrap_or_else(|| {
            if matches!(kind, ItemKind::Host | ItemKind::Group) {
                0
            } else {
                self.data.allocate_id()
            }
        });
        match kind {
            ItemKind::Host => {
                let host = Host {
                    platform: existing
                        .and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
                        .map(|host| host.platform.clone())
                        .unwrap_or_default(),
                    uuid: existing
                        .and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
                        .and_then(|host| host.uuid.clone()),
                    kind: host_kind,
                    shell_path,
                    work_dir,
                    ssh: if local {
                        Default::default()
                    } else {
                        self.editor
                            .as_ref()
                            .unwrap()
                            .host
                            .as_ref()
                            .unwrap()
                            .setup
                            .clone()
                    },
                    id,
                    name: values[0].clone(),
                    address: if local {
                        String::new()
                    } else {
                        values[1].clone()
                    },
                    port: if local || group_without_ssh {
                        22
                    } else {
                        values[2].parse().unwrap()
                    },
                    username: if local || matches!(credential, Some(CredentialRef::Identity(_))) {
                        String::new()
                    } else {
                        values[3].clone()
                    },
                    group_id: group,
                    tags: self
                        .editor
                        .as_ref()
                        .and_then(|editor| editor.tags.as_ref())
                        .map(|draft| {
                            crate::tags::names(&self.data.tags, draft.view.read(cx).selected())
                        })
                        .unwrap_or_default(),
                    credential: credential.filter(|_| !local),
                    proxy_id: proxy_id.filter(|_| !local),
                    theme,
                    connections: {
                        let draft = self.editor.as_ref().unwrap().host.as_ref().unwrap();
                        ConnectionProfiles {
                            ssh: !local && draft.ssh_enabled,
                            mosh: !local && draft.ssh_enabled && draft.mosh.enabled,
                            mosh_command: if !local && draft.ssh_enabled {
                                draft.mosh.value(cx).unwrap().unwrap_or_default().command
                            } else {
                                crate::mosh::DEFAULT_COMMAND.into()
                            },
                            telnet: (!local).then(|| draft.telnet.value(cx)).flatten(),
                        }
                    },
                };
                self.save_host_candidate(host, window, cx);
                return;
            }
            ItemKind::Group => {
                let draft = self.editor.as_ref().unwrap().host.as_ref().unwrap();
                let profile = draft.ssh_enabled.then(|| crate::host_session::Profile {
                    port: values[2].parse().unwrap(),
                    username: if matches!(credential, Some(CredentialRef::Identity(_))) {
                        String::new()
                    } else {
                        values[3].clone()
                    },
                    credential,
                    theme,
                    setup: draft.setup.clone(),
                });
                self.save_group_candidate(
                    Group {
                        id,
                        uuid: existing
                            .and_then(|id| self.data.groups.iter().find(|group| group.id == id))
                            .and_then(|group| group.uuid.clone()),
                        name: values[0].clone(),
                        parent: group,
                        proxy_id: if profile.is_some()
                            || existing
                                .and_then(|id| self.data.groups.iter().find(|group| group.id == id))
                                .is_some_and(|group| group.ssh.is_none())
                        {
                            proxy_id
                        } else {
                            None
                        },
                        ssh: profile,
                        telnet: draft.telnet.value(cx),
                        mosh: if draft.ssh_enabled {
                            draft.mosh.value(cx).unwrap()
                        } else {
                            None
                        },
                    },
                    window,
                    cx,
                );
                return;
            }
            ItemKind::Snippet | ItemKind::SnippetPackage => {
                unreachable!("snippets use their multiline candidate save path");
            }
            ItemKind::Proxy => {
                self.data.proxies.retain(|p| p.id != id);
                self.data.proxies.push(Proxy {
                    id,
                    name: values[0].clone(),
                    address: values[1].clone(),
                    port: values[2].parse().unwrap(),
                });
            }
            ItemKind::Forward => {
                unreachable!("forwarding rules use the validated save path above");
            }
        }
        if let Err(error) = self.storage_error.as_ref().map_or_else(
            || self.store.save(&mut self.data),
            |error| Err(anyhow::anyhow!(error.clone())),
        ) {
            self.data = previous;
            self.editor.as_mut().unwrap().error = Some(format!("Could not save: {error}"));
            cx.notify();
            return;
        }
        self.retire_editor();
        self.focus.focus(window, cx);
        self.selection = BTreeSet::from([id]);
        self.selection_kind = Some(kind);
        cx.notify();
    }

    fn topbar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::new(self.data.dark);
        let session_page = self.page == Page::Sessions && !self.overview;
        let terminal_sftp_page = self
            .terminals
            .iter()
            .find(|tab| Some(tab.id) == self.active_terminal())
            .is_some_and(TerminalTab::files_visible);
        let terminal_page = session_page && !terminal_sftp_page;
        let connection_page = terminal_page
            && self
                .terminals
                .iter()
                .find(|tab| Some(tab.id) == self.active_terminal())
                .is_some_and(|tab| {
                    matches!(
                        tab.active().content,
                        TabContent::Connection(_) | TabContent::Preparing(_)
                    )
                });
        let terminal_palette = self
            .terminals
            .iter()
            .find(|tab| Some(tab.id) == self.active_terminal())
            .map(|tab| tab.active().appearance(&self.data).palette())
            .unwrap_or_else(|| self.data.terminal.palette());
        let bar_color = if connection_page {
            palette.background
        } else if terminal_page {
            rgb(terminal_palette.base.background)
        } else {
            palette.titlebar
        };
        let foreground = if connection_page {
            palette.muted
        } else if terminal_page {
            rgb(terminal_palette.base.foreground)
        } else {
            if self.data.dark {
                palette.muted
            } else {
                rgb(0xc6ccda)
            }
        };
        let selected_foreground = if connection_page {
            palette.text
        } else if !terminal_page {
            if self.data.dark {
                palette.text
            } else {
                rgb(0xe9edf4)
            }
        } else {
            foreground
        };
        let mix = crate::components::motion::mix;
        let active_color = if terminal_page {
            mix(
                bar_color,
                selected_foreground,
                if connection_page { 0.09 } else { 0.13 },
            )
        } else if self.data.dark {
            rgb(0x3a3a3a)
        } else {
            rgb(0x4a5065)
        };
        let inactive_color = if terminal_page {
            mix(
                bar_color,
                selected_foreground,
                if connection_page { 0.035 } else { 0.06 },
            )
        } else if self.data.dark {
            rgb(0x303030)
        } else {
            mix(bar_color, rgb(0x40465b), 0.72)
        };
        let vault_selected = !matches!(self.page, Page::Sftp | Page::Sessions);
        let painted_bar =
            crate::components::motion::animated_color("topbar-background", bar_color, window, cx);
        let control_foreground = crate::components::motion::animated_color(
            "topbar-controls-foreground",
            if terminal_page {
                selected_foreground
            } else if self.data.dark {
                palette.muted
            } else {
                rgb(0xaeb6c8)
            },
            window,
            cx,
        );
        let plain_style = ButtonCustomVariant::new(cx)
            .foreground(control_foreground.into())
            .hover(active_color.into());
        div()
            .id("topbar")
            .h(px(44.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .pl(px(if cfg!(target_os = "macos") { 78. } else { 8. }))
            .bg(painted_bar)
            .text_color(foreground)
            .on_mouse_down(MouseButton::Left, |event, window, cx| {
                // Buttons consume their own presses. All remaining chrome,
                // including gaps between tabs, shares native drag/zoom behavior.
                if event.click_count == 2 {
                    window.zoom_window();
                } else {
                    window.start_window_move();
                }
                cx.stop_propagation();
            })
            .child(
                div()
                    .id("topbar-pinned-tabs")
                    .flex_shrink_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .role(Role::TabList)
                    .child(
                        Tab::new(
                            "vault-tab",
                            crate::localization::text(cx, "Vaults"),
                            IconName::Shield,
                        )
                        .collapsed(!vault_selected)
                        .selected(vault_selected)
                        .colors(
                            if vault_selected {
                                active_color
                            } else {
                                inactive_color
                            },
                            if vault_selected {
                                selected_foreground
                            } else {
                                foreground
                            },
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.navigate(Page::Hosts, window, cx)
                            }),
                        ),
                    )
                    .when(self.data.sftp.show_tab, |view| {
                        view.child(
                            Tab::new("sftp-tab", "SFTP", IconName::Folder)
                                .collapsed(self.page != Page::Sftp)
                                .selected(self.page == Page::Sftp)
                                .colors(
                                    if self.page == Page::Sftp {
                                        active_color
                                    } else {
                                        inactive_color
                                    },
                                    if self.page == Page::Sftp {
                                        selected_foreground
                                    } else {
                                        foreground
                                    },
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate(Page::Sftp, window, cx)
                                })),
                        )
                    })
                    .when(self.data.general.workspace_enabled, |view| {
                        view.child(
                            Tab::new(
                                "workspace-tab",
                                self.data.workspace().name.clone(),
                                IconName::LayoutGrid,
                            )
                            .selected(self.page == Page::Sessions && self.overview)
                            .colors(
                                if self.page == Page::Sessions && self.overview {
                                    active_color
                                } else {
                                    inactive_color
                                },
                                if self.page == Page::Sessions && self.overview {
                                    selected_foreground
                                } else {
                                    foreground
                                },
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.overview = true;
                                    this.navigate(Page::Sessions, window, cx)
                                },
                            )),
                        )
                    }),
            )
            .child(
                div()
                    .id("topbar-session-tabs")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_x_scroll()
                    .scrollbars_hidden()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .role(Role::TabList)
                    .children(
                        self.terminals
                            .in_workspace(self.data.selected_workspace)
                            .map(|tab| {
                                let id = tab.id;
                                let title = tab.title(cx);
                                let selected = self.page == Page::Sessions
                                    && !self.overview
                                    && self.active_terminal() == Some(id);
                                Tab::new(
                                    SharedString::from(format!("terminal-tab-{id}")),
                                    title,
                                    if tab.show_sftp {
                                        IconName::Folder
                                    } else {
                                        IconName::Terminal
                                    },
                                )
                                .width(if selected { 220. } else { 142. })
                                .selected(selected)
                                .bell_indicator(
                                    self.data.terminal.bell.tab_indicator
                                        && self.bell_tabs.contains(&id)
                                        && !selected,
                                )
                                .colors(
                                    if selected {
                                        active_color
                                    } else {
                                        inactive_color
                                    },
                                    if selected {
                                        selected_foreground
                                    } else {
                                        foreground
                                    },
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.show_session(id, false, window, cx);
                                }))
                                .on_close(cx.listener(
                                    move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.request_tab_close(id, window, cx);
                                    },
                                ))
                            }),
                    )
                    .child(
                        Button::new("new-terminal-tab")
                            .ink_ripple(false)
                            .toolbar()
                            .flex_shrink_0()
                            .custom(plain_style)
                            .icon(crate::assets::icon(IconName::Plus))
                            .tooltip("Quick Connect")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_quick_connect(false, window, cx)
                            })),
                    )
                    .child(div().flex_1().h_full()),
            )
            .child(
                Button::new("toggle-ai-assistant")
                    .toolbar()
                    .custom(if self.assistant_open() {
                        plain_style.foreground(self.assistant_accent().into())
                    } else {
                        plain_style
                    })
                    .icon(IconName::Sparkles)
                    .tooltip(if self.assistant_open() {
                        "Hide AI Assistant"
                    } else {
                        "Show AI Assistant"
                    })
                    .disabled(!self.assistant_available())
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_assistant(window, cx))),
            )
            .when(session_page, |view| {
                view.when(
                    !terminal_sftp_page
                        && !self
                            .terminals
                            .iter()
                            .find(|tab| Some(tab.id) == self.active_terminal())
                            .is_some_and(|tab| tab.active().replay.is_some()),
                    |view| {
                        view.child(
                            Button::new("toggle-terminal-tools")
                                .toolbar()
                                .custom(if self.terminal_tools_open() {
                                    plain_style.foreground(rgb(terminal_palette.accent).into())
                                } else {
                                    plain_style
                                })
                                .icon(IconName::PanelRight)
                                .tooltip(if self.terminal_tools_open() {
                                    "Hide terminal tools"
                                } else {
                                    "Show terminal tools"
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_terminal_tools(window, cx)
                                })),
                        )
                    },
                )
            })
    }

    fn sidebar(&self, collapsed: bool, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.data.dark);
        let reveal = ((width - layout::COLLAPSED_SIDEBAR_WIDTH)
            / (layout::SIDEBAR_WIDTH - layout::COLLAPSED_SIDEBAR_WIDTH))
            .clamp(0., 1.);
        let label = |text: &'static str| {
            div()
                .w(px(114.))
                .flex_shrink_0()
                .whitespace_nowrap()
                .line_height(px(20.))
                .overflow_hidden()
                .opacity(reveal)
                .child(crate::localization::text_for(
                    self.data.general.language,
                    text,
                ))
        };
        let custom = ButtonCustomVariant::new(cx).foreground(p.text.into());
        let navigation_style = |selected| {
            let colors = Palette::sidebar_button(self.data.dark, selected);
            custom
                .color(colors.background.into())
                .hover(colors.hover.into())
                .highlight(colors.highlight.into())
                .active(colors.splash.into())
        };
        div()
            .w(px(width))
            .overflow_hidden()
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(p.sidebar)
            .border_r_1()
            .border_color(p.border)
            .px(px(10.))
            .pt(px(8.))
            .children(Page::SIDEBAR.into_iter().map(|page| {
                Button::new(SharedString::from(format!("nav-{page:?}")))
                    .large()
                    .w_full()
                    .content_start()
                    .icon_size(px(16.))
                    .content_gap(px(20.))
                    .mb(px(6.))
                    .px(px(10.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .rounded(px(7.))
                    .custom(navigation_style(self.page == page))
                    .icon(page_icon(page))
                    .accessibility_label(crate::localization::text_for(
                        self.data.general.language,
                        page.label(),
                    ))
                    .child(label(page.label()))
                    .when(collapsed, |button| {
                        button.tooltip(crate::localization::text_for(
                            self.data.general.language,
                            page.label(),
                        ))
                    })
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.navigate(page, window, cx)),
                    )
            }))
            .child(div().flex_1())
            .child(
                Button::new("settings-nav")
                    .large()
                    .w_full()
                    .content_start()
                    .icon_size(px(16.))
                    .content_gap(px(20.))
                    .mb(px(10.))
                    .px(px(10.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .rounded(px(7.))
                    .custom(navigation_style(false))
                    .icon(IconName::Settings)
                    .accessibility_label(crate::localization::text(cx, "Settings"))
                    .child(label("Settings"))
                    .when(collapsed, |button| {
                        button.tooltip(crate::localization::text(cx, "Settings"))
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
            )
    }

    fn toolbar(&self, width: f32, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = self.new_kind();
        let preferences = self.collection_view();
        let page = self.page;
        let creation_group = matches!(page, Page::Hosts | Page::Snippets | Page::PortForwards);
        let primary_label = kind.map(|kind| {
            if kind == ItemKind::Forward {
                "New forwarding".to_owned()
            } else {
                format!("New {}", kind.label().to_lowercase())
            }
        });
        let mut labels = Vec::new();
        if let Some(label) = primary_label.as_deref() {
            labels.push((label, true));
        }
        if page == Page::Hosts {
            labels.extend([("Terminal", true), ("Serial", true)]);
        } else if page == Page::Snippets {
            labels.push(("Shell History", true));
        }
        let search_open = page != Page::Hosts
            && (preferences.search_open || !self.searches[&page].read(cx).value().is_empty());
        let icons = 1
            + usize::from(kind.is_some())
            + usize::from(page == Page::Hosts)
            + usize::from(page != Page::PortForwards)
            + usize::from(page != Page::Hosts && !search_open);
        let collapse_labels = crate::components::management::toolbar_labels_collapsed(
            width,
            &labels,
            icons,
            search_open,
            window,
        );
        let sort_owner = cx.weak_entity();
        crate::components::management::toolbar(self.data.dark)
            .when_some(kind, |view, kind| {
                let primary = Button::new("new-item")
                    .toolbar()
                    .icon(crate::assets::icon(IconName::Plus))
                    .label(primary_label.clone().unwrap_or_default())
                    .label_collapsed(collapse_labels && !creation_group)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if kind == ItemKind::Forward {
                            this.create_forward(crate::port_forward::Kind::Local, window, cx);
                        } else {
                            this.open_editor(kind, None, window, cx);
                        }
                    }));
                if creation_group {
                    let owner = cx.weak_entity();
                    view.child(
                        crate::components::ButtonGroup::new()
                            .button(primary)
                            .dropdown(
                                Button::new("new-item-options")
                                    .icon(IconName::ChevronDown)
                                    .tooltip("More creation actions"),
                                move |menu, _, cx| {
                                    if page == Page::PortForwards {
                                        let mut menu = menu.width(170.);
                                        for kind in crate::port_forward::Kind::ALL {
                                            let owner = owner.clone();
                                            menu = menu.item(
                                                MenuItem::localized(kind.label())
                                                    .icon(match kind {
                                                        crate::port_forward::Kind::Local => {
                                                            IconName::GitBranch
                                                        }
                                                        crate::port_forward::Kind::Remote => {
                                                            IconName::ArrowLeftRight
                                                        }
                                                        crate::port_forward::Kind::Dynamic => {
                                                            IconName::Network
                                                        }
                                                    })
                                                    .on_click(move |_, window, cx| {
                                                        let _ = owner.update(cx, |this, cx| {
                                                            this.create_forward(kind, window, cx)
                                                        });
                                                    }),
                                            );
                                        }
                                        return menu;
                                    }
                                    let create_owner = owner.clone();
                                    let group = page == Page::Hosts;
                                    let menu =
                                        menu.width(205.).item(
                                            MenuItem::localized(if group {
                                                "New group"
                                            } else {
                                                "New snippet package"
                                            })
                                            .icon(if group {
                                                crate::assets::GROUP_ICON
                                            } else {
                                                IconName::Package
                                            })
                                            .on_click(move |_, window, cx| {
                                                let _ = create_owner.update(cx, |this, cx| {
                                                    this.open_editor(
                                                        if group {
                                                            ItemKind::Group
                                                        } else {
                                                            ItemKind::SnippetPackage
                                                        },
                                                        None,
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            }),
                                        );
                                    if !group {
                                        return menu;
                                    }
                                    let (import_disabled, export_disabled) = owner
                                        .upgrade()
                                        .map(|entity| {
                                            let this = entity.read(cx);
                                            (
                                                !this.resources_ready()
                                                    || this.host_import.is_some(),
                                                this.host_exporting
                                                    || !this.data.hosts.iter().any(Host::is_remote),
                                            )
                                        })
                                        .unwrap_or((true, true));
                                    let import_owner = owner.clone();
                                    let export_owner = owner.clone();
                                    menu.separator()
                                        .item(
                                            MenuItem::localized("Import…")
                                                .icon(IconName::Upload)
                                                .disabled(import_disabled)
                                                .on_click(move |_, window, cx| {
                                                    let _ = import_owner.update(cx, |this, cx| {
                                                        this.open_host_import(window, cx)
                                                    });
                                                }),
                                        )
                                        .item(
                                            MenuItem::localized("rs.workspace.action.exportCsv")
                                                .icon(IconName::Download)
                                                .disabled(export_disabled)
                                                .on_click(move |_, window, cx| {
                                                    let _ = export_owner.update(cx, |this, cx| {
                                                        this.export_host_csv(window, cx)
                                                    });
                                                }),
                                        )
                                },
                            ),
                    )
                } else {
                    view.child(primary)
                }
            })
            .when(self.page == Page::Hosts, |view| {
                view.child(
                    Button::new("new-local")
                        .toolbar()
                        .ghost()
                        .icon(IconName::Terminal)
                        .label("Terminal")
                        .label_collapsed(collapse_labels)
                        .on_click(cx.listener(|this, _, window, cx| this.open_local(window, cx))),
                )
                .child(
                    Button::new("new-serial")
                        .toolbar()
                        .ghost()
                        .icon(IconName::Cable)
                        .label("Serial")
                        .label_collapsed(collapse_labels)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_serial_dialog(window, cx)),
                        ),
                )
            })
            .when(self.page == Page::Snippets, |view| {
                view.child(
                    Button::new("shell-history")
                        .toolbar()
                        .ghost()
                        .icon(IconName::Clock)
                        .label("Shell History")
                        .label_collapsed(collapse_labels)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_shell_history(window, cx)),
                        ),
                )
            })
            .child(div().flex_1())
            .when(
                page == Page::Snippets && self.snippet_state.error.is_some(),
                |view| {
                    view.child(
                        Button::new("reload-snippets")
                            .toolbar()
                            .ghost()
                            .icon(IconName::RefreshCw)
                            .tooltip("Reload snippets")
                            .disabled(self.snippet_state.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.load_snippets(cx))),
                    )
                },
            )
            .when(self.page == Page::Hosts, |view| {
                view.child(self.tag_filter.clone())
            })
            .child(
                Button::new("sort-order")
                    .toolbar()
                    .ghost()
                    .icon(preferences.sort.icon())
                    .tooltip(preferences.sort.label())
                    .dropdown_menu(move |menu, _, _| {
                        let owner = sort_owner.clone();
                        crate::management::sort_menu(menu, preferences.sort, move |order, _, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.collection_views.entry(page).or_default().sort = order;
                                cx.notify();
                            });
                        })
                    }),
            )
            .when(self.page != Page::Hosts, |view| {
                let owner = cx.weak_entity();
                view.child(crate::components::management::toolbar_search(
                    "collection-search",
                    &self.searches[&page],
                    preferences.search_open,
                    move |open, window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.collection_views.entry(page).or_default().search_open = open;
                            if open {
                                this.searches[&page]
                                    .update(cx, |input, cx| input.focus(window, cx));
                            } else {
                                this.focus.focus(window, cx);
                            }
                            cx.notify();
                        });
                    },
                    cx,
                ))
            })
            .when(page != Page::PortForwards, |view| {
                view.child(
                    Button::new("view-mode")
                        .toolbar()
                        .ghost()
                        .icon(if preferences.list {
                            IconName::LayoutGrid
                        } else {
                            IconName::List
                        })
                        .tooltip("Switch list/grid view")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let preferences = this.collection_views.entry(this.page).or_default();
                            preferences.list = !preferences.list;
                            cx.notify();
                        })),
                )
            })
    }

    fn card(
        &self,
        kind: ItemKind,
        id: Id,
        title: String,
        detail: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = (self.selection_kind == Some(kind) && self.selection.contains(&id))
            || self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.edited() == Some((kind, id)));
        let can_edit = self.selection_kind != Some(kind)
            || !self.selection.contains(&id)
            || self.selection.len() <= 1;
        let owner = cx.entity().downgrade();
        let remote = kind == ItemKind::Host
            && self
                .data
                .hosts
                .iter()
                .any(|host| host.id == id && host.is_remote());
        let avatar = if kind == ItemKind::Group {
            crate::avatar::group().into_any_element()
        } else if kind == ItemKind::Host && !remote {
            div()
                .size(px(40.))
                .flex_shrink_0()
                .rounded(px(10.))
                .bg(rgb(0x075e92))
                .text_color(rgb(0xffffff))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(IconName::Terminal))
                .into_any_element()
        } else if kind == ItemKind::Host {
            crate::avatar::host(&title)
                .platform(
                    self.data
                        .hosts
                        .iter()
                        .find(|host| host.id == id)
                        .map(|host| host.platform.clone())
                        .unwrap_or_default(),
                )
                .into_any_element()
        } else {
            div()
                .size(px(40.))
                .flex_shrink_0()
                .rounded(px(10.))
                .bg(rgb(0x096b9e))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0xffffff))
                .child(icon(item_icon(kind)))
                .into_any_element()
        };
        crate::components::item_card(
            SharedString::from(format!("card-{kind:?}-{id}")),
            self.data.dark,
            selected,
            self.collection_view().list,
        )
        .list_menu_hint(self.collection_view().list)
        .accessibility_label(title.clone())
        .reveal_on_layout(self.card_navigation.take_reveal((kind, id)))
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            cx.stop_propagation();
            this.focus.focus(window, cx);
            if kind == ItemKind::SnippetPackage
                && this.editor.is_none()
                && !event.modifiers().platform
                && !event.modifiers().control
                && !event.modifiers().shift
            {
                this.open_snippet_package(Some(id), window, cx);
                return;
            }
            if kind == ItemKind::Group && event.click_count() == 2 {
                this.enter_group(Some(id), window, cx);
                return;
            }
            if kind == ItemKind::Host && event.click_count() == 2 {
                if let Some(host) = this.data.hosts.iter().find(|h| h.id == id).cloned() {
                    this.open_ssh(host, window, cx);
                }
                return;
            }
            if kind == ItemKind::Snippet && event.click_count() == 2 {
                this.run_snippet_locally(id, window, cx);
                return;
            }
            if this.selection_kind == Some(kind)
                && this.selection.contains(&id)
                && this.selection.len() > 1
                && !event.modifiers().platform
                && !event.modifiers().control
                && !event.modifiers().shift
            {
                return;
            }
            this.select_collection_card(kind, id, event.modifiers(), window, cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, _, _, cx| {
                if this.selection_kind != Some(kind) || !this.selection.contains(&id) {
                    this.selection = BTreeSet::from([id]);
                    this.selection_kind = Some(kind);
                }
                cx.notify();
            }),
        )
        .child(avatar)
        .child(crate::components::card_text(title, detail, self.data.dark))
        .when(can_edit, |card| {
            card.hover_action(
                crate::components::card_edit_button(
                    SharedString::from(format!("edit-{kind:?}-{id}")),
                    self.data.dark,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_editor(kind, Some(id), window, cx);
                })),
            )
        })
        .context_menu(move |menu, _, menu_cx| {
            let entity = menu_cx.entity();
            owner
                .update(menu_cx, |this, cx| {
                    this.resource_card_menu(menu, kind, id, &entity, cx)
                })
                .unwrap_or_default()
        })
        .into_any_element()
    }

    fn collection(&self, content_width: f32, cx: &mut Context<Self>) -> AnyElement {
        if self.storage_loading {
            let p = Palette::new(self.data.dark);
            let headings: &[&str] = match self.page {
                Page::Hosts => &["Groups", "Hosts"],
                Page::Snippets => &["Packages", "Snippets"],
                Page::PortForwards => &["Port Forwarding"],
                _ => &[],
            };
            return div()
                .flex_1()
                .min_h_0()
                .map(crate::components::management::collection_padding)
                .flex()
                .flex_col()
                .gap(px(28.))
                .children(headings.iter().map(|heading| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .child(crate::components::management::section_title(heading, cx))
                        .child(
                            div()
                                .text_color(p.muted)
                                .child(crate::components::Spinner::new()),
                        )
                }))
                .into_any_element();
        }
        let tag_filter = self.active_tag_filter();
        if self.page == Page::Snippets {
            return self.snippet_collection(content_width, cx);
        }
        if self.page == Page::PortForwards {
            return self.forward_collection(cx);
        }
        let p = Palette::new(self.data.dark);
        let credentials = self
            .keychain
            .as_ref()
            .map(|view| view.read(cx).inventory())
            .unwrap_or_default();
        let query = self
            .searches
            .get(&self.page)
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_default();
        let visible_groups = self
            .data
            .groups
            .iter()
            .filter(|g| {
                g.parent == self.group && g.name.to_lowercase().contains(&query.to_lowercase())
            })
            .count();
        let visible_hosts = self
            .data
            .hosts
            .iter()
            .filter(|h| {
                (self.group.is_none() || h.group_id == self.group)
                    && self.data.host_matches(h, &query)
                    && tag_filter.matches(h)
            })
            .count();
        let layout_count = match self.page {
            Page::Hosts => visible_groups.max(visible_hosts),
            Page::Proxies => self.data.proxies.len(),
            Page::Snippets => self.data.snippets.len(),
            Page::PortForwards => self.data.forwards.len(),
            _ => 0,
        };
        let preferences = self.collection_view();
        let grid = layout::CardGrid::new(content_width - 52., layout_count, preferences.list);
        let mut groups = crate::components::card_grid(grid);
        let mut cards = crate::components::card_grid(grid);
        let mut count = 0;
        if self.page == Page::Hosts {
            let mut visible: Vec<_> = self
                .data
                .groups
                .iter()
                .filter(|g| {
                    g.parent == self.group && g.name.to_lowercase().contains(&query.to_lowercase())
                })
                .collect();
            visible.sort_by(|a, b| preferences.sort.compare((&a.name, a.id), (&b.name, b.id)));
            for group in visible {
                let host_count = self
                    .data
                    .hosts
                    .iter()
                    .filter(|h| h.group_id == Some(group.id))
                    .count();
                groups = groups.child(self.card(
                    ItemKind::Group,
                    group.id,
                    group.name.clone(),
                    format!("{host_count} Hosts"),
                    cx,
                ));
            }
            let mut hosts: Vec<_> = self
                .data
                .hosts
                .iter()
                .filter(|h| {
                    (self.group.is_none() || h.group_id == self.group)
                        && self.data.host_matches(h, &query)
                        && tag_filter.matches(h)
                })
                .collect();
            hosts.sort_by(|a, b| preferences.sort.compare((&a.name, a.id), (&b.name, b.id)));
            count = hosts.len();
            for host in hosts {
                let (fallback, identity) = if !host.connections.ssh {
                    host.connections
                        .telnet
                        .as_ref()
                        .map(|profile| (profile.username.as_str(), profile.identity_id))
                        .unwrap_or((host.username.as_str(), None))
                } else {
                    (
                        host.username.as_str(),
                        match host.credential {
                            Some(CredentialRef::Identity(id)) => Some(id),
                            _ => None,
                        },
                    )
                };
                let username = identity
                    .and_then(|id| credentials.identities.iter().find(|item| item.id == id))
                    .map(|identity| identity.username.as_str())
                    .filter(|name| !name.is_empty())
                    .unwrap_or(fallback);
                cards = cards.child(self.card(
                    ItemKind::Host,
                    host.id,
                    host.name.clone(),
                    if !host.is_remote() {
                        "local".into()
                    } else if username.is_empty() {
                        host.protocol_summary()
                    } else {
                        format!("{}, {username}", host.protocol_summary())
                    },
                    cx,
                ));
            }
        } else {
            let mut entries: Vec<(ItemKind, Id, String, String)> = match self.page {
                Page::Snippets => self
                    .data
                    .snippets
                    .iter()
                    .filter(|s| s.matches_query(&query))
                    .map(|s| (ItemKind::Snippet, s.id, s.name.clone(), s.command.clone()))
                    .collect(),
                Page::PortForwards => self
                    .data
                    .forwards
                    .iter()
                    .map(|f| {
                        (
                            ItemKind::Forward,
                            f.id,
                            f.name.clone(),
                            format!("{} → {}", f.local_port, f.destination),
                        )
                    })
                    .collect(),
                Page::Proxies => self
                    .data
                    .proxies
                    .iter()
                    .map(|p| {
                        (
                            ItemKind::Proxy,
                            p.id,
                            p.name.clone(),
                            format!("{}:{}", p.address, p.port),
                        )
                    })
                    .collect(),
                _ => vec![],
            };
            entries.retain(|(_, _, name, detail)| {
                if self.page == Page::Snippets {
                    return true;
                }
                format!("{name} {detail}")
                    .to_lowercase()
                    .contains(&query.to_lowercase())
            });
            entries.sort_by(|a, b| preferences.sort.compare((&a.2, a.1), (&b.2, b.1)));
            count += entries.len();
            for (kind, id, name, detail) in entries {
                cards = cards.child(self.card(kind, id, name, detail, cx));
            }
        }
        if count == 0 && visible_groups == 0 && self.group.is_none() {
            let filtered = !query.is_empty()
                || (tag_filter.is_active()
                    && (!self.data.hosts.is_empty() || !self.data.groups.is_empty()));
            let empty = crate::components::EmptyState::new(
                IconName::Server,
                if !filtered {
                    "No hosts yet"
                } else {
                    "No hosts found"
                },
                self.data.dark,
            )
            .description(if !filtered {
                "Save your hosts to connect to them quickly from any workspace."
            } else {
                "Try another search or clear tag filters."
            })
            .on_click(cx.listener(|this, _, window, cx| {
                this.retire_editor();
                this.selection.clear();
                this.selection_kind = None;
                this.focus.focus(window, cx);
                cx.notify();
            }));
            return if !filtered {
                empty
                    .action(
                        Button::new("empty-add-host")
                            .primary()
                            .icon(crate::assets::icon(IconName::Plus))
                            .label("Add host")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_editor(ItemKind::Host, None, window, cx)
                            })),
                    )
                    .into_any_element()
            } else {
                empty.into_any_element()
            };
        }
        div()
            .id("collection-scroll")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.selection.clear();
                this.selection_kind = None;
                this.retire_editor();
                this.focus.focus(window, cx);
                cx.notify();
            }))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .scrollbars()
            .map(crate::components::management::collection_padding)
            .flex()
            .flex_col()
            .gap(px(18.))
            .when(self.page == Page::Hosts && self.group.is_some(), |view| {
                let entries = std::iter::once((None, crate::localization::text(cx, "Hosts")))
                    .chain(
                        self.group
                            .into_iter()
                            .flat_map(|id| self.data.group_path(id))
                            .map(|group| (Some(group.id), group.name.clone().into())),
                    );
                view.child(Breadcrumb::new("host-breadcrumb", entries, p).on_select(
                    cx.listener(|this, group, window, cx| this.enter_group(*group, window, cx)),
                ))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(crate::components::management::COLLECTION_SECTION_GAP))
                    .flex_shrink_0()
                    .when(self.page == Page::Hosts && visible_groups > 0, |view| {
                        view.child(
                            crate::components::management::collection_section("Groups", cx)
                                .child(groups),
                        )
                    })
                    .when(count > 0, |view| {
                        view.child(
                            crate::components::management::collection_section(
                                self.page.label(),
                                cx,
                            )
                            .child(cards),
                        )
                    })
                    .when(
                        count == 0 && (self.page != Page::Hosts || visible_groups == 0),
                        |view| {
                            view.child(
                                div()
                                    .pt(px(64.))
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(12.))
                                    .text_color(p.muted)
                                    .child(icon(page_icon(self.page)))
                                    .child(crate::localization::view(format!(
                                        "No {}",
                                        crate::localization::text(cx, self.page.label())
                                    )))
                                    .child(div().text_size(px(12.)).child(
                                        crate::localization::view(if query.is_empty() {
                                            "Create an item to get started."
                                        } else {
                                            "Try another search."
                                        }),
                                    )),
                            )
                        },
                    ),
            )
            .into_any_element()
    }

    fn editor_field(&self, editor: &Editor, index: usize, label: &str) -> AnyElement {
        let error = editor
            .validation_error
            .as_deref()
            .filter(|error| match index {
                0 => error.starts_with("Name is required"),
                1 => {
                    error.starts_with("Address is required")
                        || error.starts_with("Command is required")
                }
                2 => error.starts_with("Port must be"),
                _ => false,
            })
            .map(|error| SharedString::from(error.to_owned()));
        Field::input(&editor.fields[index].1, label.to_owned())
            .disabled(
                (matches!(editor.kind, ItemKind::Snippet | ItemKind::SnippetPackage)
                    && self.snippet_state.busy)
                    || editor.host.as_ref().is_some_and(|draft| {
                        draft.busy
                            || (draft.kind == HostKind::Remote
                                && draft.telnet.enabled
                                && draft.telnet.auth.blocked())
                    }),
            )
            .error(error)
            .into_any_element()
    }

    fn editor_section(&self, title: &str, fields: Vec<AnyElement>) -> AnyElement {
        let p = Palette::new(self.data.dark);
        div()
            .rounded(px(10.))
            .bg(p.surface)
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .font_weight(FontWeights::MEDIUM)
                    .child(title.to_owned()),
            )
            .children(fields)
            .into_any_element()
    }

    fn drawer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if let Some(related) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.related_credential.as_ref())
        {
            let view = related.view.clone();
            view.update(cx, |view, cx| view.set_dark(self.data.dark, cx));
            return view.into_any_element();
        }
        if let Some(related) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.related_proxy.as_ref())
        {
            let view = related.view.clone();
            let inventory = self
                .keychain
                .as_ref()
                .map(|keychain| keychain.read(cx).inventory());
            view.update(cx, |view, cx| {
                view.set_dark(self.data.dark, cx);
                if let Some(inventory) = inventory {
                    view.set_inventory(inventory, cx);
                }
            });
            return view.into_any_element();
        }
        let proxy_options = self.proxy_options(cx);
        let groups = group_options(&self.data, |parent| {
            self.editor
                .as_ref()
                .is_none_or(|editor| editor.parent_group_allowed(&self.data, parent))
        });
        if let Some(editor) = &mut self.editor
            && let Some(selects) = &mut editor.selects
        {
            let inventory = self
                .keychain
                .as_ref()
                .map(|view| view.read(cx).inventory())
                .unwrap_or_default();
            selects.groups.reconcile(groups, editor.group, window, cx);
            selects
                .auth
                .reconcile(&inventory, editor.credential, window, cx);
            selects
                .proxies
                .reconcile(proxy_options, editor.proxy_id, window, cx);
        }
        if let Some(draft) = self.editor.as_mut().and_then(|editor| editor.host.as_mut()) {
            draft.reconcile(&self.data.snippets, window, cx);
            let inventory = self
                .keychain
                .as_ref()
                .map(|view| view.read(cx).inventory())
                .unwrap_or_default();
            draft.telnet.reconcile(&inventory, window, cx);
        }
        if let Some(forward) = self
            .editor
            .as_mut()
            .and_then(|editor| editor.forward.as_mut())
        {
            forward.reconcile(&self.data.hosts, window, cx);
        }
        let Some(editor) = &self.editor else {
            return div().into_any_element();
        };
        if editor
            .host
            .as_ref()
            .is_some_and(|draft| draft.environment_page.is_some())
        {
            return self.host_environment_page(cx);
        }
        if editor.theme_gallery_target().is_some() {
            return self.host_theme_gallery(cx);
        }
        let sections = if editor.kind == ItemKind::Host {
            self.host_editor_sections(editor, cx)
        } else if editor.kind == ItemKind::Group {
            self.group_editor_sections(editor, cx)
        } else if let Some(forward) = &editor.forward {
            self.forward_editor_sections(editor, forward, cx)
        } else if let Some(snippet) = &editor.snippet {
            self.snippet_editor_sections(editor, snippet, cx)
        } else {
            vec![
                self.editor_section(
                    if editor.kind == ItemKind::SnippetPackage {
                        "Package"
                    } else {
                        "General"
                    },
                    editor
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(i, (label, _))| self.editor_field(editor, i, label))
                        .collect(),
                ),
            ]
        };
        crate::components::DrawerFrame::new(
            "resource-editor",
            if editor.kind == ItemKind::Forward {
                if editor.id.is_some() {
                    "Edit Forward".to_owned()
                } else {
                    "New forwarding".to_owned()
                }
            } else if editor.kind == ItemKind::SnippetPackage {
                if editor.id.is_some() {
                    "Rename Snippet Package"
                } else {
                    "New Snippet Package"
                }
                .to_owned()
            } else {
                format!(
                    "{} {}",
                    if editor.id.is_some() { "Edit" } else { "New" },
                    editor.kind.label()
                )
            },
            self.data.dark,
        )
        .when(editor.parent.is_some(), |frame| {
            frame.leading(
                Button::new("back-related-resource")
                    .toolbar()
                    .ghost()
                    .icon(IconName::ArrowLeft)
                    .tooltip("Back")
                    .disabled(self.resource_editor_busy())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.back_related_resource(window, cx);
                    })),
            )
        })
        .actions(self.host_editor_actions(editor, cx))
        .scroll_handle(&editor.scroll)
        .actions(self.forward_editor_actions(editor, cx))
        .actions(self.snippet_editor_actions(editor, cx))
        .when(
            editor
                .host
                .as_ref()
                .is_some_and(|draft| draft.inheritance_error.is_some()),
            |frame| {
                frame.action(
                    Button::new("retry-group-inheritance")
                        .toolbar()
                        .ghost()
                        .icon(IconName::RefreshCw)
                        .tooltip("Reload group environment")
                        .on_click(cx.listener(|this, _, _, cx| this.merge_editor_environments(cx))),
                )
            },
        )
        .when(editor.parent.is_none(), |frame| {
            frame.action(
                Button::new("close-editor")
                    .toolbar()
                    .ghost()
                    .icon(IconName::ArrowRightToLine)
                    .tooltip("Close editor")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.retire_editor();
                        this.selection.clear();
                        this.focus.focus(window, cx);
                        cx.notify();
                    })),
            )
        })
        .sections(sections)
        .when(
            editor.host.as_ref().is_some_and(|draft| draft.inheriting),
            |frame| frame.section("Loading Group environment…"),
        )
        .when_some(
            crate::components::validation_message(editor.validation_error.as_deref(), cx),
            |frame, error| frame.section(error),
        )
        .footer(
            Button::new("save-editor")
                .large()
                .w_full()
                .accent()
                .label(
                    if editor
                        .forward
                        .as_ref()
                        .is_some_and(|forward| forward.saving())
                    {
                        "Saving…"
                    } else {
                        "Save"
                    },
                )
                .loading(
                    editor
                        .forward
                        .as_ref()
                        .is_some_and(|forward| forward.saving()),
                )
                .disabled(
                    !self.resources_ready()
                        || (editor.host.as_ref().is_some_and(|draft| {
                            draft.kind == HostKind::Remote && draft.ssh_enabled
                        }) && editor
                            .selects
                            .as_ref()
                            .is_some_and(|selects| selects.auth.blocked()))
                        || editor.host.as_ref().is_some_and(|draft| draft.busy)
                        || editor.host.as_ref().is_some_and(|draft| {
                            draft.inheriting || draft.inheritance_error.is_some()
                        })
                        || (matches!(editor.kind, ItemKind::Snippet | ItemKind::SnippetPackage)
                            && self.snippet_state.busy)
                        || editor
                            .forward
                            .as_ref()
                            .is_some_and(|forward| forward.saving()),
                )
                .on_click(cx.listener(|this, _, window, cx| this.save_editor(window, cx))),
        )
        .into_any_element()
    }
}

impl Render for Nauterm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_host_name(false, window, cx);
        self.sync_tag_catalog(cx);
        if !window.is_window_active() {
            self.terminal_tools_drag = None;
            self.workspace_assistant_drag = None;
        }
        self.refresh_workspace_views(window, cx);
        self.refresh_snippet_composers(cx);
        self.refresh_snippet_editors(window, cx);
        self.refresh_terminal_files(window, cx);
        self.refresh_workspace_assistants(window, cx);
        self.stage_recovery(window, cx);
        if let Some(keychain) = &self.keychain {
            keychain.update(cx, |view, cx| {
                view.set_dark(self.data.dark, cx);
                if self.page != Page::Keychain {
                    view.finish_hidden_drawer();
                }
            });
        }
        if let Some(known_hosts) = &self.known_hosts {
            known_hosts.update(cx, |view, cx| view.set_dark(self.data.dark, cx));
        }
        if let Some(logs) = &self.logs {
            logs.update(cx, |view, cx| {
                view.set_dark(self.data.dark, cx);
                view.set_hosts(&self.data.hosts, cx);
            });
        }
        if let Some(history) = &self.history {
            let can_save =
                self.snippet_state.ready && !self.snippet_state.busy && self.resources_ready();
            history.update(cx, |view, cx| view.configure(self.data.dark, can_save, cx));
        }
        if let Some(proxies) = &self.proxies {
            proxies.update(cx, |view, cx| {
                if self.page != Page::Proxies {
                    view.finish_hidden_drawer();
                }
                view.set_dark(self.data.dark, cx);
                view.set_active(self.page == Page::Proxies);
                view.resume_refresh(cx);
            });
        }
        if let Some(sftp) = &self.sftp {
            sftp.update(cx, |view, cx| {
                view.set_data(self.data.clone(), cx);
                view.set_visible(
                    self.page == Page::Sftp && self.file_pages_visible(),
                    window,
                    cx,
                );
            });
        }
        let p = Palette::new(self.data.dark);
        let viewport_width = f32::from(window.viewport_size().width);
        let can_quick_connect = self.search_target(cx).is_some();
        let connect_disabled = ButtonCustomVariant::new(cx)
            .color(
                if self.data.dark {
                    p.control
                } else {
                    rgb(0xe2eaec)
                }
                .into(),
            )
            .foreground(p.muted.into());
        let drawer_open = self.editor.is_some() || self.history.is_some();
        self.child_drawer_open = match self.page {
            Page::Keychain => self
                .keychain
                .as_ref()
                .is_some_and(|view| view.read(cx).drawer_open()),
            Page::Proxies => self
                .proxies
                .as_ref()
                .is_some_and(|view| view.read(cx).drawer_open()),
            _ => false,
        };
        let reserved_drawers = usize::from(drawer_open) + usize::from(self.child_drawer_open);
        let sidebar_target =
            layout::sidebar_width(viewport_width - layout::DRAWER_WIDTH * reserved_drawers as f32);
        let sidebar_width = crate::components::panel::width_transition(
            ("workspace-sidebar", cx.entity_id()),
            sidebar_target,
            sidebar_target,
            crate::components::panel::SIDEBAR,
            window,
            cx,
        );
        self.sidebar_width = sidebar_width;
        let drawer_amount = self.drawer_motion.frame(drawer_open, window, cx);
        let drawer = (drawer_open || drawer_amount > 0.)
            .then(|| self.animated_drawer(drawer_amount, window, cx));
        let content_width = viewport_width - sidebar_width - layout::DRAWER_WIDTH * drawer_amount;
        for view in self.keychain.iter() {
            view.update(cx, |view, cx| view.set_parent_width(content_width, cx));
        }
        for view in self.proxies.iter() {
            view.update(cx, |view, cx| view.set_parent_width(content_width, cx));
        }
        for view in self.known_hosts.iter() {
            view.update(cx, |view, cx| view.set_parent_width(content_width, cx));
        }
        let _ = &self.subscriptions;
        let content = match self.page {
            Page::Hosts | Page::Snippets | Page::PortForwards => self.collection(content_width, cx),
            Page::Proxies => self
                .proxies
                .as_ref()
                .map(|view| {
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(view.clone())
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
            Page::Sftp => self
                .sftp
                .as_ref()
                .map(|view| view.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Page::Keychain => self
                .keychain
                .as_ref()
                .map(|view| {
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(view.clone())
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
            Page::Logs => self.logs.as_ref().map(|view| div().flex_1().min_h_0().child(view.clone()).into_any_element()).unwrap_or_else(|| div().p(px(26.)).child(crate::localization::view("Session history is unavailable. Check the error above and reopen Logs to retry.")).into_any_element()),
            Page::KnownHosts => self
                .known_hosts
                .as_ref()
                .map(|view| {
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(view.clone())
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
            Page::Sessions => {
                if self.overview {
                    self.workspace_with_assistant(window, cx)
                } else {
                    match self
                        .terminals
                        .iter()
                        .find(|tab| Some(tab.id) == self.active_terminal())
                    {
                        Some(tab) => div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .child(self.terminal_area(tab, window, cx))
                            .into_any_element(),
                        None => self.workspace_with_assistant(window, cx),
                    }
                }
            }
        };
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(p.background)
            .text_color(p.text)
            .text_size(px(13.))
            .tab_group()
            .track_focus(&self.focus)
            .key_context("Nauterm")
            .on_key_down(cx.listener(Self::navigate_collection))
            .on_mouse_move(cx.listener(|this, event, window, cx| {
                this.resize_workspace_assistant(event, window, cx);
                this.resize_terminal_tools(event, window, cx);
                this.resize_split(event, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    let split = this.split_drag.take().is_some();
                    let tools = this.terminal_tools_drag.take().is_some();
                    let assistant = this.workspace_assistant_drag.take().is_some();
                    if split || tools || assistant {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    let split = this.split_drag.take().is_some();
                    let tools = this.terminal_tools_drag.take().is_some();
                    let assistant = this.workspace_assistant_drag.take().is_some();
                    if split || tools || assistant {
                        cx.notify();
                    }
                }),
            )
            .on_action(cx.listener(|this, _: &SplitRight, window, cx| {
                if crate::components::overlay::is_modal_open(window, cx) {
                    return;
                }
                this.duplicate_selected(Some(crate::pane_tree::SplitAxis::Horizontal), window, cx)
            }))
            .on_action(cx.listener(|this, _: &SplitDown, window, cx| {
                if crate::components::overlay::is_modal_open(window, cx) {
                    return;
                }
                this.duplicate_selected(Some(crate::pane_tree::SplitAxis::Vertical), window, cx)
            }))
            .on_action(cx.listener(|this, _: &NewPaneSession, window, cx| {
                if crate::components::overlay::is_modal_open(window, cx) {
                    return;
                }
                this.duplicate_selected(None, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ClosePaneSession, window, cx| {
                if crate::components::overlay::is_modal_open(window, cx) {
                    return;
                }
                this.close_selected_session(window, cx)
            }))
            .on_action(cx.listener(|this, _: &NewItem, window, cx| {
                if crate::components::overlay::is_modal_open(window, cx) {
                    return;
                }
                if this.quick_connect.is_some()
                    || this.rename_workspace.is_some()
                    || this.close_pending.is_some()
                    || this.recovery_pending.is_some()
                {
                    return;
                }
                if this.page == Page::Sessions {
                    this.open_quick_connect(false, window, cx);
                } else if this.page == Page::Keychain {
                    let view = this.ensure_keychain(cx);
                    view.update(cx, |view, cx| {
                        view.create(crate::keychain::Kind::Key, window, cx)
                    });
                } else if this.page == Page::Proxies {
                    this.ensure_proxies(cx)
                        .update(cx, |view, cx| view.create(window, cx));
                } else if let Some(kind) = this.new_kind() {
                    this.open_editor(kind, None, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &CloseEditor, window, cx| {
                if this.close_pending.is_some() {
                    this.cancel_close(cx);
                    this.focus.focus(window, cx);
                    cx.notify();
                    return;
                }
                if this.recovery_pending.is_some() {
                    this.resolve_recovery(false, window, cx);
                    return;
                }
                if this.quick_connect.is_some()
                    || this.rename_workspace.is_some()
                    || this.serial_connect.is_some()
                {
                    if let Some(dialog) = &mut this.quick_connect {
                        dialog.cancel_pending();
                    }
                    crate::components::Modal::dismiss(window, cx);
                    return;
                }
                if let Some(related) = this
                    .editor
                    .as_ref()
                    .and_then(|editor| editor.related_proxy.as_ref())
                {
                    related
                        .view
                        .update(cx, |view, cx| view.back_editor(window, cx));
                    return;
                }
                if let Some(related) = this
                    .editor
                    .as_ref()
                    .and_then(|editor| editor.related_credential.as_ref())
                {
                    related
                        .view
                        .update(cx, |view, cx| view.back_editor(window, cx));
                    return;
                }
                if this
                    .editor
                    .as_ref()
                    .and_then(|editor| editor.host.as_ref())
                    .is_some_and(|draft| draft.environment_page.is_some())
                {
                    this.leave_host_environment(window, cx);
                    return;
                }
                if this.page == Page::Proxies
                    && let Some(view) = &this.proxies
                {
                    view.update(cx, |view, cx| view.back_editor(window, cx));
                    return;
                }
                if this.back_related_resource(window, cx) {
                    return;
                }
                this.retire_editor();
                this.retire_history();
                this.selection.clear();
                this.delete_pending = false;
                this.focus.focus(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                if crate::components::overlay::is_modal_open(window, cx) {
                    return;
                }
                if this.quick_connect.is_none()
                    && this.rename_workspace.is_none()
                    && this.close_pending.is_none()
                    && this.recovery_pending.is_none()
                {
                    this.open_settings(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &crate::app_menu::About, window, cx| {
                this.open_settings_at(Some(crate::settings::Page::About), window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &crate::app_menu::CheckForUpdates, window, cx| {
                    this.check_native_updates(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app_menu::Settings, window, cx| {
                    this.workspace_shortcut(crate::shortcuts::Command::OpenSettings, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app_menu::NewConnection, window, cx| {
                    this.workspace_shortcut(crate::shortcuts::Command::QuickConnect, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app_menu::NewLocalTerminal, window, cx| {
                    this.workspace_shortcut(
                        crate::shortcuts::Command::NewLocalTerminal,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app_menu::CloseTab, window, cx| {
                    this.workspace_shortcut(crate::shortcuts::Command::CloseTab, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app_menu::PreviousTab, window, cx| {
                    this.workspace_shortcut(crate::shortcuts::Command::PreviousTab, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app_menu::NextTab, window, cx| {
                    this.workspace_shortcut(crate::shortcuts::Command::NextTab, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, action: &crate::app_menu::SelectTab, window, cx| {
                    this.workspace_shortcut(
                        crate::shortcuts::Command::Tab(action.index),
                        window,
                        cx,
                    );
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app_menu::CloseWindow, window, cx| {
                    this.request_exit(crate::settings::ExitAction::Window, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &gpui_kit::base::input::SelectAll, window, cx| {
                    if !crate::components::overlay::is_modal_open(window, cx)
                        && !this.selection.is_empty()
                    {
                        this.select_cards(cx);
                    } else {
                        cx.propagate();
                    }
                }),
            )
            .on_action(cx.listener(|this, _: &SelectCards, window, cx| {
                if crate::components::overlay::is_modal_open(window, cx) {
                    return;
                }
                if this.quick_connect.is_none()
                    && this.rename_workspace.is_none()
                    && this.close_pending.is_none()
                    && this.recovery_pending.is_none()
                {
                    this.select_cards(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &DeleteCards, window, cx| {
                if crate::components::overlay::is_modal_open(window, cx) {
                    return;
                }
                if this.quick_connect.is_none()
                    && this.rename_workspace.is_none()
                    && this.close_pending.is_none()
                    && this.recovery_pending.is_none()
                    && !this.selection.is_empty()
                {
                    if this.selection_kind == Some(ItemKind::Forward) {
                        if this
                            .selection
                            .iter()
                            .any(|id| this.forward_mutations.contains(id))
                        {
                            this.notice =
                                Some("A selected forwarding rule is being updated.".into());
                            cx.notify();
                            return;
                        }
                        this.notice = None;
                    }
                    this.delete_pending = true;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &Quit, window, cx| {
                this.request_exit(crate::settings::ExitAction::Quit, window, cx);
            }))
            .child(self.topbar(window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(!matches!(self.page, Page::Sessions | Page::Sftp), |view| {
                        view.child(self.sidebar(
                            sidebar_target == layout::COLLAPSED_SIDEBAR_WIDTH,
                            sidebar_width,
                            cx,
                        ))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .when_some(
                                self.searches
                                    .get(&self.page)
                                    .filter(|_| self.page == Page::Hosts),
                                |view, search| {
                                    view.child(
                                        div().px(px(10.)).py(px(8.)).bg(p.surface).child(
                                            Input::new(search)
                                                .palette(Palette {
                                                    surface: p.hover,
                                                    ..p
                                                })
                                                .pl(px(12.))
                                                .pr(px(4.))
                                                .py_0()
                                                .when(self.page == Page::Hosts, |input| {
                                                    input.suffix(
                                                        Button::new("quick-connect")
                                                            .xsmall()
                                                            .primary()
                                                            .h(px(24.))
                                                            .px(px(10.))
                                                            .rounded(px(4.))
                                                            .label("Connect")
                                                            .disabled(!can_quick_connect)
                                                            .when(!can_quick_connect, |button| {
                                                                button
                                                                    .custom(connect_disabled)
                                                                    .opacity(1.)
                                                            })
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.connect_search(window, cx)
                                                                },
                                                            )),
                                                    )
                                                }),
                                        ),
                                    )
                                },
                            )
                            .when(
                                self.new_kind().is_some() || self.page == Page::Hosts,
                                |view| view.child(self.toolbar(content_width, window, cx)),
                            )
                            .child(content),
                    )
                    .when_some(drawer, |view, drawer| view.child(drawer)),
            )
            .when_some(self.workspace_overlay(cx), |view, overlay| {
                view.child(overlay)
            })
            .when_some(self.serial_modal(cx), |view, modal| view.child(modal))
            .when_some(self.recovery_overlay(cx), |view, overlay| {
                view.child(overlay)
            })
            .when_some(self.confirmation_overlay(cx), |view, overlay| {
                view.child(overlay)
            })
            .when_some(self.host_import_modal(window, cx), |view, modal| {
                view.child(modal)
            })
            .when_some(self.rename_tag_modal(cx), |view, modal| view.child(modal))
    }
}
