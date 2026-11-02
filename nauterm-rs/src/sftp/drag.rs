//! Internal pane drags carry metadata and weak origins, never credentials.
use super::*;
use crate::components::drag::{self, Payload};
use crate::pane_transfer::{Batch, Endpoint};
fn keys(entries: Vec<Entry>) -> BTreeSet<(String, Option<PathBuf>, bool)> {
    entries
        .into_iter()
        .map(|entry| (entry.path, entry.native_path, entry.directory))
        .collect()
}

#[derive(Clone)]
struct Source {
    view: WeakEntity<FilePane>,
    token: u64,
    directory: String,
    remote: bool,
    entries: Vec<Entry>,
}
impl Source {
    fn matches(&self, pane: &FilePane) -> bool {
        pane.active
            && !pane.loading
            && pane.dialog.is_none()
            && pane.action_work.is_none()
            && pane.stage == Stage::Files
            && pane.endpoint_token == self.token
            && pane.history.path == self.directory
            && pane.remote.is_some() == self.remote
            && keys(pane.selected_entries()) == keys(self.entries.clone())
    }
}
#[derive(Clone)]
pub(super) struct Scope {
    source: Option<Source>,
    token: u64,
    directory: String,
    remote: bool,
}
impl Scope {
    pub(super) fn destination(pane: &FilePane) -> Self {
        Self {
            source: None,
            token: pane.endpoint_token,
            directory: pane.history.path.clone(),
            remote: pane.remote.is_some(),
        }
    }
    pub(super) fn matches(&self, pane: &FilePane, cx: &App) -> bool {
        (if self.source.is_some() {
            pane.active
        } else {
            pane.presented
        }) && (!pane.loading
            || (self.source.is_none()
                && pane.retry.as_ref().is_some_and(|(path, navigation)| {
                    path == &self.directory && matches!(navigation, Navigation::Refresh)
                })))
            && pane.stage == Stage::Files
            && pane.endpoint_token == self.token
            && pane.history.path == self.directory
            && pane.remote.is_some() == self.remote
            && self.source.as_ref().is_none_or(|origin| {
                pane.peer
                    .as_ref()
                    .is_some_and(|peer| peer.entity_id() == origin.view.entity_id())
                    && origin
                        .view
                        .upgrade()
                        .is_some_and(|source| origin.matches(source.read(cx)))
            })
    }
}
pub(super) fn row(button: Button, owner: WeakEntity<FilePane>, entry: &Entry) -> Button {
    let entry = entry.clone();
    drag::source(button, move |window, cx| {
        owner
            .update(cx, |pane, cx| {
                if !pane.active
                    || pane.loading
                    || pane.dialog.is_some()
                    || pane.action_work.is_some()
                    || pane.stage != Stage::Files
                    || !pane.entries.iter().any(|current| {
                        current.path == entry.path
                            && current.native_path == entry.native_path
                            && !current.parent
                    })
                {
                    return None;
                }
                pane.focus.focus(window, cx);
                if !pane.selection.paths.contains(&entry.path) {
                    pane.selection = Selection::single(entry.path.clone());
                }
                cx.notify();
                let entries = pane.selected_entries();
                Some(Payload {
                    label: entry.name.clone(),
                    count: entries.len(),
                    dark: pane.data.dark,
                    icon: if entry.directory {
                        IconName::Folder
                    } else {
                        IconName::File
                    },
                    value: Source {
                        view: cx.weak_entity(),
                        token: pane.endpoint_token,
                        directory: pane.history.path.clone(),
                        remote: pane.remote.is_some(),
                        entries,
                    },
                })
            })
            .ok()
            .flatten()
    })
}
impl FilePane {
    fn can_receive(&self, source: &Source, cx: &App) -> bool {
        !source.entries.is_empty()
            && !crate::transfer_queue::is_shutting_down()
            && (source.remote || self.remote.is_some())
            && self.dialog.is_none()
            && self.action_work.is_none()
            && self
                .peer
                .as_ref()
                .is_some_and(|peer| peer.entity_id() == source.view.entity_id())
            && Scope {
                source: Some(source.clone()),
                token: self.endpoint_token,
                directory: self.history.path.clone(),
                remote: self.remote.is_some(),
            }
            .matches(self, cx)
    }
    fn receive(&mut self, source: &Source, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_receive(source, cx) || crate::transfer_queue::is_shutting_down() {
            return;
        }
        let source = source.clone();
        let scope = Scope {
            source: Some(source.clone()),
            token: self.endpoint_token,
            directory: self.history.path.clone(),
            remote: self.remote.is_some(),
        };
        let source_endpoint = source.view.upgrade().unwrap().read(cx).transfer_endpoint();
        let target_endpoint = self.transfer_endpoint();
        let target_owner = self.endpoint_token;
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    Batch::plan(
                        source_endpoint,
                        target_endpoint,
                        source.entries,
                        source.token,
                        target_owner,
                        None,
                    )
                })
                .await;
            let _ = this.update_in(cx, |pane, window, cx| {
                pane.action_work = None;
                if !scope.matches(pane, cx) {
                    pane.notice = Some(Message::info(
                        "Transfer cancelled because a pane or selection changed.",
                    ));
                    cx.notify();
                    return;
                }
                match result {
                    Ok(batch) => pane.cross_dialog(batch, scope, window, cx),
                    Err(error) => {
                        pane.notice = Some(Message::error(error.to_string()));
                        cx.notify();
                    }
                }
            });
        }));
        cx.notify();
    }
    fn transfer_endpoint(&self) -> Endpoint {
        match &self.remote {
            Some(remote) => Endpoint::Remote {
                directory: self.history.path.clone(),
                options: remote.options.clone(),
            },
            None => Endpoint::Local(self.history.path.clone().into()),
        }
    }
    fn can_receive_external(&self, paths: &ExternalPaths) -> bool {
        !paths.paths().is_empty()
            && !crate::transfer_queue::is_shutting_down()
            // Finder/the system file manager still owns activation during drag.
            // A mounted destination must not require keyboard focus to accept it.
            && self.presented
            && !self.loading
            && self.stage == Stage::Files
            && self.remote.is_some()
            && self.dialog.is_none()
            && self.action_work.is_none()
            && !self.edits.opening
    }
    fn receive_external(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_receive_external(paths)
            || crate::components::overlay::is_modal_open(window, cx)
        {
            return;
        }
        self.upload_paths(paths.paths().to_vec(), window, cx);
    }
    pub(super) fn upload_paths(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.upload_paths_authorized(paths, None, window, cx);
    }
    pub(super) fn upload_paths_authorized(
        &mut self,
        paths: Vec<PathBuf>,
        sudo: Option<Arc<crate::sftp_client::Sudo>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scope = Scope::destination(self);
        if paths.is_empty()
            || !scope.matches(self, cx)
            || self.remote.is_none()
            || self.dialog.is_some()
            || self.action_work.is_some()
            || self.edits.opening
            || crate::transfer_queue::is_shutting_down()
        {
            return;
        }
        let target = self.transfer_endpoint();
        let owner = self.endpoint_token;
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::file_drop::Uploads::plan(paths, target, owner, None)
                })
                .await;
            let _ = this.update_in(cx, |pane, window, cx| {
                pane.action_work = None;
                if !scope.matches(pane, cx) {
                    pane.notice = Some(Message::info(
                        "Upload cancelled because the destination changed.",
                    ));
                    cx.notify();
                    return;
                }
                match result {
                    Ok(uploads) => {
                        pane.continue_uploads_authorized(uploads, scope, sudo, window, cx)
                    }
                    Err(error) => {
                        pane.notice = Some(Message::error(error.to_string()));
                        cx.notify();
                    }
                }
            });
        }));
        cx.notify();
    }
    pub(super) fn drop_area(&self, content: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let content = drag::target(
            format!("sftp-drop-area-{}", cx.entity_id()),
            content,
            self.data.dark,
            if self.remote.is_some() {
                "Upload here"
            } else {
                "Download here"
            },
            move |source, cx| {
                owner
                    .upgrade()
                    .is_some_and(|pane| pane.read(cx).can_receive(source, cx))
            },
            cx.listener(|pane, source: &Source, window, cx| pane.receive(source, window, cx)),
        );
        let owner = cx.weak_entity();
        drag::external_target(
            format!("sftp-external-drop-{}", cx.entity_id()),
            content,
            self.data.dark,
            move |paths, cx| {
                owner
                    .upgrade()
                    .is_some_and(|pane| pane.read(cx).can_receive_external(paths))
            },
            cx.listener(|pane, paths: &ExternalPaths, window, cx| {
                pane.receive_external(paths, window, cx)
            }),
        )
    }
}
