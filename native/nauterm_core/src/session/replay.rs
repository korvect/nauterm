//! Passive VT sessions: no PTY, sockets, credentials or executable transport.
use super::*;

impl SessionManager {
    pub fn write_replay(&mut self, id: SessionId, bytes: Vec<u8>) -> bool {
        self.sessions
            .get(&id)
            .and_then(|actor| {
                actor.call(move |session| {
                    if !matches!(session.transport, SessionTransport::Disconnected) {
                        return false;
                    }
                    session.engine.write_bytes(&bytes);
                    use zeroize::Zeroize;
                    session.engine.drain_output_capture().zeroize();
                    session.flush_terminal_writes();
                    true
                })
            })
            .unwrap_or(false)
    }
    pub fn create_replay(
        &mut self,
        columns: usize,
        rows: usize,
        options: TerminalOptions,
    ) -> SessionId {
        self.insert_new(
            columns,
            rows,
            SessionTransport::Disconnected,
            options,
            vec![],
            None,
            false,
            false,
        )
    }
}
