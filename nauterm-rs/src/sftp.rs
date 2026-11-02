//! Retained dual-pane standalone and single-pane terminal-bound file browsers.
//! Shared controls/picker/connection UI;
//! filesystem work is asynchronous and stale loads cannot replace newer paths.
use crate::components::ScrollbarsExt;
use crate::components::menu::{ContextMenuExt, DropdownMenu, MenuItem};
use crate::{
    components::overlay::Overlay,
    components::{Button, Input, Spinner, message::Message},
    connection::{Connection, ConnectionEvent},
    connection_state::Phase,
    file_model::{self, Entry, History, Navigation, Selection, Sort, format_size},
    host_picker::{HostPicker, PickerEvent},
    model::{AppState, Id},
    native::{Core, SshOptions},
    palette::Palette,
    sftp_client::{Cancellation, Request},
    sftp_tasks::{TaskAuthorizationRequested, TaskFinished, TaskQueue},
    typography::FontWeights,
    ui::{CloseEditor, SelectCards},
    vault::Vault,
};
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::{
    assets::IconName,
    base::ElementExt,
    component::{Disableable, Icon, Sizable},
    prelude::*,
    *,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{collections::BTreeSet, path::PathBuf, sync::Arc};
mod actions;
mod applications;
mod creation;
mod drag;
mod editing;
mod favorites;
mod local;
mod navigation;
mod permission;
mod terminal;
mod tool_panel;
use actions::ActionDialog;
pub use terminal::TerminalBinding;
static NEXT_ENDPOINT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy)]
enum BrowserLayout {
    Dual,
    RemoteOnly,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Empty,
    Files,
    Selecting,
    Connecting,
}
impl Stage {
    fn phase(self, failed: bool, connection: Option<Phase>) -> Phase {
        match self {
            Self::Connecting if failed => Phase::Failed,
            Self::Connecting => connection.unwrap_or(Phase::Connecting),
            Self::Files => Phase::Completing,
            Self::Empty | Self::Selecting => Phase::Exited,
        }
    }
}
struct Remote {
    options: Arc<SshOptions>,
    title: String,
    host_id: Option<Id>,
}
pub struct Sftp {
    left: Option<Entity<FilePane>>,
    right: Entity<FilePane>,
    data: AppState,
    tasks: Entity<TaskQueue>,
    reported_phase: Phase,
    _events: Vec<Subscription>,
}
pub struct ShowTerminal;
pub struct StatusChanged;
#[derive(Clone)]
pub(crate) struct RemoteConnected {
    pub host_id: Id,
    pub options: Arc<SshOptions>,
    pub owner: std::sync::Weak<()>,
}
impl EventEmitter<ShowTerminal> for Sftp {}
impl EventEmitter<StatusChanged> for Sftp {}
impl EventEmitter<ShowTerminal> for FilePane {}
impl EventEmitter<RemoteConnected> for Sftp {}
impl EventEmitter<RemoteConnected> for FilePane {}
impl Sftp {
    pub(crate) fn with_keychain(
        self,
        owner: WeakEntity<crate::keychain::Keychain>,
        cx: &mut Context<Self>,
    ) -> Self {
        for pane in self.left.iter().chain(std::iter::once(&self.right)) {
            pane.update(cx, |pane, _| pane.keychain = Some(owner.clone()));
        }
        self
    }
    pub(crate) fn with_terminal_navigation(self, enabled: bool, cx: &mut Context<Self>) -> Self {
        self.right
            .update(cx, |pane, _| pane.show_terminal = enabled);
        self
    }
    pub fn new(
        data: AppState,
        known_hosts_path: PathBuf,
        vault: Option<Vault>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(
            data,
            known_hosts_path,
            vault,
            BrowserLayout::Dual,
            window,
            cx,
        )
    }
    pub fn phase(&self, cx: &App) -> Phase {
        self.right.read(cx).phase(cx)
    }
    fn build(
        data: AppState,
        known_hosts_path: PathBuf,
        vault: Option<Vault>,
        layout: BrowserLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let tasks =
            cx.new(|cx| TaskQueue::new(data.dark, data.sftp.limits, vault.clone(), window, cx));
        let left = matches!(layout, BrowserLayout::Dual).then(|| {
            cx.new(|cx| {
                FilePane::new(
                    true,
                    data.clone(),
                    known_hosts_path.clone(),
                    vault.clone(),
                    tasks.clone(),
                    window,
                    cx,
                )
            })
        });
        let right = cx.new(|cx| {
            FilePane::new(
                false,
                data.clone(),
                known_hosts_path,
                vault,
                tasks.clone(),
                window,
                cx,
            )
        });
        if let Some(left) = &left {
            left.update(cx, |pane, _| pane.peer = Some(right.downgrade()));
            right.update(cx, |pane, _| pane.peer = Some(left.downgrade()));
        }
        let mut events: Vec<_> = left
            .iter()
            .chain(std::iter::once(&right))
            .map(|pane| cx.subscribe(pane, |_, _, _: &ShowTerminal, cx| cx.emit(ShowTerminal)))
            .collect();
        for pane in left.iter().chain(std::iter::once(&right)) {
            events.push(cx.subscribe(pane, |_, _, event: &RemoteConnected, cx| {
                cx.emit(event.clone());
            }));
        }
        // Only status transitions invalidate the owning tab/overview; listing,
        // selection and transfer progress retain their own local view updates.
        events.push(cx.observe(&right, |this, _, cx| {
            let phase = this.phase(cx);
            if phase != this.reported_phase {
                this.reported_phase = phase;
                cx.emit(StatusChanged);
                cx.notify();
            }
        }));
        Self {
            left,
            right,
            data,
            tasks,
            reported_phase: Phase::Exited,
            _events: events,
        }
    }
    pub fn refresh_theme(&self, cx: &mut Context<Self>) {
        for pane in self.left.iter().chain(std::iter::once(&self.right)) {
            pane.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }
    pub fn set_data(&mut self, data: AppState, cx: &mut Context<Self>) {
        if self.data.hosts == data.hosts
            && self.data.groups == data.groups
            && self.data.dark == data.dark
            && self.data.terminal == data.terminal
            && self.data.sftp_editors == data.sftp_editors
            && self.data.sftp == data.sftp
        {
            return;
        }
        let limits_changed = self.data.sftp.limits != data.sftp.limits;
        self.data = data.clone();
        self.tasks
            .update(cx, |queue, cx| queue.set_dark(data.dark, cx));
        if limits_changed {
            self.tasks
                .update(cx, |queue, cx| queue.set_limits(data.sftp.limits, cx));
        }
        for pane in self.left.iter().chain(std::iter::once(&self.right)) {
            pane.update(cx, |pane, cx| {
                pane.data = data.clone();
                if let Some(connection) = &pane.connection {
                    connection.update(cx, |connection, cx| connection.set_dark(data.dark, cx));
                }
                pane.picker
                    .update(cx, |picker, cx| picker.set_data(data.clone(), cx));
                cx.notify();
            });
        }
    }
    pub fn has_request(&self, cx: &App) -> bool {
        self.tasks.read(cx).has_work()
            || self
                .left
                .iter()
                .chain(std::iter::once(&self.right))
                .any(|pane| {
                    let pane = pane.read(cx);
                    pane.stage == Stage::Connecting
                        || pane.edits.has_work()
                        || pane.action_work.is_some()
                        || pane.has_pending_drop()
                        || (pane.remote.is_some() && pane.loading)
                })
    }
    pub fn has_external_edits(&self, cx: &App) -> bool {
        self.left
            .iter()
            .chain(std::iter::once(&self.right))
            .any(|pane| pane.read(cx).edits.has_work())
    }
}
impl Render for Sftp {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.data.dark);
        div()
            .size_full()
            .min_h_0()
            .flex()
            .when_some(self.left.clone(), |view, left| {
                view.child(div().flex_1().min_w_0().h_full().child(left))
                    .child(div().w(px(1.)).h_full().bg(p.border).flex_shrink_0())
            })
            .child(div().flex_1().min_w_0().h_full().child(self.right.clone()))
    }
}
struct FilePane {
    data: AppState,
    known_hosts_path: PathBuf,
    vault: Option<Vault>,
    keychain: Option<WeakEntity<crate::keychain::Keychain>>,
    home: PathBuf,
    initial_directory: PathBuf,
    stage: Stage,
    allow_local: bool,
    show_terminal: bool,
    picker: Entity<HostPicker>,
    connection: Option<Entity<Connection>>,
    connection_events: Vec<Subscription>,
    remote: Option<Remote>,
    platform_owner: Option<Arc<()>>,
    history: History,
    entries: Vec<Entry>,
    visible: Vec<Entry>,
    selection: Selection,
    sort: Sort,
    ascending: bool,
    hidden: bool,
    path: Entity<InputState>,
    filter: Entity<InputState>,
    editing_path: bool,
    filter_open: bool,
    loading: bool,
    error: Option<String>,
    retry: Option<(String, Navigation)>,
    scroll: UniformListScrollHandle,
    epoch: u64,
    cancel: Option<Cancellation>,
    load: Option<Task<()>>,
    focus: FocusHandle,
    tasks: Entity<TaskQueue>,
    tasks_open: bool,
    favorites: favorites::Favorites,
    endpoint_token: u64,
    task_owners: BTreeSet<u64>,
    pending_permissions: std::collections::VecDeque<crate::transfer_queue::PermissionTicket>,
    needs_refresh: bool,
    dialog: Option<ActionDialog>,
    sudo: Option<Arc<crate::sftp_client::Sudo>>,
    action_work: Option<Task<()>>,
    saved_local: Option<local::LocalSnapshot>,
    pending_selection: Option<Selection>,
    notice: Option<Message>,
    edits: editing::Edits,
    compact: bool,
    narrow: bool,
    terminal: Option<terminal::DirectoryBinding>,
    follow: crate::directory_follow::DirectoryFollow,
    active: bool,
    presented: bool,
    peer: Option<WeakEntity<FilePane>>,
    _inputs: Vec<Subscription>,
    _overlay: Option<Subscription>,
}

impl FilePane {
    fn phase(&self, cx: &App) -> Phase {
        self.stage.phase(
            self.error.is_some(),
            self.connection
                .as_ref()
                .map(|connection| connection.read(cx).phase()),
        )
    }
    fn palette(&self) -> Palette {
        if self.compact {
            let mut p = Palette::terminal(
                self.data
                    .terminal_for_host(self.remote.as_ref().and_then(|remote| remote.host_id))
                    .palette(),
            );
            p.background = p.sidebar;
            p.surface = p.sidebar;
            p
        } else {
            Palette::new(self.data.dark)
        }
    }
    fn new(
        local: bool,
        data: AppState,
        known_hosts_path: PathBuf,
        vault: Option<Vault>,
        tasks: Entity<TaskQueue>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        crate::components::message::watch(window, cx, |view: &Self| {
            (!view.edits.opening).then(|| view.notice.clone()).flatten()
        });
        let home = directories::UserDirs::new()
            .map(|dirs| dirs.home_dir().to_owned())
            .unwrap_or_else(|| PathBuf::from("."));
        let initial_directory = data.sftp.initial_directory(&home);
        let hidden = data.sftp.show_hidden_files;
        let path = cx.new(|cx| InputState::new(window, cx));
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter files"));
        let picker = cx.new(|cx| HostPicker::new(data.clone(), window, cx).allow_local());
        let inputs = vec![
            cx.observe(&tasks, |_, _, cx| cx.notify()),
            cx.subscribe(&tasks, |pane, _, event: &TaskAuthorizationRequested, cx| {
                pane.queue_permission(event.0.clone(), cx);
            }),
            cx.subscribe_in(
                &tasks,
                window,
                |this, _, event: &TaskFinished, window, cx| {
                    let local_changed = this.remote.is_none()
                        && event
                            .local_target
                            .as_ref()
                            .and_then(|path| std::path::Path::new(path).parent())
                            .is_some_and(|parent| {
                                parent == std::path::Path::new(&this.history.path)
                            });
                    if this.endpoint_token == event.owner
                        && let Some(error) = &event.error
                    {
                        this.notice = Some(Message::error(error.clone()));
                    }
                    if ((event.refresh_owner && this.endpoint_token == event.owner)
                        || local_changed)
                        && this.stage == Stage::Files
                    {
                        if this.loading {
                            this.needs_refresh = true;
                        } else {
                            this.refresh(window, cx);
                        }
                    }
                },
            ),
            cx.subscribe_in(&path, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    let target = this.path.read(cx).value().to_string();
                    this.editing_path = false;
                    this.focus.focus(window, cx);
                    this.navigate(target, Navigation::Visit, window, cx);
                } else if matches!(event, InputEvent::Blur) {
                    this.editing_path = false;
                    this.try_follow(window, cx);
                    cx.notify();
                }
            }),
            cx.subscribe(&filter, |this, filter, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Blur) && filter.read(cx).value().is_empty() {
                    this.filter_open = false;
                }
                cx.notify();
            }),
            cx.subscribe_in(
                &picker,
                window,
                |this, _, event: &PickerEvent, window, cx| match event {
                    PickerEvent::UseLocal => this.use_local(window, cx),
                    PickerEvent::Back => {
                        this.stage = if this.remote.is_some() || !this.history.path.is_empty() {
                            Stage::Files
                        } else {
                            Stage::Empty
                        };
                        this.focus.focus(window, cx);
                        this.try_follow(window, cx);
                        cx.notify();
                    }
                    PickerEvent::Selected(id) => this.connect(*id, window, cx),
                },
            ),
        ];
        let mut this = Self {
            data,
            known_hosts_path,
            vault,
            keychain: None,
            home,
            initial_directory,
            stage: Stage::Empty,
            allow_local: true,
            picker,
            connection: None,
            connection_events: vec![],
            show_terminal: false,
            remote: None,
            platform_owner: None,
            history: History::default(),
            entries: vec![],
            visible: vec![],
            selection: Selection::default(),
            sort: Sort::Name,
            ascending: true,
            hidden,
            path,
            filter,
            editing_path: false,
            filter_open: false,
            loading: false,
            error: None,
            retry: None,
            scroll: UniformListScrollHandle::default(),
            epoch: 0,
            cancel: None,
            load: None,
            focus: cx.focus_handle(),
            tasks,
            tasks_open: false,
            favorites: Default::default(),
            endpoint_token: NEXT_ENDPOINT.fetch_add(1, Ordering::Relaxed),
            task_owners: BTreeSet::new(),
            pending_permissions: Default::default(),
            needs_refresh: false,
            dialog: None,
            sudo: None,
            action_work: None,
            saved_local: None,
            pending_selection: None,
            notice: None,
            edits: Default::default(),
            compact: false,
            narrow: false,
            terminal: None,
            follow: Default::default(),
            active: true,
            presented: true,
            peer: None,
            _inputs: inputs,
            _overlay: None,
        };
        this.task_owners.insert(this.endpoint_token);
        if local {
            this.use_local(window, cx);
        }
        this
    }
    fn reset_load(&mut self) {
        self.epoch += 1;
        self.cancel = None;
        self.load = None;
        self.loading = false;
    }
    fn use_local(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.allow_local {
            return;
        }
        self.remember_local(cx);
        self.change_endpoint();
        self.reset_load();
        self.connection = None;
        self.connection_events.clear();
        self.remote = None;
        self.editing_path = false;
        self.stage = Stage::Files;
        let (path, navigation) = self.restore_local(window, cx);
        self.navigate(path, navigation, window, cx);
    }
    fn connect(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        let Some(host) = self.data.hosts.iter().find(|host| host.id == id).cloned() else {
            return;
        };
        if !host.supports_ssh() {
            self.error = Some("Select an SSH host for SFTP.".into());
            cx.notify();
            return;
        }
        self.remember_local(cx);
        self.change_endpoint();
        self.reset_load();
        self.connection = None;
        self.connection_events.clear();
        self.stage = Stage::Connecting;
        self.error = None;
        let epoch = self.epoch;
        let options = SshOptions::new(
            host.address,
            host.port,
            host.username,
            self.known_hosts_path.clone(),
        );
        let vault = self.vault.clone();
        self.load = Some(cx.spawn_in(window, async move |this, cx| {
            let result = async {
                let core = Core::discover()?;
                let options = if host.credential.is_some()
                    || host.proxy_id.is_some()
                    || host.ssh.password_id.is_some()
                {
                    vault
                        .context("Keychain storage is unavailable")?
                        .apply_connection(host.credential, host.proxy_id, host.ssh.clone(), options)
                        .await?
                } else {
                    options
                };
                Ok::<_, anyhow::Error>((core, options))
            }
            .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.epoch != epoch || this.stage != Stage::Connecting {
                    return;
                }
                match result {
                    Err(error) => this.error = Some(error.to_string()),
                    Ok((core, options)) => {
                        this.install_remote(
                            core,
                            terminal::RemoteSetup {
                                options,
                                title: host.name,
                                host_id: Some(id),
                                path: "~".into(),
                            },
                            window,
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn navigate(
        &mut self,
        target: String,
        navigation: Navigation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(navigation, Navigation::Refresh) && target != self.history.path {
            self.follow.stop();
        }
        if !matches!(navigation, Navigation::Refresh) {
            self.pending_selection = None;
        }
        self.reset_load();
        self.loading = true;
        self.error = None;
        self.retry = Some((target.clone(), navigation));
        let epoch = self.epoch;
        let remote = self.remote.as_ref().map(|remote| remote.options.clone());
        let request = if remote.is_some() {
            match Core::discover().and_then(Request::new) {
                Ok((request, cancel)) => {
                    self.cancel = Some(cancel);
                    Some(request)
                }
                Err(error) => {
                    self.error = Some(error.to_string());
                    self.loading = false;
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let home = self.home.clone();
        let current = if self.history.path.is_empty() {
            home.clone()
        } else {
            PathBuf::from(&self.history.path)
        };
        self.load = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if let (Some(options), Some(request)) = (remote, request) {
                        let target = if target.starts_with('/') || target.starts_with('~') {
                            target
                        } else {
                            format!(
                                "{}/{}",
                                current.to_string_lossy().trim_end_matches('/'),
                                target
                            )
                        };
                        let listing = request.list(&options, &target)?;
                        if let Some(error) = &listing.error {
                            anyhow::bail!("{error}");
                        }
                        let entries = file_model::remote_entries(&listing);
                        Ok::<_, anyhow::Error>((listing.directory, entries))
                    } else {
                        let path = file_model::local_path(&target, &home, &current)?;
                        let entries = file_model::list_local(&path)?;
                        Ok((path.to_string_lossy().into_owned(), entries))
                    }
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.epoch != epoch {
                    return;
                }
                this.cancel = None;
                this.loading = false;
                match result {
                    Ok((path, entries)) => {
                        this.history.commit(path, navigation);
                        this.retry = None;
                        this.entries = entries;
                        this.selection = this.pending_selection.take().unwrap_or_default();
                        this.selection
                            .paths
                            .retain(|path| this.entries.iter().any(|entry| &entry.path == path));
                        if this.selection.primary.as_ref().is_some_and(|path| {
                            !this.entries.iter().any(|entry| &entry.path == path)
                        }) {
                            this.selection.primary = None;
                        }
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                if std::mem::take(&mut this.needs_refresh) {
                    this.refresh(window, cx);
                }
                this.try_follow(window, cx);
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn title(&self) -> String {
        self.remote
            .as_ref()
            .map(|remote| remote.title.clone())
            .unwrap_or("Local".into())
    }
    fn home(&self) -> String {
        if self.remote.is_some() {
            "~".into()
        } else {
            self.home.to_string_lossy().into_owned()
        }
    }
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(self.history.path.clone(), Navigation::Refresh, window, cx);
    }
    fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((target, navigation)) = self.retry.clone() {
            self.navigate(target, navigation, window, cx);
        } else {
            self.refresh(window, cx);
        }
    }
    fn disconnect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_local(cx);
        self.change_endpoint();
        self.reset_load();
        self.connection = None;
        self.connection_events.clear();
        self.remote = None;
        self.history = History::default();
        self.entries.clear();
        self.selection = Selection::default();
        self.error = None;
        self.retry = None;
        self.stage = Stage::Empty;
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn show_picker(&mut self, cx: &mut Context<Self>) {
        if self.compact {
            return;
        }
        self.stage = Stage::Selecting;
        cx.notify();
    }
    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.compact {
            return self.tool_header(cx);
        }
        let p = self.palette();
        let endpoint = cx.entity().downgrade();
        let operations = endpoint.clone();
        div()
            .w_full()
            .h(px(if self.compact { 40. } else { 54. }))
            .flex_shrink_0()
            .px(px(if self.compact { 6. } else { 14. }))
            .bg(p.surface)
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div().flex_1().min_w_0().flex().child(
                    Button::new("file-endpoint")
                        .toolbar()
                        .ghost()
                        .custom(crate::components::ButtonCustomVariant::for_surface(p))
                        .content_start()
                        .max_w_full()
                        .flex_shrink_1()
                        .min_w_0()
                        .h(px(34.))
                        .px(px(4.))
                        .localized_accessibility_label("Select host")
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .min_w_0()
                                .child(if let Some(remote) = &self.remote {
                                    let host = remote.host_id.and_then(|id| {
                                        self.data.hosts.iter().find(|host| host.id == id)
                                    });
                                    crate::avatar::host(
                                        host.map_or(remote.title.as_str(), |host| {
                                            host.name.as_str()
                                        }),
                                    )
                                    .dimension(24.)
                                    .platform(
                                        host.map(|host| host.platform.clone()).unwrap_or_default(),
                                    )
                                    .into_any_element()
                                } else {
                                    div()
                                        .size(px(24.))
                                        .flex_shrink_0()
                                        .rounded(px(6.))
                                        .bg(rgb(0x075e92))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            Icon::new(IconName::Folder)
                                                .size(px(14.))
                                                .text_color(rgb(0xffffff)),
                                        )
                                        .into_any_element()
                                })
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_size(px(13.))
                                        .line_height(px(18.))
                                        .font_weight(FontWeights::SEMIBOLD)
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(self.title()),
                                )
                                .child(
                                    Icon::new(IconName::ChevronDown)
                                        .size(px(18.))
                                        .flex_shrink_0()
                                        .text_color(p.text),
                                ),
                        )
                        .on_click(move |_, _, cx| {
                            let _ = endpoint.update(cx, |this, cx| this.show_picker(cx));
                        }),
                ),
            )
            .when(self.show_terminal, |view| {
                view.child(
                    Button::new("sftp-show-terminal")
                        .toolbar()
                        .ghost()
                        .custom(crate::components::ButtonCustomVariant::for_surface(p))
                        .icon(IconName::Terminal)
                        .tooltip("Show terminal")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(ShowTerminal))),
                )
            })
            .when(
                self.filter_open || !self.filter.read(cx).value().is_empty(),
                |view| {
                    view.child(
                        div().w(px(130.)).child(
                            Input::new(&self.filter)
                                .palette(p)
                                .small()
                                .appearance(false)
                                .aria_label(crate::localization::text(cx, "Filter files"))
                                .cleanable(true),
                        ),
                    )
                },
            )
            .when(
                !self.filter_open && self.filter.read(cx).value().is_empty(),
                |view| {
                    view.child(
                        Button::new("file-filter")
                            .toolbar()
                            .ghost()
                            .custom(crate::components::ButtonCustomVariant::for_surface(p))
                            .icon(IconName::Search)
                            .when(!self.compact && !self.narrow, |button| {
                                button.label("Filter")
                            })
                            .tooltip("Filter files")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.filter_open = true;
                                this.filter
                                    .update(cx, |filter, cx| filter.focus(window, cx));
                                cx.notify();
                            })),
                    )
                },
            )
            .child(
                Button::new("file-actions")
                    .toolbar()
                    .ghost()
                    .custom(crate::components::ButtonCustomVariant::for_surface(p))
                    .when(!self.compact && !self.narrow, |button| {
                        button.label("Actions").trailing_icon(IconName::ChevronDown)
                    })
                    .when(self.compact, |button| button.icon(IconName::Ellipsis))
                    .when(!self.compact && self.narrow, |button| {
                        button.icon(IconName::ChevronDown)
                    })
                    .tooltip("Actions")
                    .dropdown_menu(move |menu, _, cx| {
                        actions::browser_menu(menu, operations.clone(), cx)
                    }),
            )
            .into_any_element()
    }
    fn path_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        if self.editing_path && (self.compact || self.narrow) {
            return div()
                .w_full()
                .h(px(36.))
                .px(px(6.))
                .bg(p.surface)
                .flex()
                .items_center()
                .gap(px(4.))
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&self.path)
                            .palette(p)
                            .small()
                            .aria_label(crate::localization::text(cx, "Edit path")),
                    ),
                )
                .child(
                    Button::new("file-cancel-path")
                        .xsmall()
                        .ghost()
                        .custom(crate::components::ButtonCustomVariant::for_surface(p))
                        .icon(crate::assets::icon(IconName::X))
                        .tooltip("Cancel path editing")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.editing_path = false;
                            this.focus.focus(window, cx);
                            this.try_follow(window, cx);
                            cx.notify();
                        })),
                )
                .into_any_element();
        }
        if self.compact {
            return self.tool_path_bar(cx);
        }
        div()
            .w_full()
            .h(px(if self.compact { 36. } else { 46. }))
            .flex_shrink_0()
            .px(px(if self.compact || self.narrow { 6. } else { 14. }))
            .pb(px(if self.compact { 0. } else { 8. }))
            .bg(p.surface)
            .flex()
            .items_center()
            .gap(px(if self.compact || self.narrow { 2. } else { 6. }))
            .child(
                Button::new("file-home")
                    .toolbar()
                    .w(px(24.))
                    .h(px(26.))
                    .p_0()
                    .ghost()
                    .custom(crate::components::ButtonCustomVariant::for_surface(p))
                    .icon(IconName::House)
                    .tooltip("Home")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate(this.home(), Navigation::Visit, window, cx)
                    })),
            )
            .when_some(self.directory_buttons(cx), |view, buttons| {
                view.child(buttons)
            })
            .child(
                Button::new("file-back")
                    .toolbar()
                    .w(px(24.))
                    .h(px(26.))
                    .p_0()
                    .ghost()
                    .custom(crate::components::ButtonCustomVariant::for_surface(p))
                    .icon(IconName::ChevronLeft)
                    .tooltip("Back")
                    .disabled(self.history.back.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(path) = this.history.back.last().cloned() {
                            this.navigate(path, Navigation::Back, window, cx);
                        }
                    })),
            )
            .child(
                Button::new("file-forward")
                    .toolbar()
                    .w(px(24.))
                    .h(px(26.))
                    .p_0()
                    .ghost()
                    .custom(crate::components::ButtonCustomVariant::for_surface(p))
                    .icon(IconName::ChevronRight)
                    .tooltip("Forward")
                    .disabled(self.history.forward.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(path) = this.history.forward.last().cloned() {
                            this.navigate(path, Navigation::Forward, window, cx);
                        }
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .when(self.editing_path, |view| {
                        view.child(
                            Input::new(&self.path)
                                .palette(p)
                                .small()
                                .aria_label(crate::localization::text(cx, "Edit path")),
                        )
                    })
                    .when(!self.editing_path, |view| view.child(self.breadcrumbs(cx))),
            )
            .when(self.remote.is_some(), |view| {
                view.child(
                    Overlay::new("sftp-tasks-popover")
                        .p_0()
                        .anchor(Anchor::TopRight)
                        .open(self.tasks_open)
                        .on_open_change(cx.listener(|this, open, _, cx| {
                            this.tasks_open = *open;
                            if *open {
                                this.tasks.read(cx).reload_history();
                            }
                            cx.notify();
                        }))
                        .trigger(
                            Button::new("sftp-tasks")
                                .toolbar()
                                .w(px(24.))
                                .h(px(26.))
                                .p_0()
                                .ghost()
                                .custom(crate::components::ButtonCustomVariant::for_surface(p))
                                .icon(IconName::List)
                                .tooltip("Tasks")
                                .when(self.tasks.read(cx).active() > 0, |button| {
                                    button.child(
                                        div()
                                            .absolute()
                                            .top(px(-2.))
                                            .right(px(-2.))
                                            .min_w(px(12.))
                                            .h(px(12.))
                                            .px(px(2.))
                                            .rounded_full()
                                            .bg(p.accent)
                                            .text_color(rgb(0xffffff))
                                            .text_size(px(8.))
                                            .line_height(px(12.))
                                            .text_center()
                                            .child(
                                                self.tasks.read(cx).active().min(99).to_string(),
                                            ),
                                    )
                                })
                                .localized_accessibility_label("Tasks"),
                        )
                        .content({
                            let tasks = self.tasks.clone();
                            move |_, _| tasks.clone().into_any_element()
                        }),
                )
            })
            .when(self.remote.is_some() && !self.editing_path, |view| {
                view.children(self.favorite_controls(cx))
            })
            .into_any_element()
    }
    fn row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let entry = self.visible[index].clone();
        let p = self.palette();
        let selected = self.selection.paths.contains(&entry.path);
        let dark =
            p.background.r * 0.2126 + p.background.g * 0.7152 + p.background.b * 0.0722 < 0.5;
        let row_color = if self.compact {
            if selected { p.control } else { rgba(0) }
        } else if selected {
            if dark {
                p.background.blend(p.accent.alpha(0.48))
            } else {
                p.accent
            }
        } else {
            p.background
        };
        let text = if selected && !self.compact {
            rgb(0xffffff)
        } else {
            p.text
        };
        let muted = if selected && !self.compact {
            text
        } else {
            p.muted
        };
        let entry_for_press = entry.clone();
        let entry_for_click = entry.clone();
        let entry_for_menu = entry.clone();
        let modified = entry
            .modified
            .and_then(|seconds| i64::try_from(seconds).ok())
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|time| {
                time.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or("—".into());
        let menu_owner = cx.entity().downgrade();
        Button::new(SharedString::from(entry.path.clone()))
            .when(!entry.parent && self.peer.is_some(), |button| {
                drag::row(button, cx.weak_entity(), &entry)
            })
            .ghost()
            .focusable(false)
            .tab_stop(false)
            .ink_ripple(self.compact)
            .custom(
                crate::components::ButtonCustomVariant::for_surface(p)
                    .color(row_color.into())
                    .foreground(text.into())
                    .hover(
                        if self.compact {
                            p.control
                        } else if selected {
                            row_color
                        } else {
                            p.hover
                        }
                        .into(),
                    )
                    .highlight(
                        if self.compact {
                            p.accent.alpha(0.10)
                        } else {
                            rgba(0)
                        }
                        .into(),
                    )
                    .active(
                        if self.compact {
                            p.accent.alpha(0.16)
                        } else {
                            rgba(0)
                        }
                        .into(),
                    ),
            )
            .px(px(0.))
            .rounded(px(if self.compact { 6. } else { 0. }))
            .accessibility_label(entry.name.clone())
            .h(px(if self.compact { 39. } else { 40. }))
            .w_full()
            .flex()
            .items_center()
            .text_size(px(12.))
            .text_color(text)
            .capture_any_mouse_down(
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if event.button != MouseButton::Left {
                        return;
                    }
                    this.focus.focus(window, cx);
                    let additive = event.modifiers.platform || event.modifiers.control;
                    if additive
                        || event.modifiers.shift
                        || !this.selection.paths.contains(&entry_for_press.path)
                    {
                        this.selection.select(
                            &entry_for_press.path,
                            &this.visible,
                            additive,
                            event.modifiers.shift,
                        );
                        cx.notify();
                    }
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                this.focus.focus(window, cx);
                if event.click_count() == 2 {
                    this.open_entry(entry_for_click.clone(), window, cx);
                } else if !event.modifiers().platform
                    && !event.modifiers().control
                    && !event.modifiers().shift
                {
                    this.selection.select(
                        &entry_for_click.path,
                        &this.visible,
                        event.modifiers().platform || event.modifiers().control,
                        event.modifiers().shift,
                    );
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, window, cx| {
                    this.focus.focus(window, cx);
                    if !this.selection.paths.contains(&entry_for_menu.path) {
                        this.selection
                            .select(&entry_for_menu.path, &this.visible, false, false);
                    }
                    cx.notify();
                }),
            )
            .context_menu(move |menu, _, cx| {
                let owner = menu_owner.clone();
                let menu = menu.item(MenuItem::localized("rs.sftp.action.copyPath").on_click(
                    move |_, _, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                this.selection
                                    .paths
                                    .iter()
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ))
                        });
                    },
                ));
                let (selected, remote) = menu_owner
                    .upgrade()
                    .map(|pane| {
                        let pane = pane.read(cx);
                        (pane.selected_entries().len(), pane.remote.is_some())
                    })
                    .unwrap_or_default();
                actions::menu(menu, menu_owner.clone(), selected, remote, cx)
            })
            .when(self.compact, |button| {
                let metadata = entry.compact_subtitle();
                button.child(
                    div()
                        .w_full()
                        .px(px(7.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            Icon::new(if entry.directory {
                                IconName::Folder
                            } else {
                                IconName::File
                            })
                            .size(px(15.))
                            .text_color(if entry.directory { p.accent } else { p.muted }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(px(11.5))
                                        .line_height(px(16.))
                                        .font_weight(FontWeights::MEDIUM)
                                        .child(entry.name.clone()),
                                )
                                .when(!metadata.is_empty(), |view| {
                                    view.child(
                                        div()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .text_size(px(9.))
                                            .line_height(px(12.))
                                            .text_color(muted)
                                            .child(metadata),
                                    )
                                }),
                        ),
                )
            })
            .when(!self.compact, |button| {
                button.child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .text_size(px(12.))
                        .font_weight(FontWeights::MEDIUM)
                        .text_color(text)
                        .child(
                            div()
                                .w(relative(0.4))
                                .min_w_0()
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    Icon::new(if entry.directory {
                                        IconName::Folder
                                    } else {
                                        IconName::File
                                    })
                                    .with_size(px(20.))
                                    .text_color(
                                        if entry.directory {
                                            rgb(0x5ec4f2)
                                        } else {
                                            muted
                                        },
                                    ),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .line_height(px(18.))
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(entry.name.clone()),
                                ),
                        )
                        .child(
                            div()
                                .w(relative(0.25))
                                .min_w_0()
                                .px(px(8.))
                                .text_color(muted)
                                .line_height(px(18.))
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(if entry.parent {
                                    String::new()
                                } else {
                                    modified.clone()
                                }),
                        )
                        .child(
                            div()
                                .w(relative(0.15))
                                .min_w_0()
                                .px(px(8.))
                                .line_height(px(18.))
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(if entry.directory {
                                    "—".into()
                                } else {
                                    format_size(entry.size)
                                }),
                        )
                        .child(
                            div()
                                .w(relative(0.2))
                                .min_w_0()
                                .px(px(8.))
                                .line_height(px(18.))
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(entry.kind()),
                        ),
                )
            })
            .into_any_element()
    }
}
use anyhow::Context as _;
impl Render for FilePane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self._overlay.is_none() {
            self._overlay = Some(cx.observe(
                &crate::components::overlay::manager(window, cx),
                |this, _, cx| {
                    if this.active
                        && (this.edits.awaiting_dialog() || !this.pending_permissions.is_empty())
                    {
                        cx.notify();
                    }
                },
            ));
        }
        self.sync_edit_dialog(window, cx);
        self.sync_permission_dialog(window, cx);
        let p = self.palette();
        self.visible = file_model::visible(
            &self.entries,
            &self.filter.read(cx).value(),
            self.hidden,
            self.sort,
            self.ascending,
        );
        let blank_menu_owner = cx.weak_entity();
        let blank_gesture_owner = blank_menu_owner.clone();
        let content = match self.stage {
            Stage::Selecting if self.compact => self.tool_closed(cx),
            Stage::Selecting => self.picker.clone().into_any_element(),
            Stage::Connecting => self
                .connection
                .as_ref()
                .map(|connection| connection.clone().into_any_element())
                .unwrap_or_else(|| {
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(16.))
                        .when(self.error.is_none(), |view| view.child(Spinner::new()))
                        .when_some(self.error.clone(), |view, error| {
                            view.child(crate::localization::message(cx, &error)).child(
                                Button::new("file-connect-back")
                                    .label(if self.compact { "Retry" } else { "Back" })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.compact {
                                            this.reconnect_bound(window, cx);
                                        } else {
                                            this.show_picker(cx);
                                        }
                                    })),
                            )
                        })
                        .into_any_element()
                }),
            Stage::Empty if self.compact => self.tool_closed(cx),
            Stage::Empty => div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(60.))
                        .rounded(px(17.))
                        .bg(p.hover)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(IconName::Folder).with_size(px(28.))),
                )
                .child(
                    div()
                        .mt(px(28.))
                        .text_size(px(16.))
                        .font_weight(FontWeights::SEMIBOLD)
                        .child(crate::localization::text(cx, "sftp.label.connectToHost")),
                )
                .child(
                    div()
                        .mt(px(10.))
                        .max_w(px(290.))
                        .text_center()
                        .text_color(p.muted)
                        .child(crate::localization::text(
                            cx,
                            "Start by connecting to a saved host\nto manage your files with SFTP.",
                        )),
                )
                .child(
                    Button::new("file-select-host")
                        .mt(px(22.))
                        .secondary()
                        .label("Select host")
                        .on_click(cx.listener(|this, _, _, cx| this.show_picker(cx))),
                )
                .when(self.terminal.is_none(), |view| {
                    view.child(
                        Button::new("file-use-local")
                            .mt(px(8.))
                            .ghost()
                            .custom(crate::components::ButtonCustomVariant::for_surface(p))
                            .icon(IconName::Folder)
                            .label("Local")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.use_local(window, cx)),
                            ),
                    )
                })
                .into_any_element(),
            Stage::Files => div()
                .size_full()
                .min_h_0()
                .flex()
                .flex_col()
                .child(self.header(cx))
                .child(self.path_bar(cx))
                .when(self.edits.opening, |view| {
                    view.child(
                        div()
                            .p(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .bg(p.surface)
                            .child(div().flex_1().min_w_0().child(crate::localization::text(
                                cx,
                                "rs.ui.copy.openingRemoteFile",
                            )))
                            .child(
                                Button::new("cancel-remote-edit-open")
                                    .xsmall()
                                    .ghost()
                                    .custom(crate::components::ButtonCustomVariant::for_surface(p))
                                    .label("Cancel")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.cancel_edit_opening(cx)),
                                    ),
                            ),
                    )
                })
                .when(!self.compact, |view| {
                    view.child(
                        div()
                            .w_full()
                            .h(px(45.))
                            .flex_shrink_0()
                            .border_y_1()
                            .border_color(p.border)
                            .flex()
                            .items_center()
                            .children(
                                [
                                    (Sort::Name, "Name", 0.4),
                                    (Sort::Modified, "Date Modified", 0.25),
                                    (Sort::Size, "Size", 0.15),
                                    (Sort::Kind, "Kind", 0.2),
                                ]
                                .into_iter()
                                .map(|(sort, label, width)| {
                                    Button::new(SharedString::from(format!("file-sort-{label}")))
                                        .ghost()
                                        .custom(
                                            crate::components::ButtonCustomVariant::for_surface(p),
                                        )
                                        .content_start()
                                        .h_full()
                                        .w(relative(width))
                                        .px(px(12.))
                                        .rounded(px(0.))
                                        .text_size(px(12.))
                                        .border_r_1()
                                        .border_color(p.border)
                                        .raw_label(label)
                                        .when(self.sort == sort, |button| {
                                            button
                                                .trailing_icon(if self.ascending {
                                                    IconName::ChevronUp
                                                } else {
                                                    IconName::ChevronDown
                                                })
                                                .icon_size(px(14.))
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this.sort == sort {
                                                this.ascending = !this.ascending;
                                            } else {
                                                this.sort = sort;
                                                this.ascending = true;
                                            }
                                            cx.notify();
                                        }))
                                }),
                            ),
                    )
                })
                .child(
                    div()
                        .id("file-table-body")
                        .when(self.compact, |view| view.px(px(5.)).py(px(3.)))
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.selection = Selection::default();
                            this.focus.focus(window, cx);
                            cx.notify();
                        }))
                        .when(self.loading, |view| {
                            view.flex()
                                .items_center()
                                .justify_center()
                                .child(Spinner::new())
                        })
                        .when(!self.loading && self.error.is_some(), |view| {
                            view.flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(14.))
                                .p(px(20.))
                                .child(crate::localization::message(
                                    cx,
                                    self.error.as_deref().unwrap(),
                                ))
                                .child(Button::new("file-retry").small().label("Retry").on_click(
                                    cx.listener(|this, _, window, cx| this.retry(window, cx)),
                                ))
                        })
                        .when(
                            !self.loading && self.error.is_none() && self.visible.is_empty(),
                            |view| {
                                view.flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(p.muted)
                                    .child(crate::localization::text(cx, "rs.ui.copy.noFilesFound"))
                            },
                        )
                        .when(
                            !self.loading && self.error.is_none() && !self.visible.is_empty(),
                            |view| {
                                view.child(crate::components::uniform_scrollbar(
                                    "sftp-list-scrollbar",
                                    uniform_list(
                                        "sftp-file-list",
                                        self.visible.len(),
                                        cx.processor(
                                            |this, range: std::ops::Range<usize>, _, cx| {
                                                range.map(|index| this.row(index, cx)).collect()
                                            },
                                        ),
                                    )
                                    .track_scroll(&self.scroll)
                                    .size_full(),
                                    &self.scroll,
                                ))
                            },
                        )
                        .context_menu(move |menu, _, cx| {
                            actions::browser_menu(menu, blank_menu_owner.clone(), cx)
                        })
                        .enabled_when(move |window, cx| {
                            blank_gesture_owner
                                .update(cx, |pane, cx| {
                                    if pane.stage != Stage::Files
                                        || pane.loading
                                        || pane.dialog.is_some()
                                        || pane.error.is_some()
                                    {
                                        return false;
                                    }
                                    pane.selection = Selection::default();
                                    pane.focus.focus(window, cx);
                                    cx.notify();
                                    true
                                })
                                .unwrap_or(false)
                        }),
                )
                .into_any_element(),
        };
        div()
            .size_full()
            .relative()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(p.background)
            .text_color(p.text)
            .text_size(px(13.))
            .track_focus(&self.focus)
            .on_prepaint({
                let view = cx.entity().downgrade();
                move |bounds, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        let narrow = bounds.size.width < px(340.);
                        if this.narrow != narrow {
                            this.narrow = narrow;
                            cx.notify();
                        }
                    });
                }
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.dialog.is_some()
                    || this.stage != Stage::Files
                    || this.loading
                    || !this.focus.is_focused(window)
                    || this.visible.is_empty()
                {
                    return;
                }
                let key = &event.keystroke;
                if key.modifiers.platform || key.modifiers.control || key.modifiers.alt {
                    return;
                }
                let current = this
                    .selection
                    .primary
                    .as_ref()
                    .and_then(|path| this.visible.iter().position(|entry| &entry.path == path));
                let index = match key.key.as_str() {
                    "up" => current
                        .map(|index| index.saturating_sub(1))
                        .unwrap_or(this.visible.len() - 1),
                    "down" => current
                        .map(|index| (index + 1).min(this.visible.len() - 1))
                        .unwrap_or(0),
                    "home" => 0,
                    "end" => this.visible.len() - 1,
                    "enter" => {
                        if let Some(entry) =
                            current.and_then(|index| this.visible.get(index)).cloned()
                        {
                            this.open_entry(entry, window, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    _ => return,
                };
                this.selection.select(
                    &this.visible[index].path,
                    &this.visible,
                    false,
                    key.modifiers.shift,
                );
                this.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectCards, _, cx| {
                if this.dialog.is_some() || this.stage != Stage::Files {
                    return;
                }
                this.selection.paths = this
                    .visible
                    .iter()
                    .filter(|entry| !entry.parent)
                    .map(|entry| entry.path.clone())
                    .collect::<BTreeSet<_>>();
                cx.stop_propagation();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &CloseEditor, window, cx| {
                if this.edits.opening {
                    this.cancel_edit_opening(cx);
                } else if this.dialog.is_some() {
                    this.dialog = None;
                    this.action_work = None;
                } else if this.tasks_open || this.favorites.open {
                    this.tasks_open = false;
                    this.favorites.open = false;
                } else if this.editing_path {
                    this.editing_path = false;
                } else if this.stage == Stage::Selecting {
                    this.stage = if this.history.path.is_empty() {
                        Stage::Empty
                    } else {
                        Stage::Files
                    };
                }
                this.focus.focus(window, cx);
                cx.stop_propagation();
                this.try_follow(window, cx);
                cx.notify();
            }))
            .when_some(self.edit_authorization_notice(cx), |view, notice| {
                view.child(notice)
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.drop_area(content, cx)),
            )
            .when(
                self.compact && (self.tasks_open || self.favorites.open),
                |view| view.child(self.tool_overlay(cx)),
            )
            .when(
                self.dialog.as_ref().is_some_and(|dialog| dialog.visible),
                |view| view.child(self.action_dialog(cx)),
            )
            .when_some(self.edit_dialog(cx), |view, dialog| view.child(dialog))
    }
}

#[cfg(test)]
mod status_tests {
    use super::{Phase, Stage};

    #[test]
    fn sftp_page_phase_follows_its_own_connection_and_current_stage() {
        assert_eq!(Stage::Connecting.phase(false, None), Phase::Connecting);
        assert_eq!(Stage::Connecting.phase(true, None), Phase::Failed);
        for phase in [
            Phase::Connecting,
            Phase::HostKey,
            Phase::Authentication,
            Phase::Completing,
            Phase::Failed,
            Phase::Exited,
        ] {
            assert_eq!(Stage::Connecting.phase(false, Some(phase)), phase);
            assert_eq!(Stage::Connecting.phase(true, Some(phase)), Phase::Failed);
            // A retained/failed connection does not override the ready page,
            // nor make a disconnected host picker look live.
            assert_eq!(Stage::Files.phase(false, Some(phase)), Phase::Completing);
            assert_eq!(Stage::Files.phase(true, Some(phase)), Phase::Completing);
            assert_eq!(Stage::Selecting.phase(false, Some(phase)), Phase::Exited);
            assert_eq!(Stage::Empty.phase(false, Some(phase)), Phase::Exited);
        }
    }
}
