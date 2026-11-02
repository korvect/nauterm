//! Remote actions share basic controls; preflight never changes a destination.
use super::*;
use crate::components::menu::Menu;
use crate::{
    components::Field,
    file_operation::{self, Operation},
};
use anyhow::{Context as _, Result, bail, ensure};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Open,
    Close,
    SelectAll,
    NewFolder,
    Upload,
    Rename,
    Move,
    Copy,
    Download,
    Delete,
}
impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Close => "Close",
            Self::SelectAll => "Select All",
            Self::NewFolder => "New Folder",
            Self::Upload => "Upload",
            Self::Rename => "Rename",
            Self::Move => "Move to",
            Self::Copy => "Copy to",
            Self::Download => "Download",
            Self::Delete => "Delete",
        }
    }
}

fn destination_operation(
    action: Action,
    source: &str,
    text: &str,
    base: &str,
) -> Result<Operation> {
    if action == Action::Rename {
        file_operation::filename(text)?;
    }
    let target = file_operation::destination(text, base)?;
    let operation = match action {
        Action::Copy => Operation::Copy {
            source_path: source.to_owned(),
            target_path: target,
            replace_existing: false,
        },
        Action::Rename | Action::Move => Operation::Move {
            source_path: source.to_owned(),
            target_path: target,
            replace_existing: false,
        },
        _ => bail!("This action does not accept a destination."),
    };
    operation.validate()?;
    Ok(operation)
}

pub(super) struct ActionDialog {
    pub visible: bool,
    id: u64,
    action: Action,
    input: Entity<InputState>,
    entries: Vec<Entry>,
    options: Arc<SshOptions>,
    token: u64,
    base: String,
    operations: Vec<Operation>,
    prepared: Option<Prepared>,
    busy: bool,
    error: Option<String>,
    validation_error: Option<SharedString>,
    _cancel: Vec<Cancellation>,
    _input: Subscription,
    cross: Option<(super::drag::Scope, crate::pane_transfer::Batch)>,
    uploads: Option<crate::file_drop::Uploads>,
    sudo: Option<Arc<crate::sftp_client::Sudo>>,
    awaiting_sudo: bool,
    open_with: Option<(Option<crate::editor_config::Editor>, bool)>,
    edit_authorization: Option<(u64, u64)>,
    task_permission: Option<crate::transfer_queue::PermissionTicket>,
    permission_details: Option<(String, String, String)>,
    preflight_input: Option<String>,
}
static NEXT_ACTION: AtomicU64 = AtomicU64::new(1);
struct Prepared {
    operations: Vec<Operation>,
    alternatives: BTreeMap<usize, String>,
}
impl Prepared {
    fn resolve(mut self, replace: bool, keep_both: bool) -> (Vec<Operation>, HashSet<String>) {
        let mut local_replacements = HashSet::new();
        for (index, operation) in self.operations.iter_mut().enumerate() {
            if keep_both && let Some(path) = self.alternatives.get(&index) {
                *target_mut(operation) = path.clone();
            }
            if replace && self.alternatives.contains_key(&index) {
                match operation {
                    Operation::Move {
                        replace_existing, ..
                    }
                    | Operation::Copy {
                        replace_existing, ..
                    }
                    | Operation::Upload {
                        replace_existing, ..
                    } => *replace_existing = true,
                    Operation::Download { local_path, .. } => {
                        local_replacements.insert(local_path.clone());
                    }
                    _ => {}
                }
            }
        }
        (self.operations, local_replacements)
    }
}
fn path_text(path: &std::path::Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .context("This filename cannot be represented by the SFTP API. No items were transferred.")
}

/// Shared pane actions for the toolbar and the file-list background. Row menus
/// keep their own file selection; a background gesture clears it before opening.
pub(super) fn browser_menu(menu: Menu, owner: WeakEntity<FilePane>, cx: &App) -> Menu {
    let Some(pane) = owner.upgrade() else {
        return menu;
    };
    let pane = pane.read(cx);
    let hidden = pane.hidden;
    let remote = pane.remote.is_some();
    let close_allowed = remote && !pane.compact;
    let selected = pane.selected_entries().len();
    let refresh = owner.clone();
    let toggle = owner.clone();
    let copy = owner.clone();
    let close = owner.clone();
    let menu = menu
        .max_visible_rows(None)
        .item(
            MenuItem::localized("Refresh").on_click(move |_, window, cx| {
                let _ = refresh.update(cx, |pane, cx| pane.refresh(window, cx));
            }),
        )
        .item(
            MenuItem::localized("Show hidden files")
                .checked(hidden)
                .on_click(move |_, _, cx| {
                    let _ = toggle.update(cx, |pane, cx| {
                        pane.hidden = !pane.hidden;
                        cx.notify();
                    });
                }),
        )
        .item(
            MenuItem::localized("rs.sftp.action.copyPath").on_click(move |_, _, cx| {
                let _ = copy.update(cx, |pane, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(pane.history.path.clone()))
                });
            }),
        )
        .when(close_allowed, |menu| {
            menu.item(MenuItem::localized("Close").destructive(true).on_click(
                move |_, window, cx| {
                    let _ = close.update(cx, |pane, cx| pane.disconnect(window, cx));
                },
            ))
        });
    self::menu(menu, owner, selected, remote, cx)
}

pub(super) fn menu(
    mut menu: Menu,
    owner: WeakEntity<FilePane>,
    selected: usize,
    remote: bool,
    cx: &App,
) -> Menu {
    if let Some(pane) = owner.upgrade()
        && pane.read(cx).compact
    {
        menu = menu.palette(pane.read(cx).tool_menu_palette());
    }
    menu = menu.separator();
    let actions = if remote {
        &[
            Action::Open,
            Action::Download,
            Action::Upload,
            Action::Rename,
            Action::Move,
            Action::Copy,
            Action::Delete,
            Action::NewFolder,
        ][..]
    } else {
        &[
            Action::Open,
            Action::NewFolder,
            Action::SelectAll,
            Action::Close,
        ][..]
    };
    let openable = owner.upgrade().is_some_and(|pane| {
        let entries = pane.read(cx).selected_entries();
        entries.len() == 1
            && entries
                .iter()
                .all(|entry| entry.directory || entry.can_open)
    });
    for &action in actions {
        let invoke = owner.clone();
        let disabled = match action {
            Action::Open => !openable,
            Action::Close | Action::SelectAll => false,
            Action::NewFolder | Action::Upload => false,
            Action::Rename | Action::Move | Action::Copy => selected != 1,
            _ => selected == 0,
        };
        menu = menu.item(
            MenuItem::localized(action.label())
                .destructive(matches!(action, Action::Delete | Action::Close))
                .disabled(disabled)
                .on_click(move |_, window, cx| {
                    let _ = invoke.update(cx, |this, cx| this.action(action, window, cx));
                }),
        );
        if action == Action::Open {
            menu = super::applications::menu(menu, owner.clone(), cx);
        }
    }
    if remote {
        let mut elevated = Menu::default();
        for action in [
            Action::Open,
            Action::Download,
            Action::Upload,
            Action::Rename,
            Action::Move,
            Action::Copy,
            Action::Delete,
            Action::NewFolder,
        ] {
            let invoke = owner.clone();
            let disabled = match action {
                Action::Open => !openable,
                Action::Upload | Action::NewFolder => false,
                Action::Rename | Action::Move | Action::Copy => selected != 1,
                _ => selected == 0,
            };
            elevated = elevated.item(
                MenuItem::localized(action.label())
                    .disabled(disabled)
                    .destructive(true)
                    .on_click(move |_, window, cx| {
                        let _ = invoke.update(cx, |pane, cx| pane.action_sudo(action, window, cx));
                    }),
            );
            if action == Action::Open {
                elevated = super::applications::sudo_menu(elevated, owner.clone(), cx);
            }
        }
        menu = menu.separator().item(
            MenuItem::localized("rs.sftp.action.withSudo")
                .destructive(true)
                .submenu(elevated),
        );
    }
    menu
}
fn parent(path: &str) -> (&str, &str) {
    let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
    (if parent.is_empty() { "/" } else { parent }, name)
}
pub(super) fn alternate(
    path: &str,
    is_directory: bool,
    mut exists: impl FnMut(&str) -> bool,
) -> String {
    let (directory, name) = parent(path);
    let (stem, extension) = if !is_directory {
        name.rsplit_once('.')
            .filter(|(stem, _)| !stem.is_empty())
            .map(|(stem, extension)| (stem, format!(".{extension}")))
            .unwrap_or((name, String::new()))
    } else {
        (name, String::new())
    };
    for n in 1.. {
        let candidate = format!(
            "{}/{} ({n}){}",
            directory.trim_end_matches('/'),
            stem,
            extension
        );
        if !exists(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}
fn cross_alternate(
    path: &str,
    local: bool,
    mut exists: impl FnMut(&str) -> bool,
) -> Result<String> {
    let (directory, name) = if local {
        let path = std::path::Path::new(path);
        (
            path_text(
                path.parent()
                    .context("Choose a local destination directory.")?,
            )?,
            path.file_name()
                .and_then(|name| name.to_str())
                .context("Invalid local filename.")?
                .to_owned(),
        )
    } else {
        let (directory, name) = parent(path);
        (directory.to_owned(), name.to_owned())
    };
    let (stem, extension) = name
        .rsplit_once('.')
        .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty())
        .map(|(stem, ext)| (stem, format!(".{ext}")))
        .unwrap_or((&name, String::new()));
    for index in 1.. {
        let suffix = if local {
            format!(" {}", index + 1)
        } else if index == 1 {
            " copy".into()
        } else {
            format!(" copy {index}")
        };
        let name = format!("{stem}{suffix}{extension}");
        let candidate = if local {
            path_text(&std::path::Path::new(&directory).join(name))?
        } else {
            format!("{}/{name}", directory.trim_end_matches('/'))
        };
        if !exists(&candidate) {
            return Ok(candidate);
        }
    }
    unreachable!()
}
fn target_mut(operation: &mut Operation) -> &mut String {
    match operation {
        Operation::Mkdir { target_path }
        | Operation::Delete { target_path }
        | Operation::Copy { target_path, .. }
        | Operation::Move { target_path, .. } => target_path,
        Operation::Upload { remote_path, .. } => remote_path,
        Operation::Download { local_path, .. } => local_path,
    }
}
fn prepare(
    operations: Vec<Operation>,
    requests: Vec<(String, Request)>,
    options: &SshOptions,
    kinds: &BTreeMap<String, bool>,
    cross: bool,
    reserved: &HashSet<String>,
) -> Result<Prepared> {
    prepare_authorized(operations, requests, options, kinds, cross, reserved, None)
}
fn prepare_authorized(
    operations: Vec<Operation>,
    requests: Vec<(String, Request)>,
    options: &SshOptions,
    kinds: &BTreeMap<String, bool>,
    cross: bool,
    reserved: &HashSet<String>,
    sudo: Option<&crate::sftp_client::Sudo>,
) -> Result<Prepared> {
    let mut remote = BTreeMap::new();
    for (directory, request) in requests {
        let listing = request.list_authorized(options, &directory, sudo, None)?;
        if let Some(error) = listing.error {
            bail!("{error}");
        }
        remote.insert(
            directory,
            listing
                .entries
                .into_iter()
                .map(|entry| entry.name)
                .collect::<HashSet<_>>(),
        );
    }
    let mut alternatives = BTreeMap::new();
    let mut targets = HashSet::new();
    for operation in &operations {
        operation.validate()?;
        ensure!(
            targets.insert(operation.target().to_owned()),
            "Two selected items have the same destination. Transfer them separately or choose distinct names."
        );
    }
    for (index, operation) in operations.iter().enumerate() {
        if matches!(
            operation,
            Operation::Delete { .. } | Operation::Mkdir { .. }
        ) {
            continue;
        }
        let target = operation.target();
        let exists = reserved.contains(target)
            || if matches!(operation, Operation::Download { .. }) {
                match std::fs::symlink_metadata(target) {
                    Ok(_) => true,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                    Err(error) => return Err(error.into()),
                }
            } else {
                let (directory, name) = parent(target);
                remote
                    .get(directory)
                    .is_some_and(|names| names.contains(name))
            };
        if exists {
            let is_directory = match operation {
                Operation::Upload { local_path, .. } => match kinds.get(local_path) {
                    Some(directory) => *directory,
                    None => std::fs::metadata(local_path)?.is_dir(),
                },
                _ => kinds.get(operation.source()).copied().unwrap_or(false),
            };
            let exists = |candidate: &str| {
                targets.contains(candidate)
                    || reserved.contains(candidate)
                    || if matches!(operation, Operation::Download { .. }) {
                        std::fs::symlink_metadata(candidate).is_ok()
                    } else {
                        let (directory, name) = parent(candidate);
                        remote
                            .get(directory)
                            .is_some_and(|names| names.contains(name))
                    }
            };
            let alternative = if cross {
                cross_alternate(
                    target,
                    matches!(operation, Operation::Download { .. }),
                    exists,
                )?
            } else {
                alternate(target, is_directory, exists)
            };
            targets.insert(alternative.clone());
            alternatives.insert(index, alternative);
        }
    }
    Ok(Prepared {
        operations,
        alternatives,
    })
}

impl FilePane {
    pub(super) fn has_pending_drop(&self) -> bool {
        self.dialog
            .as_ref()
            .is_some_and(|dialog| dialog.uploads.is_some())
    }
    pub(super) fn change_endpoint(&mut self) {
        self.platform_owner = None;
        self.follow.stop();
        self.endpoint_token = NEXT_ENDPOINT.fetch_add(1, Ordering::Relaxed);
        self.task_owners.insert(self.endpoint_token);
        self.needs_refresh = false;
        self.dialog = None;
        self.sudo = None;
        self.action_work = None;
        self.edits.opening = false;
        self.tasks_open = false;
        self.favorites = Default::default();
        self.notice = None;
        self.pending_selection = None;
    }
    pub(super) fn selected_entries(&self) -> Vec<Entry> {
        self.entries
            .iter()
            .filter(|entry| !entry.parent && self.selection.paths.contains(&entry.path))
            .cloned()
            .collect()
    }
    fn action_sudo(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if action == Action::Open
            && let Some(entry) = self.selected_entries().into_iter().next()
            && entry.directory
        {
            // The reference's With sudo/Open directory action is ordinary
            // navigation, not a persistent privileged browsing mode.
            if self.active
                && self.stage == Stage::Files
                && !self.loading
                && self.dialog.is_none()
                && self.action_work.is_none()
            {
                self.open_entry(entry, window, cx);
            }
            return;
        }
        self.begin_sudo(action, None, window, cx);
    }
    pub(super) fn action_sudo_open(
        &mut self,
        token: u64,
        path: String,
        selected: Option<crate::editor_config::Editor>,
        choose: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.remote_edit_current(token, &path) {
            self.begin_sudo(Action::Open, Some((selected, choose)), window, cx);
        }
    }
    fn begin_sudo(
        &mut self,
        action: Action,
        open_with: Option<(Option<crate::editor_config::Editor>, bool)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.remote.is_none()
            || !self.active
            || self.stage != Stage::Files
            || self.loading
            || self.dialog.is_some()
            || self.action_work.is_some()
            || self.edits.opening
        {
            return;
        }
        let sudo = match self
            .sudo
            .as_ref()
            .filter(|sudo| sudo.current())
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| Core::discover().and_then(crate::sftp_client::Sudo::new))
        {
            Ok(sudo) => sudo,
            Err(error) => {
                self.notice = Some(Message::error(error.to_string()));
                cx.notify();
                return;
            }
        };
        self.dialog(action, vec![], window, cx);
        let dialog = self.dialog.as_mut().unwrap();
        dialog.sudo = Some(sudo.clone());
        dialog.open_with = open_with;
        if sudo.current() {
            self.sudo_authorized(window, cx);
        } else {
            dialog.awaiting_sudo = true;
            dialog.input.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.set_masked(true, window, cx);
                input.focus(window, cx);
            });
        }
        cx.notify();
    }
    fn authenticate_sudo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .dialog
            .as_ref()
            .and_then(|dialog| dialog.task_permission.as_ref())
            .is_some_and(|ticket| !ticket.current())
        {
            self.cancel_action(window, cx);
            return;
        }
        if self
            .dialog
            .as_ref()
            .and_then(|dialog| dialog.edit_authorization)
            .is_some_and(|(edit, generation)| !self.edit_authorization_current(edit, generation))
        {
            self.cancel_action(window, cx);
            return;
        }
        let Some(dialog) = self
            .dialog
            .as_mut()
            .filter(|dialog| dialog.awaiting_sudo && !dialog.busy)
        else {
            return;
        };
        let password = zeroize::Zeroizing::new(dialog.input.read(cx).value().to_string());
        if password.is_empty() || password.contains(['\n', '\r', '\0']) {
            dialog.validation_error = Some("Enter a single-line sudo password.".into());
            cx.notify();
            return;
        }
        dialog.validation_error = None;
        let (request, cancel) = match Core::discover().and_then(Request::new) {
            Ok(request) => request,
            Err(error) => {
                dialog.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        dialog
            .input
            .update(cx, |input, cx| input.set_value("", window, cx));
        dialog._cancel.push(cancel);
        dialog.busy = true;
        dialog.error = None;
        let id = dialog.id;
        let token = dialog.token;
        let base = dialog.base.clone();
        let edit_authorization = dialog.edit_authorization;
        let task_permission = dialog.task_permission.clone();
        let options = Arc::new(dialog.options.for_new_connection());
        let sudo = dialog.sudo.as_ref().unwrap().clone();
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            let path = base.clone();
            let result = cx
                .background_spawn(async move {
                    let listing =
                        request.list_authorized(&options, &path, Some(&sudo), Some(&password))?;
                    if let Some(error) = listing.error {
                        bail!("{error}");
                    }
                    Ok::<_, anyhow::Error>(())
                })
                .await;
            let _ = this.update_in(cx, |pane, window, cx| {
                if pane.dialog.as_ref().is_none_or(|dialog| dialog.id != id) {
                    return;
                }
                pane.action_work = None;
                let current = if let Some(ticket) = &task_permission {
                    pane.active && ticket.current()
                } else if let Some((edit, generation)) = edit_authorization {
                    pane.edit_authorization_current(edit, generation)
                } else {
                    pane.endpoint_token == token
                        && pane.history.path == base
                        && pane.active
                        && pane.stage == Stage::Files
                };
                if !current {
                    pane.dialog = None;
                    pane.notice = Some(Message::info(
                        "Sudo action cancelled because the pane changed.",
                    ));
                    cx.notify();
                    return;
                }
                let dialog = pane.dialog.as_mut().unwrap();
                dialog.busy = false;
                dialog._cancel.clear();
                match result {
                    Ok(()) => pane.sudo_authorized(window, cx),
                    Err(error) => {
                        dialog.error = Some(error.to_string());
                        dialog.input.update(cx, |input, cx| input.focus(window, cx));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn sudo_authorized(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let sudo = dialog.sudo.as_ref().unwrap().clone();
        if let Some(ticket) = dialog.task_permission.take() {
            if let Err(error) = ticket.retry(sudo) {
                self.notice = Some(Message::error(error.to_string()));
            }
            self.focus.focus(window, cx);
            cx.notify();
            return;
        }
        if let Some(text) = dialog.preflight_input.take() {
            // Restore the original path editor, not its password/history. The
            // captured entries/options/operations remain bound to this action.
            let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
            dialog._input =
                cx.subscribe_in(&input, window, |pane, _, event: &InputEvent, window, cx| {
                    pane.action_input_event(event, window, cx);
                });
            dialog.input = input;
            dialog.awaiting_sudo = false;
            dialog.error = None;
            dialog.validation_error = None;
            dialog.visible = dialog.cross.is_none();
            if dialog.visible {
                dialog.input.update(cx, |input, cx| input.focus(window, cx));
            }
            self.dialog = Some(dialog);
            self.prepare_action(window, cx);
            return;
        }
        if let Some((edit, generation)) = dialog.edit_authorization {
            self.complete_edit_authorization(edit, generation, sudo, window, cx);
            return;
        }
        self.sudo = Some(sudo.clone());
        dialog.awaiting_sudo = false;
        match dialog.action {
            Action::Open => {
                if let Some(entry) = dialog.entries.first() {
                    let (editor, choose) = dialog.open_with.take().unwrap_or_default();
                    let path = entry.path.clone();
                    let token = dialog.token;
                    let owner = cx.weak_entity();
                    // Unmount the password modal before a native application
                    // chooser attaches to this window. No timer or busy retry.
                    window.on_next_frame(move |window, cx| {
                        let _ = owner.update(cx, |pane, cx| {
                            if pane.remote_edit_current(token, &path) {
                                pane.open_remote_file_authorized(
                                    token,
                                    path,
                                    super::editing::OpenOptions {
                                        editor,
                                        choose,
                                        sudo: Some(sudo),
                                    },
                                    window,
                                    cx,
                                );
                            } else {
                                pane.notice = Some(Message::info(
                                    "Open cancelled because the selected file changed.",
                                ));
                                cx.notify();
                            }
                        });
                    });
                }
            }
            Action::Download => self.download_entries(dialog.entries, Some(sudo), window, cx),
            Action::Upload => {
                let scope = super::drag::Scope::destination(self);
                let owner = cx.weak_entity();
                window.on_next_frame(move |window, cx| {
                    let _ = owner.update(cx, |pane, cx| {
                        if scope.matches(pane, cx)
                            && pane.dialog.is_none()
                            && pane.action_work.is_none()
                            && window.is_window_active()
                        {
                            pane.choose_upload_authorized(Some(sudo), window, cx);
                        }
                    });
                });
            }
            Action::NewFolder => {
                self.create_remote_folder(dialog.options, dialog.base, Some(sudo), window, cx);
                self.focus.focus(window, cx);
            }
            _ => {
                let text = match dialog.action {
                    Action::Rename => dialog.entries.first().map(|entry| entry.name.clone()),
                    Action::Move => dialog.entries.first().map(|entry| entry.path.clone()),
                    Action::Copy => dialog.entries.first().map(|entry| {
                        alternate(&entry.path, entry.directory, |path| {
                            self.entries.iter().any(|entry| entry.path == path)
                        })
                    }),
                    _ => None,
                }
                .unwrap_or_default();
                // Replace the password state, including its editing history.
                let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
                dialog._input =
                    cx.subscribe_in(&input, window, |pane, _, event: &InputEvent, window, cx| {
                        pane.action_input_event(event, window, cx);
                    });
                dialog.input = input.clone();
                self.dialog = Some(dialog);
                input.update(cx, |input, cx| input.focus(window, cx));
            }
        }
        cx.notify();
    }
    fn sudo_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let dialog = self.dialog.as_ref().unwrap();
        let id = dialog.id;
        let edit = dialog.edit_authorization.is_some();
        let permission = dialog.task_permission.is_some() || dialog.preflight_input.is_some();
        let title = if permission {
            "Permission denied"
        } else {
            "Authenticate with sudo"
        };
        let body = div().w_full().flex().flex_col().gap(px(16.))
            .child(crate::localization::view(if permission { "The server denied this operation. Enter the sudo password to retry this task once. Some parts may already have completed; this retries the original task." } else if edit { "Enter the sudo password to save the pending edits. The remote version will be checked again before uploading. Cancelling keeps your local changes." } else { "Enter the sudo password to authorize this and future With sudo actions for 15 minutes." }))
            .child(format!("{} · {}@{}", crate::localization::text(cx, if permission { "Retry task" } else if edit { "Save changes" } else { dialog.action.label() }), dialog.options.username, dialog.options.host))
            .when_some(dialog.permission_details.clone(), |view, (label, source, target)| view.child(div().text_size(px(12.)).child(crate::localization::view(label)).child(div().child(crate::localization::view(format!("From: {source}")))).child(div().child(crate::localization::view(format!("To: {target}"))))))
            .child(Field::input(&dialog.input, "Password").secret(true).disabled(dialog.busy).error(dialog.validation_error.clone()))
            .when(dialog.busy, |view| view.child(div().flex().gap(px(8.)).child(Spinner::new()).child(crate::localization::text(cx, "rs.ui.copy.authenticating"))))
            .when_some(dialog.error.clone(), |view, error| view.child(div().text_color(rgb(0xc94740)).child(crate::localization::message(cx, &error))));
        crate::components::Modal::new(SharedString::from(format!("sftp-action-modal-{id}")), title)
            .initial_focus(&dialog.input.read(cx).focus_handle(cx))
            .return_focus(&self.focus)
            .on_dismiss(cx.listener(move |pane, _, window, cx| {
                if pane.dialog.as_ref().is_some_and(|dialog| dialog.id == id) {
                    pane.cancel_action(window, cx);
                }
            }))
            .content(
                crate::components::DialogFrame::new(
                    SharedString::from(format!("sftp-sudo-dialog-{id}")),
                    title,
                )
                .width(420.)
                .colors(crate::components::DialogColors::workspace(self.data.dark))
                .content(body)
                .actions([
                    Button::new("sftp-sudo-cancel")
                        .ghost()
                        .label("Cancel")
                        .on_click(|_, window, cx| crate::components::Modal::dismiss(window, cx))
                        .into_any_element(),
                    Button::new("sftp-sudo-authenticate")
                        .primary()
                        .label(if permission {
                            "Retry with sudo"
                        } else if edit {
                            "Authenticate and save"
                        } else {
                            "Authenticate"
                        })
                        .disabled(dialog.busy)
                        .on_click(cx.listener(move |pane, _, window, cx| {
                            if pane.dialog.as_ref().is_some_and(|dialog| dialog.id == id) {
                                pane.authenticate_sudo(window, cx);
                            }
                        }))
                        .into_any_element(),
                ]),
            )
            .into_any_element()
    }
    pub(super) fn edit_sudo_dialog(
        &mut self,
        edit: u64,
        generation: u64,
        target: crate::remote_edit::AuthorizationTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dialog_inner(Action::Open, vec![], target.options, None, window, cx);
        let dialog = self.dialog.as_mut().unwrap();
        dialog.base = target.directory;
        dialog.sudo = Some(target.sudo);
        dialog.edit_authorization = Some((edit, generation));
        dialog.awaiting_sudo = true;
        dialog.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_masked(true, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }
    pub(super) fn has_task_permission_dialog(
        &self,
        ticket: &crate::transfer_queue::PermissionTicket,
    ) -> bool {
        self.dialog
            .as_ref()
            .and_then(|dialog| dialog.task_permission.as_ref())
            .is_some_and(|current| current.same(ticket))
    }
    pub(super) fn task_sudo_dialog(
        &mut self,
        ticket: crate::transfer_queue::PermissionTicket,
        target: crate::transfer_queue::PermissionTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dialog_inner(
            Action::Open,
            vec![],
            target.authorization.options,
            None,
            window,
            cx,
        );
        let dialog = self.dialog.as_mut().unwrap();
        dialog.base = target.authorization.directory;
        dialog.sudo = Some(target.authorization.sudo);
        dialog.task_permission = Some(ticket);
        dialog.permission_details = Some((target.label, target.source, target.target));
        dialog.error = Some(target.error);
        dialog.awaiting_sudo = true;
        dialog.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_masked(true, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }
    pub(super) fn cancel_edit_sudo_dialog(&mut self, edit: u64, cx: &mut Context<Self>) {
        if self
            .dialog
            .as_ref()
            .and_then(|dialog| dialog.edit_authorization)
            .is_some_and(|(id, _)| id == edit)
        {
            self.dialog = None;
            self.action_work = None;
            cx.notify();
        }
    }
    fn enqueue_authorized(
        &mut self,
        operations: Vec<Operation>,
        options: Arc<SshOptions>,
        local_replacements: HashSet<String>,
        sudo: Option<Arc<crate::sftp_client::Sudo>>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        for operation in &operations {
            operation.validate()?;
        }
        for operation in operations {
            let replace_local = local_replacements.contains(operation.target());
            self.tasks.update(cx, |queue, cx| {
                if sudo.is_none() {
                    return queue.enqueue(
                        self.endpoint_token,
                        options.clone(),
                        operation,
                        replace_local,
                        cx,
                    );
                }
                queue.enqueue_authorized(
                    self.endpoint_token,
                    options.clone(),
                    operation,
                    replace_local,
                    sudo.clone(),
                    cx,
                )
            })?;
        }
        cx.notify();
        Ok(())
    }
    fn dialog(
        &mut self,
        action: Action,
        operations: Vec<Operation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(options) = self.remote.as_ref().map(|remote| remote.options.clone()) else {
            return;
        };
        self.dialog_inner(action, operations, options, None, window, cx);
    }
    pub(super) fn cross_dialog(
        &mut self,
        batch: crate::pane_transfer::Batch,
        scope: super::drag::Scope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let action = if batch.download() {
            Action::Download
        } else {
            Action::Upload
        };
        self.dialog_inner(
            action,
            batch.operations(),
            batch.options(),
            Some((scope, batch)),
            window,
            cx,
        );
        self.prepare_action(window, cx);
    }
    pub(super) fn continue_uploads_authorized(
        &mut self,
        mut uploads: crate::file_drop::Uploads,
        scope: super::drag::Scope,
        sudo: Option<Arc<crate::sftp_client::Sudo>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !scope.matches(self, cx) || crate::transfer_queue::is_shutting_down() {
            self.notice = Some(Message::info(
                "Upload cancelled because the destination changed.",
            ));
        } else if let Some(batch) = uploads.next() {
            self.dialog_inner(
                Action::Upload,
                batch.operations(),
                batch.options(),
                Some((scope, batch)),
                window,
                cx,
            );
            self.dialog.as_mut().unwrap().uploads = Some(uploads);
            self.dialog.as_mut().unwrap().sudo = sudo;
            self.prepare_action(window, cx);
        } else {
            if uploads.skipped > 0 {
                self.notice = Some(Message::info(format!(
                    "Skipped {} unavailable file paths.",
                    uploads.skipped
                )));
            }
            self.focus.focus(window, cx);
            self.try_follow(window, cx);
        }
        cx.notify();
    }
    fn dialog_inner(
        &mut self,
        action: Action,
        operations: Vec<Operation>,
        options: Arc<SshOptions>,
        cross: Option<(super::drag::Scope, crate::pane_transfer::Batch)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = self.selected_entries();
        let text = match action {
            Action::Rename => entries
                .first()
                .map(|entry| entry.name.clone())
                .unwrap_or_default(),
            Action::Move => entries
                .first()
                .map(|entry| entry.path.clone())
                .unwrap_or_default(),
            Action::Copy => entries
                .first()
                .map(|entry| {
                    alternate(&entry.path, entry.directory, |path| {
                        self.entries.iter().any(|entry| entry.path == path)
                    })
                })
                .unwrap_or_default(),
            _ => String::new(),
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                this.action_input_event(event, window, cx);
            });
        // Transfers are already authorized by the drop/chooser gesture. Keep
        // their destination check offscreen until it needs a user decision.
        let visible = cross.is_none();
        self.notice = None;
        self.dialog = Some(ActionDialog {
            visible,
            id: NEXT_ACTION.fetch_add(1, Ordering::Relaxed),
            action,
            input: input.clone(),
            entries,
            options,
            token: self.endpoint_token,
            base: self.history.path.clone(),
            operations,
            prepared: None,
            busy: false,
            error: None,
            validation_error: None,
            _cancel: vec![],
            _input: subscription,
            cross,
            uploads: None,
            sudo: None,
            awaiting_sudo: false,
            open_with: None,
            edit_authorization: None,
            task_permission: None,
            permission_details: None,
            preflight_input: None,
        });
        if visible {
            self.tasks_open = false;
        }
        if matches!(action, Action::Rename | Action::Move | Action::Copy) {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }
    pub(super) fn action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.stage != Stage::Files
            || self.dialog.is_some()
            || self.loading
            || self.action_work.is_some()
        {
            return;
        }
        match action {
            Action::Open => {
                if let Some(entry) = self.selected_entries().into_iter().next() {
                    self.open_entry(entry, window, cx);
                }
                return;
            }
            Action::Close => {
                self.disconnect(window, cx);
                return;
            }
            Action::SelectAll => {
                self.selection.paths = self
                    .visible
                    .iter()
                    .filter(|entry| !entry.parent)
                    .map(|entry| entry.path.clone())
                    .collect();
                self.focus.focus(window, cx);
                cx.notify();
                return;
            }
            Action::NewFolder if self.remote.is_none() => {
                self.local_folder(window, cx);
                return;
            }
            _ => {}
        }
        if self.remote.is_none() {
            return;
        }
        if action == Action::NewFolder {
            self.create_remote_folder(
                self.remote.as_ref().unwrap().options.clone(),
                self.history.path.clone(),
                None,
                window,
                cx,
            );
        } else if action == Action::Download {
            self.download_selected(window, cx);
        } else if action == Action::Upload {
            self.choose_upload(window, cx);
        } else if !self.selected_entries().is_empty() {
            self.dialog(action, vec![], window, cx);
        }
    }
    fn download_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.download_entries(self.selected_entries(), None, window, cx);
    }
    fn download_entries(
        &mut self,
        entries: Vec<Entry>,
        sudo: Option<Arc<crate::sftp_client::Sudo>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if entries.is_empty() {
            return;
        }
        let Some(remote) = &self.remote else {
            return;
        };
        let request = crate::file_download::Request {
            entries,
            source_directory: self.history.path.clone(),
            options: remote.options.clone(),
            owner: self.endpoint_token,
        };
        let scope = super::drag::Scope::destination(self);
        let home = self.home.clone();
        let reserved = self.tasks.read(cx).download_targets();
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move {
                let directory = crate::file_download::system_directory(&home)?;
                request.prepare(directory, reserved, None)
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.action_work = None;
                if !scope.matches(this, cx) || crate::transfer_queue::is_shutting_down() {
                    this.notice = Some(Message::info("Download cancelled because the source changed."));
                } else {
                    match result {
                        Err(error) => this.notice = Some(Message::error(format!("Unable to download: {error}"))),
                        Ok(plan) => match this.tasks.update(cx, |queue, cx| {
                            if sudo.is_none() { queue.enqueue_downloads(&plan, cx) }
                            else { queue.enqueue_downloads_authorized(&plan, sudo.as_ref(), cx) }
                        }) {
                            Ok(()) => {},
                            Err(error) => this.notice = Some(Message::error(format!("{error} Check the task queue before trying again; some downloads may already have been queued."))),
                        }
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn choose_upload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.choose_upload_authorized(None, window, cx);
    }
    fn choose_upload_authorized(
        &mut self,
        sudo: Option<Arc<crate::sftp_client::Sudo>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scope = super::drag::Scope::destination(self);
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(crate::localization::text(cx, "Upload")),
        });
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            let response = receiver.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.action_work = None;
                if !scope.matches(this, cx) {
                    return;
                }
                match response {
                    Ok(Ok(Some(paths))) => {
                        this.upload_paths_authorized(paths, sudo, window, cx);
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => {
                        this.notice =
                            Some(Message::error(format!("Unable to choose files: {error}")));
                        cx.notify();
                    }
                    Err(error) => {
                        this.notice =
                            Some(Message::error(format!("Unable to choose files: {error}")));
                        cx.notify();
                    }
                }
                this.try_follow(window, cx);
                cx.notify();
            });
        }));
    }
    fn action_input_event(
        &mut self,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                if let Some(dialog) = &mut self.dialog
                    && dialog.validation_error.take().is_some()
                {
                    cx.notify();
                }
            }
            InputEvent::PressEnter { .. }
                if self
                    .dialog
                    .as_ref()
                    .is_some_and(|dialog| dialog.prepared.is_none() && !dialog.busy) =>
            {
                self.prepare_action(window, cx);
            }
            _ => {}
        }
    }
    fn action_error(&mut self, message: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.as_ref().is_some_and(|dialog| !dialog.visible) {
            self.dialog = None;
            self.action_work = None;
            self.try_follow(window, cx);
        } else if let Some(dialog) = &mut self.dialog {
            dialog.error = Some(message.clone());
        }
        self.notice = Some(Message::error(message));
        cx.notify();
    }
    fn prepare_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.awaiting_sudo)
        {
            self.authenticate_sudo(window, cx);
            return;
        }
        if self
            .dialog
            .as_ref()
            .and_then(|dialog| dialog.cross.as_ref())
            .is_some_and(|(scope, _)| !scope.matches(self, cx))
        {
            self.dialog = None;
            self.notice = Some(Message::info(
                "Transfer cancelled because a pane or selection changed.",
            ));
            cx.notify();
            return;
        }
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };
        if dialog.busy {
            return;
        }
        let build = || -> Result<Vec<Operation>> {
            if !dialog.operations.is_empty() {
                return Ok(dialog.operations.clone());
            }
            if dialog.action == Action::Delete {
                return Ok(dialog
                    .entries
                    .iter()
                    .map(|entry| Operation::Delete {
                        target_path: entry.path.clone(),
                    })
                    .collect());
            }
            let entry = dialog.entries.first().context("Choose an item first.")?;
            let text = dialog.input.read(cx).value();
            Ok(vec![destination_operation(
                dialog.action,
                &entry.path,
                &text,
                &dialog.base,
            )?])
        };
        let operations = match build() {
            Ok(operations) => operations,
            Err(error) => {
                dialog.validation_error = Some(error.to_string().into());
                cx.notify();
                return;
            }
        };
        dialog.validation_error = None;
        let parents: BTreeSet<_> = operations
            .iter()
            .filter(|operation| {
                !matches!(
                    operation,
                    Operation::Download { .. } | Operation::Delete { .. } | Operation::Mkdir { .. }
                )
            })
            .map(|operation| parent(operation.target()).0.to_owned())
            .collect();
        let mut requests = vec![];
        for parent in parents {
            match Core::discover().and_then(Request::new) {
                Ok((request, cancel)) => {
                    requests.push((parent, request));
                    dialog._cancel.push(cancel);
                }
                Err(error) => {
                    self.action_error(error.to_string(), window, cx);
                    return;
                }
            }
        }
        dialog.busy = true;
        dialog.error = None;
        let id = dialog.id;
        let mut kinds: BTreeMap<_, _> = dialog
            .entries
            .iter()
            .map(|entry| (entry.path.clone(), entry.directory))
            .collect();
        if let Some((_, batch)) = &dialog.cross {
            kinds.extend(batch.kinds.clone());
        }
        let cross = dialog.cross.is_some();
        let reserved = dialog
            .uploads
            .as_ref()
            .map(|uploads| uploads.reserved.clone())
            .unwrap_or_default();
        let options = dialog.options.clone();
        let sudo = dialog.sudo.clone();
        let permission_details = operations.first().map(|operation| {
            let source = dialog
                .cross
                .as_ref()
                .and_then(|(_, batch)| batch.display_source(0))
                .unwrap_or_else(|| operation.source());
            (
                dialog.action.label().to_owned(),
                source.to_owned(),
                operation.target().to_owned(),
            )
        });
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if sudo.is_some() {
                        prepare_authorized(
                            operations,
                            requests,
                            &options,
                            &kinds,
                            cross,
                            &reserved,
                            sudo.as_deref(),
                        )
                    } else {
                        prepare(operations, requests, &options, &kinds, cross, &reserved)
                    }
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.dialog.as_ref().is_none_or(|dialog| dialog.id != id) {
                    return;
                }
                this.action_work = None;
                if this
                    .dialog
                    .as_ref()
                    .and_then(|dialog| dialog.cross.as_ref())
                    .is_some_and(|(scope, _)| !scope.matches(this, cx))
                {
                    this.dialog = None;
                    this.notice = Some(Message::info(
                        "Transfer cancelled because a pane or selection changed.",
                    ));
                    cx.notify();
                    return;
                }
                let Some(dialog) = this.dialog.as_mut().filter(|dialog| dialog.id == id) else {
                    return;
                };
                dialog.busy = false;
                dialog._cancel.clear();
                match result {
                    Err(error) => {
                        let message = error.to_string();
                        if dialog.sudo.is_none()
                            && crate::sftp_client::remote_permission_denied(&message)
                        {
                            match Core::discover()
                                .and_then(crate::sftp_client::Sudo::new_single_use)
                            {
                                Ok(sudo) => {
                                    dialog.preflight_input =
                                        Some(dialog.input.read(cx).value().to_string());
                                    dialog.sudo = Some(sudo);
                                    dialog.awaiting_sudo = true;
                                    dialog.visible = true;
                                    dialog.permission_details = permission_details;
                                    dialog.input.update(cx, |input, cx| {
                                        input.set_value("", window, cx);
                                        input.set_masked(true, window, cx);
                                        input.focus(window, cx);
                                    });
                                }
                                Err(error) => {
                                    this.action_error(format!("{message}. {error}"), window, cx);
                                    return;
                                }
                            }
                            dialog.error = Some(message);
                        } else {
                            this.action_error(message, window, cx);
                        }
                    }
                    Ok(prepared) => {
                        let conflicts = !prepared.alternatives.is_empty();
                        if conflicts && dialog.action == Action::Rename {
                            dialog.validation_error =
                                Some("The destination already exists. Choose another name.".into());
                        } else {
                            dialog.prepared = Some(prepared);
                            let download = dialog
                                .cross
                                .as_ref()
                                .is_some_and(|(_, batch)| batch.download());
                            if conflicts && !download {
                                dialog.visible = true;
                            }
                            if !conflicts || download {
                                this.commit_action(false, download, window, cx);
                            }
                        }
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn commit_action(
        &mut self,
        replace: bool,
        keep_both: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        if dialog.token != self.endpoint_token {
            return;
        }
        if dialog
            .cross
            .as_ref()
            .is_some_and(|(scope, _)| !scope.matches(self, cx))
        {
            self.notice = Some(Message::info(
                "Transfer cancelled because a pane or selection changed.",
            ));
            cx.notify();
            return;
        }
        let Some(prepared) = dialog.prepared.take() else {
            self.dialog = Some(dialog);
            return;
        };
        let (operations, local_replacements) = prepared.resolve(replace, keep_both);
        let targets: Vec<_> = operations
            .iter()
            .map(|operation| operation.target().to_owned())
            .collect();
        let queued = if let Some((_, batch)) = &dialog.cross {
            self.tasks.update(cx, |queue, cx| {
                if dialog.sudo.is_none() {
                    queue.enqueue_batch(batch, operations, &local_replacements, cx)
                } else {
                    queue.enqueue_batch_authorized(
                        batch,
                        operations,
                        &local_replacements,
                        dialog.sudo.as_ref(),
                        cx,
                    )
                }
            })
        } else {
            self.enqueue_authorized(
                operations,
                dialog.options.clone(),
                local_replacements,
                dialog.sudo.clone(),
                cx,
            )
        };
        match queued {
            Ok(()) => {
                if let (Some(mut uploads), Some((scope, _))) =
                    (dialog.uploads.take(), dialog.cross.take())
                {
                    uploads.reserve(targets);
                    self.continue_uploads_authorized(
                        uploads,
                        scope,
                        dialog.sudo.clone().filter(|sudo| !sudo.single_use()),
                        window,
                        cx,
                    );
                } else {
                    self.focus.focus(window, cx);
                }
            }
            Err(error) => {
                let message = format!(
                    "{error}. Check the task queue before submitting again; some items may already have been queued."
                );
                self.dialog = Some(dialog);
                self.action_error(message, window, cx);
            }
        }
        cx.notify();
    }
    pub(super) fn action_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let dialog = self.dialog.as_ref().unwrap();
        if dialog.awaiting_sudo {
            return self.sudo_dialog(cx);
        }
        let id = dialog.id;
        let p = Palette::new(self.data.dark);
        let conflict = dialog
            .prepared
            .as_ref()
            .is_some_and(|prepared| !prepared.alternatives.is_empty());
        let destructive = dialog.action == Action::Delete;
        let title = if conflict {
            "Destination already exists".to_owned()
        } else if dialog.sudo.is_some() {
            format!("{} with sudo", dialog.action.label())
        } else {
            dialog.action.label().to_owned()
        };
        let details = dialog
            .prepared
            .as_ref()
            .map(|prepared| &prepared.operations)
            .unwrap_or(&dialog.operations);
        let mut actions = vec![
            Button::new("sftp-cancel-action")
                .ghost()
                .label("Cancel")
                .on_click(|_, window, cx| {
                    cx.stop_propagation();
                    crate::components::Modal::dismiss(window, cx);
                })
                .into_any_element(),
        ];
        if conflict {
            actions.push(
                Button::new("sftp-keep-both")
                    .secondary()
                    .label("Keep Both")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.dialog.as_ref().is_some_and(|dialog| dialog.id == id) {
                            this.commit_action(false, true, window, cx)
                        }
                    }))
                    .into_any_element(),
            );
        }
        actions.push(
            Button::new("sftp-confirm-action")
                .label(if conflict {
                    "Replace"
                } else {
                    dialog.action.label()
                })
                .when(destructive, |button| button.danger())
                .when(!destructive, |button| button.primary())
                .disabled(dialog.busy)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this.dialog.as_ref().is_none_or(|dialog| dialog.id != id) {
                        return;
                    }
                    if conflict {
                        this.commit_action(true, false, window, cx);
                    } else {
                        this.prepare_action(window, cx);
                    }
                }))
                .into_any_element(),
        );
        let body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(16.))
            .when(dialog.sudo.is_some(), |view| {
                view.child(crate::localization::view(format!(
                    "This operation runs with sudo on {}@{}.",
                    dialog.options.username, dialog.options.host
                )))
            })
            .when(
                matches!(dialog.action, Action::Rename | Action::Copy | Action::Move) && !conflict,
                |view| {
                    view.child(
                        Field::input(
                            &dialog.input,
                            if dialog.action == Action::Rename {
                                "Name"
                            } else {
                                "Destination path"
                            },
                        )
                        .disabled(dialog.busy)
                        .error(dialog.validation_error.clone()),
                    )
                },
            )
            .child(
                div()
                    .id("sftp-action-details")
                    .max_h(px(150.))
                    .overflow_y_scroll()
                    .scrollbars()
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .when(details.is_empty(), |view| {
                        view.children(
                            dialog
                                .entries
                                .iter()
                                .take(6)
                                .map(|entry| div().child(entry.path.clone())),
                        )
                    })
                    .children(details.iter().enumerate().map(|(index, operation)| {
                        let source = dialog
                            .cross
                            .as_ref()
                            .and_then(|(_, batch)| batch.display_source(index))
                            .unwrap_or_else(|| operation.source());
                        div()
                            .mb(px(8.))
                            .child(
                                div().child(crate::localization::view(format!("From: {source}"))),
                            )
                            .child(div().child(crate::localization::view(format!(
                                "To: {}",
                                operation.target()
                            ))))
                    }))
                    .children(
                        dialog
                            .prepared
                            .iter()
                            .flat_map(|prepared| prepared.alternatives.values())
                            .map(|path| {
                                div().child(crate::localization::view(format!("Keep both: {path}")))
                            }),
                    ),
            )
            .when(destructive, |view| {
                view.child(crate::localization::text(
                    cx,
                    "rs.ui.copy.deleteRemoteItemsWarning",
                ))
            })
            .when(conflict, |view| {
                view.child(crate::localization::text(
                    cx,
                    "rs.ui.copy.keepOrReplaceItems",
                ))
            })
            .when(dialog.busy, |view| {
                view.child(div().flex().gap(px(8.)).child(Spinner::new()).child(
                    crate::localization::text(cx, "rs.ui.copy.checkingDestination"),
                ))
            })
            .when_some(dialog.error.clone(), |view, error| {
                view.child(
                    div()
                        .text_color(rgb(0xc94740))
                        .child(crate::localization::message(cx, &error)),
                )
            });
        crate::components::Modal::new(
            SharedString::from(format!("sftp-action-modal-{}", dialog.id)),
            title.clone(),
        )
        .return_focus(&self.focus)
        .when(
            matches!(dialog.action, Action::Rename | Action::Copy | Action::Move) && !conflict,
            |modal| modal.initial_focus(&dialog.input.read(cx).focus_handle(cx)),
        )
        .on_dismiss(cx.listener(move |this, _, window, cx| {
            if this.dialog.as_ref().is_some_and(|dialog| dialog.id == id) {
                this.cancel_action(window, cx);
            }
        }))
        .content(
            crate::components::DialogFrame::new(
                SharedString::from(format!("sftp-action-dialog-{}", dialog.id)),
                title,
            )
            .width(if conflict {
                460.
            } else if destructive {
                380.
            } else {
                420.
            })
            .colors(crate::components::DialogColors::workspace(self.data.dark))
            .content(body)
            .actions(actions),
        )
        .into_any_element()
    }
    fn cancel_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog = self.dialog.take();
        self.action_work = None;
        if let Some(mut dialog) = dialog
            && let (Some(uploads), Some((scope, _))) = (dialog.uploads.take(), dialog.cross.take())
        {
            self.continue_uploads_authorized(
                uploads,
                scope,
                dialog.sudo.clone().filter(|sudo| !sudo.single_use()),
                window,
                cx,
            );
            return;
        }
        self.focus.focus(window, cx);
        self.try_follow(window, cx);
        cx.notify();
    }
}
#[cfg(test)]
mod tests {
    use super::{Action, alternate, cross_alternate, destination_operation, prepare};
    use crate::{file_operation::Operation, native::SshOptions};
    #[test]
    fn editable_destination_rejects_invalid_parameters_before_preflight() {
        for (action, text) in [
            (Action::Rename, ""),
            (Action::Rename, "../renamed"),
            (Action::Rename, "folder"),
            (Action::Move, "/"),
            (Action::Move, "/base/folder/child"),
            (Action::Copy, "/base"),
            (Action::Copy, "/base/folder"),
        ] {
            assert!(
                destination_operation(action, "/base/folder", text, "/base").is_err(),
                "{text:?} must be rejected before any native request"
            );
        }
        let renamed =
            destination_operation(Action::Rename, "/base/folder", "新名称", "/base").unwrap();
        assert_eq!(renamed.target(), "/base/新名称");
        let moved = destination_operation(Action::Move, "/base/folder", "../other/folder", "/base")
            .unwrap();
        assert_eq!(moved.target(), "/other/folder");
    }
    #[test]
    fn replacement_approval_applies_only_to_the_conflicting_items() {
        let download = |path: &str| Operation::Download {
            remote_path: "/file".into(),
            local_path: path.into(),
            transfer_threads: 8,
        };
        let prepared = super::Prepared {
            operations: vec![download("/existing"), download("/new")],
            alternatives: [(0, "/existing (1)".into())].into(),
        };
        let (_, approvals) = prepared.resolve(true, false);
        assert!(approvals.contains("/existing"));
        assert!(!approvals.contains("/new"));
    }
    #[test]
    fn conflict_preflight_never_overwrites_and_proposes_distinct_names() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("file");
        std::fs::write(&path, b"original").unwrap();
        let options = SshOptions::new(
            "localhost".into(),
            22,
            "fixture".into(),
            temp.path().join("known_hosts"),
        );
        let operation = Operation::Download {
            remote_path: "/remote/file".into(),
            local_path: path.to_str().unwrap().into(),
            transfer_threads: 8,
        };
        let result = prepare(
            vec![operation.clone()],
            vec![],
            &options,
            &Default::default(),
            false,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(result.alternatives.len(), 1);
        assert_ne!(result.alternatives[&0], path.to_str().unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert!(
            prepare(
                vec![operation.clone(), operation],
                vec![],
                &options,
                &Default::default(),
                false,
                &Default::default(),
            )
            .is_err()
        );
        assert_eq!(
            alternate("/a/b", true, |candidate| candidate == "/a/b (1)"),
            "/a/b (2)"
        );
        assert_eq!(
            alternate("/a/中文.txt", false, |_| false),
            "/a/中文 (1).txt"
        );
        assert_eq!(
            alternate("/a/folder.with.dots", true, |_| false),
            "/a/folder.with.dots (1)"
        );
    }
    #[test]
    fn cross_pane_conflicts_use_reference_names_and_reserve_other_selected_targets() {
        assert_eq!(
            cross_alternate("/a/中文.txt", false, |name| name == "/a/中文 copy.txt").unwrap(),
            "/a/中文 copy 2.txt"
        );
        assert_eq!(
            cross_alternate("/a/.hidden", false, |_| false).unwrap(),
            "/a/.hidden copy"
        );
        let temp = tempfile::tempdir().unwrap();
        let existing = temp.path().join("file.txt");
        std::fs::write(&existing, b"existing").unwrap();
        let options = SshOptions::new(
            "127.0.0.1".into(),
            1,
            "synthetic".into(),
            temp.path().join("unused-trust"),
        );
        let download = |name: &str| Operation::Download {
            remote_path: format!("/source/{name}"),
            local_path: temp.path().join(name).to_str().unwrap().into(),
            transfer_threads: 8,
        };
        let prepared = prepare(
            vec![download("file.txt"), download("file 2.txt")],
            vec![],
            &options,
            &Default::default(),
            true,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            prepared.alternatives[&0],
            temp.path().join("file 3.txt").to_str().unwrap()
        );
        let (operations, replacements) = prepared.resolve(false, true);
        assert_eq!(
            operations[0].target(),
            temp.path().join("file 3.txt").to_str().unwrap()
        );
        assert_eq!(
            operations[1].target(),
            temp.path().join("file 2.txt").to_str().unwrap()
        );
        assert!(replacements.is_empty());
        assert_eq!(std::fs::read(existing).unwrap(), b"existing");
    }
}
