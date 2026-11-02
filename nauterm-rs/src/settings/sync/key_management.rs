//! Key details are a content Modal. Forgetting the shared local cache is a
//! separate decision Dialog; cloud rotation is an uncancellable storage job.
use super::*;
use crate::sync::keys::{self as model, Phase, Plan, Progress, Target};
use std::{collections::BTreeMap, sync::Arc};
use zeroize::Zeroizing;
mod view;

pub(super) struct Management {
    info: Option<Arc<model::Management>>,
    changing: bool,
    attempted: bool,
    confirm_forget: bool,
    current: Entity<InputState>,
    new: Entity<InputState>,
    confirm: Entity<InputState>,
    phases: BTreeMap<String, Phase>,
    completed: Vec<Target>,
    focus: FocusHandle,
    forget_focus: FocusHandle,
    _events: Vec<Subscription>,
}
impl Management {
    fn new(window: &mut Window, cx: &mut Context<Panel>) -> Self {
        let fields: Vec<_> = [
            "Current Master Key",
            "New Master Key",
            "Confirm New Master Key",
        ]
        .into_iter()
        .map(|label| cx.new(|cx| InputState::new(window, cx).masked(true).placeholder(label)))
        .collect();
        let events = fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                cx.subscribe_in(field, window, move |this, _, event: &InputEvent, _, cx| {
                    if index == 2 && matches!(event, InputEvent::PressEnter { .. }) {
                        this.request_key_rotation(cx);
                    }
                    cx.notify();
                })
            })
            .collect();
        Self {
            info: None,
            changing: false,
            attempted: false,
            confirm_forget: false,
            current: fields[0].clone(),
            new: fields[1].clone(),
            confirm: fields[2].clone(),
            phases: BTreeMap::new(),
            completed: Vec::new(),
            focus: cx.focus_handle(),
            forget_focus: cx.focus_handle(),
            _events: events,
        }
    }
    fn clear_inputs(&mut self, window: &mut Window, cx: &mut Context<Panel>) {
        for field in [&self.current, &self.new, &self.confirm] {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        self.attempted = false;
    }
}
impl Panel {
    pub(super) fn open_key_management(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.management = Some(Management::new(window, cx));
        self.notice = None;
        self.read_key_management(window, cx);
    }
    fn read_key_management(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.management.is_none() {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.loading = true;
        self.error = None;
        self.validation_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.key_management().await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.loading = false;
                let success = result.is_ok();
                if let Some(management) = &mut this.management {
                    match result {
                        Ok(info) => {
                            management.info = Some(info);
                            management.phases.clear();
                        }
                        Err(error) => {
                            management.info = None;
                            this.error = Some(error.to_string());
                        }
                    }
                }
                cx.emit(Event::Settled(success));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn request_key_rotation(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(management) = &self.management else {
            return;
        };
        if !management.changing || management.confirm_forget {
            return;
        }
        let Some(reviewed) = &management.info else {
            return;
        };
        let change = model::Change::new(
            Zeroizing::new(management.current.read(cx).value().to_string()),
            Zeroizing::new(management.new.read(cx).value().to_string()),
            Zeroizing::new(management.confirm.read(cx).value().to_string()),
        );
        match change {
            Ok(change) if reviewed.can_rotate() => {
                let plan = Plan {
                    reviewed: reviewed.clone(),
                    change,
                    backup_count: self.preferences.backup_count,
                };
                self.mutating = true;
                self.error = None;
                self.validation_error = None;
                self.notice = None;
                cx.emit(Event::MutationRequested(Operation::RotateKey(plan)));
            }
            Ok(_) => {
                self.validation_error =
                    Some("Connect a sync provider before changing the Master Key.".into())
            }
            Err(error) => self.validation_error = Some(format!("{error:#}")),
        }
        cx.notify();
    }
    pub(super) fn run_key_rotation(
        &mut self,
        plan: Plan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            self.operation_failed("Sync storage is unavailable.".into(), cx);
            return;
        };
        if let Some(management) = &mut self.management {
            management.attempted = true;
            management.phases.clear();
        }
        let (sender, receiver) = async_channel::unbounded::<Progress>();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.rotate_key(plan, sender);
            futures_util::pin_mut!(result);
            let result = loop {
                let event = receiver.recv();
                futures_util::pin_mut!(event);
                match futures_util::future::select(result.as_mut(), event).await {
                    futures_util::future::Either::Left((result, _)) => break result,
                    futures_util::future::Either::Right((Ok(event), _)) => {
                        let _ = this.update_in(cx, |this, _, cx| {
                            if let Some(management) = &mut this.management {
                                management.phases.insert(event.target.scope, event.phase);
                            }
                            cx.notify();
                        });
                    }
                    futures_util::future::Either::Right((Err(_), result)) => break result.await,
                }
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.finish_key_rotation(result, window, cx)
            });
        })
        .detach();
        cx.notify();
    }
    fn finish_key_rotation(
        &mut self,
        result: anyhow::Result<model::Attempt>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mutating = false;
        match result {
            Err(error) => self.error = Some(error.to_string()),
            Ok(attempt) => {
                match attempt.overview {
                    Ok(overview) => {
                        if overview.has_key {
                            self.key.clear(window, cx);
                        }
                        self.overview = Some(overview);
                    }
                    Err(error) => {
                        self.error = Some(format!("Could not refresh sync status: {error}"))
                    }
                }
                match attempt.outcome {
                    // Preflight failure did not perform a cloud rewrap. An
                    // explicit Reload is required before a different snapshot.
                    Err(error) => {
                        self.append_key_error(error);
                        if let Some(management) = &mut self.management {
                            management.info = None;
                        }
                    }
                    Ok(rotation) => {
                        let successful = rotation.failed.is_none();
                        let mut reload_error = None;
                        if let Some(management) = &mut self.management {
                            for target in rotation.completed {
                                management
                                    .phases
                                    .insert(target.scope.clone(), Phase::Updated);
                                if !management.completed.contains(&target) {
                                    management.completed.push(target);
                                }
                            }
                            if let Some((target, error)) = rotation.failed {
                                management
                                    .phases
                                    .insert(target.scope.clone(), Phase::Failed);
                                management.info = rotation.retry.map(Arc::new);
                                let error = format!(
                                    "{} {}: {error}",
                                    crate::localization::text(
                                        cx,
                                        &model::completed_message(&management.completed)
                                    ),
                                    target.name
                                );
                                self.append_key_error(error);
                            } else {
                                management.clear_inputs(window, cx);
                                management.changing = false;
                                management.completed.clear();
                                management.phases.clear();
                                management.info = match attempt.management {
                                    Ok(info) => Some(info),
                                    Err(error) => {
                                        reload_error =
                                            Some(format!("Could not reload key details: {error}"));
                                        None
                                    }
                                };
                                self.notice =
                                    Some("Master Key updated for every connected provider.".into());
                            }
                        }
                        if let Some(error) = reload_error {
                            self.append_key_error(error);
                        }
                        self.backups = None;
                        cx.emit(Event::DataChanged(
                            if successful {
                                "Sync Master Key updated. Saved data was refreshed."
                            } else {
                                "Master Key rotation did not complete. Saved data was refreshed."
                            }
                            .into(),
                        ));
                    }
                }
            }
        }
        cx.emit(Event::Settled(self.error.is_none()));
        cx.notify();
    }
    fn append_key_error(&mut self, error: String) {
        self.error = Some(match self.error.take() {
            Some(previous) => format!("{previous} {error}"),
            None => error,
        });
    }
    fn forget_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(expected) = self
            .management
            .as_ref()
            .filter(|management| management.confirm_forget)
            .and_then(|management| management.info.clone())
        else {
            return;
        };
        let Some(service) = self.service.clone() else {
            return;
        };
        self.mutating = true;
        self.error = None;
        self.validation_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.forget_key(expected).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.mutating = false;
                match result {
                    Ok(forgotten) => {
                        if let Some(management) = &mut this.management {
                            management.clear_inputs(window, cx);
                            Modal::complete(&management.forget_focus, window, cx);
                            Modal::complete(&management.focus, window, cx);
                        }
                        this.management = None;
                        this.key.clear(window, cx);
                        match forgotten.overview {
                            Ok(overview) => this.overview = Some(overview),
                            Err(error) => {
                                if let Some(overview) = &mut this.overview {
                                    overview.has_key = false;
                                    overview.status.sync_snapshot = None;
                                }
                                this.error = Some(format!(
                                    "Sync key cache cleared. Could not refresh status: {error}"
                                ));
                            }
                        }
                        this.notice = Some("Local sync key cache cleared.".into());
                        cx.emit(Event::Unlocked);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.emit(Event::Settled(this.error.is_none()));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
