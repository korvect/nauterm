use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct State {
    pub host_id: Option<Id>,
    cancel: Arc<AtomicBool>,
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl Nauterm {
    pub(super) fn open_recording_replay(
        &mut self,
        entry: crate::recordings::Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.ensure_recordings(cx) else {
            return;
        };
        let workspace = self.data.selected_workspace;
        let id = self.next_tab;
        self.next_tab += 1;
        let host_id = entry
            .host_uuid
            .as_deref()
            .and_then(|uuid| {
                self.data
                    .hosts
                    .iter()
                    .find(|host| host.uuid.as_deref() == Some(uuid))
            })
            .map(|host| host.id);
        let appearance = self.data.terminal_for_host(host_id);
        let cancel = Arc::new(AtomicBool::new(false));
        self.terminals.push(TerminalTab {
            id,
            workspace,
            show_sftp: false,
            tools: Default::default(),
            tree: PaneTree::new_with_composer(
                id,
                SessionView {
                    id,
                    title: format!("Replay: {}", entry.title),
                    content: TabContent::Preparing(None),
                    ssh: None,
                    restore_target: None,
                    restore_directory: None,
                    files: None,
                    tool_files: None,
                    replay: Some(State {
                        host_id,
                        cancel: cancel.clone(),
                    }),
                    _file_events: vec![],
                    _metadata: vec![],
                    preparation_generation: 0,
                },
                false,
            ),
        });
        if !self.dismiss_editor(cx) {
            return;
        }
        self.retire_history();
        self.selection.clear();
        self.notice = None;
        self.show_session(id, false, window, cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = service.replay(entry, appearance.palette(), cancel).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !this.terminals.iter().any(|tab| {
                    tab.workspace == workspace
                        && tab
                            .tree
                            .session(id)
                            .is_some_and(|session| session.replay.is_some())
                }) {
                    return;
                }
                match result {
                    Ok(session) => {
                        let appearance = this.session(id).unwrap().appearance(&this.data);
                        let terminal = cx
                            .new(|cx| Terminal::new(session, vec![], appearance, None, window, cx));
                        let metadata = this.terminal_subscriptions(&terminal, window, cx);
                        if this.session_auto_focus(id) {
                            terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
                        }
                        if let Some(view) = this.session_mut(id) {
                            view.content = TabContent::Terminal(terminal);
                            view._metadata = metadata;
                        }
                    }
                    Err(error) => {
                        if let Some(view) = this.session_mut(id) {
                            view.content = TabContent::Preparing(Some(format!(
                                "Unable to replay this recording: {error}"
                            )));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
