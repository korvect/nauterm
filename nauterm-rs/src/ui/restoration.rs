use super::*;
use crate::{
    components::Checkbox,
    pane_tree::{Node, Pane, Split, SplitAxis},
    recovery::{
        self, LaunchAction, LayoutSnapshot, PaneSnapshot, SessionSnapshot, TabSnapshot,
        WindowGeometry, WindowMode, WorkspaceSnapshot,
    },
};
use anyhow::Context as _;
use std::time::Duration;

pub fn initial_window_bounds(
    saved: Option<WindowGeometry>,
    configured: crate::general_preferences::WindowSize,
    cx: &App,
) -> WindowBounds {
    let fallback = || {
        let dimensions = cx
            .primary_display()
            .map(|display| {
                let bounds = display.bounds();
                configured.constrained(f32::from(bounds.size.width), f32::from(bounds.size.height))
            })
            .unwrap_or(configured);
        WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(dimensions.width), px(dimensions.height)),
            cx,
        ))
    };
    let Some(saved) = saved else {
        return fallback();
    };
    let Some(display) = cx.displays().into_iter().find(|display| {
        let bounds = display.bounds();
        let overlap_x = (saved.left + saved.width).min(f32::from(bounds.right()))
            - saved.left.max(f32::from(bounds.left()));
        let overlap_y = (saved.top + saved.height).min(f32::from(bounds.bottom()))
            - saved.top.max(f32::from(bounds.top()));
        overlap_x >= 64. && overlap_y >= 32.
    }) else {
        return fallback();
    };
    let screen = display.bounds();
    let width = saved
        .width
        .max(750.)
        .min(f32::from(screen.size.width).max(750.));
    let height = saved
        .height
        .max(440.)
        .min(f32::from(screen.size.height).max(440.));
    let left = saved
        .left
        .max(f32::from(screen.left()))
        .min(f32::from(screen.right()) - width);
    let top = saved
        .top
        .max(f32::from(screen.top()))
        .min(f32::from(screen.bottom()) - height);
    let bounds = Bounds::new(point(px(left), px(top)), size(px(width), px(height)));
    match saved.mode {
        WindowMode::Windowed => WindowBounds::Windowed(bounds),
        WindowMode::Maximized => WindowBounds::Maximized(bounds),
        WindowMode::Fullscreen => WindowBounds::Fullscreen(bounds),
    }
}
fn window_geometry(window: &Window) -> WindowGeometry {
    let bounds = window.window_bounds();
    let mode = match bounds {
        WindowBounds::Windowed(_) => WindowMode::Windowed,
        WindowBounds::Maximized(_) => WindowMode::Maximized,
        WindowBounds::Fullscreen(_) => WindowMode::Fullscreen,
    };
    let bounds = bounds.get_bounds();
    WindowGeometry {
        left: f32::from(bounds.origin.x),
        top: f32::from(bounds.origin.y),
        width: f32::from(bounds.size.width),
        height: f32::from(bounds.size.height),
        mode,
    }
}
impl Nauterm {
    fn snapshot_layout(node: &Node<SessionView>, cx: &App) -> Option<LayoutSnapshot> {
        match node {
            Node::Pane(pane) => {
                let sessions: Vec<_> = pane
                    .sessions
                    .iter()
                    .filter_map(|session| {
                        let target = session.restore_target.clone()?;
                        // Exited local shells are not useful recovery candidates.
                        if session.ssh.is_none()
                            && session.content.phase(cx) == crate::connection_state::Phase::Exited
                        {
                            return None;
                        }
                        Some(SessionSnapshot {
                            id: session.id,
                            title: session.title.clone(),
                            target,
                        })
                    })
                    .collect();
                if sessions.is_empty() {
                    return None;
                }
                let selected_session = sessions
                    .iter()
                    .find(|session| session.id == pane.active().id)
                    .unwrap_or(&sessions[0])
                    .id;
                // Directory and selection must belong to the same retained
                // session. A temporary connection/replay may be active but is
                // deliberately absent from this recovery checkpoint.
                let active = pane
                    .sessions
                    .iter()
                    .find(|session| session.id == selected_session)
                    .expect("retained sessions came from this pane");
                let directory = match &active.content {
                    TabContent::Terminal(terminal) => terminal
                        .read(cx)
                        .working_directory()
                        .or_else(|| active.restore_directory.clone()),
                    _ => active.restore_directory.clone(),
                };
                Some(LayoutSnapshot::Pane {
                    pane: PaneSnapshot {
                        id: pane.id,
                        selected_session,
                        composer_visible: pane.composer_visible,
                        directory,
                        sessions,
                    },
                })
            }
            Node::Split(split) => {
                let retained: Vec<_> = split
                    .children
                    .iter()
                    .zip(&split.fractions)
                    .filter_map(|(child, fraction)| {
                        Self::snapshot_layout(child, cx).map(|child| (child, *fraction))
                    })
                    .collect();
                if retained.is_empty() {
                    return None;
                }
                if retained.len() == 1 {
                    return retained.into_iter().next().map(|(node, _)| node);
                }
                let total: f32 = retained.iter().map(|(_, fraction)| fraction).sum();
                Some(LayoutSnapshot::Split {
                    id: split.id,
                    axis: if split.axis == SplitAxis::Horizontal {
                        recovery::Axis::Horizontal
                    } else {
                        recovery::Axis::Vertical
                    },
                    fractions: retained
                        .iter()
                        .map(|(_, fraction)| fraction / total)
                        .collect(),
                    children: retained.into_iter().map(|(node, _)| node).collect(),
                })
            }
        }
    }
    fn runtime_snapshot(&self, window: &Window, cx: &App) -> Snapshot {
        let mut geometry = window_geometry(window);
        if let Some(size) = self.pending_window_resize {
            // Fullscreen covers the display regardless of the normal size.
            // Preserve a deferred resize as the next restoration rectangle.
            geometry.width = size.width;
            geometry.height = size.height;
        }
        Snapshot {
            resource_database: Some(self.store.database_path()),
            selected_workspace: Some(self.data.selected_workspace),
            window: Some(geometry),
            active_forwards: crate::port_forward::recovery_ids(&self.forward_states),
            workspaces: self
                .data
                .workspaces
                .iter()
                .map(|workspace| WorkspaceSnapshot {
                    id: workspace.id,
                    name: workspace.name.clone(),
                    color: workspace.color,
                    selected_tab: self.terminals.selected(workspace.id).filter(|id| {
                        self.terminals
                            .iter()
                            .find(|tab| tab.id == *id)
                            .is_some_and(|tab| {
                                tab.tree
                                    .root()
                                    .and_then(|node| Self::snapshot_layout(node, cx))
                                    .is_some()
                            })
                    }),
                    tabs: self
                        .terminals
                        .in_workspace(workspace.id)
                        .filter_map(|tab| {
                            Some(TabSnapshot {
                                id: tab.id,
                                selected_pane: tab.tree.selected_pane(),
                                show_sftp: tab.show_sftp,
                                tools: tab.tools.snapshot(),
                                layout: Self::snapshot_layout(tab.tree.root()?, cx)?,
                            })
                        })
                        .collect(),
                })
                .collect(),
        }
    }
    pub(super) fn stage_recovery(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.exiting.is_some() || self.recovery_pending.is_some() || !self.resources_ready() {
            return;
        }
        let Some(journal) = self.journal.clone() else {
            return;
        };
        let snapshot = self.runtime_snapshot(window, cx);
        if self
            .last_snapshot
            .as_ref()
            .is_some_and(|(previous, behavior)| {
                previous == &snapshot && *behavior == self.data.restore_behavior
            })
        {
            return;
        }
        let generation = match journal.stage(snapshot.clone(), self.data.restore_behavior) {
            Ok(Some(generation)) => generation,
            Ok(None) => return,
            Err(error) => {
                self.notice = Some(format!("Could not prepare workspace recovery: {error}"));
                return;
            }
        };
        self.last_snapshot = Some((snapshot, self.data.restore_behavior));
        self.checkpoint_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            let result = cx
                .background_executor()
                .spawn(async move { journal.checkpoint(generation) })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.notice = Some(format!("Could not save workspace recovery: {error}"));
                    cx.notify();
                });
            }
        }));
    }
    pub(super) fn save_exit_checkpoint(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.checkpoint_task = None;
        if !self.resources_ready() {
            // Keep an unanswered recovery candidate while the resource worker
            // is awaiting Keychain consent; an empty catalog is not a session.
            return Ok(());
        }
        let journal = self
            .journal
            .as_ref()
            .context("Workspace recovery is unavailable.")?;
        let snapshot = self.runtime_snapshot(window, cx);
        if let Some(generation) = journal.stage(snapshot.clone(), self.data.restore_behavior)? {
            journal.checkpoint(generation)?;
            self.last_snapshot = Some((snapshot, self.data.restore_behavior));
        }
        // Mark clean only in the final app-quit hook. A failed native restart
        // launch must not disable later checkpoints while this process lives.
        Ok(())
    }
    pub(super) fn initialize_recovery(
        &mut self,
        previous: Option<Checkpoint>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.journal.is_none() || !self.resources_ready() {
            return;
        }
        if let Some(previous) = previous {
            let action = previous.launch_action(self.data.restore_behavior);
            if action == LaunchAction::Ask {
                self.recovery_pending = Some(previous);
                self.recovery_focus.focus(window, cx);
                return;
            } else if action == LaunchAction::Restore {
                self.recovery_pending = Some(previous);
                self.resolve_recovery(true, window, cx);
                return;
            }
        }
        // No candidate (or Never) must not recreate stored workspaces merely
        // because another launch completed without restorable sessions.
        let snapshot = self.runtime_snapshot(window, cx);
        if let Some(journal) = &self.journal
            && let Err(error) = journal.consume(snapshot, self.data.restore_behavior)
        {
            self.notice = Some(format!("Could not initialize workspace recovery: {error}"));
        }
    }
    fn metadata_for_recovery(&self, snapshot: &Snapshot, restore: bool) -> AppState {
        let mut data = self.data.clone();
        if restore && !snapshot.workspaces.is_empty() {
            data.workspaces = snapshot
                .workspaces
                .iter()
                .map(|workspace| Workspace {
                    id: workspace.id,
                    name: workspace.name.clone(),
                    color: workspace.color,
                })
                .collect();
            data.selected_workspace = snapshot
                .selected_workspace
                .filter(|id| data.workspaces.iter().any(|workspace| workspace.id == *id))
                .unwrap_or(data.workspaces[0].id);
            data.next_id = data.next_id.max(
                data.workspaces
                    .iter()
                    .map(|workspace| workspace.id)
                    .max()
                    .unwrap_or(0)
                    + 1,
            );
        } else {
            let id = data.allocate_id();
            data.workspaces = vec![Workspace {
                id,
                name: "Default".into(),
                color: 0x075e92,
            }];
            data.selected_workspace = id;
        }
        data
    }
    pub(super) fn resolve_recovery(
        &mut self,
        restore: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(previous) = &self.recovery_pending else {
            return;
        };
        let mut snapshot = previous.snapshot.clone();
        let mut candidate = self.metadata_for_recovery(&snapshot, restore);
        if restore && previous.clean_shutdown && self.recovery_always {
            candidate.restore_behavior = RestoreBehavior::Always;
        }
        if !restore {
            snapshot.resource_database = Some(self.store.database_path());
            snapshot.active_forwards.clear();
            snapshot.workspaces = candidate
                .workspaces
                .iter()
                .map(|workspace| WorkspaceSnapshot {
                    id: workspace.id,
                    name: workspace.name.clone(),
                    color: workspace.color,
                    selected_tab: None,
                    tabs: vec![],
                })
                .collect();
            snapshot.selected_workspace = Some(candidate.selected_workspace);
        }
        snapshot.window = Some(window_geometry(window));
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(
                self.resources_ready(),
                "Resolve the metadata error before restoring this workspace."
            );
            self.store.commit_recovery(
                &mut self.data,
                &mut candidate,
                self.journal.as_ref().unwrap(),
                snapshot.clone(),
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            self.recovery_error = Some(error.to_string());
            cx.notify();
            return;
        }
        self.data = candidate;
        self.recovery_pending = None;
        self.recovery_always = false;
        self.recovery_error = None;
        self.last_snapshot = None;
        self.focus.focus(window, cx);
        if restore {
            self.restore_snapshot(snapshot, window, cx);
        } else {
            self.switch_drawer_page(Page::Hosts);
            self.overview = false;
        }
        cx.notify();
    }
    fn restored_layout(
        &self,
        layout: &LayoutSnapshot,
        restore_resources: bool,
    ) -> Option<Node<SessionView>> {
        match layout {
            LayoutSnapshot::Pane { pane } => {
                let sessions = pane
                    .sessions
                    .iter()
                    .filter_map(|session| {
                        if let RestoreTarget::Host { id, .. }
                        | RestoreTarget::TelnetHost { id, .. }
                        | RestoreTarget::MoshHost { id, .. } = session.target
                            && (!restore_resources
                                || !self.data.hosts.iter().any(|host| host.id == id))
                        {
                            return None;
                        }
                        let title = match session.target {
                            RestoreTarget::Host { id, .. }
                            | RestoreTarget::TelnetHost { id, .. }
                            | RestoreTarget::MoshHost { id, .. } => self
                                .data
                                .hosts
                                .iter()
                                .find(|host| host.id == id)
                                .map(|host| host.name.clone())
                                .unwrap_or_else(|| session.title.clone()),
                            _ => session.title.clone(),
                        };
                        Some(SessionView {
                            id: session.id,
                            title,
                            content: TabContent::Preparing(None),
                            ssh: None,
                            restore_target: Some(session.target.clone()),
                            restore_directory: pane.directory.clone(),
                            files: None,
                            tool_files: None,
                            _file_events: vec![],
                            replay: None,
                            _metadata: vec![],
                            preparation_generation: 0,
                        })
                    })
                    .collect();
                Pane::restored(
                    pane.id,
                    sessions,
                    pane.selected_session,
                    pane.composer_visible,
                )
                .map(Node::Pane)
            }
            LayoutSnapshot::Split {
                id,
                axis,
                fractions,
                children,
            } => {
                let kept: Vec<_> = children
                    .iter()
                    .zip(fractions)
                    .filter_map(|(child, fraction)| {
                        self.restored_layout(child, restore_resources)
                            .map(|child| (child, *fraction))
                    })
                    .collect();
                if kept.is_empty() {
                    return None;
                }
                if kept.len() == 1 {
                    return kept.into_iter().next().map(|(node, _)| node);
                }
                let total: f32 = kept.iter().map(|(_, fraction)| fraction).sum();
                Some(Node::Split(Split {
                    id: *id,
                    axis: if *axis == recovery::Axis::Horizontal {
                        SplitAxis::Horizontal
                    } else {
                        SplitAxis::Vertical
                    },
                    fractions: kept.iter().map(|(_, fraction)| fraction / total).collect(),
                    children: kept.into_iter().map(|(node, _)| node).collect(),
                }))
            }
        }
    }
    fn restore_snapshot(
        &mut self,
        mut snapshot: Snapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let restore_resources = snapshot.resources_match(&self.store.database_path());
        if !restore_resources {
            snapshot.active_forwards.clear();
            self.notice = Some(
                "Saved host connections and forwarding rules were skipped because their resource database changed."
                    .into(),
            );
        }
        self.next_tab = snapshot.maximum_runtime_id() + 1;
        self.next_split = snapshot.maximum_split_id() + 1;
        for workspace in &snapshot.workspaces {
            for tab in &workspace.tabs {
                if let Some(root) = self.restored_layout(&tab.layout, restore_resources) {
                    self.terminals.push(TerminalTab {
                        id: tab.id,
                        workspace: workspace.id,
                        show_sftp: tab.show_sftp,
                        tools: super::terminal_files::ToolsState::restore(tab.tools),
                        tree: PaneTree::from_root(root, tab.selected_pane),
                    });
                }
            }
            if let Some(selected) = workspace.selected_tab.filter(|selected| {
                self.terminals
                    .in_workspace(workspace.id)
                    .any(|tab| tab.id == *selected)
            }) {
                self.terminals.select(selected);
            }
        }
        if !self.terminals.is_empty() {
            let page = if !self.data.general.workspace_enabled && self.active_terminal().is_none() {
                Page::Hosts
            } else {
                Page::Sessions
            };
            self.switch_drawer_page(page);
            self.overview = false;
        } else if !snapshot.active_forwards.is_empty() {
            self.switch_drawer_page(Page::PortForwards);
        }
        let forward_ids = snapshot.forward_plan(&self.data);
        let missing_forwards = snapshot.active_forwards.len() - forward_ids.len();
        if missing_forwards > 0 {
            self.notice = Some(format!(
                "Skipped {missing_forwards} saved forwarding rule(s) that are no longer available."
            ));
        }
        // Each start registers its pending state synchronously, then resolves
        // credentials/connects off-thread. A checkpoint cannot omit queued rules.
        for id in forward_ids {
            self.start_forward(id, cx);
        }
        let ids: Vec<_> = self
            .terminals
            .iter()
            .flat_map(|tab| tab.tree.sessions().map(|session| session.id))
            .collect();
        self.restore_task = Some(cx.spawn_in(window, async move |this, cx| {
            for id in ids {
                if this
                    .update_in(cx, |this, window, cx| {
                        this.start_restored_session(id, window, cx)
                    })
                    .is_err()
                {
                    break;
                }
                // Yield between creations; native output remains callback-driven.
                cx.background_executor()
                    .timer(Duration::from_millis(1))
                    .await;
            }
            let _ = this.update_in(cx, |this, _, _| this.restore_task = None);
        }));
    }
    fn start_restored_session(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.exiting.is_some() {
            return;
        }
        let Some(view) = self.session(id) else {
            return;
        };
        let Some(target) = view.restore_target.clone() else {
            return;
        };
        let directory = view.restore_directory.clone();
        let result = (|| -> anyhow::Result<()> {
            let core = self
                .core
                .as_ref()
                .cloned()
                .map(Ok)
                .unwrap_or_else(Core::discover)?;
            self.core = Some(core.clone());
            match target {
                RestoreTarget::Host { id: host_id, port } => {
                    let host = self
                        .data
                        .hosts
                        .iter()
                        .find(|host| host.id == host_id)
                        .cloned()
                        .context("The saved host is no longer available")?;
                    if !host.is_remote() {
                        return self.install_restored_local(
                            id,
                            core,
                            (host.shell_path, directory.or(host.work_dir)),
                            window,
                            cx,
                        );
                    }
                    anyhow::ensure!(
                        host.connections.ssh,
                        "SSH is no longer enabled for this Host."
                    );
                    let workspace = self
                        .terminals
                        .iter()
                        .find(|tab| tab.tree.session(id).is_some())
                        .unwrap()
                        .workspace;
                    let options = SshOptions::new(
                        host.address,
                        port.unwrap_or(host.port),
                        host.username,
                        self.store.paths.known_hosts(),
                    );
                    self.load_host_options(
                        (id, workspace),
                        core,
                        options,
                        (host.credential, host.proxy_id, host.ssh),
                        window,
                        cx,
                    );
                }
                RestoreTarget::MoshHost {
                    id: host_id, port, ..
                } => {
                    let host = self
                        .data
                        .hosts
                        .iter()
                        .find(|host| {
                            host.id == host_id && host.is_remote() && host.connections.mosh
                        })
                        .cloned()
                        .context("Mosh is no longer available for this Host")?;
                    let workspace = self
                        .terminals
                        .iter()
                        .find(|tab| tab.tree.session(id).is_some())
                        .unwrap()
                        .workspace;
                    let options = SshOptions::new(
                        host.address,
                        port.unwrap_or(host.port),
                        host.username,
                        self.store.paths.known_hosts(),
                    );
                    self.load_host_options(
                        (id, workspace),
                        core,
                        options,
                        (host.credential, host.proxy_id, host.ssh),
                        window,
                        cx,
                    );
                }
                RestoreTarget::TelnetHost { id: host_id, port } => {
                    let host = self
                        .data
                        .hosts
                        .iter()
                        .find(|host| host.id == host_id && host.is_remote())
                        .context("The saved Telnet Host is no longer available")?;
                    let source = host
                        .connections
                        .telnet
                        .as_ref()
                        .context("Telnet is no longer enabled for this Host")?
                        .source
                        .clone();
                    self.load_saved_telnet_connection(id, core, (source, port), window, cx);
                }
                RestoreTarget::Local { shell } => {
                    self.install_restored_local(id, core, (shell, directory), window, cx)?;
                }
            }
            Ok(())
        })();
        if let Err(error) = result
            && let Some(view) = self.session_mut(id)
        {
            view.content = TabContent::Preparing(Some(error.to_string()));
        }
        cx.notify();
    }
    fn install_restored_local(
        &mut self,
        id: u64,
        core: Arc<Core>,
        config: (Option<String>, Option<String>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let directory = crate::shells::expand_directory(config.1.as_deref())?;
        let view = self
            .session(id)
            .context("The restored pane no longer exists.")?;
        let title = view.title.clone();
        let appearance = view.appearance(&self.data);
        let host_id = view.host_id();
        let mut session = core.restore_local(
            config.0.as_deref(),
            directory.as_deref(),
            appearance.palette(),
        )?;
        self.record_session(&mut session, &title, &appearance, host_id, cx);
        let terminal = cx.new(|cx| Terminal::new(session, vec![], appearance, None, window, cx));
        if self.session_auto_focus(id) {
            terminal.update(cx, |view, cx| view.focus(window, cx));
        }
        let metadata = self.terminal_subscriptions(&terminal, window, cx);
        if let Some(view) = self.session_mut(id) {
            view.content = TabContent::Terminal(terminal);
            view.ssh = None;
            view._metadata = metadata;
        }
        Ok(())
    }
    pub(super) fn recovery_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let previous = self.recovery_pending.as_ref()?;
        let unexpected = !previous.clean_shutdown;
        let title = if unexpected {
            "Nauterm closed unexpectedly"
        } else {
            "Restore previous workspace?"
        };
        Some(
            crate::components::Modal::new("workspace-recovery-modal", title)
                .focus_scope(&self.recovery_focus)
                .return_focus(&self.focus)
                .on_dismiss(
                    cx.listener(|this, _, window, cx| this.resolve_recovery(false, window, cx)),
                )
                .on_confirm(
                    cx.listener(|this, _, window, cx| this.resolve_recovery(true, window, cx)),
                )
                .content(
                    crate::components::DialogFrame::new("workspace-recovery-dialog", title)
                        .width(430.)
                        .colors(crate::components::DialogColors::workspace(self.data.dark))
                        .content(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(16.))
                                .child(crate::localization::view(if previous.snapshot.active_forwards.is_empty() {
                                    "Restore your previous workspace? Terminals will reconnect."
                                } else if previous.snapshot.has_sessions() {
                                    "Restore your previous workspace? Terminals will reconnect and active port forwards will restart."
                                } else {
                                    "Restore your previous workspace? Active port forwards will restart."
                                }))
                                .when(!unexpected, |view| {
                                    view.child(
                                        Checkbox::new(
                                            "recovery-always",
                                            "Don't ask again",
                                            self.recovery_always,
                                        )
                                        .on_change(
                                            cx.listener(|this, value: &bool, _, cx| {
                                                this.recovery_always = *value;
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                })
                                .when_some(self.recovery_error.clone(), |view, error| {
                                    view.child(
                                        div()
                                            .text_color(rgb(0xca3f37))
                                            .child(crate::localization::message(cx, &error)),
                                    )
                                }),
                        )
                        .action(
                            Button::new("recovery-fresh")
                                .ghost()
                                .label("Start Fresh")
                                .on_click(|_, window, cx| {
                                    crate::components::Modal::dismiss(window, cx)
                                }),
                        )
                        .action(
                            Button::new("recovery-restore")
                                .primary()
                                .label("Restore Workspace")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.resolve_recovery(true, window, cx)
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}
