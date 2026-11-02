//! GPUI presentation of the shared background transfer scheduler. Both ordinary
//! file operations and remote editors use its same slots, controls and rows.
use crate::{
    components::Button,
    file_operation::Operation,
    native::SshOptions,
    palette::Palette,
    transfer_queue::{Client, Service, Snapshot, Spec, Status},
    typography::FontWeights,
};
use anyhow::Result;
use gpui_kit::{
    assets::IconName,
    component::{Disableable, Sizable},
    prelude::*,
    *,
};
use std::{
    collections::HashSet,
    sync::{Arc, Condvar, Mutex, OnceLock},
};

mod collection;
mod presentation;
mod row;

#[derive(Default)]
struct Running {
    active: usize,
    closing: bool,
}
fn running() -> &'static (Mutex<Running>, Condvar) {
    static RUNNING: OnceLock<(Mutex<Running>, Condvar)> = OnceLock::new();
    RUNNING.get_or_init(Default::default)
}
struct RunGuard;
impl Drop for RunGuard {
    fn drop(&mut self) {
        let mut state = running().0.lock().unwrap();
        state.active -= 1;
        running().1.notify_all();
    }
}
pub fn run_local<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
    let mut state = running().0.lock().unwrap();
    anyhow::ensure!(!state.closing, "The application is shutting down.");
    state.active += 1;
    drop(state);
    let _guard = RunGuard;
    work()
}
pub fn prepare_shutdown() {
    crate::transfer_history::prepare_shutdown();
    running().0.lock().unwrap().closing = true;
    crate::sftp_client::prepare_shutdown();
    let mut state = running().0.lock().unwrap();
    while state.active > 0 {
        state = running().1.wait(state).unwrap();
    }
}
#[cfg(target_os = "macos")]
pub fn close_window() {
    crate::sftp_client::close_window();
    let mut state = running().0.lock().unwrap();
    while state.active > 0 {
        state = running().1.wait(state).unwrap();
    }
}

pub struct TaskFinished {
    pub owner: u64,
    pub local_target: Option<String>,
    pub refresh_owner: bool,
    pub error: Option<String>,
}
pub struct TaskAuthorizationRequested(pub crate::transfer_queue::PermissionTicket);
pub struct TaskQueue {
    history: Option<Arc<crate::transfer_history::History>>,
    history_error: Option<String>,
    service: Service,
    rows: collection::Rows,
    next_removal: u64,
    scroll: UniformListScrollHandle,
    tool_scroll: UniformListScrollHandle,
    dark: bool,
    finished: HashSet<(u64, u64)>,
    updates: Option<Task<()>>,
}
impl EventEmitter<TaskFinished> for TaskQueue {}
impl EventEmitter<TaskAuthorizationRequested> for TaskQueue {}
impl TaskQueue {
    pub fn new(
        dark: bool,
        limits: crate::sftp_preferences::Limits,
        vault: Option<crate::vault::Vault>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.history_error
                .clone()
                .or_else(|| view.history.as_ref().and_then(|history| history.error()))
                .map(crate::components::message::Message::error)
        });
        let (history, history_error) =
            match vault.map(|vault| crate::transfer_history::History::new(Arc::new(vault))) {
                Some(Ok(history)) => (Some(history), None),
                Some(Err(error)) => (None, Some(error.to_string())),
                None => (None, None),
            };
        let service = Service::new(true);
        service
            .set_limits(limits)
            .expect("AppState validated transfer limits");
        if let Some(history) = &history {
            service.set_history(history.clone());
        }
        let mut queue = Self {
            history,
            history_error,
            service,
            rows: collection::Rows::default(),
            next_removal: 1,
            scroll: UniformListScrollHandle::new(),
            tool_scroll: UniformListScrollHandle::new(),
            dark,
            finished: HashSet::new(),
            updates: None,
        };
        queue.subscribe(cx);
        queue
    }
    fn subscribe(&mut self, cx: &mut Context<Self>) {
        if self.updates.is_none() {
            let changes = self.service.changes();
            let history_changes = self.history.as_ref().map(|history| history.changes());
            self.updates = Some(cx.spawn(async move |this, cx| {
                loop {
                    let result = if let Some(history) = &history_changes {
                        match futures_util::future::select(
                            Box::pin(changes.recv()),
                            Box::pin(history.recv()),
                        )
                        .await
                        {
                            futures_util::future::Either::Left((result, _))
                            | futures_util::future::Either::Right((result, _)) => result,
                        }
                    } else {
                        changes.recv().await
                    };
                    if result.is_err() {
                        break;
                    }
                    if this.update(cx, |this, cx| this.sync(cx)).is_err() {
                        break;
                    }
                }
            }));
        }
        self.sync(cx);
    }
    fn sync(&mut self, cx: &mut Context<Self>) {
        self.rows.update(
            self.service.snapshots(),
            self.history.as_ref().map(|history| history.snapshots()),
        );
        let live_keys: HashSet<_> = self
            .rows
            .live()
            .iter()
            .map(|job| (job.id, job.attempt))
            .collect();
        self.finished.retain(|key| live_keys.contains(key));
        for job in self.rows.live() {
            if job.history_id.is_none()
                && job.status.finished()
                && self.finished.insert((job.id, job.attempt))
            {
                cx.emit(TaskFinished {
                    owner: job.owner,
                    local_target: if job.status == Status::Completed {
                        job.local_target.clone()
                    } else {
                        None
                    },
                    refresh_owner: job.refresh_owner,
                    error: job.error.clone(),
                });
                if job.permission_retry
                    && let Some(ticket) = self.service.permission_ticket(job.id, job.attempt)
                {
                    cx.emit(TaskAuthorizationRequested(ticket));
                }
            }
        }
        cx.notify();
    }
    pub fn client(&mut self, owner: u64, cx: &mut Context<Self>) -> Client {
        self.subscribe(cx);
        self.service.client(owner)
    }
    pub fn reload_history(&self) {
        if let Some(history) = &self.history {
            history.reload();
        }
    }
    pub fn register_history_owner(&self, owner: u64, identity: crate::transfer_history::Identity) {
        self.service.register_history_owner(owner, identity);
    }
    fn begin_removal(&mut self, uuid: Option<String>, cx: &mut Context<Self>) -> bool {
        let Some(history) = &self.history else {
            return true;
        };
        let outcome = match &uuid {
            Some(uuid) => history.delete(uuid.clone()),
            None => history.clear(),
        };
        let receiver = match outcome {
            Ok(receiver) => receiver,
            Err(error) => {
                self.history_error = Some(format!("SFTP task history: {error}"));
                return false;
            }
        };
        let token = self.next_removal;
        self.next_removal += 1;
        self.rows.hide_finished(token, uuid.as_deref());
        self.history_error = None;
        cx.spawn(async move |this, cx| {
            let outcome = receiver
                .recv()
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Task history worker stopped.")));
            let _ = this.update(cx, |this, cx| {
                this.rows.settle_removal(token, outcome.is_ok());
                if let Err(error) = outcome {
                    this.history_error = Some(format!("SFTP task history: {error}"));
                }
                this.sync(cx);
            });
        })
        .detach();
        true
    }
    fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(job) = self.rows.find(id).filter(|job| job.status.finished()) else {
            return;
        };
        let uuid = job.uuid.clone();
        if self.begin_removal(Some(uuid), cx) {
            self.service.dismiss(id);
        }
    }
    fn clear_finished(&mut self, cx: &mut Context<Self>) {
        if self.begin_removal(None, cx) {
            self.service.clear_finished();
        }
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        cx.notify();
    }
    pub fn set_limits(&mut self, limits: crate::sftp_preferences::Limits, cx: &mut Context<Self>) {
        self.service
            .set_limits(limits)
            .expect("AppState validated transfer limits");
        self.sync(cx);
    }
    pub fn active(&self) -> usize {
        self.service
            .snapshots()
            .iter()
            .filter(|job| !job.status.finished())
            .count()
    }
    pub fn has_work(&self) -> bool {
        self.service.has_work()
    }
    pub fn enqueue(
        &mut self,
        owner: u64,
        options: Arc<SshOptions>,
        operation: Operation,
        replace_local: bool,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.enqueue_authorized(owner, options, operation, replace_local, None, cx)
    }
    pub fn enqueue_authorized(
        &mut self,
        owner: u64,
        options: Arc<SshOptions>,
        operation: Operation,
        replace_local: bool,
        sudo: Option<Arc<crate::sftp_client::Sudo>>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.subscribe(cx);
        let mut spec = Spec::new(owner, options, operation);
        if sudo.is_some() {
            spec.label = Some(format!("{} with sudo", spec.operation.label()));
        }
        spec.sudo = sudo;
        spec.inherit_transfer_threads = true;
        spec.replace_local = replace_local;
        self.service.enqueue(spec)?;
        self.sync(cx);
        Ok(())
    }
    pub fn enqueue_batch(
        &mut self,
        batch: &crate::pane_transfer::Batch,
        operations: Vec<Operation>,
        replacements: &std::collections::HashSet<String>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.enqueue_batch_authorized(batch, operations, replacements, None, cx)
    }
    pub fn enqueue_batch_authorized(
        &mut self,
        batch: &crate::pane_transfer::Batch,
        operations: Vec<Operation>,
        replacements: &std::collections::HashSet<String>,
        sudo: Option<&Arc<crate::sftp_client::Sudo>>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.subscribe(cx);
        if sudo.is_some() {
            batch.enqueue_authorized(&self.service, operations, replacements, sudo)?;
        } else {
            batch.enqueue(&self.service, operations, replacements)?;
        }
        self.sync(cx);
        Ok(())
    }
    pub fn download_targets(&self) -> std::collections::HashSet<std::path::PathBuf> {
        self.service.download_targets()
    }
    pub fn enqueue_downloads(
        &mut self,
        plan: &crate::file_download::Plan,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.enqueue_downloads_authorized(plan, None, cx)
    }
    pub fn enqueue_downloads_authorized(
        &mut self,
        plan: &crate::file_download::Plan,
        sudo: Option<&Arc<crate::sftp_client::Sudo>>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.subscribe(cx);
        if sudo.is_some() {
            plan.enqueue_authorized(&self.service, sudo)?;
        } else {
            plan.enqueue(&self.service)?;
        }
        self.sync(cx);
        Ok(())
    }
}
impl Drop for TaskQueue {
    fn drop(&mut self) {
        self.service.detach_view();
    }
}
impl Render for TaskQueue {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.updates.is_none() {
            self.subscribe(cx);
        }
        let p = Palette::new(self.dark);
        div()
            .w(px(360.))
            .h(px(246.))
            .rounded(px(8.))
            .overflow_hidden()
            .bg(p.surface)
            .text_color(p.text)
            .text_size(px(12.))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(40.))
                    .flex_shrink_0()
                    .px(px(13.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .font_weight(FontWeights::SEMIBOLD)
                            .child(crate::localization::text(cx, "common.label.tasks")),
                    )
                    .child(
                        Button::new("clear-sftp-tasks")
                            .xsmall()
                            .ghost()
                            .h(px(24.))
                            .px(px(4.))
                            .text_size(px(11.))
                            .label("Clear Tasks")
                            .disabled(!self.rows.has_finished())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_finished(cx);
                                this.sync(cx);
                            })),
                    ),
            )
            .child(self.task_list(p, false, cx))
    }
}

/// A presentation of the same queue using the owning terminal's palette. The
/// queue can be shown in two panes without changing its shared theme/model.
#[derive(IntoElement)]
pub struct TaskPanel {
    queue: Entity<TaskQueue>,
    palette: Palette,
}
impl TaskPanel {
    pub fn new(queue: Entity<TaskQueue>, palette: Palette) -> Self {
        Self { queue, palette }
    }
}
impl RenderOnce for TaskPanel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        self.queue
            .update(cx, |queue, cx| queue.terminal_panel(self.palette, cx))
    }
}
impl TaskQueue {
    fn task_list(&self, p: Palette, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let content = if self.rows.is_empty() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(p.muted)
                .child(crate::localization::text(cx, "common.label.noTasks"))
                .into_any_element()
        } else {
            let owner = cx.weak_entity();
            let scroll = if compact {
                &self.tool_scroll
            } else {
                &self.scroll
            };
            crate::components::uniform_scrollbar(
                if compact {
                    "tool-sftp-task-scrollbar"
                } else {
                    "sftp-task-scrollbar"
                },
                uniform_list(
                    if compact {
                        "tool-sftp-task-list"
                    } else {
                        "sftp-task-list"
                    },
                    self.rows.len(),
                    move |range, _, cx| {
                        let Some(owner) = owner.upgrade() else {
                            return vec![];
                        };
                        owner.update(cx, |queue, cx| {
                            range
                                .filter_map(|index| {
                                    queue
                                        .rows
                                        .get(index)
                                        .map(|job| queue.task_row(job, p, compact, cx))
                                })
                                .collect::<Vec<_>>()
                        })
                    },
                )
                .track_scroll(scroll)
                .size_full(),
                scroll,
            )
            .into_any_element()
        };
        div().flex_1().min_h_0().child(content).into_any_element()
    }
    fn terminal_panel(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.updates.is_none() {
            self.subscribe(cx);
        }
        div()
            .w_full()
            .h(px(190.))
            .rounded(px(9.))
            .overflow_hidden()
            .bg(p.surface)
            .text_color(p.text)
            .text_size(px(11.))
            .line_height(px(16.))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(38.))
                    .flex_shrink_0()
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.5))
                            .font_weight(FontWeights::SEMIBOLD)
                            .child(crate::localization::text(cx, "common.label.tasks")),
                    )
                    .child(
                        Button::new("tool-clear-sftp-tasks")
                            .ghost()
                            .xsmall()
                            .h(px(26.))
                            .px(px(6.))
                            .text_size(px(10.5))
                            .custom(
                                crate::components::ButtonCustomVariant::for_surface(p)
                                    .foreground(p.muted.into())
                                    .hover(p.control.into())
                                    .active(p.accent.alpha(0.12).into()),
                            )
                            .label("Clear Tasks")
                            .disabled(!self.rows.has_finished())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_finished(cx);
                                this.sync(cx);
                            })),
                    ),
            )
            .child(self.task_list(p, true, cx))
            .into_any_element()
    }
}
