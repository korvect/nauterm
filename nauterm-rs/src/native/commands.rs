//! Session-bound raw output for one approved command, with bounded ownership.
use super::*;
use crate::ai::{client::Cancel, terminal_runner::Integration};
use zeroize::Zeroizing;
const OUTPUT_BUDGET: usize = 2 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct CommandOwner {
    id: u64,
    lease: SessionLease,
    _core: Arc<Core>,
    pub label: String,
}
impl CommandOwner {
    pub fn validate(&self) -> Result<()> {
        self.lease.validate()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.id == other.id
            && self.lease.generation == other.lease.generation
            && Arc::ptr_eq(&self.lease.counter, &other.lease.counter)
    }
    fn access<T>(&self, work: impl FnOnce(&mut SessionManager) -> Result<T>) -> Result<T> {
        sessions(|manager| {
            self.validate()?;
            work(manager)
        })?
    }
    fn prompt(manager: &mut SessionManager, id: u64) -> Result<()> {
        manager.poll(id);
        let frame = manager
            .snapshot(id)
            .context("The terminal session is closed.")?;
        ensure!(
            !manager.is_exited(id) && frame.input_echo_enabled && !frame.alternate_screen,
            "The terminal is not available for a shell command."
        );
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.access(|manager| Self::prompt(manager, self.id))
            .is_ok()
    }
}
pub(super) struct Tap {
    sender: async_channel::Sender<Output>,
    bytes: Arc<AtomicUsize>,
    overflow: Arc<AtomicBool>,
    pub interrupted: Cancel,
}
pub(crate) struct Output {
    pub bytes: Zeroizing<Vec<u8>>,
    budget: Arc<AtomicUsize>,
    length: usize,
}
impl Drop for Output {
    fn drop(&mut self) {
        self.budget.fetch_sub(self.length, Ordering::AcqRel);
    }
}
impl Tap {
    pub fn publish(&self, bytes: &[u8]) {
        if bytes.is_empty() || self.sender.is_closed() {
            return;
        }
        if self
            .bytes
            .try_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(bytes.len())
                    .filter(|total| *total <= OUTPUT_BUDGET)
            })
            .is_err()
        {
            self.overflow.store(true, Ordering::Release);
            self.sender.close();
            return;
        }
        let output = Output {
            bytes: Zeroizing::new(bytes.to_vec()),
            budget: self.bytes.clone(),
            length: bytes.len(),
        };
        if self.sender.try_send(output).is_err() {
            self.overflow.store(true, Ordering::Release);
            self.sender.close();
        }
    }
}
pub(crate) struct CommandTarget {
    pub owner: CommandOwner,
    pub shell: Option<String>,
    pub interrupted: Cancel,
    receiver: async_channel::Receiver<Output>,
    overflow: Arc<AtomicBool>,
    integration: Arc<Mutex<Option<Integration>>>,
    wake: async_channel::Sender<()>,
}
impl Drop for CommandTarget {
    fn drop(&mut self) {
        self.receiver.close();
        while self.receiver.try_recv().is_ok() {}
        self.cancel_suppression();
    }
}
impl CommandTarget {
    pub fn integration(&self) -> Option<Integration> {
        self.integration.lock().unwrap().clone()
    }
    pub fn remember(&self, integration: Integration) -> Result<()> {
        self.owner.access(|_| {
            *self.integration.lock().unwrap() = Some(integration);
            Ok(())
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.owner.validate()?;
        ensure!(
            !self.overflow.load(Ordering::Acquire),
            "Terminal command output exceeded its capture queue; the result cannot be tracked."
        );
        Ok(())
    }
    pub async fn next(&self) -> Result<Output> {
        self.validate()?;
        let output = self
            .receiver
            .recv()
            .await
            .context("The terminal session closed before the command finished.")?;
        self.validate()?;
        Ok(output)
    }
    pub fn send(&self, bytes: &[u8], cancel: &Cancel) -> Result<()> {
        self.owner.access(|manager| {
            CommandOwner::prompt(manager, self.owner.id)?;
            ensure!(
                !cancel.cancelled() && !self.interrupted.cancelled(),
                "The command was stopped before submission."
            );
            ensure!(
                manager.send_input_bytes(self.owner.id, bytes),
                "The terminal rejected command input."
            );
            Ok(())
        })?;
        let _ = self.wake.try_send(());
        Ok(())
    }
    pub fn submit(&self, command: &str, cancel: &Cancel) -> Result<()> {
        self.owner.access(|manager| {
            CommandOwner::prompt(manager, self.owner.id)?;
            let paste = Zeroizing::new(
                manager
                    .encode_paste(self.owner.id, command.to_owned())
                    .map_err(anyhow::Error::msg)?,
            );
            let mut input = Zeroizing::new(paste.as_bytes().to_vec());
            if !input.ends_with(b"\r") {
                input.push(b'\r');
            }
            ensure!(
                !cancel.cancelled() && !self.interrupted.cancelled(),
                "The command was stopped before submission."
            );
            ensure!(
                manager.send_input_bytes(self.owner.id, &input),
                "The terminal rejected command input."
            );
            Ok(())
        })?;
        let _ = self.wake.try_send(());
        Ok(())
    }
    pub fn interrupt(&self) -> Result<()> {
        self.owner.access(|manager| {
            ensure!(
                manager.send_input_bytes(self.owner.id, b"\x03"),
                "The terminal rejected interruption."
            );
            Ok(())
        })?;
        let _ = self.wake.try_send(());
        Ok(())
    }
    pub fn suppress(&self, marker: &[u8]) -> Result<()> {
        self.owner.access(|manager| {
            ensure!(
                manager.suppress_output_until(self.owner.id, marker),
                "Could not prepare terminal command tracking."
            );
            Ok(())
        })
    }
    pub fn cancel_suppression(&self) {
        let _ = self.owner.access(|manager| {
            manager.cancel_output_suppression(self.owner.id);
            Ok(())
        });
    }
}
impl Session {
    pub(crate) fn command_owner(&self) -> CommandOwner {
        let label = match &self.history_source {
            HistorySource::Ssh { host, username, .. } => format!("{username}@{host}"),
            HistorySource::Local(_) => "Local Terminal".into(),
            _ => "Terminal".into(),
        };
        CommandOwner {
            id: self.id,
            lease: self.lease(),
            _core: self.core.clone(),
            label,
        }
    }
    pub(crate) fn capture_command(&mut self, expected: &CommandOwner) -> Result<CommandTarget> {
        ensure!(!self.is_replay(), "Recording replay is read-only.");
        ensure!(
            expected.same(&self.command_owner()),
            "The terminal connection changed. Review a new command before running it."
        );
        ensure!(
            self.pending_recording.is_none(),
            "The terminal is still initializing. Try again shortly."
        );
        ensure!(
            self.command_output
                .as_ref()
                .is_none_or(|tap| tap.sender.is_closed()),
            "Another AI command is already using this terminal."
        );
        let (sender, receiver) = async_channel::bounded(64);
        let bytes = Arc::new(AtomicUsize::new(0));
        let overflow = Arc::new(AtomicBool::new(false));
        let interrupted = Cancel::new();
        self.command_output = Some(Tap {
            sender,
            bytes,
            overflow: overflow.clone(),
            interrupted: interrupted.clone(),
        });
        Ok(CommandTarget {
            owner: expected.clone(),
            shell: self
                .history_capture
                .reported_shell
                .clone()
                .or_else(|| self.local_shell()),
            interrupted,
            receiver,
            overflow,
            integration: self.command_integration.clone(),
            wake: self.sender.clone(),
        })
    }
    pub(super) fn close_command_output(&mut self) {
        if let Some(tap) = self.command_output.take() {
            tap.sender.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_capture_releases_its_budget_and_closes_on_lost_chunks() {
        let (sender, receiver) = async_channel::bounded(1);
        let bytes = Arc::new(AtomicUsize::new(0));
        let overflow = Arc::new(AtomicBool::new(false));
        let tap = Tap {
            sender,
            bytes: bytes.clone(),
            overflow: overflow.clone(),
            interrupted: Cancel::new(),
        };
        tap.publish(b"one");
        assert_eq!(bytes.load(Ordering::Acquire), 3);
        drop(receiver.try_recv().unwrap());
        assert_eq!(bytes.load(Ordering::Acquire), 0);
        tap.publish(b"one");
        tap.publish(b"two");
        assert!(overflow.load(Ordering::Acquire));
        assert!(tap.sender.is_closed());
        drop(receiver.try_recv().unwrap());
        assert_eq!(bytes.load(Ordering::Acquire), 0);
    }
    #[test]
    fn changed_session_generation_invalidates_existing_request_leases() {
        let counter = Arc::new(AtomicUsize::new(7));
        let lease = SessionLease {
            counter: counter.clone(),
            generation: 7,
        };
        assert!(lease.validate().is_ok());
        counter.fetch_add(1, Ordering::AcqRel);
        assert!(lease.validate().is_err());
    }
}
