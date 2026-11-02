//! Shared cancellation for asynchronous network operations and UI owners.
#[derive(Clone)]
pub struct Cancel {
    sender: async_channel::Sender<()>,
    receiver: async_channel::Receiver<()>,
}
impl Cancel {
    pub fn new() -> Self {
        let (sender, receiver) = async_channel::bounded(1);
        Self { sender, receiver }
    }
    pub fn cancel(&self) {
        self.sender.close();
    }
    pub fn cancelled(&self) -> bool {
        self.sender.is_closed()
    }
    pub(crate) async fn wait(&self) {
        let _ = self.receiver.recv().await;
    }
}
impl Default for Cancel {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_closes_every_clone_and_wakes_its_waiters() {
        let owner = Cancel::new();
        let worker = owner.clone();
        assert!(!worker.cancelled());
        owner.cancel();
        assert!(worker.cancelled());
        smol::block_on(worker.wait());
        smol::block_on(owner.wait());
    }
}
