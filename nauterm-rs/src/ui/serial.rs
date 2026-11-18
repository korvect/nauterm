use super::*;
use crate::{
    components::Modal,
    serial::{
        Config,
        dialog::{Connection as SerialConnection, Event},
    },
};
use anyhow::Context as _;

pub(super) struct SerialDialog {
    view: Entity<SerialConnection>,
    _events: Subscription,
}
impl Nauterm {
    pub(super) fn open_serial_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.data.selected_workspace;
        let view = cx.new(|cx| {
            SerialConnection::new(self.core.clone(), window, cx).allow_save(self.resources_ready())
        });
        let events = cx.subscribe_in(
            &view,
            window,
            move |this, view, event: &Event, window, cx| {
                if !this
                    .serial_connect
                    .as_ref()
                    .is_some_and(|dialog| dialog.view == *view)
                {
                    return;
                }
                let Event::Connect {
                    config,
                    save_as_host,
                } = event;
                let core = view.read(cx).core();
                if let Err(error) = this.open_serial(
                    config,
                    core,
                    (workspace, None, false),
                    *save_as_host,
                    window,
                    cx,
                ) {
                    view.update(cx, |view, cx| view.failed(error.to_string(), cx));
                }
            },
        );
        self.quick_connect = None;
        self.rename_workspace = None;
        self.serial_connect = Some(SerialDialog {
            view,
            _events: events,
        });
        cx.notify();
    }
    fn save_serial_host(&mut self, config: &Config) -> anyhow::Result<(Id, String)> {
        anyhow::ensure!(self.resources_ready(), "Workspace storage is unavailable.");
        let mut candidate = self.data.clone();
        let id = candidate.allocate_id();
        let name = config.path.clone();
        candidate.hosts.push(Host {
            id,
            name: name.clone(),
            kind: HostKind::Serial,
            serial: Some(config.clone()),
            connections: ConnectionProfiles {
                ssh: false,
                ..Default::default()
            },
            ..Default::default()
        });
        self.store.save(&mut candidate)?;
        self.data = candidate;
        Ok((id, name))
    }
    pub(super) fn open_saved_serial(
        &mut self,
        host: Host,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let config = host
            .serial
            .as_ref()
            .context("Serial configuration is required.")?;
        self.open_serial(
            config,
            None,
            (workspace, Some((host.id, host.name)), overview),
            false,
            window,
            cx,
        )
    }

    fn open_serial(
        &mut self,
        config: &Config,
        core: Option<Arc<Core>>,
        target: (Id, Option<(Id, String)>, bool),
        save_as_host: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let (workspace, host, overview) = target;
        anyhow::ensure!(
            self.data.workspaces.iter().any(|item| item.id == workspace),
            "The destination workspace is no longer available."
        );
        let core = core
            .or_else(|| self.core.clone())
            .map(Ok)
            .unwrap_or_else(Core::discover)?;
        config.validate()?;
        let host = if save_as_host {
            Some(self.save_serial_host(config)?)
        } else {
            host
        };
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
                    title: host
                        .as_ref()
                        .map_or_else(|| config.path.clone(), |(_, name)| name.clone()),
                    content: TabContent::Preparing(None),
                    ssh: None,
                    restore_target: host.map(|(id, _)| RestoreTarget::Host { id, port: None }),
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
        self.core = Some(core.clone());
        self.serial_connect = None;
        self.install_serial_connection(id, core, config.clone(), window, cx);
        self.show_session(id, overview, window, cx);
        self.notice = None;
        cx.notify();
        Ok(())
    }
    pub(super) fn install_serial_connection(
        &mut self,
        id: u64,
        core: Arc<Core>,
        config: Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exiting.is_some() {
            return;
        }
        let Some(session) = self.session(id) else {
            return;
        };
        let label = session.title.clone();
        let appearance = session.appearance(&self.data);
        let recording = self.prepare_connection_recording(&label, session.host_id(), cx);
        let connection = cx.new(|cx| {
            Connection::new_serial_unfocused(
                core,
                config,
                crate::connection::TerminalPresentation {
                    title: label,
                    dark: self.data.dark,
                    palette: appearance.palette(),
                    recording,
                },
                window,
                cx,
            )
        });
        self.watch_connection(id, connection, window, cx);
    }
    pub(super) fn serial_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.serial_connect.as_ref()?;
        Some(
            Modal::new("serial-connection-modal", "Serial connection")
                .initial_focus(&dialog.view.read(cx).focus_handle(cx))
                .return_focus(&self.focus)
                .on_dismiss(cx.listener(|this, _, window, cx| {
                    this.serial_connect = None;
                    this.focus.focus(window, cx);
                    cx.notify();
                }))
                .content(dialog.view.clone())
                .into_any_element(),
        )
    }
}
