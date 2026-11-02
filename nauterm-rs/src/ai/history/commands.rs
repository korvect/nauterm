//! Command proposals/results live in the reference message and command rows.
//! Persist the reviewed running state before any terminal input is submitted.
use super::*;
use crate::{
    ai::{terminal_runner, tools},
    native::CommandTarget,
};
use nauterm_core::AiCommandBlockEntry;
use std::collections::BTreeMap;

pub(super) fn unresolved(entry: &AiConversationEntry) -> bool {
    entry
        .command_blocks
        .iter()
        .any(|block| matches!(block.status.as_str(), "pending" | "running"))
        || tools::pending_calls(&entry.messages).is_ok_and(|calls| !calls.is_empty())
}
pub(super) fn running(entry: &AiConversationEntry) -> bool {
    entry
        .command_blocks
        .iter()
        .any(|block| block.status == "running")
}
pub(super) fn next_sequence(entry: &AiConversationEntry) -> Result<i64> {
    entry
        .messages
        .iter()
        .map(|message| message.sequence)
        .chain(entry.command_blocks.iter().map(|block| block.sequence))
        .max()
        .map_or(Some(0), |sequence| sequence.checked_add(1))
        .context("This conversation has too many items.")
}
pub(super) fn supported(entry: &AiConversationEntry) -> Result<()> {
    tools::pending_calls(&entry.messages)?;
    let mut calls = BTreeMap::new();
    for message in &entry.messages {
        super::super::attachment::parse(&message.attachments)?;
        for call in tools::calls(&message.tool_calls)? {
            calls.insert(call.id.clone(), call);
        }
    }
    ensure!(
        calls.len() == entry.command_blocks.len(),
        "This history contains unsupported command records."
    );
    for block in &entry.command_blocks {
        let call = calls
            .remove(&block.tool_call_id)
            .context("A command does not match its original tool call.")?;
        let arguments = call.terminal()?;
        ensure!(
            arguments.command == block.command.replace("\r\n", "\n").trim(),
            "The command changed since it was proposed."
        );
        ensure!(
            arguments.explanation == block.explanation.trim(),
            "The command explanation changed since it was proposed."
        );
        ensure!(
            matches!(
                block.status.as_str(),
                "pending"
                    | "running"
                    | "submitted"
                    | "succeeded"
                    | "failed"
                    | "cancelled"
                    | "skipped"
            ),
            "This history has an unsupported command status."
        );
    }
    Ok(())
}
pub(super) fn propose(entry: &mut AiConversationEntry, calls: Vec<tools::Call>) -> Result<()> {
    ensure!(
        entry.scope == "terminal",
        "Only terminal conversations can propose commands."
    );
    tools::validate_history(&entry.messages)?;
    let mut existing: std::collections::BTreeSet<_> = entry
        .command_blocks
        .iter()
        .map(|block| block.tool_call_id.as_str())
        .collect();
    ensure!(
        calls.iter().all(|call| existing.insert(call.id.as_str())),
        "The AI provider repeated an earlier command ID."
    );
    let mut sequence = next_sequence(entry)?;
    let mut blocks = vec![];
    for call in &calls {
        let arguments = call.terminal()?;
        blocks.push(serde_json::from_value::<AiCommandBlockEntry>(json!({
            "uuid":uuid::Uuid::new_v4().to_string(),"tool_call_id":call.id,"command":arguments.command,"explanation":arguments.explanation,"status":"pending","sequence":sequence
        }))?);
        sequence = sequence
            .checked_add(1)
            .context("This conversation has too many items.")?;
    }
    let message = entry
        .messages
        .last_mut()
        .filter(|message| message.role == "assistant")
        .context("The response message is unavailable.")?;
    ensure!(
        message.tool_calls.is_empty(),
        "The AI provider repeated a command proposal batch."
    );
    message.tool_calls = calls
        .into_iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<_, _>>()?;
    if message.content.trim().is_empty() {
        message.content = blocks
            .first()
            .map(|block| block.explanation.clone())
            .unwrap_or_default();
    }
    entry.command_blocks.extend(blocks);
    Ok(())
}
fn tool_result(block: &AiCommandBlockEntry, call: &tools::Call) -> serde_json::Value {
    let submitted = block.status == "submitted";
    let date = |value: Option<i64>| {
        value
            .and_then(chrono::DateTime::from_timestamp_millis)
            .map(|date| date.to_rfc3339())
    };
    let content = json!({"command":block.command,"explanation":block.explanation,"exit_code":block.exit_code,"output":block.output,"error":block.error,
        "started_at":date(block.started_at),"finished_at":date(block.finished_at),"cancelled":block.status == "cancelled","skipped":block.status == "skipped","submitted":submitted,"result_tracked":block.exit_code.is_some()}).to_string();
    let mut result = json!({"tool_call_id":block.tool_call_id,"name":tools::NAME,"content":content,"is_error":!matches!(block.status.as_str(),"succeeded"|"submitted")});
    if call.omit_provider_id {
        result["omit_provider_id"] = true.into();
    }
    result
}
pub(super) fn append_results(entry: &mut AiConversationEntry) -> Result<bool> {
    let calls = tools::pending_calls(&entry.messages)?;
    if calls.is_empty() {
        return Ok(false);
    }
    let mut results = vec![];
    let mut sequence = next_sequence(entry)?;
    for call in calls {
        let block = entry
            .command_blocks
            .iter()
            .find(|block| block.tool_call_id == call.id)
            .context("A command result is unavailable.")?;
        if matches!(block.status.as_str(), "pending" | "running") {
            return Ok(false);
        }
        let mut result = message("tool", String::new(), String::new(), sequence);
        result.tool_result = Some(tool_result(block, &call));
        results.push(result);
        sequence = sequence
            .checked_add(1)
            .context("This conversation has too many items.")?;
    }
    entry.messages.extend(results);
    Ok(true)
}
/// Close an approval round without submitting any of its remaining commands.
pub(super) fn cancel_pending(entry: &mut AiConversationEntry) -> Result<bool> {
    supported(entry)?;
    ensure!(!running(entry), "Wait for the running command to stop.");
    let mut planned = entry.clone();
    let mut changed = false;
    for block in &mut planned.command_blocks {
        if block.status == "pending" {
            block.status = "cancelled".into();
            block.finished_at = Some(chrono::Utc::now().timestamp_millis());
            block.error = Some("Stopped by user before execution.".into());
            changed = true;
        }
    }
    changed |= append_results(&mut planned)?;
    if changed {
        *entry = planned;
    }
    Ok(changed)
}
impl Session {
    pub fn stop_turn(self: &Arc<Self>) -> Result<()> {
        let (done, idle) = async_channel::bounded(1);
        {
            let mut state = self.data.lock().unwrap();
            if let Some(cancel) = state.cancel.clone() {
                state.stop_requested = true;
                cancel.cancel();
                drop(state);
                self.notify();
                return Ok(());
            }
            ensure!(
                !self.released.load(Ordering::Acquire) && !SHUTTING_DOWN.load(Ordering::Acquire),
                "The conversation is closed."
            );
            if !cancel_pending(&mut state.entry)? {
                return Ok(());
            }
            state.stopped = true;
            state.stop_requested = true;
            state.error = None;
            state.commands_changed();
            let cancel = Cancel::new();
            cancel.cancel();
            state.cancel = Some(cancel);
            state.idle = Some(idle);
        }
        self.notify();
        let session = self.clone();
        smol::spawn(async move {
            let saved = session.persist().await;
            {
                let mut state = session.data.lock().unwrap();
                state.cancel = None;
                if let Err(error) = saved {
                    state.error = Some(error.to_string());
                }
            }
            session.notify();
            drop(done);
        })
        .detach();
        Ok(())
    }
    pub fn matches_host(&self, host: Option<&str>) -> bool {
        self.data.lock().unwrap().entry.host_uuid.as_deref() == host
    }
    pub fn has_unresolved(&self) -> bool {
        unresolved(&self.data.lock().unwrap().entry)
    }
    pub fn review(
        &self,
        id: &str,
        fallback: Option<CommandOwner>,
    ) -> Result<(CommandOwner, String)> {
        let state = self.data.lock().unwrap();
        ensure!(
            !self.released.load(Ordering::Acquire) && !SHUTTING_DOWN.load(Ordering::Acquire),
            "The conversation is closed."
        );
        ensure!(
            state.cancel.is_none()
                && state.revision == state.saved_revision
                && state.supported
                && !running(&state.entry),
            "Finish or save the current operation first."
        );
        ensure!(
            state.entry.scope == "terminal",
            "This conversation is not bound to a terminal."
        );
        let block = state
            .entry
            .command_blocks
            .iter()
            .find(|block| block.tool_call_id == id && block.status == "pending")
            .context("The command is no longer pending review.")?;
        let owner = state
            .command_owner
            .clone()
            .or(fallback)
            .context("Connect the original terminal before running this command.")?;
        owner.validate()?;
        Ok((owner, block.command.replace("\r\n", "\n")))
    }
    pub fn execute(self: &Arc<Self>, id: String, target: CommandTarget) -> Result<()> {
        let (owner, command) = self.review(&id, Some(target.owner.clone()))?;
        ensure!(
            owner.same(&target.owner),
            "The terminal connection changed. Review a new command before running it."
        );
        let cancel = Cancel::new();
        let (done, idle) = async_channel::bounded::<()>(1);
        {
            let mut state = self.data.lock().unwrap();
            ensure!(
                state.cancel.is_none() && state.revision == state.saved_revision,
                "Finish or save the current operation first."
            );
            let block = state
                .entry
                .command_blocks
                .iter_mut()
                .find(|block| block.tool_call_id == id && block.status == "pending")
                .context("The command changed before approval.")?;
            block.status = "running".into();
            block.started_at = Some(chrono::Utc::now().timestamp_millis());
            block.error = None;
            state.command_owner = Some(owner);
            state.active_command = Some(id.clone());
            state.cancel = Some(cancel.clone());
            state.idle = Some(idle);
            state.error = None;
            state.stopped = false;
            state.stop_requested = false;
            state.commands_changed();
        }
        self.notify();
        let session = self.clone();
        smol::spawn(async move {
            // A failed optimistic save leaves the command pending and sends no input.
            if let Err(error) = session.persist().await {
                let mut state = session.data.lock().unwrap();
                if let Some(block) = state
                    .entry
                    .command_blocks
                    .iter_mut()
                    .find(|block| block.tool_call_id == id)
                {
                    block.status = "pending".into();
                    block.started_at = None;
                }
                state.commands_changed();
                state.error = Some(error.to_string());
                state.active_command = None;
                state.cancel = None;
                drop(state);
                session.notify();
                drop(done);
                return;
            }
            let result = terminal_runner::run(target, command, cancel.clone()).await;
            {
                let mut state = session.data.lock().unwrap();
                if let Some(block) = state
                    .entry
                    .command_blocks
                    .iter_mut()
                    .find(|block| block.tool_call_id == id)
                {
                    match result {
                        Ok(result) => {
                            block.status = if result.cancelled {
                                "cancelled"
                            } else if !result.tracked {
                                "submitted"
                            } else if result.exit_code == Some(0) {
                                "succeeded"
                            } else {
                                "failed"
                            }
                            .into();
                            block.output = Some(result.output);
                            block.exit_code = result.exit_code;
                            block.started_at = Some(result.started_at);
                            block.finished_at = Some(result.finished_at);
                            block.error = result.cancelled.then(|| {
                                if result.tracked {
                                    "Stopped by user."
                                } else {
                                    "Stopped by user; command completion was not observed."
                                }
                                .into()
                            });
                        }
                        Err(error) => {
                            block.status = if cancel.cancelled() {
                                "cancelled"
                            } else {
                                "failed"
                            }
                            .into();
                            block.error = Some(error.to_string());
                            block.finished_at = Some(chrono::Utc::now().timestamp_millis());
                        }
                    }
                }
                state.commands_changed();
                state.active_command = None;
                state.stopped = cancel.cancelled();
                if state.stop_requested {
                    match cancel_pending(&mut state.entry) {
                        Ok(true) => state.commands_changed(),
                        Ok(false) => {}
                        Err(error) => state.error = Some(error.to_string()),
                    }
                }
            }
            let saved = session.persist().await;
            {
                let mut state = session.data.lock().unwrap();
                state.cancel = None;
                if let Err(error) = saved {
                    state.error = Some(error.to_string());
                }
            }
            session.notify();
            drop(done);
            if let Err(error) = session.continue_commands().await {
                session.data.lock().unwrap().error = Some(error.to_string());
                session.notify();
            }
        })
        .detach();
        Ok(())
    }
    pub async fn skip(self: &Arc<Self>, id: String) -> Result<()> {
        {
            let mut state = self.data.lock().unwrap();
            ensure!(
                !self.released.load(Ordering::Acquire) && !SHUTTING_DOWN.load(Ordering::Acquire),
                "The conversation is closed."
            );
            ensure!(
                state.cancel.is_none()
                    && state.revision == state.saved_revision
                    && state.supported
                    && !running(&state.entry),
                "Finish or save the current operation first."
            );
            let block = state
                .entry
                .command_blocks
                .iter_mut()
                .find(|block| block.tool_call_id == id && block.status == "pending")
                .context("The command is no longer pending review.")?;
            block.status = "skipped".into();
            block.error = Some("Skipped by user.".into());
            block.finished_at = Some(chrono::Utc::now().timestamp_millis());
            state.commands_changed();
        }
        self.notify();
        self.persist().await?;
        self.continue_commands().await
    }
    pub(super) async fn continue_commands(self: &Arc<Self>) -> Result<()> {
        let generation = {
            let mut state = self.data.lock().unwrap();
            if SHUTTING_DOWN.load(Ordering::Acquire)
                || self.released.load(Ordering::Acquire)
                || state.cancel.is_some()
                || state.revision != state.saved_revision
                || !state.supported
                || state.stop_requested
            {
                return Ok(());
            }
            if !append_results(&mut state.entry)? {
                return Ok(());
            }
            let messages = state.entry.messages.clone();
            let sequence = next_sequence(&state.entry)?;
            state
                .entry
                .messages
                .push(message("assistant", String::new(), String::new(), sequence));
            let cancel = Cancel::new();
            let (done, idle) = async_channel::bounded(1);
            state.cancel = Some(cancel.clone());
            state.idle = Some(idle);
            state.revision += 1;
            state.stopped = false;
            Generation {
                provider: state.entry.provider_uuid.clone().unwrap_or_default(),
                model: state.entry.model.clone(),
                messages,
                cancel,
                prepared: None,
                target: state.command_owner.clone(),
                done,
            }
        };
        self.notify();
        self.launch(generation);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry() -> AiConversationEntry {
        let mut entry = scoped_blank(Scope::Terminal, None);
        append_turn(
            &mut entry,
            message("user", "inspect".into(), String::new(), 0),
            Turn::Append,
        )
        .unwrap();
        entry
    }
    fn call(id: &str) -> tools::Call {
        serde_json::from_value(json!({"id":id,"name":tools::NAME,"arguments":{"command":"pwd","explanation":"Read the directory"}})).unwrap()
    }
    #[test]
    fn proposals_wait_for_every_review_and_results_are_appended_once() -> Result<()> {
        let mut entry = entry();
        propose(&mut entry, vec![call("a"), call("b")])?;
        supported(&entry)?;
        assert!(unresolved(&entry));
        assert!(!append_results(&mut entry)?);
        entry.command_blocks[0].status = "succeeded".into();
        entry.command_blocks[0].output = Some("/fixture".into());
        entry.command_blocks[0].exit_code = Some(0);
        let before = entry.messages.clone();
        assert!(!append_results(&mut entry)?);
        assert_eq!(entry.messages, before);
        entry.command_blocks[1].status = "skipped".into();
        assert!(append_results(&mut entry)?);
        assert!(!append_results(&mut entry)?);
        assert!(!unresolved(&entry));
        tools::validate_history(&entry.messages)?;
        let result = entry.messages.last().unwrap().tool_result.as_ref().unwrap();
        assert_eq!(result["is_error"], true);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(result["content"].as_str().unwrap())?["skipped"],
            true
        );
        Ok(())
    }
    #[test]
    fn optional_provider_ids_and_unconfirmed_cancellation_survive_result_persistence() -> Result<()>
    {
        let mut entry = entry();
        let mut proposal = call("internal-a");
        proposal.omit_provider_id = true;
        propose(&mut entry, vec![proposal])?;
        entry.command_blocks[0].status = "cancelled".into();
        entry.command_blocks[0].error = Some("Completion was not observed.".into());
        assert!(append_results(&mut entry)?);
        tools::validate_history(&entry.messages)?;
        let result = entry.messages.last().unwrap().tool_result.as_ref().unwrap();
        let content: serde_json::Value = serde_json::from_str(result["content"].as_str().unwrap())?;
        assert_eq!(result["omit_provider_id"], true);
        assert_eq!(content["result_tracked"], false);
        assert_eq!(content["cancelled"], true);
        assert!(content["exit_code"].is_null());
        Ok(())
    }
    #[test]
    fn stopping_pending_commands_closes_the_round_without_execution() -> Result<()> {
        let mut entry = entry();
        propose(
            &mut entry,
            vec![call("done"), call("pending-a"), call("pending-b")],
        )?;
        entry.command_blocks[0].status = "succeeded".into();
        entry.command_blocks[0].exit_code = Some(0);
        entry.command_blocks[0].output = Some("previous result".into());
        let completed = entry.command_blocks[0].clone();
        assert!(cancel_pending(&mut entry)?);
        assert_eq!(entry.command_blocks[0], completed);
        for block in &entry.command_blocks[1..] {
            assert_eq!(block.status, "cancelled");
            assert!(block.started_at.is_none());
            assert!(block.exit_code.is_none());
            assert!(block.output.is_none());
            assert!(block.finished_at.is_some());
        }
        assert!(!unresolved(&entry));
        assert_eq!(
            entry
                .messages
                .iter()
                .filter(|message| message.role == "tool")
                .count(),
            3
        );
        tools::validate_history(&entry.messages)?;
        let stopped = entry.clone();
        assert!(!cancel_pending(&mut entry)?);
        assert_eq!(entry, stopped);
        let wire = append_turn(
            &mut entry,
            message("user", "next question".into(), String::new(), 0),
            Turn::Append,
        )?;
        tools::validate_history(&wire)?;
        assert_eq!(entry.command_blocks, stopped.command_blocks);
        Ok(())
    }
    #[test]
    fn approval_cancellation_does_not_mutate_a_running_command() -> Result<()> {
        let mut entry = entry();
        propose(&mut entry, vec![call("running"), call("pending")])?;
        entry.command_blocks[0].status = "running".into();
        let before = entry.clone();
        assert!(cancel_pending(&mut entry).is_err());
        assert_eq!(entry, before);
        Ok(())
    }
    #[test]
    fn a_followup_keeps_executed_commands_and_their_results() -> Result<()> {
        let mut entry = entry();
        propose(&mut entry, vec![call("a")])?;
        entry.command_blocks[0].status = "succeeded".into();
        entry.command_blocks[0].exit_code = Some(0);
        append_results(&mut entry)?;
        let block = entry.command_blocks[0].clone();
        let next = next_sequence(&entry)?;
        entry.messages.push(message(
            "assistant",
            "old answer".into(),
            String::new(),
            next,
        ));
        let wire = append_turn(
            &mut entry,
            message("user", "follow-up question".into(), String::new(), 0),
            Turn::Append,
        )?;
        assert_eq!(entry.command_blocks, vec![block]);
        assert!(wire.iter().any(|message| message.role == "tool"));
        assert!(wire.iter().any(|message| message.content == "old answer"));
        assert_eq!(wire.last().unwrap().content, "follow-up question");
        tools::validate_history(&wire)?;
        Ok(())
    }
    #[test]
    fn changed_reviewed_commands_and_duplicate_proposals_are_rejected() -> Result<()> {
        let mut entry = entry();
        let before = entry.clone();
        assert!(propose(&mut entry, vec![call("a"), call("a")]).is_err());
        assert_eq!(entry, before);
        propose(&mut entry, vec![call("a")])?;
        entry.command_blocks[0].command = "different command".into();
        assert!(supported(&entry).is_err());
        Ok(())
    }
}
