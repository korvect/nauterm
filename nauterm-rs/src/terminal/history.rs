use super::*;
use crate::shell_history::{Entry, Refresh};

#[derive(Default)]
pub(super) struct History {
    tool_visible: bool,
    refresh: Refresh,
    entries: Vec<Entry>,
    live: Vec<Entry>,
}
impl Terminal {
    pub fn set_history_tool_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        let visible = visible && !self.session.is_replay();
        if self.history.tool_visible != visible {
            self.history.tool_visible = visible;
            if visible {
                self.refresh_history(cx);
            }
        }
    }
    pub fn history_entries(&self) -> Vec<Entry> {
        let mut seen = std::collections::HashSet::new();
        self.history
            .live
            .iter()
            .rev()
            .chain(self.history.entries.iter().rev())
            .filter(|entry| seen.insert(entry.command.as_str()))
            .cloned()
            .collect()
    }
    pub fn can_run_history(&self) -> bool {
        !self.session.is_replay()
            && self.phase() == Phase::Completing
            && !self.frame.alternate_screen
            && !self.composer_sensitive()
    }
    pub fn run_history(
        &mut self,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.can_run_history(),
            "Return to the shell before running a history command."
        );
        self.apply_tool_command(command, true, true, window, cx)
    }
    pub(super) fn refresh_history(&mut self, cx: &mut Context<Self>) {
        if !(self.composer_visible || self.history.tool_visible)
            || !self.session.supports_shell_history()
            || self.phase() != Phase::Completing
            || !self
                .history
                .refresh
                .begin(Instant::now(), self.session.history_interval())
        {
            return;
        }
        let request = self.session.history_request();
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { request.read() }).await;
            let _ = this.update(cx, |this, cx| {
                this.history.refresh.finish();
                if let Ok(entries) = result {
                    this.history.entries = entries;
                    this.publish_history(cx);
                }
                // Optional shell history failure keeps the last inventory. It
                // must not obscure terminal connection/output with an error.
            });
        })
        .detach();
    }
    pub(super) fn record_composer_command(&mut self, command: &str, cx: &mut Context<Self>) {
        let command = command.trim();
        if !crate::shell_history::live_command_allowed(command) || self.composer_sensitive() {
            return;
        }
        self.add_live_history(command.to_owned());
        self.publish_history(cx);
    }
    pub(super) fn take_shell_commands(&mut self, cx: &mut Context<Self>) {
        let commands = self.session.take_shell_commands();
        if commands.is_empty() {
            return;
        }
        for command in commands {
            self.note_reconnect_command(&command);
            self.add_live_history(command);
        }
        self.publish_history(cx);
    }
    fn add_live_history(&mut self, command: String) {
        self.history
            .live
            .push(Entry::observed(command, chrono::Utc::now()));
        if self.history.live.len() > crate::shell_history::LIMIT {
            self.history.live.remove(0);
        }
    }
    fn publish_history(&mut self, cx: &mut Context<Self>) {
        let history = self
            .history
            .live
            .iter()
            .rev()
            .map(|entry| entry.command.clone())
            .chain(crate::shell_history::newest_first(&self.history.entries))
            .collect();
        self.completion
            .update(cx, |completion, cx| completion.set_history(history, cx));
        cx.emit(TerminalMetadataChanged);
    }
}
