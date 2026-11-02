//! Lightweight directory creation is a pane operation, not a transfer-queue job.
use super::*;
use crate::{
    file_operation::Operation,
    sftp_client::{ProgressFeed, Sudo},
};
use anyhow::{Result, ensure};

impl FilePane {
    pub(super) fn create_remote_folder(
        &mut self,
        options: Arc<SshOptions>,
        base: String,
        sudo: Option<Arc<Sudo>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = format!("{}/New Folder", base.trim_end_matches('/'));
        let target = if self.entries.iter().any(|entry| entry.path == name) {
            actions::alternate(&name, true, |path| {
                self.entries.iter().any(|entry| entry.path == path)
            })
        } else {
            name
        };
        let (request, cancel) = match Core::discover().and_then(Request::new) {
            Ok(request) => request,
            Err(error) => {
                self.notice = Some(Message::error(error.to_string()));
                cx.notify();
                return;
            }
        };
        let token = self.endpoint_token;
        self.notice = None;
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            // Dropping the pane operation cancels its native request. The shared
            // request registry also cancels and joins it during application exit.
            let _cancel = cancel;
            let result: Result<String> = cx
                .background_spawn(async move {
                    let operation = Operation::Mkdir {
                        target_path: target.clone(),
                    };
                    let result = request.execute_authorized(
                        &options,
                        &operation,
                        &ProgressFeed::with_notifier(None),
                        sudo.as_deref(),
                    )?;
                    ensure!(
                        result.ok,
                        "{}",
                        result
                            .error
                            .unwrap_or_else(|| "Could not create the folder.".into())
                    );
                    Ok(target)
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.endpoint_token != token {
                    return;
                }
                this.action_work = None;
                match result {
                    Ok(path) if this.stage == Stage::Files && this.history.path == base => {
                        this.pending_selection = Some(Selection::single(path));
                        this.refresh(window, cx);
                    }
                    Ok(_) => {}
                    Err(error) => this.notice = Some(Message::error(error.to_string())),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
