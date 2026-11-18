use super::*;
use crate::{
    components::Spinner,
    localization::format_for,
    url_handlers::{self, Scheme, SetError},
};
use gpui_kit::component::Disableable;

const DESCRIPTION: &str = "settings.general.defaultUrlHandler.description";

#[derive(Default)]
pub(super) struct State {
    status: [Option<bool>; 3],
    checked: bool,
    busy: bool,
    setting: Option<usize>,
    error: Option<SetError>,
}

impl Settings {
    pub(super) fn refresh_url_handlers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_url_handlers(None, window, cx);
    }

    fn update_url_handlers(
        &mut self,
        setting: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.url_handlers.busy {
            return;
        }
        self.url_handlers.busy = true;
        self.url_handlers.setting = setting;
        if setting.is_some() {
            self.url_handlers.error = None;
        }
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let executor = cx.background_executor().clone();
            let error = match setting {
                Some(index) => url_handlers::set(Scheme::ALL[index], &executor).await.err(),
                None => None,
            };
            let status = url_handlers::query(&executor).await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.url_handlers.status = status;
                this.url_handlers.checked = true;
                this.url_handlers.busy = false;
                this.url_handlers.setting = None;
                if setting.is_some() {
                    this.url_handlers.error = error;
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn url_handler_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.url_handlers;
        let p = Palette::settings(self.values.dark);
        let controls = Scheme::ALL.into_iter().enumerate().map(|(index, scheme)| {
            let status = state.status[index];
            let checking = !state.checked || state.setting == Some(index);
            let label = scheme.label();
            let message = match (checking, status) {
                (true, _) => "settings.general.defaultUrlHandler.checking",
                (false, Some(true)) => "settings.general.defaultUrlHandler.isDefault",
                (false, _) if cfg!(target_os = "windows") => {
                    "settings.general.defaultUrlHandler.openSettings"
                }
                (false, _) => "settings.general.defaultUrlHandler.setDefault",
            };
            let tooltip = format_for(self.values.general.language, message, &[("scheme", label)]);
            let indicator = if checking {
                Spinner::new().with_size(px(13.)).into_any_element()
            } else if let Some(current) = status {
                div()
                    .size_full()
                    .rounded_full()
                    .bg(rgb(if current { 0x218653 } else { 0xd04444 }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(if current {
                            IconName::Check
                        } else {
                            IconName::X
                        })
                        .with_size(px(14.))
                        .text_color(rgb(0xffffff)),
                    )
                    .into_any_element()
            } else {
                Icon::new(IconName::Info)
                    .with_size(px(16.))
                    .text_color(p.muted)
                    .into_any_element()
            };
            Button::new(("default-url-handler", index))
                .flex_1()
                .min_w_0()
                .ghost()
                .h(px(40.))
                .px(px(12.))
                .raw_tooltip(tooltip.clone())
                .accessibility_label(format!("{label} {tooltip}"))
                .disabled(state.busy || status == Some(true))
                .child(
                    SettingsText::Label
                        .apply(div())
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(label)
                        .child(
                            div()
                                .size(px(20.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(indicator),
                        ),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.update_url_handlers(Some(index), window, cx);
                }))
        });
        let error = match state.error {
            #[cfg(not(target_os = "windows"))]
            Some(SetError::NotRegistered) => {
                Some("settings.general.defaultUrlHandler.notRegistered")
            }
            Some(SetError::Failed) => Some("settings.general.defaultUrlHandler.setFailed"),
            None if state.checked && state.status.contains(&None) => {
                Some("settings.general.defaultUrlHandler.queryFailed")
            }
            None => None,
        };
        self.section(
            Section::UrlHandlers,
            IconName::Link,
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    SettingsText::Description
                        .apply(div())
                        .text_color(Palette::settings_faint(self.values.dark))
                        .child(self.text(DESCRIPTION)),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .children(controls),
                )
                .when_some(error, |view, error| {
                    view.child(
                        SettingsText::Description
                            .apply(div())
                            .text_color(rgb(0xd04444))
                            .child(self.text(error)),
                    )
                }),
        )
    }
}
