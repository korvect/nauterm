//! Terminal-bound presentation reuses FilePane and its ordinary trust/task flow.
use super::*;
use crate::{
    directory_follow::Endpoint,
    terminal::{Terminal, TerminalDirectoryChanged, TerminalMetadataChanged},
};

pub struct TerminalBinding {
    pub terminal: Entity<Terminal>,
    pub options: SshOptions,
    pub title: String,
    pub host_id: Option<Id>,
    pub compact: bool,
}
pub(super) struct DirectoryBinding {
    terminal: Entity<Terminal>,
    endpoint: Endpoint,
    _events: Vec<Subscription>,
}
pub(super) struct RemoteSetup {
    pub options: SshOptions,
    pub title: String,
    pub host_id: Option<Id>,
    pub path: String,
}
fn endpoint(options: &SshOptions, host_id: Option<Id>) -> Endpoint {
    Endpoint {
        host: options.host.clone(),
        port: options.port,
        username: options.username.clone(),
        host_id,
    }
}

impl Sftp {
    pub fn for_terminal(
        data: AppState,
        known_hosts_path: PathBuf,
        vault: Option<Vault>,
        binding: TerminalBinding,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let this = Self::build(
            data,
            known_hosts_path,
            vault,
            BrowserLayout::RemoteOnly,
            window,
            cx,
        );
        this.right
            .update(cx, |pane, cx| pane.bind_terminal(binding, window, cx));
        this
    }
    pub fn set_visible(&mut self, visible: bool, window: &mut Window, cx: &mut Context<Self>) {
        let active = visible && window.is_window_active();
        for pane in self.left.iter().chain(std::iter::once(&self.right)) {
            pane.update(cx, |pane, cx| {
                pane.presented = visible;
                if pane.active != active {
                    pane.active = active;
                    if let Some(connection) = &pane.connection {
                        connection
                            .update(cx, |connection, cx| connection.allow_auto_focus(active, cx));
                    }
                    if active {
                        pane.try_follow(window, cx);
                    }
                }
            });
        }
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.right.update(cx, |pane, cx| {
            if let Some(connection) = &pane.connection {
                connection.update(cx, |connection, cx| connection.focus(window, cx));
            } else {
                pane.focus.focus(window, cx);
            }
        });
    }
    /// Attach changed-only directory/SSH-editor metadata to an already opened
    /// SFTP page. Never reconnect it or replace its current host/path/selection.
    pub fn bind_terminal_metadata(
        &mut self,
        terminal: Entity<Terminal>,
        source: Endpoint,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.right.update(cx, |pane, cx| {
            if pane
                .terminal
                .as_ref()
                .is_some_and(|current| current.terminal == terminal && current.endpoint == source)
            {
                return;
            }
            pane.bind_directory(terminal, source, window, cx);
            cx.notify();
        });
    }
}
impl FilePane {
    fn register_task_history(&self, cx: &mut Context<Self>) {
        let Some(remote) = &self.remote else {
            return;
        };
        let uuid = self
            .data
            .hosts
            .iter()
            .find(|host| Some(host.id) == remote.host_id)
            .and_then(|host| host.uuid.clone());
        self.tasks.update(cx, |queue, _| {
            queue.register_history_owner(
                self.endpoint_token,
                crate::transfer_history::Identity {
                    uuid,
                    host: remote.options.host.clone(),
                    username: remote.options.username.clone(),
                    port: remote.options.port,
                },
            )
        });
    }
    pub(super) fn ssh_editor_available(&self, cx: &App) -> bool {
        self.active
            && self.stage == Stage::Files
            && !self.loading
            && !crate::transfer_queue::is_shutting_down()
            && !self.data.sftp_editors.ssh_editor.trim().is_empty()
            && self.shares_terminal(cx)
    }
    pub(super) fn open_ssh_editor(
        &mut self,
        token: u64,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_ssh_editor_mode(token, path, false, window, cx);
    }
    pub(super) fn open_ssh_editor_mode(
        &mut self,
        token: u64,
        path: &str,
        sudo: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = self.selected_entries();
        if self.endpoint_token != token
            || !self.ssh_editor_available(cx)
            || self.dialog.is_some()
            || self.action_work.is_some()
            || entries.len() != 1
            || entries[0].path != path
            || entries[0].directory
            || entries[0].parent
        {
            return;
        }
        let command = if sudo {
            self.data.sftp_editors.sudo_ssh_command(path)
        } else {
            self.data.sftp_editors.ssh_command(path)
        };
        let result = command.and_then(|command| {
            self.terminal
                .as_ref()
                .unwrap()
                .terminal
                .update(cx, |terminal, cx| {
                    terminal.open_ssh_editor(&command, window, cx)
                })
        });
        match result {
            Ok(()) => {
                self.notice = None;
                cx.emit(ShowTerminal);
            }
            Err(error) => {
                self.notice = Some(Message::error(format!(
                    "Unable to open SSH editor: {error}"
                )))
            }
        }
        cx.notify();
    }
    fn bind_terminal(
        &mut self,
        binding: TerminalBinding,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.compact = binding.compact;
        self.allow_local = false;
        self.picker
            .update(cx, |picker, cx| picker.set_allow_local(false, cx));
        self.bind_directory(
            binding.terminal,
            endpoint(&binding.options, binding.host_id),
            window,
            cx,
        );
        self.change_endpoint();
        self.reset_load();
        self.stage = Stage::Connecting;
        self.remote = Some(Remote {
            options: Arc::new(binding.options.clone()),
            title: binding.title.clone(),
            host_id: binding.host_id,
        });
        self.register_task_history(cx);
        match Core::discover() {
            Ok(core) => self.install_remote(
                core,
                RemoteSetup {
                    options: binding.options.for_new_connection(),
                    title: binding.title,
                    host_id: binding.host_id,
                    path: "~".into(),
                },
                window,
                cx,
            ),
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    fn bind_directory(
        &mut self,
        terminal: Entity<Terminal>,
        source: Endpoint,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.follow.rebind(terminal.read(cx).working_directory());
        let events = vec![
            cx.subscribe_in(
                &terminal,
                window,
                |this, _, event: &TerminalDirectoryChanged, window, cx| {
                    if this.follow.update(event.0.clone()) {
                        this.try_follow(window, cx);
                        cx.notify();
                    }
                },
            ),
            cx.subscribe(&terminal, |_, _, _: &TerminalMetadataChanged, cx| {
                cx.notify()
            }),
        ];
        self.terminal = Some(DirectoryBinding {
            endpoint: source,
            terminal,
            _events: events,
        });
        self.try_follow(window, cx);
    }
    pub(super) fn install_remote(
        &mut self,
        core: Arc<Core>,
        setup: RemoteSetup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let RemoteSetup {
            options,
            title,
            host_id,
            path,
        } = setup;
        let epoch = self.epoch;
        let connection = cx.new(|cx| {
            let mut connection = Connection::new_sftp_unfocused(
                core,
                options,
                title.clone(),
                self.data.dark,
                path,
                window,
                cx,
            )
            .with_host_platform(
                host_id
                    .and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
                    .map(|host| host.platform.clone())
                    .unwrap_or_default(),
            );
            connection = connection.with_authentication(
                self.vault.clone(),
                self.keychain.clone(),
                host_id
                    .and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
                    .and_then(crate::vault::authentication::Binding::from_host),
            );
            connection.allow_auto_focus(self.active, cx);
            if self.active {
                connection.focus(window, cx);
            }
            if self.compact {
                connection.in_drawer()
            } else {
                connection
            }
        });
        let events = cx.subscribe_in(
            &connection,
            window,
            move |this, connection, event: &ConnectionEvent, window, cx| {
                if this.epoch != epoch || this.stage != Stage::Connecting {
                    return;
                }
                match event {
                    ConnectionEvent::Close => {
                        this.connection = None;
                        this.connection_events.clear();
                        this.stage = if this.compact {
                            Stage::Empty
                        } else {
                            Stage::Selecting
                        };
                        if this.active {
                            this.focus.focus(window, cx);
                        }
                    }
                    ConnectionEvent::Ready => {
                        let Some((options, listing)) =
                            connection.update(cx, |view, _| view.take_sftp())
                        else {
                            return;
                        };
                        if this.active && connection.read(cx).contains_focus(window, cx) {
                            this.focus.focus(window, cx);
                        }
                        this.history = History::default();
                        this.history
                            .commit(listing.directory.clone(), Navigation::Visit);
                        this.entries = file_model::remote_entries(&listing);
                        this.selection = Selection::default();
                        let options = Arc::new(options);
                        this.remote = Some(Remote {
                            options: options.clone(),
                            title: title.clone(),
                            host_id,
                        });
                        this.register_task_history(cx);
                        this.connection = None;
                        this.connection_events.clear();
                        this.stage = Stage::Files;
                        this.load_favorites(cx);
                        this.error = None;
                        if let Some(host_id) = host_id {
                            let owner = Arc::new(());
                            let event = RemoteConnected {
                                host_id,
                                options,
                                owner: Arc::downgrade(&owner),
                            };
                            this.platform_owner = Some(owner);
                            cx.emit(event);
                        }
                        this.try_follow(window, cx);
                    }
                }
                cx.notify();
            },
        );
        let status = cx.observe(&connection, |_, _, cx| cx.notify());
        self.connection_events = vec![events, status];
        self.connection = Some(connection);
    }
    fn shares_terminal(&self, cx: &App) -> bool {
        let (Some(binding), Some(remote)) = (&self.terminal, &self.remote) else {
            return false;
        };
        binding.terminal.read(cx).phase() == crate::connection_state::Phase::Completing
            && binding
                .endpoint
                .matches(&endpoint(&remote.options, remote.host_id))
    }
    pub(super) fn try_follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let related = self.shares_terminal(cx);
        let blocked = !self.active
            || self.loading
            || self.editing_path
            || self.stage != Stage::Files
            || self.dialog.is_some()
            || self.action_work.is_some();
        if let Some(target) = self.follow.target(related, blocked, &self.history.path) {
            // Refresh commits the followed path without adding back/forward history.
            self.navigate(target, Navigation::Refresh, window, cx);
        }
    }
    pub(super) fn directory_buttons(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.terminal.as_ref()?;
        self.remote.as_ref()?;
        let related = self.shares_terminal(cx);
        let p = self.palette();
        Some(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(2.))
                .child(
                    Button::new("file-ssh-directory")
                        .toolbar()
                        .w(px(24.))
                        .h(px(26.))
                        .p_0()
                        .ghost()
                        .custom(crate::components::ButtonCustomVariant::for_surface(p))
                        .icon(IconName::FolderInput)
                        .tooltip("Go to SSH current directory")
                        .disabled(!related || self.loading)
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.shares_terminal(cx) {
                                if let Some(path) = this.follow.directory().map(str::to_owned) {
                                    this.navigate(path, Navigation::Visit, window, cx);
                                } else {
                                    this.notice = Some(Message::info(
                                        "SSH current directory is not available yet.",
                                    ));
                                    cx.notify();
                                }
                            }
                        })),
                )
                .child(
                    Button::new("file-follow-ssh-directory")
                        .toolbar()
                        .w(px(24.))
                        .h(px(26.))
                        .p_0()
                        .ghost()
                        .custom(crate::components::ButtonCustomVariant::for_surface(p))
                        .icon(IconName::Link)
                        .tooltip(if self.follow.enabled {
                            "Stop following SSH current directory"
                        } else {
                            "Follow SSH current directory"
                        })
                        .text_color(if self.follow.enabled {
                            p.accent
                        } else {
                            p.muted
                        })
                        .disabled(!self.follow.enabled && !related)
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.follow.enabled || this.shares_terminal(cx) {
                                this.follow.toggle();
                                this.try_follow(window, cx);
                                cx.notify();
                            }
                        })),
                )
                .into_any_element(),
        )
    }
}
