use super::*;

impl Nauterm {
    pub(super) fn refresh_theme_catalog(&mut self, cx: &mut Context<Self>) {
        if self.theme_refreshing {
            self.theme_refresh_pending = true;
            return;
        }
        self.theme_refreshing = true;
        let paths = self.store.paths.clone();
        cx.spawn(async move |view, cx| {
            let loaded = smol::unblock(move || crate::theme_catalog::load(&paths)).await;
            let _ = view.update(cx, |this, cx| {
                this.theme_refreshing = false;
                if !loaded.warnings.is_empty() {
                    this.notice = Some(format!(
                        "Some terminal themes could not be loaded: {}",
                        loaded
                            .warnings
                            .iter()
                            .take(3)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("; ")
                    ));
                }
                if crate::theme_catalog::publish(loaded) {
                    if this.editor.is_none()
                        && this.resources_ready()
                        && let Err(error) = this.store.refresh_resources(&mut this.data)
                    {
                        this.notice = Some(error.to_string());
                    }
                    for tab in this.terminals.iter() {
                        for session in tab.tree.sessions() {
                            let appearance = session.appearance(&this.data);
                            let result = match &session.content {
                                TabContent::Terminal(terminal) => terminal
                                    .update(cx, |terminal, cx| {
                                        terminal.set_appearance(&appearance, cx)
                                    }),
                                TabContent::Connection(connection) => {
                                    connection.update(cx, |connection, cx| {
                                        let result =
                                            connection.set_terminal_palette(appearance.palette());
                                        cx.notify();
                                        result
                                    })
                                }
                                TabContent::Preparing(_) => Ok(()),
                            };
                            if let Err(error) = result {
                                this.notice =
                                    Some(format!("Could not refresh terminal theme: {error}"));
                            }
                            for files in [&session.files, &session.tool_files].into_iter().flatten()
                            {
                                files.update(cx, |files, cx| {
                                    files.set_data(this.data.clone(), cx);
                                    files.refresh_theme(cx);
                                });
                            }
                        }
                        tab.tools
                            .refresh_theme(tab.active().appearance(&this.data).palette(), cx);
                    }
                    if let Some(files) = &this.sftp {
                        files.update(cx, |files, cx| {
                            files.set_data(this.data.clone(), cx);
                            files.refresh_theme(cx);
                        });
                    }
                    if let Some(keychain) = &this.keychain {
                        keychain.update(cx, |keychain, cx| {
                            keychain.set_resources(this.data.clone(), cx)
                        });
                    }
                    if let Some(settings) = &this.settings_window {
                        settings
                            ._view
                            .update(cx, |settings, cx| settings.refresh_themes(cx));
                    }
                }
                if this.theme_refresh_pending {
                    this.theme_refresh_pending = false;
                    this.refresh_theme_catalog(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}
