//! Full reference conversation records, saved through the shared serial worker.
//! A stream owns its session until the final save, even after its UI is dropped.
use super::{
    Record, Service,
    client::{self, Cancel},
};
use anyhow::{Context, Result, ensure};
use nauterm_core::{AiConversationEntry, AiMessageEntry, NautermDatabase};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::{
    Arc, Mutex, OnceLock, Weak,
    atomic::{AtomicBool, Ordering},
};
mod commands;
mod presentation;
use crate::native::CommandOwner;

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Terminal,
    Workspace,
}
impl Scope {
    pub fn id(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Workspace => "workspace",
        }
    }
}

fn registry() -> &'static Mutex<Vec<Weak<Session>>> {
    static SESSIONS: OnceLock<Mutex<Vec<Weak<Session>>>> = OnceLock::new();
    SESSIONS.get_or_init(Default::default)
}
pub async fn prepare_shutdown() {
    SHUTTING_DOWN.store(true, Ordering::Release);
    close_window().await;
}
pub async fn close_window() {
    let sessions: Vec<_> = registry()
        .lock()
        .unwrap()
        .iter()
        .filter_map(Weak::upgrade)
        .collect();
    for session in &sessions {
        session.stop();
    }
    for session in sessions {
        session.wait_idle().await;
        if let Err(error) = session.persist().await {
            eprintln!("Could not save AI conversation: {error}");
        }
    }
}
fn stamp(entry: &AiConversationEntry) -> Result<[u8; 32]> {
    Ok(Sha256::digest(serde_json::to_vec(entry)?).into())
}
fn message(role: &str, content: String, context: String, sequence: i64) -> AiMessageEntry {
    serde_json::from_value(json!({"uuid":uuid::Uuid::new_v4().to_string(),"role":role,"content":content,"context":context,"sequence":sequence,"created_at":chrono::Utc::now().timestamp_millis()})).expect("valid new message")
}
fn blank(host_uuid: Option<String>) -> AiConversationEntry {
    serde_json::from_value(json!({"uuid":uuid::Uuid::new_v4().to_string(),"title":"New conversation","scope":"terminal","host_uuid":host_uuid})).expect("valid new conversation")
}
fn scoped_blank(scope: Scope, host_uuid: Option<String>) -> AiConversationEntry {
    let mut entry = blank(if scope == Scope::Terminal {
        host_uuid
    } else {
        None
    });
    entry.scope = scope.id().into();
    entry
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Turn {
    Append,
    Edit,
}
fn text_history(entry: &AiConversationEntry) -> bool {
    commands::supported(entry).is_ok()
}
fn edited_user(
    entry: &AiConversationEntry,
    uuid: &str,
    content: &str,
) -> Result<Option<AiMessageEntry>> {
    let original = entry
        .messages
        .iter()
        .find(|message| message.role == "user" && message.uuid.as_deref() == Some(uuid))
        .context("The user message is no longer available.")?;
    let content = content.trim();
    ensure!(
        !content.is_empty() || !original.attachments.is_empty(),
        "Enter a message or keep an attachment."
    );
    if content == original.content {
        return Ok(None);
    }
    let mut user = original.clone();
    user.content = content.to_owned();
    user.updated_at = Some(chrono::Utc::now().timestamp_millis());
    Ok(Some(user))
}
fn append_turn(
    entry: &mut AiConversationEntry,
    mut user: AiMessageEntry,
    turn: Turn,
) -> Result<Vec<AiMessageEntry>> {
    let keep = if turn != Turn::Append {
        ensure!(user.uuid.is_some(), "The user message has no stable ID.");
        entry
            .messages
            .iter()
            .position(|message| message.role == "user" && message.uuid == user.uuid)
            .context("The user message changed before resending.")?
    } else {
        entry.messages.len()
    };
    let sequence = if turn == Turn::Append {
        entry.messages[..keep]
            .last()
            .map_or(Some(0), |message| message.sequence.checked_add(1))
            .context("This conversation has too many messages.")?
    } else {
        user.sequence
    };
    let response_sequence = sequence
        .checked_add(1)
        .context("This conversation has too many messages.")?;
    if (turn == Turn::Append && entry.messages.is_empty())
        || (turn == Turn::Edit
            && !entry.messages[..keep]
                .iter()
                .any(|message| message.role == "user"))
    {
        let content = user.content.trim();
        let fallback = user
            .attachments
            .first()
            .and_then(|value| value["name"].as_str())
            .unwrap_or("New conversation");
        let content = if content.is_empty() {
            fallback
        } else {
            content
        };
        entry.title = content
            .lines()
            .next()
            .unwrap_or(content)
            .chars()
            .take(80)
            .collect();
    }
    entry.messages.truncate(keep);
    if turn != Turn::Append {
        entry
            .command_blocks
            .retain(|block| block.sequence < sequence);
    }
    user.sequence = sequence;
    entry.messages.push(user);
    let messages = entry.messages.clone();
    entry.messages.push(message(
        "assistant",
        String::new(),
        String::new(),
        response_sequence,
    ));
    Ok(messages)
}

pub struct View {
    pub entry: ConversationView,
    pub busy: bool,
    pub error: Option<String>,
    pub stopped: bool,
    pub active_command: Option<String>,
    pub command_owner: Option<CommandOwner>,
    pub supported: bool,
    pub stopping: bool,
}
/// Presentation only: large attachment bodies remain in the session's full
/// record for persistence/retries, never copied for every render or text delta.
pub struct ConversationView {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub provider_uuid: Option<String>,
    pub model: String,
    pub messages: Vec<Arc<MessageView>>,
    pub host_uuid: Option<String>,
    pub command_blocks: presentation::CommandRecords,
}
impl ConversationView {
    pub fn commands_between(
        &self,
        after: i64,
        before: i64,
    ) -> &[Arc<nauterm_core::AiCommandBlockEntry>] {
        presentation::between(&self.command_blocks, after, before)
    }
}
pub struct MessageView {
    pub uuid: Option<String>,
    pub role: String,
    pub content: String,
    pub sequence: i64,
    pub created_at: Option<i64>,
    pub attachments: Vec<super::attachment::Summary>,
}
struct Data {
    command_views: presentation::Commands,
    view_messages: Vec<Arc<MessageView>>,
    view_revision: Option<u64>,
    entry: AiConversationEntry,
    supported: bool,
    stamp: Option<[u8; 32]>,
    revision: u64,
    saved_revision: u64,
    cancel: Option<Cancel>,
    idle: Option<async_channel::Receiver<()>>,
    error: Option<String>,
    stopped: bool,
    stop_requested: bool,
    command_owner: Option<CommandOwner>,
    active_command: Option<String>,
}
impl Data {
    fn commands_changed(&mut self) {
        self.command_views.invalidate();
        self.revision += 1;
    }
}
struct Generation {
    provider: String,
    model: String,
    messages: Vec<AiMessageEntry>,
    cancel: Cancel,
    prepared: Option<Record>,
    target: Option<CommandOwner>,
    done: async_channel::Sender<()>,
}
pub struct Session {
    service: Service,
    data: Mutex<Data>,
    changed: async_channel::Sender<()>,
    changes: async_channel::Receiver<()>,
    released: AtomicBool,
}
impl Session {
    fn from_entry(service: Service, entry: AiConversationEntry, saved: bool) -> Result<Arc<Self>> {
        let supported = text_history(&entry);
        let fingerprint = saved.then(|| stamp(&entry)).transpose()?;
        let (changed, changes) = async_channel::bounded(1);
        let session = Arc::new(Self {
            service,
            data: Mutex::new(Data {
                command_views: presentation::Commands::default(),
                view_messages: vec![],
                view_revision: None,
                entry,
                supported,
                stamp: fingerprint,
                revision: 0,
                saved_revision: 0,
                cancel: None,
                idle: None,
                error: None,
                stopped: false,
                stop_requested: false,
                command_owner: None,
                active_command: None,
            }),
            changed,
            changes,
            released: AtomicBool::new(false),
        });
        let mut sessions = registry().lock().unwrap();
        sessions.retain(|session| session.strong_count() > 0);
        sessions.push(Arc::downgrade(&session));
        Ok(session)
    }
    pub fn new(service: Service, scope: Scope, host_uuid: Option<String>) -> Arc<Self> {
        Self::from_entry(service, scoped_blank(scope, host_uuid), false)
            .expect("valid blank conversation")
    }
    pub fn subscribe(&self) -> async_channel::Receiver<()> {
        self.changes.clone()
    }
    pub fn is_empty(&self) -> bool {
        let state = self.data.lock().unwrap();
        state.entry.messages.is_empty() && state.entry.command_blocks.is_empty()
    }
    fn notify(&self) {
        let _ = self.changed.try_send(());
    }
    pub fn view(&self) -> View {
        let mut state = self.data.lock().unwrap();
        let Data {
            command_views,
            entry,
            ..
        } = &mut *state;
        let command_blocks = command_views.snapshot(&entry.command_blocks);
        if state.view_revision != Some(state.revision) {
            let previous = std::mem::take(&mut state.view_messages);
            state.view_messages = state
                .entry
                .messages
                .iter()
                .enumerate()
                .map(|(index, message)| {
                    if let Some(view) = previous.get(index).filter(|view| {
                        view.uuid == message.uuid
                            && view.role == message.role
                            && view.sequence == message.sequence
                            && view.created_at == message.created_at
                            && view.content == message.content
                            && view.attachments.len() == message.attachments.len()
                            && view.attachments.iter().zip(&message.attachments).all(
                                |(summary, value)| {
                                    summary.name == value["name"].as_str().unwrap_or("Attachment")
                                        && summary.image
                                            == (value["kind"].as_str() == Some("image"))
                                        && summary.redacted
                                            == value["redacted"].as_bool().unwrap_or(false)
                                },
                            )
                    }) {
                        return view.clone();
                    }
                    Arc::new(MessageView {
                        uuid: message.uuid.clone(),
                        role: message.role.clone(),
                        content: message.content.clone(),
                        sequence: message.sequence,
                        created_at: message.created_at,
                        attachments: message
                            .attachments
                            .iter()
                            .map(super::attachment::Summary::from_value)
                            .collect(),
                    })
                })
                .collect();
            state.view_revision = Some(state.revision);
        }
        View {
            entry: ConversationView {
                id: state.entry.id,
                uuid: state.entry.uuid.clone(),
                provider_uuid: state.entry.provider_uuid.clone(),
                model: state.entry.model.clone(),
                host_uuid: state.entry.host_uuid.clone(),
                command_blocks,
                messages: state.view_messages.clone(),
            },
            busy: state.cancel.is_some(),
            error: state.error.clone(),
            stopped: state.stopped,
            active_command: state.active_command.clone(),
            command_owner: state.command_owner.clone(),
            supported: state.supported,
            stopping: state.cancel.as_ref().is_some_and(Cancel::cancelled),
        }
    }
    pub fn busy(&self) -> bool {
        self.data.lock().unwrap().cancel.is_some()
    }
    pub fn has_running_command(&self) -> bool {
        commands::running(&self.data.lock().unwrap().entry)
    }
    pub fn stopping(&self) -> bool {
        self.data
            .lock()
            .unwrap()
            .cancel
            .as_ref()
            .is_some_and(Cancel::cancelled)
    }
    pub fn error(&self) -> Option<String> {
        self.data.lock().unwrap().error.clone()
    }
    pub fn dirty(&self) -> bool {
        let state = self.data.lock().unwrap();
        state.revision != state.saved_revision
    }
    pub async fn save_draft(self: &Arc<Self>) -> Result<()> {
        ensure!(!self.busy(), "Wait for the reply to finish before saving.");
        self.persist().await?;
        self.data.lock().unwrap().error = None;
        self.notify();
        self.continue_commands().await
    }
    pub fn stop(&self) {
        if let Some(cancel) = &self.data.lock().unwrap().cancel {
            cancel.cancel();
        }
    }
    pub fn release(&self) {
        self.released.store(true, Ordering::Release);
        self.stop();
    }
    pub fn can_edit(&self) -> bool {
        let state = self.data.lock().unwrap();
        state.cancel.is_none()
            && state.revision == state.saved_revision
            && state.supported
            && !commands::running(&state.entry)
    }
    async fn wait_idle(&self) {
        let receiver = self.data.lock().unwrap().idle.clone();
        if let Some(receiver) = receiver {
            let _ = receiver.recv().await;
        }
    }
    pub fn start(
        self: &Arc<Self>,
        prompt: String,
        context: String,
        attachments: Vec<serde_json::Value>,
        provider: String,
        model: String,
        target: Option<CommandOwner>,
    ) -> Result<()> {
        super::attachment::parse(&attachments)?;
        let mut user = message("user", prompt, context, 0);
        user.attachments = attachments;
        self.begin(user, Turn::Append, provider, model, None, target)
    }
    pub async fn edit(
        self: &Arc<Self>,
        uuid: String,
        content: String,
        provider: String,
        model: String,
        target: Option<CommandOwner>,
    ) -> Result<()> {
        let user = edited_user(&self.data.lock().unwrap().entry, &uuid, &content)?;
        let Some(user) = user else {
            return Ok(());
        };
        self.preflight(user, Turn::Edit, provider, model, target)
            .await
    }
    async fn preflight(
        self: &Arc<Self>,
        user: AiMessageEntry,
        turn: Turn,
        provider: String,
        model: String,
        target: Option<CommandOwner>,
    ) -> Result<()> {
        ensure!(
            (!user.content.is_empty() || !user.attachments.is_empty())
                && user.content.len() <= 512 * 1024,
            "Enter a message of at most 512 KiB."
        );
        ensure!(
            !provider.is_empty() && !model.is_empty(),
            "Choose a provider and model first."
        );
        let (mut planned, revision) = {
            let state = self.data.lock().unwrap();
            ensure!(
                state.cancel.is_none() && state.revision == state.saved_revision,
                "Finish or save the current reply first."
            );
            ensure!(
                text_history(&state.entry),
                "This history contains unsupported tools or attachments. Start a new conversation."
            );
            ensure!(
                !commands::running(&state.entry),
                "A command in this conversation is still running."
            );
            (state.entry.clone(), state.revision)
        };
        let messages = append_turn(&mut planned, user.clone(), turn)?;
        let record = self
            .service
            .provider_for_chat(Some(provider.clone()))
            .await?;
        client::validate(
            &record,
            &model,
            &messages,
            target.as_ref().is_some_and(CommandOwner::ready),
        )?;
        // A failed provider/model validation leaves the original timeline intact.
        self.begin(
            user,
            turn,
            provider,
            model,
            Some((record, revision)),
            target,
        )
    }
    fn begin(
        self: &Arc<Self>,
        user: AiMessageEntry,
        turn: Turn,
        provider: String,
        model: String,
        prepared: Option<(Record, u64)>,
        target: Option<CommandOwner>,
    ) -> Result<()> {
        ensure!(
            !SHUTTING_DOWN.load(Ordering::Acquire),
            "The application is closing."
        );
        ensure!(
            !self.released.load(Ordering::Acquire),
            "The conversation was closed."
        );
        ensure!(
            (!user.content.trim().is_empty() || !user.attachments.is_empty())
                && user.content.len() <= 512 * 1024,
            "Enter a message of at most 512 KiB."
        );
        ensure!(
            !provider.is_empty() && !model.trim().is_empty(),
            "Choose a provider and model first."
        );
        let (done, idle) = async_channel::bounded::<()>(1);
        let cancel = Cancel::new();
        let messages = {
            let mut state = self.data.lock().unwrap();
            ensure!(state.cancel.is_none(), "A reply is already in progress.");
            ensure!(
                state.revision == state.saved_revision,
                "Save the current conversation before continuing."
            );
            ensure!(
                turn != Turn::Append || !commands::unresolved(&state.entry),
                "Review the pending commands before sending another message."
            );
            ensure!(
                text_history(&state.entry),
                "This history contains unsupported tools or attachments. Start a new conversation."
            );
            ensure!(
                prepared
                    .as_ref()
                    .is_none_or(|(_, revision)| *revision == state.revision),
                "The conversation changed before resending."
            );
            ensure!(
                !commands::running(&state.entry),
                "A command in this conversation is still running."
            );
            let messages = append_turn(&mut state.entry, user, turn)?;
            state.entry.provider_uuid = Some(provider.clone());
            state.entry.model = model.clone();
            state.commands_changed();
            state.cancel = Some(cancel.clone());
            state.idle = Some(idle);
            state.error = None;
            state.stopped = false;
            state.stop_requested = false;
            state.command_owner = target.clone();
            messages
        };
        self.notify();
        self.launch(Generation {
            provider,
            model,
            messages,
            cancel,
            prepared: prepared.map(|(record, _)| record),
            target,
            done,
        });
        Ok(())
    }
    fn launch(self: &Arc<Self>, generation: Generation) {
        let owner = self.clone();
        smol::spawn(async move {
            let Generation {
                provider,
                model,
                messages,
                cancel,
                prepared,
                target,
                done,
            } = generation;
            let result = owner
                .generate(provider, model, messages, cancel.clone(), prepared, target)
                .await;
            {
                let mut state = owner.data.lock().unwrap();
                if state.entry.messages.last().is_some_and(|m| {
                    m.role == "assistant" && m.content.is_empty() && m.tool_calls.is_empty()
                }) {
                    state.entry.messages.pop();
                    state.revision += 1;
                }
                state.stopped = cancel.cancelled();
                state.error = result.err().map(|error| error.to_string());
                if state.stop_requested {
                    match commands::cancel_pending(&mut state.entry) {
                        Ok(true) => state.commands_changed(),
                        Ok(false) => {}
                        Err(error) => state.error = Some(error.to_string()),
                    }
                }
            }
            if let Err(error) = owner.persist().await {
                owner.data.lock().unwrap().error = Some(error.to_string());
            }
            owner.data.lock().unwrap().cancel = None;
            owner.notify();
            drop(done);
        })
        .detach();
    }
    async fn generate(
        self: &Arc<Self>,
        provider: String,
        model: String,
        messages: Vec<AiMessageEntry>,
        cancel: Cancel,
        prepared: Option<Record>,
        target: Option<CommandOwner>,
    ) -> Result<()> {
        let record = match prepared {
            Some(record) => record,
            None => self.service.provider_for_chat(Some(provider)).await?,
        };
        // Validate before saving a request that cannot be sent. The worker owns
        // the credential record; it never becomes part of conversation state.
        let terminal_tool = target.as_ref().is_some_and(CommandOwner::ready);
        client::validate(&record, &model, &messages, terminal_tool)?;
        self.persist().await?;
        if cancel.cancelled() {
            return Ok(());
        }
        let stream = client::stream(record, model, messages, cancel.clone(), terminal_tool);
        while let Ok(event) = stream.events.recv().await {
            let text = match event? {
                client::Event::Text(text) => text,
                client::Event::Metadata(_) if cancel.cancelled() => continue,
                client::Event::Metadata(metadata) => {
                    let mut state = self.data.lock().unwrap();
                    if let Some(message) = state
                        .entry
                        .messages
                        .last_mut()
                        .filter(|message| message.role == "assistant")
                    {
                        message.provider_metadata = Some(metadata);
                        state.revision += 1;
                    }
                    drop(state);
                    self.notify();
                    continue;
                }
                client::Event::Tools(_) if cancel.cancelled() => continue,
                client::Event::Tools(calls) => {
                    ensure!(
                        target.is_some(),
                        "This conversation is not bound to a terminal."
                    );
                    let mut state = self.data.lock().unwrap();
                    commands::propose(&mut state.entry, calls)?;
                    state.commands_changed();
                    drop(state);
                    self.notify();
                    continue;
                }
            };
            {
                let mut state = self.data.lock().unwrap();
                let message = state
                    .entry
                    .messages
                    .last_mut()
                    .context("The response message is unavailable.")?;
                ensure!(
                    message.content.len().saturating_add(text.len()) <= client::MAX_OUTPUT,
                    "The AI response exceeded the output limit."
                );
                message.content.push_str(&text);
                state.revision += 1;
            }
            self.notify();
        }
        Ok(())
    }
    async fn persist(self: &Arc<Self>) -> Result<()> {
        let session = self.clone();
        self.service
            .vault
            .dispatch(move |db| {
                let (entry, expected, revision) = {
                    let state = session.data.lock().unwrap();
                    if state.revision == state.saved_revision || state.entry.messages.is_empty() {
                        return Ok(());
                    }
                    (state.entry.clone(), state.stamp, state.revision)
                };
                let saved = db.inner.atomic(|db| save(db, &entry, expected))?;
                let fingerprint = stamp(&saved)?;
                let mut state = session.data.lock().unwrap();
                ensure!(
                    state.revision == revision,
                    "The conversation changed while saving; the local draft was retained."
                );
                state.entry = saved;
                state.command_views.invalidate();
                state.view_revision = None;
                state.stamp = Some(fingerprint);
                state.saved_revision = revision;
                Ok(())
            })
            .await
    }
}
fn save(
    db: &mut NautermDatabase,
    entry: &AiConversationEntry,
    expected: Option<[u8; 32]>,
) -> Result<AiConversationEntry> {
    let uuid = entry
        .uuid
        .as_deref()
        .context("The conversation has no UUID.")?;
    let current = db.get_ai_conversation(uuid)?;
    ensure!(
        current.as_ref().map(stamp).transpose()? == expected,
        "This conversation changed in another window or application. Its current contents were preserved; reload the history before continuing."
    );
    Ok(db.save_ai_conversation(entry)?)
}
#[derive(Clone)]
pub struct DeleteTarget {
    pub uuid: String,
    pub title: String,
    fingerprint: [u8; 32],
}
impl Service {
    pub async fn prepare_conversation_delete(
        &self,
        uuid: String,
        scope: Scope,
    ) -> Result<DeleteTarget> {
        self.vault
            .dispatch(move |db| {
                let entry = db
                    .inner
                    .get_ai_conversation(&uuid)?
                    .context("This conversation was deleted.")?;
                ensure!(
                    entry.scope == scope.id(),
                    "This conversation moved to another scope. Reload history before deleting it."
                );
                Ok(DeleteTarget {
                    uuid,
                    title: entry.title.clone(),
                    fingerprint: stamp(&entry)?,
                })
            })
            .await
    }
    pub async fn delete_conversation(&self, target: DeleteTarget) -> Result<()> {
        self.vault.dispatch(move |db| db.inner.atomic(|db| {
            if let Some(current) = db.get_ai_conversation(&target.uuid)? {
                ensure!(stamp(&current)? == target.fingerprint, "This conversation changed after the dialog opened. Close this dialog and review it again before deleting.");
                db.delete_ai_conversation(&target.uuid)?;
            }
            Ok(())
        })).await
    }
    pub async fn conversations(
        &self,
        scope: Scope,
        host_uuid: Option<String>,
    ) -> Result<Vec<AiConversationEntry>> {
        let host_uuid = if scope == Scope::Terminal {
            host_uuid
        } else {
            None
        };
        self.vault
            .dispatch(move |db| {
                Ok(db.inner.list_ai_conversations(
                    Some(scope.id()),
                    host_uuid.as_deref(),
                    Some(100),
                )?)
            })
            .await
    }
    pub async fn conversation(&self, uuid: String, scope: Scope) -> Result<Arc<Session>> {
        let service = self.clone();
        self.vault
            .dispatch(move |db| {
                let entry = db
                    .inner
                    .get_ai_conversation(&uuid)?
                    .context("This conversation was deleted.")?;
                ensure!(
                    entry.scope == scope.id(),
                    "This conversation moved to another scope. Reload history before opening it."
                );
                Session::from_entry(service, entry, true)
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_attachment_only_turns_keeps_the_file() -> Result<()> {
        let mut entry = scoped_blank(Scope::Workspace, None);
        let mut user = message("user", String::new(), String::new(), 0);
        user.attachments.push(json!({"id":"file", "name":"example.txt", "mime_type":"text/plain", "size":3,"kind":"text","text":"abc","bytes":null,"redacted":false}));
        append_turn(&mut entry, user.clone(), Turn::Append)?;
        assert_eq!(entry.title, "example.txt");
        assert!(text_history(&entry));
        user.content = "Explain this".into();
        append_turn(&mut entry, user.clone(), Turn::Edit)?;
        assert_eq!(entry.title, "Explain this");
        assert_eq!(entry.messages[0].uuid, user.uuid);
        assert_eq!(entry.messages[0].attachments, user.attachments);
        let cleared = edited_user(&entry, user.uuid.as_deref().unwrap(), " \n")?.unwrap();
        assert!(cleared.content.is_empty());
        assert_eq!(cleared.attachments, user.attachments);
        entry.messages[0]
            .tool_calls
            .push(json!({"name":"unsupported"}));
        assert!(!text_history(&entry));
        Ok(())
    }
    #[test]
    fn workspace_conversation_payload_has_its_own_scope_and_no_terminal_host() -> Result<()> {
        let workspace =
            serde_json::to_value(scoped_blank(Scope::Workspace, Some("terminal-host".into())))?;
        assert_eq!(workspace["scope"], "workspace");
        assert!(workspace["host_uuid"].is_null());
        let terminal =
            serde_json::to_value(scoped_blank(Scope::Terminal, Some("terminal-host".into())))?;
        assert_eq!(terminal["scope"], "terminal");
        assert_eq!(terminal["host_uuid"], "terminal-host");
        Ok(())
    }
    #[test]
    fn editing_an_earlier_user_retains_context_and_replaces_only_its_tail() -> Result<()> {
        let mut entry = blank(None);
        let first = message("user", "first".into(), "first context".into(), 0);
        append_turn(&mut entry, first.clone(), Turn::Append)?;
        entry.messages.last_mut().unwrap().content = "first reply".into();
        let mut second = message("user", "second".into(), "old terminal context".into(), 2);
        append_turn(&mut entry, second.clone(), Turn::Append)?;
        entry.messages.last_mut().unwrap().content = "second reply".into();
        append_turn(
            &mut entry,
            message("user", "third".into(), "other context".into(), 4),
            Turn::Append,
        )?;
        let original_created_at = second.created_at;
        second = edited_user(&entry, second.uuid.as_deref().unwrap(), " edited second\n")?.unwrap();
        let wire = append_turn(&mut entry, second.clone(), Turn::Edit)?;
        assert_eq!(wire.len(), 3);
        assert_eq!(wire[0], first);
        assert_eq!(wire[2].uuid, second.uuid);
        assert_eq!(wire[2].context, "old terminal context");
        assert_eq!(wire[2].content, "edited second");
        assert_eq!(wire[2].created_at, original_created_at);
        assert!(wire[2].updated_at.is_some());
        assert_eq!(entry.messages.len(), 4);
        assert_eq!(entry.title, "first");
        let mut edited_first = first;
        edited_first.content = "new title".into();
        append_turn(&mut entry, edited_first, Turn::Edit)?;
        assert_eq!(entry.title, "new title");
        assert_eq!(entry.messages.len(), 2);
        Ok(())
    }
    #[test]
    fn unchanged_edit_preserves_the_existing_reply_and_command_timeline() -> Result<()> {
        let mut entry = blank(None);
        let user = message("user", "hello".into(), "original context".into(), 0);
        append_turn(&mut entry, user.clone(), Turn::Append)?;
        entry.messages.last_mut().unwrap().content = "existing reply".into();
        entry.command_blocks.push(serde_json::from_value(json!({
            "tool_call_id": "command", "command": "printf hello", "explanation": "",
            "status": "succeeded", "sequence": 2, "output": "hello", "exit_code": 0
        }))?);
        let before = entry.clone();
        let uuid = user.uuid.as_deref().unwrap();
        assert!(edited_user(&entry, uuid, " \nhello\t")?.is_none());
        assert!(edited_user(&entry, uuid, " \n").is_err());
        assert!(edited_user(&entry, "missing", "changed").is_err());
        assert_eq!(entry, before);
        Ok(())
    }
    #[test]
    fn an_unknown_edit_target_does_not_change_the_timeline() -> Result<()> {
        let mut entry = blank(None);
        append_turn(
            &mut entry,
            message("user", "original".into(), String::new(), 0),
            Turn::Append,
        )?;
        let before = entry.clone();
        assert!(
            append_turn(
                &mut entry,
                message("user", "unknown".into(), String::new(), 9),
                Turn::Edit
            )
            .is_err()
        );
        assert_eq!(entry, before);
        Ok(())
    }
    #[test]
    fn full_record_fingerprints_include_tools_attachments_and_context() -> Result<()> {
        let mut entry = blank(Some("host".into()));
        entry
            .messages
            .push(message("user", "hello".into(), "context".into(), 0));
        let original = stamp(&entry)?;
        entry.messages[0]
            .attachments
            .push(json!({"unknownFutureField":true}));
        assert_ne!(original, stamp(&entry)?);
        let encoded = serde_json::to_vec(&entry)?;
        assert_eq!(
            entry,
            serde_json::from_slice::<AiConversationEntry>(&encoded)?
        );
        assert_eq!(entry.messages[0].context, "context");
        Ok(())
    }
}
