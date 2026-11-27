//! The About page uses the Settings skin. License browsing is a content Modal.
use super::*;
use crate::typography::{FontWeights, SettingsText};
use crate::{
    about::{
        self as model,
        licenses::Catalog,
        releases::{Release, Repository, Target},
    },
    components::{DialogColors, DialogFrame, Modal, Spinner},
};
use gpui_kit::base::input::TextareaState;
use gpui_kit::component::Disableable;
use std::sync::Arc;
mod download;
pub enum Event {
    Restart,
}
impl EventEmitter<Event> for Panel {}
mod licenses;

pub(super) struct Panel {
    dark: bool,
    repository: Result<Repository, String>,
    checking: bool,
    release: Option<Release>,
    error: Option<String>,
    check_task: Option<Task<()>>,
    update_directory: PathBuf,
    download: Option<download::Download>,
    download_notice: Option<String>,
    opening_package: bool,
    restart_available: bool,
    open_task: Option<Task<()>>,
    focus: FocusHandle,
    source_focus: FocusHandle,
    license_focus: FocusHandle,
    source_open: bool,
    licenses_open: bool,
    license_loading: bool,
    license_error: Option<String>,
    catalog: Option<Arc<Catalog>>,
    filtered: Vec<usize>,
    selected: Option<usize>,
    query: Entity<InputState>,
    text: Entity<TextareaState>,
    source: Entity<TextareaState>,
    notes: Entity<TextareaState>,
    list_scroll: UniformListScrollHandle,
    license_task: Option<Task<()>>,
    _query: Subscription,
}
impl Panel {
    pub fn new(
        dark: bool,
        update_directory: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        crate::components::message::watch(window, cx, |view: &Self| {
            view.license_error
                .clone()
                .map(crate::components::message::Message::error)
        });
        let query =
            cx.new(|cx| InputState::new(window, cx).placeholder("settings.search.packages.hint"));
        let event = cx.subscribe_in(&query, window, |this, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Change) {
                this.filter_licenses(window, cx);
            }
        });
        Self {
            dark,
            repository: Repository::configured().map_err(|error| error.to_string()),
            checking: false,
            release: None,
            error: None,
            check_task: None,
            update_directory,
            download: None,
            download_notice: None,
            opening_package: false,
            restart_available: false,
            open_task: None,
            focus: cx.focus_handle(),
            source_focus: cx.focus_handle(),
            license_focus: cx.focus_handle(),
            source_open: false,
            licenses_open: false,
            license_loading: false,
            license_error: None,
            catalog: None,
            filtered: Vec::new(),
            selected: None,
            query,
            text: cx.new(|cx| TextareaState::new(window, cx)),
            source: cx
                .new(|cx| TextareaState::new(window, cx).default_value(model::SOURCE_LICENSE)),
            notes: cx.new(|cx| TextareaState::new(window, cx)),
            list_scroll: UniformListScrollHandle::new(),
            license_task: None,
            _query: event,
        }
    }
    fn check_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.checking || self.download_running() || self.opening_package {
            return;
        }
        if cfg!(target_os = "macos") {
            #[cfg(target_os = "macos")]
            {
                self.checking = true;
                self.error = None;
                self.check_task = Some(cx.spawn_in(window, async move |this, cx| {
                    let result = crate::sparkle::check_for_updates();
                    let _ = this.update_in(cx, |this, _, cx| {
                        this.checking = false;
                        this.error = result.err().map(|error| error.to_string());
                        this.check_task = None;
                        cx.notify();
                    });
                }));
                cx.notify();
            }
            return;
        }
        let repository = match &self.repository {
            Ok(repository) => repository.clone(),
            Err(error) => {
                self.error = Some(error.clone());
                cx.notify();
                return;
            }
        };
        self.checking = true;
        self.error = None;
        self.release = None;
        self.download = None;
        self.download_notice = None;
        self.check_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || {
                model::releases::check(repository, model::VERSION, Target::current())
            })
            .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.checking = false;
                this.check_task = None;
                match result {
                    Ok(release) => {
                        this.notes.update(cx, |notes, cx| {
                            notes.set_value(release.notes.clone(), window, cx)
                        });
                        this.release = Some(release);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn colors(&self) -> DialogColors {
        let p = Palette::settings(self.dark);
        DialogColors {
            surface: p.surface,
            header: p.sidebar,
            border: p.border,
            foreground: p.text,
            dark: self.dark,
        }
    }
    fn section(
        &self,
        title: &'static str,
        icon: IconName,
        body: impl IntoElement,
        cx: &App,
    ) -> AnyElement {
        sections::frame(
            title,
            crate::localization::text(cx, title),
            icon,
            Palette::settings(self.dark),
            body,
            None,
        )
    }
    fn row(
        &self,
        title: &'static str,
        subtitle: impl Into<SharedString>,
        value: impl IntoElement,
    ) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(px(16.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        SettingsText::Label
                            .apply(div())
                            .child(crate::localization::view(title)),
                    )
                    .child(
                        SettingsText::Description
                            .apply(div())
                            .text_color(Palette::settings_faint(self.dark))
                            .child(crate::localization::view(subtitle.into())),
                    ),
            )
            .child(div().flex_shrink_0().child(value))
            .into_any_element()
    }
}
impl Panel {
    fn update_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        if cfg!(target_os = "macos") {
            #[cfg(target_os = "macos")]
            {
                let p = Palette::settings(self.dark);
                let configured = crate::sparkle::configuration().is_ok();
                return div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(div().text_size(px(12.)).text_color(p.muted).child(
                        crate::localization::text(cx, "settings.update.sparkle.managed"),
                    ))
                    .when(!configured, |view| {
                        view.child(div().text_size(px(12.)).text_color(p.muted).child(
                            crate::localization::text(cx, "settings.update.sparkle.notConfigured"),
                        ))
                    })
                    .child(
                        div().flex().justify_end().child(
                            Button::new("about-check-sparkle")
                                .outline()
                                .icon(IconName::RefreshCw)
                                .label("settings.update.action.check")
                                .disabled(self.checking || !configured)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.check_updates(window, cx)
                                })),
                        ),
                    )
                    .into_any_element();
            }
        }
        {
            let p = Palette::settings(self.dark);
            let mut updates = div().flex().flex_col().gap(px(14.));
            if self.checking {
                updates = updates.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(Spinner::new())
                        .child(crate::localization::text(
                            cx,
                            "settings.update.action.checking",
                        )),
                );
            }
            if let Some(release) = &self.release {
                let message = if release.asset.is_none() {
                    crate::localization::format(
                        cx,
                        "settings.update.status.noPackage",
                        &[
                            ("appName", crate::APPLICATION_NAME),
                            ("version", &release.version),
                        ],
                    )
                    .to_string()
                } else if release.newer {
                    crate::localization::format_for(
                        crate::localization::language(cx),
                        "settings.update.status.available",
                        &[
                            ("appName", crate::APPLICATION_NAME),
                            ("version", &release.version),
                        ],
                    )
                    .to_string()
                } else {
                    crate::localization::text(cx, "settings.update.status.current").to_string()
                };
                updates = updates.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.muted)
                        .child(crate::localization::message(cx, &message)),
                );
                if release.newer && release.asset.is_some() && release.checksums.is_none() {
                    updates = updates.child(div().text_size(px(12.)).text_color(p.muted).child(
                        crate::localization::text(cx, "settings.update.status.browserOnly"),
                    ));
                }
                let release_url = release.url.clone();
                let mut actions = div().flex().justify_end().flex_wrap().gap(px(8.)).child(
                    Button::new("about-release-details")
                        .outline()
                        .label("settings.update.action.releaseDetails")
                        .on_click(move |_, _, cx| cx.open_url(&release_url)),
                );
                if release.newer
                    && let Some(asset) = &release.asset
                {
                    let url = asset.url.clone();
                    actions = actions.child(
                        Button::new("about-download-rs")
                            .outline()
                            .icon(IconName::Download)
                            .label("settings.update.action.downloadInBrowser")
                            .disabled(self.download_running() || self.opening_package)
                            .tooltip(asset.name.clone())
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    );
                    if self
                        .download
                        .as_ref()
                        .is_none_or(|download| download.ready.is_none())
                    {
                        let plan_error = model::download::Plan::new(release)
                            .err()
                            .map(|error| error.to_string());
                        actions = actions.child(
                            Button::new("about-managed-download")
                                .outline()
                                .icon(IconName::Download)
                                .label("settings.update.action.download")
                                .tooltip(plan_error.clone().unwrap_or_else(|| {
                                    "Download and verify the Nauterm RS package.".into()
                                }))
                                .disabled(
                                    self.download_running()
                                        || self.checking
                                        || plan_error.is_some(),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.download_update(window, cx)
                                })),
                        );
                    }
                }
                updates = updates.child(actions);
            }
            updates = updates.children(self.download_view(cx));
            if let Err(error) = &self.repository {
                updates = updates.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.muted)
                        .child(crate::localization::message(cx, error)),
                );
            }
            updates = updates.child(
                div().flex().justify_end().child(
                    Button::new("about-check-updates")
                        .outline()
                        .icon(IconName::RefreshCw)
                        .label("settings.update.action.check")
                        .disabled(
                            self.checking
                                || self.download_running()
                                || self.opening_package
                                || self.repository.is_err(),
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.check_updates(window, cx)),
                        ),
                ),
            );
            updates.into_any_element()
        }
    }
}
impl Render for Panel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let updates = self.update_controls(cx);
        let mut content = sections::stack()
            .track_focus(&self.focus)
            .child(
                self.section(
                    "common.label.application",
                    IconName::Info,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(self.row(
                            "common.label.version",
                            "settings.label.releaseChannelStable",
                            div().font_weight(FontWeights::MEDIUM).child(model::VERSION),
                        ))
                        .child(self.row("common.label.build", model::PACKAGE, model::BUILD))
                        .child(self.row(
                            "common.label.platform",
                            "settings.label.currentPlatformArchitecture",
                            model::platform(),
                        )),
                    cx,
                ),
            )
            .child(
                self.section(
                    "common.label.license",
                    IconName::Scale,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(18.))
                        .child(
                            self.row(
                                "settings.label.sourceLicense",
                                "settings.about.license.help",
                                Button::new("about-source-license")
                                    .ghost()
                                    .label("MIT + Commons Clause")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.source_open = true;
                                        cx.notify();
                                    })),
                            ),
                        )
                        .child(
                            self.row(
                                "settings.label.thirdPartyLicenses",
                                "settings.about.licenses.rustDescription",
                                Button::new("about-third-party-licenses")
                                    .outline()
                                    .label("common.label.view")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_licenses(window, cx)
                                    })),
                            ),
                        ),
                    cx,
                ),
            )
            .child(self.section("common.action.update", IconName::Download, updates, cx));
        if self
            .release
            .as_ref()
            .is_some_and(|release| !release.notes.trim().is_empty())
        {
            content = content.child(
                self.section(
                    "settings.label.releaseNotes",
                    IconName::ScrollText,
                    Input::multiline(&self.notes)
                        .readonly(true)
                        .h(px(240.))
                        .aria_label(crate::localization::text(cx, "settings.label.releaseNotes")),
                    cx,
                ),
            );
        }
        content
            .children(self.source_modal(cx))
            .children(self.licenses_modal(window, cx))
            .into_any_element()
    }
}
impl Settings {
    pub(crate) fn check_for_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.select_page(Page::About, window, cx);
        self.about
            .update(cx, |panel, cx| panel.check_updates(window, cx));
    }
    pub(super) fn about_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.about.update(cx, |about, cx| {
            if about.dark != self.values.dark {
                about.dark = self.values.dark;
                cx.notify();
            }
        });
        self.search.section(Section::About, self.about.clone())
    }
}
