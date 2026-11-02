//! Render the normal workspace immediately. Only encrypted resource regions
//! wait for Keychain; no loading root replaces the public chrome/navigation.
use super::*;
use std::time::Duration;

const PRESENTATION_DELAY: Duration = Duration::from_millis(120);

impl Nauterm {
    pub(super) fn resources_ready(&self) -> bool {
        !self.storage_loading && self.storage_error.is_none()
    }

    pub(super) fn load_initial_resources(
        &mut self,
        previous: Option<Checkpoint>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.exiting.is_some() {
                    return;
                }
                #[cfg(unix)]
                if std::env::var_os("NAUTERM_RS_DATA_DIR").is_none()
                    && let Some(home) =
                        directories::UserDirs::new().map(|dirs| dirs.home_dir().to_owned())
                {
                    let data = this.store.paths.data_directory.clone();
                    cx.spawn(async move |owner, cx| {
                        let result = cx
                            .background_executor()
                            .spawn(async move {
                                nauterm_core::shell_integration::synchronize(&data, &home)
                            })
                            .await;
                        if let Err(error) = result {
                            let _ = owner.update(cx, |this, cx| {
                                this.notice = Some(format!(
                                    "Unable to configure nested shell integration: {error}"
                                ));
                                cx.notify();
                            });
                        }
                    })
                    .detach();
                }
                this.startup_task = Some(cx.spawn_in(window, async move |owner, cx| {
                    cx.background_executor().timer(PRESENTATION_DELAY).await;
                    let Ok(Some(store)) = owner.update(cx, |this, _| {
                        this.exiting.is_none().then(|| this.store.clone())
                    }) else {
                        return;
                    };
                    let loaded = store.load().await;
                    let _ = owner.update_in(cx, |this, window, cx| {
                        if this.exiting.is_some() {
                            return;
                        }
                        this.storage_loading = false;
                        match loaded {
                            Ok(mut data) => {
                                let selected = this.data.selected_workspace;
                                if data
                                    .workspaces
                                    .iter()
                                    .any(|workspace| workspace.id == selected)
                                {
                                    data.selected_workspace = selected;
                                }
                                data.dark = data.app_theme().dark(window.appearance());
                                crate::tags::materialize(&mut data);
                                crate::native::configure_terminal_defaults(
                                    &data.terminal,
                                    &data.terminal_preferences,
                                );
                                cx.set_global(data.host_icons);
                                this.data = data;
                                this.storage_error = None;
                                this.sync_app_theme(window, cx);
                                if this.data.hosts.iter().any(|host| host.credential.is_some()) {
                                    this.ensure_keychain(cx);
                                }
                                if this.data.hosts.iter().any(|host| host.proxy_id.is_some())
                                    || this
                                        .data
                                        .groups
                                        .iter()
                                        .any(|group| group.proxy_id.is_some())
                                {
                                    this.ensure_proxies(cx);
                                }
                                if let Some(view) = &this.keychain {
                                    view.update(cx, |view, cx| {
                                        view.set_resources(this.data.clone(), cx)
                                    });
                                }
                                this.initialize_recovery(previous, window, cx);
                                if this.page == Page::Snippets {
                                    this.load_snippets(cx);
                                }
                                this.schedule_secure_preload(window, cx);
                            }
                            Err(error) => this.storage_error = Some(error.to_string()),
                        }
                        cx.notify();
                    });
                }));
            });
        });
    }
}

impl Nauterm {
    pub(super) fn schedule_secure_preload(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.resources_ready() {
            return;
        }
        let owner = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.exiting.is_some() {
                    return;
                }
                this.secure_preload = Some(cx.spawn_in(window, async move |owner, cx| {
                    cx.background_executor().timer(PRESENTATION_DELAY).await;
                    let Ok(Some(service)) = owner.update(cx, |this, _| {
                        if this.exiting.is_some() {
                            return None;
                        }
                        Some(this.store.ai_providers())
                    }) else {
                        return;
                    };
                    // Optional settings may retry failures when opened, as in
                    // Flutter. Keep only summaries, never provider secrets.
                    if let Ok(service) = service {
                        let _ = service.provider_summaries().await;
                    }
                    let service = owner
                        .update(cx, |this, _| {
                            if this.exiting.is_some() {
                                return None;
                            }
                            this.store.sync_service().ok()
                        })
                        .ok()
                        .flatten();
                    if let Some(service) = service {
                        let _ = service.overview().await;
                    }
                    let _ = owner.update_in(cx, |this, window, cx| {
                        this.start_automatic_sync(window, cx);
                    });
                }));
            });
        });
    }
}
