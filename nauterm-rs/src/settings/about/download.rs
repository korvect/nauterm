use super::*;
use crate::{
    about::download::{self as transfer, Event, Plan, Verified},
    cancel::Cancel,
};

pub(super) struct Download {
    id: String,
    pub ready: Option<Arc<Verified>>,
    running: bool,
    phase: &'static str,
    received: u64,
    total: u64,
    cancel: Cancel,
    _task: Task<()>,
}
impl Drop for Download {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
impl Panel {
    pub(super) fn download_running(&self) -> bool {
        self.download
            .as_ref()
            .is_some_and(|download| download.running)
    }
    pub(super) fn download_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.checking || self.download_running() || self.opening_package {
            return;
        }
        let Some(release) = &self.release else {
            return;
        };
        let plan = match Plan::new(release) {
            Ok(plan) => plan,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let flow = match transfer::start(plan, self.update_directory.clone()) {
            Ok(flow) => flow,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let id = uuid::Uuid::new_v4().to_string();
        let generation = id.clone();
        let cancel = flow.cancel.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = flow.events.recv().await {
                let terminal = matches!(event, Event::Ready(_) | Event::Failed(_));
                let live = this
                    .update_in(cx, |this, _, cx| {
                        let Some(download) = this
                            .download
                            .as_mut()
                            .filter(|download| download.id == generation)
                        else {
                            return false;
                        };
                        match event {
                            Event::Checksums => download.phase = "Checking release checksums…",
                            Event::Progress { received, total } => {
                                download.received = received;
                                download.total = total;
                                download.phase = "Downloading update…";
                            }
                            Event::Verifying => download.phase = "Verifying SHA-256…",
                            Event::Ready(ready) => {
                                download.running = false;
                                download.ready = Some(Arc::new(ready));
                                this.download_notice =
                                    Some("Download complete. SHA-256 verification passed.".into());
                            }
                            Event::Failed(error) => {
                                download.running = false;
                                this.error = Some(error);
                            }
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if terminal || !live {
                    break;
                }
            }
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(download) = this
                    .download
                    .as_mut()
                    .filter(|download| download.id == generation && download.running)
                {
                    download.running = false;
                    this.error =
                        Some("The update download stopped before completion. Try again.".into());
                    cx.notify();
                }
            });
        });
        self.download = Some(Download {
            id,
            ready: None,
            running: true,
            phase: "Preparing download…",
            received: 0,
            total: 0,
            cancel,
            _task: task,
        });
        self.error = None;
        self.download_notice = None;
        cx.notify();
    }
    fn open_verified_package(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.download_running() || self.opening_package {
            return;
        }
        let Some(ready) = self
            .download
            .as_ref()
            .and_then(|download| download.ready.clone())
        else {
            return;
        };
        self.opening_package = true;
        self.error = None;
        self.open_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || ready.retain_for_handoff()).await;
            let launched = match result {
                Ok(path) => crate::external_application::open_editor(&path, None),
                Err(error) => Err(error),
            };
            let outcome = match launched {
                Ok(receiver) => receiver.recv().await.unwrap_or_else(|_| Err(anyhow::anyhow!("The system did not confirm opening the update package."))),
                Err(error) => Err(error),
            };
            let _ = this.update_in(cx, |this, _, cx| {
                this.opening_package = false;
                this.open_task = None;
                match outcome {
                    Ok(()) => { this.restart_available = true; this.download_notice = Some("The verified package is open. Complete installation before restarting Nauterm RS.".into()); },
                    Err(error) => this.error = Some(format!("Could not open the verified update package: {error}")),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn download_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = Palette::settings(self.dark);
        let Some(download) = self.download.as_ref() else {
            return self.download_notice.clone().map(|notice| {
                div()
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(crate::localization::message(cx, &notice))
                    .into_any_element()
            });
        };
        let mut body = div().flex().flex_col().gap(px(10.));
        if !download.running && download.ready.is_none() && self.download_notice.is_none() {
            return None;
        }
        if download.running {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Spinner::new())
                    .child(crate::localization::message(cx, download.phase)),
            );
            if download.total > 0 {
                let progress = (download.received as f32 / download.total as f32).clamp(0., 1.);
                body = body
                    .child(
                        div().w_full().h(px(4.)).rounded_full().bg(p.border).child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(p.accent)
                                .w(relative(progress)),
                        ),
                    )
                    .child(div().text_size(px(11.)).text_color(p.muted).child(format!(
                        "{:.1} / {:.1} MB",
                        download.received as f64 / 1048576.,
                        download.total as f64 / 1048576.
                    )));
            }
            body = body.child(
                div().flex().justify_end().child(
                    Button::new("cancel-update-download")
                        .outline()
                        .label("Cancel download")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.download = None;
                            this.error = None;
                            this.download_notice = Some("Download cancelled.".into());
                            cx.notify();
                        })),
                ),
            );
        }
        if let Some(ready) = &download.ready {
            body = body
                .child(div().text_size(px(12.)).text_color(p.muted).child(format!(
                    "Nauterm RS {} · {}",
                    ready.version,
                    ready.filename()
                )))
                .child(
                    div().flex().justify_end().child(
                        Button::new("open-verified-update")
                            .outline()
                            .icon(IconName::PackageOpen)
                            .label(if self.opening_package {
                                "Opening…"
                            } else {
                                "Open verified package"
                            })
                            .disabled(self.opening_package)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_verified_package(window, cx)
                            })),
                    ),
                );
            body = body.child(
                div().flex().justify_end().child(
                    Button::new("discard-verified-update")
                        .ghost()
                        .label(if ready.retained() {
                            "Dismiss"
                        } else {
                            "Discard download"
                        })
                        .disabled(self.opening_package)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.download = None;
                            this.download_notice = None;
                            this.error = None;
                            cx.notify();
                        })),
                ),
            );
        }
        if let Some(notice) = &self.download_notice {
            body = body.child(
                div()
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(crate::localization::message(cx, notice)),
            );
        }
        if self.restart_available {
            body = body.child(
                div().flex().justify_end().child(
                    Button::new("restart-nauterm-after-update")
                        .outline()
                        .icon(IconName::RefreshCw)
                        .label("Restart Nauterm RS")
                        .disabled(self.opening_package || self.download_running())
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(super::Event::Restart))),
                ),
            );
        }
        Some(body.into_any_element())
    }
}
