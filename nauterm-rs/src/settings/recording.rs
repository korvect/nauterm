//! Recording settings use the Settings skin and the existing typed Select.
use super::*;
use crate::recordings::Preferences;
use gpui_kit::component::Disableable;

fn prepare_location(path: &std::path::Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Recording location must be a real directory."
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Limit {
    Retention,
    Session,
    Total,
}
impl Limit {
    fn title(self) -> &'static str {
        match self {
            Self::Retention => "Retention Period",
            Self::Session => "Per-session Limit",
            Self::Total => "Total Storage Limit",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Retention => "Delete completed sessions after this many days.",
            Self::Session => "Keep the latest output when a session exceeds this size.",
            Self::Total => "Delete the oldest completed sessions when storage is full.",
        }
    }
    fn value(self, config: Preferences) -> u64 {
        match self {
            Self::Retention => config.retention_days,
            Self::Session => config.max_session_bytes,
            Self::Total => config.max_total_bytes,
        }
    }
    fn options(self) -> Vec<SelectOption<u64>> {
        let (values, multiplier, unit) = match self {
            Self::Retention => (Preferences::DAYS, 1, "days"),
            Self::Session => (Preferences::SESSION_MIB, 1024 * 1024, "MB"),
            Self::Total => (Preferences::TOTAL_GIB, 1024 * 1024 * 1024, "GB"),
        };
        values
            .into_iter()
            .map(|value| SelectOption::localized(value * multiplier, format!("{value} {unit}")))
            .collect()
    }
}
pub(super) struct Control {
    limit: Limit,
    select: SelectModel<u64>,
    _events: Subscription,
}
impl Settings {
    fn open_recording_location(&mut self, cx: &mut Context<Self>) {
        if self.recording_location_busy {
            return;
        }
        self.recording_location_busy = true;
        let directory = self.recording_directory.clone();
        cx.spawn(async move |this, cx| {
            let prepared = cx
                .background_spawn(async move {
                    prepare_location(&directory)?;
                    Ok::<_, anyhow::Error>(directory)
                })
                .await;
            // NSWorkspace must be called on the UI thread. The shared launcher
            // supplies completion/errors on every supported platform.
            let launched = this.update(cx, |_, _| {
                prepared.and_then(|path| crate::external_application::open_editor(&path, None))
            });
            let result = match launched {
                Ok(Ok(receiver)) => receiver.recv().await.unwrap_or_else(|_| {
                    Err(anyhow::anyhow!("The directory opener did not complete."))
                }),
                Ok(Err(error)) => Err(error),
                Err(_) => return,
            };
            let _ =
                this.update(cx, |this, cx| {
                    this.recording_location_busy = false;
                    if let Err(error) = result {
                        this.error = Some(format!("Unable to open recording location: {error}"));
                    } else if this.error.as_deref().is_some_and(|error| {
                        error.starts_with("Unable to open recording location:")
                    }) {
                        this.error = None;
                    }
                    cx.notify();
                });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn init_recording_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.recording_controls.is_empty() {
            return;
        }
        for limit in [Limit::Retention, Limit::Session, Limit::Total] {
            let select = SelectModel::new(
                limit.options(),
                Some(limit.value(self.values.recordings)),
                window,
                cx,
            );
            let events = cx.subscribe_in(
                &select.state,
                window,
                move |this, _, event: &SelectEvent<u64>, window, cx| {
                    if let SelectEvent::Confirm(Some(Some(value))) = event {
                        let mut config = this.values.recordings;
                        match limit {
                            Limit::Retention => config.retention_days = *value,
                            Limit::Session => config.max_session_bytes = *value,
                            Limit::Total => config.max_total_bytes = *value,
                        }
                        if let Err(error) = config.validate() {
                            this.error = Some(error.to_string());
                            cx.notify();
                            return;
                        }
                        this.values.recordings = config;
                        this.changed(window, cx);
                    }
                },
            );
            self.recording_controls.push(Control {
                limit,
                select,
                _events: events,
            });
        }
    }
    pub(super) fn recording_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        self.section(
            Section::Recording,
            IconName::Clock,
            div()
                .flex()
                .flex_col()
                .gap(px(18.))
                .child(
                    self.row(
                        "Terminal History",
                        "Save connection history on this device.",
                        Switch::new(
                            "session-history",
                            "Terminal History",
                            self.values.recordings.enabled,
                        )
                        .on_change(cx.listener(
                            |this, enabled, window, cx| {
                                this.values.recordings.enabled = *enabled;
                                this.changed(window, cx);
                            },
                        )),
                    ),
                )
                .child(
                    self.row(
                        "Record Terminal Sessions",
                        "Encrypt and save terminal output for later replay.",
                        Switch::new(
                            "record-terminal-output",
                            "Record Terminal Sessions",
                            self.values.recordings.capture,
                        )
                        .on_change(cx.listener(
                            |this, enabled, window, cx| {
                                this.values.recordings.capture = *enabled;
                                this.changed(window, cx);
                            },
                        )),
                    ),
                )
                .children(self.recording_controls.iter().map(|control| {
                    self.row(
                        control.limit.title(),
                        control.limit.description(),
                        control.select.control(control.limit.title(), cx),
                    )
                }))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(12.))
                                .text_color(Palette::settings(self.values.dark).muted)
                                .child(crate::localization::text(cx, "common.label.location")),
                        )
                        .child(
                            Button::new("recording-location")
                                .ghost()
                                .h(px(28.))
                                .px(px(7.))
                                .min_w_0()
                                .max_w_full()
                                .content_start()
                                .icon(IconName::Folder)
                                .label(self.recording_directory.to_string_lossy().into_owned())
                                .localized_accessibility_label(format!(
                                    "Open recording location {}",
                                    self.recording_directory.display()
                                ))
                                .tooltip("Open location")
                                .disabled(self.recording_location_busy)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.open_recording_location(cx)),
                                ),
                        ),
                ),
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::prepare_location;
    #[test]
    fn recording_location_uses_its_supplied_private_directory() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let own = root.path().join("rs-data").join("recordings");
        prepare_location(&own)?;
        prepare_location(&own)?;
        assert!(own.is_dir());
        let reference = root.path().join("original-sentinel");
        std::fs::write(&reference, "untouched")?;
        assert!(prepare_location(&reference).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&own)?.permissions().mode() & 0o777, 0o700);
            let link = root.path().join("link");
            std::os::unix::fs::symlink(&own, &link)?;
            assert!(prepare_location(&link).is_err());
        }
        assert_eq!(std::fs::read_to_string(reference)?, "untouched");
        Ok(())
    }
}
