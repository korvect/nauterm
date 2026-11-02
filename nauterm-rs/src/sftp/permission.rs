//! Permission dialogs follow a captured queue ticket, never the pane's new host.
use super::*;
use crate::transfer_queue::PermissionTicket;

impl FilePane {
    pub(super) fn queue_permission(&mut self, ticket: PermissionTicket, cx: &mut Context<Self>) {
        if self.task_owners.contains(&ticket.owner)
            && ticket.current()
            && !self.has_task_permission_dialog(&ticket)
            && !self
                .pending_permissions
                .iter()
                .any(|pending| pending.same(&ticket))
        {
            self.pending_permissions.push_back(ticket);
            cx.notify();
        }
    }
    pub(super) fn sync_permission_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.active
            || self.dialog.is_some()
            || self.action_work.is_some()
            || self.edits.has_dialog()
            || !window.is_window_active()
            || crate::components::overlay::is_modal_open(window, cx)
        {
            return;
        }
        while let Some(ticket) = self.pending_permissions.pop_front() {
            if !ticket.current() {
                continue;
            }
            match ticket.target() {
                Ok(target) => self.task_sudo_dialog(ticket, target, window, cx),
                Err(error) => {
                    self.notice = Some(Message::error(error.to_string()));
                    cx.notify();
                }
            }
            break;
        }
    }
}
