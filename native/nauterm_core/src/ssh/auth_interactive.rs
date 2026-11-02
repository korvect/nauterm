use super::*;
use tokio::sync::oneshot;

#[derive(Clone, Debug, serde::Serialize)]
pub struct KeyboardInteractivePrompt {
    pub prompt: String,
    pub echo: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct KeyboardInteractiveChallenge {
    pub id: u64,
    pub name: String,
    pub instructions: String,
    pub prompts: Vec<KeyboardInteractivePrompt>,
}

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthenticationCredentials {
    pub method: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub certificate: Option<String>,
    pub passphrase: Option<String>,
}

impl Drop for AuthenticationCredentials {
    fn drop(&mut self) {
        self.password.zeroize();
        self.private_key.zeroize();
        self.passphrase.zeroize();
    }
}

struct PendingCredentials {
    id: u64,
    methods: Vec<String>,
    sender: oneshot::Sender<AuthenticationCredentials>,
}

struct PendingResponse {
    id: u64,
    count: usize,
    sender: oneshot::Sender<Vec<String>>,
}

#[derive(Clone, Default)]
pub(crate) struct AuthenticationInteraction {
    pending: Arc<Mutex<Option<PendingResponse>>>,
    credentials: Arc<Mutex<Option<PendingCredentials>>>,
}

static NEXT_CHALLENGE_ID: AtomicU64 = AtomicU64::new(1);

impl AuthenticationInteraction {
    pub fn respond(&self, id: u64, responses: Option<Vec<String>>) -> bool {
        if responses.is_none() {
            let mut pending = self.credentials.lock().unwrap();
            if pending.as_ref().is_some_and(|request| request.id == id) {
                pending.take();
                return true;
            }
        }
        let mut pending = self.pending.lock().unwrap();
        if !pending.as_ref().is_some_and(|request| {
            request.id == id
                && responses
                    .as_ref()
                    .is_none_or(|responses| request.count == responses.len())
        }) {
            if let Some(mut responses) = responses {
                responses.zeroize();
            }
            return false;
        }
        let request = pending.take().unwrap();
        let Some(responses) = responses else {
            return true;
        };
        match request.sender.send(responses) {
            Ok(()) => true,
            Err(mut responses) => {
                responses.zeroize();
                false
            }
        }
    }

    pub fn respond_credentials(&self, id: u64, credentials: AuthenticationCredentials) -> bool {
        let mut pending = self.credentials.lock().unwrap();
        if !pending.as_ref().is_some_and(|request| {
            request.id == id
                && request.methods.contains(&credentials.method)
                && match credentials.method.as_str() {
                    "password" => credentials.password.is_some(),
                    "publickey" => credentials.private_key.is_some(),
                    _ => false,
                }
        }) {
            return false;
        }
        pending.take().unwrap().sender.send(credentials).is_ok()
    }

    fn begin_credentials_request(
        &self,
        methods: &russh::MethodSet,
        intervention: Option<SessionEvent>,
        events: &Arc<Mutex<Vec<SessionEvent>>>,
        wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    ) -> oneshot::Receiver<AuthenticationCredentials> {
        let id = NEXT_CHALLENGE_ID.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        let methods: Vec<String> = methods
            .iter()
            .map(|method| <&str>::from(method).to_owned())
            .collect();
        *self.credentials.lock().unwrap() = Some(PendingCredentials {
            id,
            methods: methods.clone(),
            sender,
        });
        let mut event = intervention.unwrap_or_else(|| {
            SessionEvent::new(
                "auth_credentials_required",
                "SSH authentication requires credentials.",
            )
        });
        event.authentication_methods = Some(methods);
        event.authentication_request_id = Some(id);
        push_event(events, wakeup, event);
        receiver
    }

    pub(super) async fn request_credentials<H: client::Handler>(
        &self,
        handle: &mut client::Handle<H>,
        methods: &russh::MethodSet,
        intervention: Option<SessionEvent>,
        events: &Arc<Mutex<Vec<SessionEvent>>>,
        wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    ) -> Result<AuthenticationCredentials, String> {
        let receiver = self.begin_credentials_request(methods, intervention, events, wakeup);
        let _pending = PendingCredentialRequest(self);
        tokio::select! {
            response = receiver => response.map_err(|_| "SSH authentication cancelled.".to_owned()),
            _ = handle => Err("SSH connection closed during authentication.".to_owned()),
        }
    }

    pub(super) async fn request_key_unlock(
        &self,
        intervention: SessionEvent,
        events: &Arc<Mutex<Vec<SessionEvent>>>,
        wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    ) -> Result<AuthenticationCredentials, String> {
        let methods = russh::MethodSet::from(&[russh::MethodKind::PublicKey][..]);
        let receiver = self.begin_credentials_request(&methods, Some(intervention), events, wakeup);
        let _pending = PendingCredentialRequest(self);
        receiver
            .await
            .map_err(|_| "SSH authentication cancelled.".to_owned())
    }

    pub(super) async fn request<H: client::Handler>(
        &self,
        handle: &mut client::Handle<H>,
        name: String,
        instructions: String,
        prompts: Vec<client::Prompt>,
        events: &Arc<Mutex<Vec<SessionEvent>>>,
        wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    ) -> Result<Vec<String>, String> {
        let id = NEXT_CHALLENGE_ID.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        *self.pending.lock().unwrap() = Some(PendingResponse {
            id,
            count: prompts.len(),
            sender,
        });
        // Clear the sender even when shutdown cancels this future.
        let _pending = PendingChallenge(self);
        let mut event = SessionEvent::new(
            "auth_interactive_request",
            "SSH keyboard-interactive authentication requires a response.",
        )
        .with_method("keyboard-interactive");
        event.challenge = Some(KeyboardInteractiveChallenge {
            id,
            name,
            instructions,
            prompts: prompts
                .into_iter()
                .map(|prompt| KeyboardInteractivePrompt {
                    prompt: prompt.prompt,
                    echo: prompt.echo,
                })
                .collect(),
        });
        push_event(events, wakeup, event);
        tokio::select! {
            response = receiver => response.map_err(|_| "SSH authentication cancelled.".to_owned()),
            _ = handle => Err("SSH connection closed during keyboard-interactive authentication.".to_owned()),
        }
    }
}

struct PendingChallenge<'a>(&'a AuthenticationInteraction);

impl Drop for PendingChallenge<'_> {
    fn drop(&mut self) {
        self.0.pending.lock().unwrap().take();
    }
}

struct PendingCredentialRequest<'a>(&'a AuthenticationInteraction);
impl Drop for PendingCredentialRequest<'_> {
    fn drop(&mut self) {
        self.0.credentials.lock().unwrap().take();
    }
}
