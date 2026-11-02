//! Saved-rule changes wait for this controller's native stop receipt. Public
//! configuration is published only afterwards; failures keep the previous file.
use super::*;
use async_channel::Receiver;

struct StopReceipt {
    id: Id,
    generation: u64,
    receipt: Receiver<port_forward::StopResult>,
}
async fn stopped(receipts: Vec<StopReceipt>) -> Result<Vec<(Id, u64)>> {
    let mut done = vec![];
    for stop in receipts {
        stop.receipt.recv().await.context("The forwarding controller closed before confirming the stop. Saved configuration was preserved.")?.map_err(anyhow::Error::msg)?;
        done.push((stop.id, stop.generation));
    }
    Ok(done)
}

impl Nauterm {
    fn stop_for_forward_change(&mut self, id: Id) -> Result<Option<StopReceipt>> {
        let Some(service) = &self.forward_service else {
            ensure!(
                !self.forward_active(id),
                "The running forwarding controller is unavailable."
            );
            return Ok(None);
        };
        let generation = self.next_forward_generation;
        self.next_forward_generation += 1;
        let receipt = service.stop_with_ack(id, generation)?;
        // Invalidates credentials still loading on the UI side, and rejects any
        // older native start/status event already queued by the worker.
        let mut status = self
            .forward_states
            .get(&id)
            .map(|run| run.status.clone())
            .unwrap_or_else(|| Status::new(id, State::Stopping));
        status.state = State::Stopping;
        status.error = None;
        self.forward_states
            .insert(id, Runtime { generation, status });
        Ok(Some(StopReceipt {
            id,
            generation,
            receipt,
        }))
    }
    fn settle_forward_stops(&mut self, done: &[(Id, u64)]) {
        for (id, generation) in done {
            if let Some(run) = self.forward_states.get_mut(id)
                && run.generation == *generation
            {
                run.status = Status::new(*id, State::Stopped);
            }
        }
    }
    fn failed_forward_stops(&mut self, ids: impl IntoIterator<Item = Id>, message: &str) {
        for id in ids {
            if let Some(run) = self.forward_states.get_mut(&id)
                && run.status.state == State::Stopping
            {
                run.status.stop_failed(message);
            }
        }
    }
    fn forward_save_error(
        &mut self,
        token: EntityId,
        error: anyhow::Error,
        cx: &mut Context<Self>,
    ) {
        let message = format!("Could not save forwarding rule: {error}");
        if let Some(editor) =
            self.find_editor_mut(&|editor| editor.fields[0].1.entity_id() == token)
        {
            if let Some(forward) = &mut editor.forward {
                forward.saving = false;
            }
            editor.error = Some(message);
        } else {
            self.notice = Some(message);
        }
        cx.notify();
    }
    fn publish_forward_change(
        &mut self,
        mut rule: Forward,
        before: Option<Forward>,
        token: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        ensure!(self.resources_ready(), "Workspace storage is unavailable.");
        ensure!(
            before.as_ref().is_none_or(|before| self
                .data
                .forwards
                .iter()
                .find(|saved| saved.id == before.id)
                == Some(before)),
            "This forwarding rule changed while it was stopping. Reopen it and try again."
        );
        ensure!(
            self.data
                .hosts
                .iter()
                .any(|host| host.supports_ssh() && Some(host.id) == rule.host_id),
            "The selected SSH host is no longer available."
        );
        let mut candidate = self.data.clone();
        if before.is_none() {
            rule.id = candidate.allocate_id();
        }
        rule.spec()?;
        let id = rule.id;
        candidate.forwards.retain(|saved| saved.id != id);
        candidate.forwards.push(rule);
        self.store.save(&mut candidate)?;
        self.data = candidate;
        self.forward_states.remove(&id);
        // Saving the reference rule leaves it stopped, without an automatic restart.
        self.with_editor_context(
            |editor| editor.fields[0].1.entity_id() == token,
            window,
            cx,
            |this, window, cx| this.complete_resource_editor(ItemKind::Forward, id, window, cx),
        );
        cx.notify();
        Ok(())
    }
    pub(super) fn save_forward_after_stop(
        &mut self,
        rule: Forward,
        token: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self
            .editor
            .as_ref()
            .and_then(|editor| editor.id)
            .and_then(|id| self.data.forwards.iter().find(|saved| saved.id == id))
            .cloned();
        if let Some(id) = self.editor.as_ref().and_then(|editor| editor.id)
            && before.is_none()
        {
            self.forward_save_error(
                token,
                anyhow::anyhow!("Forwarding rule #{id} no longer exists."),
                cx,
            );
            return;
        }
        let Some(id) = before.as_ref().map(|before| before.id) else {
            if let Err(error) = self.publish_forward_change(rule, before, token, window, cx) {
                self.forward_save_error(token, error, cx);
            }
            return;
        };
        self.forward_mutations.insert(id);
        let receipt = match self.stop_for_forward_change(id) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.forward_mutations.remove(&id);
                self.forward_save_error(token, error, cx);
                return;
            }
        };
        let Some(receipt) = receipt else {
            self.forward_mutations.remove(&id);
            if let Err(error) = self.publish_forward_change(rule, before, token, window, cx) {
                self.forward_save_error(token, error, cx);
            }
            return;
        };
        self.editor
            .as_mut()
            .unwrap()
            .forward
            .as_mut()
            .unwrap()
            .saving = true;
        self.editor.as_mut().unwrap().error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = stopped(vec![receipt]).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.forward_mutations.remove(&id);
                let result = result.and_then(|done| {
                    this.settle_forward_stops(&done);
                    this.publish_forward_change(rule, before, token, window, cx)
                });
                if let Err(error) = result {
                    this.failed_forward_stops([id], &error.to_string());
                    this.forward_save_error(token, error, cx);
                }
            });
        })
        .detach();
        cx.notify();
    }
    fn publish_forward_deletion(&mut self, before: &[Forward]) -> Result<()> {
        ensure!(self.resources_ready(), "Workspace storage is unavailable.");
        ensure!(
            before.iter().all(
                |rule| self.data.forwards.iter().find(|saved| saved.id == rule.id) == Some(rule)
            ),
            "A selected forwarding rule changed while stopping. Review the selection and retry."
        );
        let ids: BTreeSet<_> = before.iter().map(|rule| rule.id).collect();
        let mut candidate = self.data.clone();
        candidate.delete(ItemKind::Forward, &ids);
        self.store.save(&mut candidate)?;
        self.data = candidate;
        self.forward_states.retain(|id, _| !ids.contains(id));
        Ok(())
    }
    /// true: synchronous deletion completed; false: keep the busy confirmation
    /// open while this same selected batch waits for native stop receipts.
    pub(in crate::ui) fn delete_forward_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<bool> {
        ensure!(
            !self.forward_delete_busy,
            "Forwarding deletion is already in progress."
        );
        ensure!(self.resources_ready(), "Workspace storage is unavailable.");
        let ids = self.selection.clone();
        ensure!(!ids.is_empty(), "Select a forwarding rule to delete.");
        ensure!(
            ids.iter().all(|id| !self.forward_mutations.contains(id)),
            "A selected forwarding rule is being updated."
        );
        let before: Vec<_> = ids
            .iter()
            .map(|id| {
                self.data
                    .forwards
                    .iter()
                    .find(|rule| rule.id == *id)
                    .cloned()
                    .context("A selected forwarding rule no longer exists.")
            })
            .collect::<Result<_>>()?;
        self.forward_mutations.extend(ids.iter().copied());
        let mut receipts = vec![];
        for id in &ids {
            match self.stop_for_forward_change(*id) {
                Ok(Some(receipt)) => receipts.push(receipt),
                Ok(None) => {}
                Err(error) => {
                    self.forward_mutations.retain(|id| !ids.contains(id));
                    self.failed_forward_stops(ids.iter().copied(), &error.to_string());
                    return Err(error);
                }
            }
        }
        if receipts.is_empty() {
            self.forward_mutations.retain(|id| !ids.contains(id));
            self.publish_forward_deletion(&before)?;
            return Ok(true);
        }
        self.forward_delete_busy = true;
        self.notice = None;
        let page = self.page;
        cx.spawn_in(window, async move |this, cx| {
            let result = stopped(receipts).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.forward_mutations.retain(|id| !ids.contains(id));
                this.forward_delete_busy = false;
                let result = result.and_then(|done| {
                    this.settle_forward_stops(&done);
                    this.publish_forward_deletion(&before)
                });
                match result {
                    Ok(()) => {
                        this.in_drawer_page(page, window, cx, |this, _, _| {
                            this.selection.clear();
                            this.selection_kind = None;
                            this.retire_editor();
                        });
                        this.delete_pending = false;
                    }
                    Err(error) => {
                        this.failed_forward_stops(ids.iter().copied(), &error.to_string());
                        this.notice = Some(format!("Could not delete forwarding rules: {error}"))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        Ok(false)
    }
}
