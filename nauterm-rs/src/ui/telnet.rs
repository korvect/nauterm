use super::*;
use anyhow::Context as _;

enum Source {
    Direct(crate::telnet::Options),
    Saved(Box<Host>),
}

impl Nauterm {
    pub(super) fn open_telnet_in(
        &mut self,
        options: crate::telnet::Options,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_telnet_source(Source::Direct(options), workspace, overview, window, cx);
    }
    pub(super) fn open_saved_telnet_in(
        &mut self,
        host: Host,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_telnet_source(
            Source::Saved(Box::new(host)),
            workspace,
            overview,
            window,
            cx,
        );
    }
    fn open_telnet_source(
        &mut self,
        source: Source,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| {
            match &source {
                Source::Direct(options) => options.validate()?,
                Source::Saved(host) => {
                    anyhow::ensure!(host.is_remote(), "Select a remote Telnet Host.");
                    host.connections
                        .telnet
                        .as_ref()
                        .context("Telnet is not enabled for this Host.")?
                        .validate()?;
                }
            }
            anyhow::ensure!(
                self.data.workspaces.iter().any(|item| item.id == workspace),
                "The destination workspace is no longer available."
            );
            self.core.clone().map(Ok).unwrap_or_else(Core::discover)
        })();
        let core = match result {
            Ok(core) => core,
            Err(error) => {
                self.notice = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let id = self.next_tab;
        self.next_tab += 1;
        let (title, restore_target) = match &source {
            Source::Direct(options) => (format!("Telnet {}", options.endpoint()), None),
            Source::Saved(host) => (
                host.name.clone(),
                Some(RestoreTarget::TelnetHost {
                    id: host.id,
                    port: None,
                }),
            ),
        };
        self.terminals.push(TerminalTab {
            id,
            workspace,
            show_sftp: false,
            tools: Default::default(),
            tree: PaneTree::new_with_composer(
                id,
                SessionView {
                    id,
                    title,
                    content: TabContent::Preparing(None),
                    ssh: None,
                    restore_target,
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
        match source {
            Source::Direct(options) => {
                self.install_telnet_connection(id, core, options, window, cx)
            }
            Source::Saved(host) => self.load_saved_telnet_connection(
                id,
                core,
                (host.connections.telnet.unwrap().source, None),
                window,
                cx,
            ),
        }
        self.show_session(id, overview, window, cx);
        self.notice = None;
        cx.notify();
    }
    pub(super) fn load_saved_telnet_connection(
        &mut self,
        id: u64,
        core: Arc<Core>,
        target: (String, Option<u16>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (source, port) = target;
        let Some(workspace) = self
            .terminals
            .iter()
            .find(|tab| tab.tree.session(id).is_some())
            .map(|tab| tab.workspace)
        else {
            return;
        };
        let Some(session) = self
            .session_mut(id)
            .filter(|session| matches!(session.content, TabContent::Preparing(_)))
        else {
            return;
        };
        session.preparation_generation = session.preparation_generation.wrapping_add(1);
        let generation = session.preparation_generation;
        session.content = TabContent::Preparing(None);
        let vault = self.ensure_keychain(cx).read(cx).vault();
        cx.spawn_in(window, async move |this, cx| {
            let result = match vault {
                Some(vault) => vault.saved_telnet_host(source.clone()).await,
                None => Err(anyhow::anyhow!("Saved Telnet settings cannot be loaded. Resolve the Keychain storage error and retry.")),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if !this.owns_preparation((id, workspace), generation) { return; }
                let current = crate::resource_reference::Reference::parse(&source)
                    .and_then(|reference| this.data.hosts.iter().find(|host|
                        host.id == reference.id as Id && host.uuid.as_deref() == Some(&reference.uuid))).is_some();
                match (result, current) {
                    (Ok(mut resolved), true) => {
                        if let Some(port) = port { resolved.options.port = port; }
                        this.session_mut(id).unwrap().title = resolved.title;
                        this.install_telnet_connection(id, core, resolved.options, window, cx);
                    }
                    (Err(error), _) => this.session_mut(id).unwrap().content = TabContent::Preparing(Some(error.to_string())),
                    (_, false) => this.session_mut(id).unwrap().content = TabContent::Preparing(Some("The saved Telnet Host was deleted or replaced.".into())),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    pub(super) fn install_telnet_connection(
        &mut self,
        id: u64,
        core: Arc<Core>,
        options: crate::telnet::Options,
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
        let host_id = session.host_id();
        let recording = self.prepare_connection_recording(&label, &appearance, host_id, cx);
        let connection = cx.new(|cx| {
            Connection::new_telnet_unfocused(
                core,
                options,
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
}
