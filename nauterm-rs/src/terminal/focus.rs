//! DECSET 1004 follows effective terminal focus, not incidental redraws.
//! The transport write is separate from user input (selection/scroll/history).
use super::*;

#[derive(Default)]
pub(super) struct Reporting {
    pub visible: bool,
    focused: bool,
}
impl Reporting {
    fn observe(&mut self, focused: bool, enabled: bool) -> Option<&'static [u8]> {
        if std::mem::replace(&mut self.focused, focused) == focused || !enabled {
            return None;
        }
        Some(if focused { b"\x1b[I" } else { b"\x1b[O" })
    }
}

impl Terminal {
    pub(super) fn owns_keyboard(&self, window: &Window) -> bool {
        self.focus_reporting.visible && self.focus.is_focused(window) && window.is_window_active()
    }
    pub(crate) fn set_focus_scope(
        &mut self,
        visible: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self.focus_reporting.visible != visible {
            self.focus_reporting.visible = visible;
            if !visible {
                self.cancel_mouse(cx);
                self.selecting = None;
                self.selection_drag = None;
                self.scroll_drag = None;
                self.prompt_click = None;
                self.marked.clear();
            }
            self.report_focus(window);
            cx.notify();
        }
    }
    pub(super) fn report_focus(&mut self, window: &Window) {
        let focused = self.owns_keyboard(window);
        let enabled = self.frame.keyboard_mode & (1 << 3) != 0
            && !self.session.is_replay()
            && self.phase() == Phase::Completing;
        if let Some(bytes) = self.focus_reporting.observe(focused, enabled) {
            // No Terminal::send: protocol reports must not jump to the bottom,
            // clear a selection or enter the reconnect command draft.
            // A transport loss is reported by its normal native phase events.
            let _ = self.session.send(bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Reporting;

    #[test]
    fn overlapping_focus_and_activation_notifications_report_one_transition() {
        let mut reports = Reporting::default();
        assert_eq!(reports.observe(false, true), None);
        assert_eq!(reports.observe(true, true), Some(b"\x1b[I".as_slice()));
        assert_eq!(reports.observe(true, true), None);
        assert_eq!(reports.observe(false, true), Some(b"\x1b[O".as_slice()));
        assert_eq!(reports.observe(false, true), None);
        assert_eq!(reports.observe(true, true), Some(b"\x1b[I".as_slice()));
    }

    #[test]
    fn disabled_modes_replay_and_disconnection_do_not_replay_old_focus_changes() {
        let mut reports = Reporting::default();
        assert_eq!(reports.observe(true, false), None);
        // Enabling mode 1004 is not itself a focus change.
        assert_eq!(reports.observe(true, true), None);
        assert_eq!(reports.observe(false, false), None);
        assert_eq!(reports.observe(false, true), None);
        assert_eq!(reports.observe(true, true), Some(b"\x1b[I".as_slice()));
        assert_eq!(reports.observe(true, false), None);
        assert_eq!(reports.observe(false, false), None);
        assert_eq!(reports.observe(false, true), None);
    }
}
