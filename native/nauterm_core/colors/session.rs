impl SessionManager {
    pub fn rs_set_palette(
        &self,
        id: SessionId,
        colors: crate::terminal::TerminalDefaultColors,
    ) -> bool {
        self.sessions
            .get(&id)
            .and_then(|actor| actor.call(move |session| session.engine.rs_set_palette(colors)))
            .unwrap_or(false)
    }
}
