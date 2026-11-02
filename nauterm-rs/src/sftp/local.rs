//! Local endpoints use actual paths, never a shell or a lossy displayed name.
use super::*;

pub(super) struct LocalSnapshot {
    history: History,
    selection: Selection,
    sort: Sort,
    ascending: bool,
    hidden: bool,
    filter: String,
    scroll: UniformListScrollHandle,
}
impl FilePane {
    pub(super) fn remember_local(&mut self, cx: &App) {
        if self.remote.is_none() && !self.history.path.is_empty() {
            self.saved_local = Some(LocalSnapshot {
                history: self.history.clone(),
                selection: self.selection.clone(),
                sort: self.sort,
                ascending: self.ascending,
                hidden: self.hidden,
                filter: self.filter.read(cx).value().to_string(),
                scroll: self.scroll.clone(),
            });
        }
    }
    pub(super) fn restore_local(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (String, Navigation) {
        if let Some(state) = self.saved_local.take() {
            self.history = state.history;
            self.pending_selection = Some(state.selection);
            self.sort = state.sort;
            self.ascending = state.ascending;
            self.hidden = state.hidden;
            self.scroll = state.scroll;
            self.filter
                .update(cx, |filter, cx| filter.set_value(state.filter, window, cx));
            (self.history.path.clone(), Navigation::Refresh)
        } else {
            self.history = History::default();
            self.pending_selection = None;
            (
                self.initial_directory.to_string_lossy().into_owned(),
                Navigation::Visit,
            )
        }
    }
    pub(super) fn local_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let token = self.endpoint_token;
        let directory = self.history.path.clone();
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn({
                    let directory = directory.clone();
                    async move {
                        crate::sftp_tasks::run_local(|| {
                            file_model::create_local_folder(std::path::Path::new(&directory))
                        })
                    }
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.action_work = None;
                if this.endpoint_token != token {
                    return;
                }
                match result {
                    Ok(path) if this.stage == Stage::Files && this.history.path == directory => {
                        let path = path.to_string_lossy().into_owned();
                        this.pending_selection = Some(Selection::single(path));
                        this.refresh(window, cx);
                    }
                    Ok(_) => {}
                    Err(error) => this.notice = Some(Message::error(error.to_string())),
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn open_entry(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        if entry.directory {
            let target = if let Some(path) = &entry.native_path {
                let Some(path) = path.to_str() else {
                    self.notice = Some(Message::error(
                        "This directory's name cannot be represented in the path editor.",
                    ));
                    cx.notify();
                    return;
                };
                path.to_owned()
            } else {
                entry.path
            };
            self.navigate(target, Navigation::Visit, window, cx);
        } else if self.remote.is_some() {
            self.open_remote_file(self.endpoint_token, entry.path, None, false, window, cx);
        } else if self.remote.is_none()
            && let Some(path) = entry.native_path
        {
            self.open_local_default(path, window, cx);
        }
    }
}
