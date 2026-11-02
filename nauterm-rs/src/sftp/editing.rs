//! The remote edit continues to its captured host, never a later pane binding.
use super::*;
use crate::{
    editor_config::Editor,
    external_application::{self},
    remote_edit::{self, Document, Event, Handle, Resolution},
};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Edits {
    sessions: BTreeMap<u64, Session>,
    visible: Option<u64>,
    pub(super) opening: bool,
}
struct Session {
    handle: Handle,
    host: String,
    conflict: Option<Conflict>,
    authorization: Option<PendingAuthorization>,
    _events: Task<()>,
}
struct Conflict {
    generation: u64,
    path: String,
    missing: bool,
}
struct PendingAuthorization {
    generation: u64,
    path: String,
    local_path: PathBuf,
    error: String,
    offered: bool,
}
impl Edits {
    pub(super) fn has_dialog(&self) -> bool {
        self.visible.is_some()
    }
    pub(super) fn has_work(&self) -> bool {
        self.sessions.values().any(|edit| edit.handle.is_running())
    }
    pub(super) fn awaiting_dialog(&self) -> bool {
        self.visible.is_none()
            && self.sessions.values().any(|edit| {
                edit.conflict.is_some()
                    || edit
                        .authorization
                        .as_ref()
                        .is_some_and(|auth| !auth.offered)
            })
    }
}
struct Opening(Option<Arc<remote_edit::Connection>>);
impl Drop for Opening {
    fn drop(&mut self) {
        if let Some(connection) = &self.0 {
            connection.cancel();
        }
    }
}
static NEXT_EDIT: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(super) struct OpenOptions {
    pub editor: Option<Editor>,
    pub choose: bool,
    pub sudo: Option<Arc<crate::sftp_client::Sudo>>,
}

impl FilePane {
    pub(super) fn remote_edit_current(&self, token: u64, path: &str) -> bool {
        let entries = self.selected_entries();
        !crate::transfer_queue::is_shutting_down()
            && self.active
            && !self.loading
            && self.stage == Stage::Files
            && self.remote.is_some()
            && self.endpoint_token == token
            && entries.len() == 1
            && entries[0].path == path
            && !entries[0].directory
    }
    pub(super) fn open_remote_file(
        &mut self,
        token: u64,
        path: String,
        selected: Option<Editor>,
        choose: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_remote_file_authorized(
            token,
            path,
            OpenOptions {
                editor: selected,
                choose,
                sudo: None,
            },
            window,
            cx,
        );
    }
    pub(super) fn open_remote_file_authorized(
        &mut self,
        token: u64,
        path: String,
        open: OpenOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.remote_edit_current(token, &path)
            || self.action_work.is_some()
            || self.dialog.is_some()
        {
            return;
        }
        let Some(remote) = &self.remote else {
            return;
        };
        let core = match Core::discover() {
            Ok(core) => core,
            Err(error) => {
                self.notice = Some(Message::error(error.to_string()));
                cx.notify();
                return;
            }
        };
        let client = self.tasks.update(cx, |queue, cx| queue.client(token, cx));
        let elevated = open.sudo.is_some();
        let connection = if elevated {
            remote_edit::Connection::new_queued_authorized(
                core,
                remote.options.clone(),
                client,
                open.sudo,
            )
        } else {
            remote_edit::Connection::new_queued(core, remote.options.clone(), client)
        };
        let mut host = format!(
            "{}@{}:{}",
            remote.options.username, remote.options.host, remote.options.port
        );
        if elevated {
            host.push_str(" · sudo");
        }
        let selected = open.editor;
        let choose = open.choose;
        let mut opening = Opening(Some(connection.clone()));
        self.edits.opening = true;
        self.notice = Some(Message::info("Downloading a private copy for editing…"));
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            let result = async {
                #[cfg(target_os = "macos")]
                let selected = if choose {
                    let choice = this.update_in(cx, |pane, window, cx| {
                        if pane.remote_edit_current(token, &path)
                            && window.is_window_active()
                            && !crate::components::overlay::is_modal_open(window, cx)
                        {
                            Some(external_application::choose())
                        } else {
                            None
                        }
                    })?;
                    let Some(choice) = choice else {
                        return Ok(false);
                    };
                    let Some(selected) = choice?.receive().await? else {
                        return Ok(false);
                    };
                    Some(Editor::from(selected))
                } else {
                    selected
                };
                let explicit = choose || selected.is_some();
                let download_path = path.clone();
                let document = cx
                    .background_spawn(async move {
                        Document::download(connection, download_path, explicit)
                    })
                    .await?;
                let started = this.update_in(cx, |pane, window, cx| {
                    if !pane.remote_edit_current(token, &path)
                        || crate::components::overlay::is_modal_open(window, cx)
                    {
                        return Ok::<_, anyhow::Error>(None);
                    }
                    let local_path = document.local_path.clone();
                    let handle = Handle::start(document)?;
                    pane.edits.opening = false;
                    opening.0.take();
                    let id = pane.install_edit(handle, host, window, cx);
                    {
                        let receiver =
                            match external_application::open_editor(&local_path, selected.as_ref())
                            {
                                Ok(receiver) => receiver,
                                Err(error) => {
                                    pane.edits.sessions.remove(&id);
                                    return Err(error);
                                }
                            };
                        // Once handed to the OS, launch completion belongs to
                        // the installed edit, not the cancellable opening task.
                        cx.spawn_in(window, async move |pane, cx| {
                            let result = async {
                                receiver.recv().await??;
                                Ok::<_, anyhow::Error>(())
                            }
                            .await;
                            if let Err(error) = result {
                                let _ = pane.update(cx, |pane, cx| {
                                    if let Some(edit) = pane.edits.sessions.remove(&id) {
                                        pane.notice = Some(Message::error(format!(
                                            "{}: Unable to open external application: {error}",
                                            edit.host
                                        )));
                                        cx.notify();
                                    }
                                });
                            }
                        })
                        .detach();
                        Ok(Some(id))
                    }
                })??;
                Ok::<_, anyhow::Error>(started.is_some())
            }
            .await;
            let _ = this.update(cx, |pane, cx| {
                if pane.endpoint_token != token {
                    return;
                }
                pane.action_work = None;
                pane.edits.opening = false;
                pane.notice = match result {
                    Ok(true) => Some(Message::info(
                        "External edit request started; saves are checked before upload.",
                    )),
                    Ok(false) => None,
                    Err(error) => Some(Message::error(format!(
                        "Unable to open remote file: {error}"
                    ))),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn install_edit(
        &mut self,
        handle: Handle,
        host: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> u64 {
        let id = NEXT_EDIT.fetch_add(1, Ordering::Relaxed);
        let receiver = handle.events.clone();
        let events = cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                let _ = this.update(cx, |pane, cx| {
                    let Some(edit) = pane.edits.sessions.get_mut(&id) else {
                        return;
                    };
                    match event {
                        Event::Status(message) => {
                            pane.notice = Some(Message::info(format!("{}: {message}", edit.host)))
                        }
                        Event::Conflict {
                            generation,
                            remote_path,
                            missing,
                        } => {
                            edit.conflict = Some(Conflict {
                                generation,
                                path: remote_path,
                                missing,
                            })
                        }
                        Event::AuthorizationRequired {
                            generation,
                            remote_path,
                            local_path,
                            error,
                        } => {
                            edit.conflict = None;
                            edit.authorization = Some(PendingAuthorization {
                                generation,
                                path: remote_path,
                                local_path,
                                error,
                                offered: false,
                            });
                        }
                        Event::Stopped { recovery } => {
                            pane.notice = recovery.map(|path| {
                                Message::info(format!(
                                    "Unsaved edits are retained in {}",
                                    path.display()
                                ))
                            });
                            pane.edits.sessions.remove(&id);
                            pane.cancel_edit_sudo_dialog(id, cx);
                            if pane.edits.visible == Some(id) {
                                pane.edits.visible = None;
                            }
                        }
                    }
                    cx.notify();
                });
            }
        });
        self.edits.sessions.insert(
            id,
            Session {
                handle,
                host,
                conflict: None,
                authorization: None,
                _events: events,
            },
        );
        id
    }
    pub(super) fn cancel_edit_opening(&mut self, cx: &mut Context<Self>) {
        if self.edits.opening {
            self.action_work = None;
            self.edits.opening = false;
            self.notice = Some(Message::info("Remote file opening cancelled."));
            cx.notify();
        }
    }
    pub(super) fn sync_edit_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.active
            || self.edits.visible.is_some_and(|id| {
                self.edits
                    .sessions
                    .get(&id)
                    .is_none_or(|edit| edit.conflict.is_none())
            })
        {
            self.edits.visible = None;
        }
        if self.active
            && self.edits.visible.is_none()
            && self.dialog.is_none()
            && !crate::components::overlay::is_modal_open(window, cx)
        {
            self.edits.visible = self
                .edits
                .sessions
                .iter()
                .find(|(_, edit)| edit.conflict.is_some())
                .map(|(id, _)| *id);
            if self.edits.visible.is_none()
                && window.is_window_active()
                && let Some((id, generation)) = self.edits.sessions.iter().find_map(|(id, edit)| {
                    edit.authorization
                        .as_ref()
                        .filter(|auth| !auth.offered)
                        .map(|auth| (*id, auth.generation))
                })
            {
                self.begin_edit_authorization(id, generation, window, cx);
            }
        }
    }
    pub(super) fn edit_authorization_current(&self, id: u64, generation: u64) -> bool {
        self.active
            && !crate::transfer_queue::is_shutting_down()
            && self.edits.sessions.get(&id).is_some_and(|edit| {
                edit.handle.is_running()
                    && edit
                        .authorization
                        .as_ref()
                        .is_some_and(|auth| auth.generation == generation)
            })
    }
    fn begin_edit_authorization(
        &mut self,
        id: u64,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.edit_authorization_current(id, generation)
            || self.dialog.is_some()
            || self.action_work.is_some()
            || self.edits.visible.is_some()
            || crate::components::overlay::is_modal_open(window, cx)
            || !window.is_window_active()
        {
            return;
        }
        let edit = self.edits.sessions.get_mut(&id).unwrap();
        let auth = edit.authorization.as_mut().unwrap();
        auth.offered = true;
        match edit.handle.authorization_target(generation, &auth.path) {
            Ok(target) => self.edit_sudo_dialog(id, generation, target, window, cx),
            Err(error) => self.notice = Some(Message::error(error.to_string())),
        }
        cx.notify();
    }
    pub(super) fn complete_edit_authorization(
        &mut self,
        id: u64,
        generation: u64,
        sudo: Arc<crate::sftp_client::Sudo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.edit_authorization_current(id, generation) {
            return;
        }
        let edit = self.edits.sessions.get_mut(&id).unwrap();
        match edit.handle.reauthorize(generation, sudo) {
            Ok(()) => {
                edit.authorization = None;
                self.notice = Some(Message::info(format!(
                    "{}: Authentication renewed; checking changes before saving.",
                    edit.host
                )));
            }
            Err(error) => self.notice = Some(Message::error(error.to_string())),
        }
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn edit_authorization_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let waiting: Vec<_> = self
            .edits
            .sessions
            .iter()
            .filter_map(|(id, edit)| edit.authorization.as_ref().map(|auth| (*id, edit, auth)))
            .collect();
        if waiting.is_empty() {
            return None;
        }
        let p = Palette::new(self.data.dark);
        Some(
            div()
                .id("sftp-edit-authorization-notices")
                .w_full()
                .flex_shrink_0()
                .max_h(px(150.))
                .overflow_y_scroll()
                .scrollbars()
                .children(waiting.into_iter().map(|(id, edit, auth)| {
                    let generation = auth.generation;
                    div()
                        .w_full()
                        .p(px(8.))
                        .border_b_1()
                        .border_color(p.border)
                        .bg(p.surface)
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .child(div().text_size(px(12.)).truncate().child(
                                    crate::localization::view(format!(
                                        "Save paused · {}",
                                        edit.host
                                    )),
                                ))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.muted)
                                        .truncate()
                                        .child(auth.path.clone()),
                                )
                                .child(div().text_size(px(11.)).text_color(p.muted).child(
                                    crate::localization::text(cx, "rs.ui.copy.localChangesKept"),
                                )),
                        )
                        .child(
                            Button::new(("sftp-edit-reauthenticate", id))
                                .xsmall()
                                .secondary()
                                .label("Authenticate")
                                .tooltip(format!(
                                    "{}\nLocal copy: {}",
                                    auth.error,
                                    auth.local_path.display()
                                ))
                                .disabled(
                                    !self.active
                                        || self.dialog.is_some()
                                        || self.action_work.is_some()
                                        || self.edits.visible.is_some(),
                                )
                                .on_click(cx.listener(move |pane, _, window, cx| {
                                    pane.begin_edit_authorization(id, generation, window, cx)
                                })),
                        )
                }))
                .into_any_element(),
        )
    }
    fn resolve_edit(
        &mut self,
        id: u64,
        generation: u64,
        resolution: Resolution,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.edits.sessions.get_mut(&id)
            && let Some(conflict) = &edit.conflict
            && conflict.generation == generation
        {
            match edit.handle.resolve(conflict.generation, resolution) {
                Ok(()) => {
                    edit.conflict = None;
                    self.edits.visible = None;
                }
                Err(error) => self.notice = Some(Message::error(error.to_string())),
            }
        }
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn edit_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.edits.visible?;
        let edit = self.edits.sessions.get(&id)?;
        let conflict = edit.conflict.as_ref()?;
        let generation = conflict.generation;
        let owner = cx.weak_entity();
        Some(
            crate::components::FileConflictDialog::new(
                SharedString::from(format!("remote-edit-{id}-{generation}")),
                edit.host.clone(),
                conflict.path.clone(),
                conflict.missing,
                self.data.dark,
                &self.focus,
                move |resolution, window, cx| {
                    let _ = owner.update(cx, |pane, cx| {
                        pane.resolve_edit(id, generation, resolution, window, cx)
                    });
                },
            )
            .into_any_element(),
        )
    }
}
