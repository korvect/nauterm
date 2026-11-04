//! SFTP editor controls use the settings skin of the product Input/Select.
//! Detection is supplied by the owning settings window, never done in render.
use crate::{
    components::{SelectEvent, SelectModel, SelectOption, settings::Input},
    editor_config::{Editor, Editors, parse_extensions},
    palette::Palette,
    typography::SettingsText,
};
use gpui_kit::{
    base::input::{InputEvent, InputState},
    prelude::*,
    *,
};

fn option(editor: &Editor) -> SelectOption<String> {
    let option = SelectOption::new(editor.id(), editor.label().to_owned())
        .icon(gpui_kit::assets::IconName::AppWindow);
    if let Some(image) = crate::external_application::cached_icon(editor) {
        option.image(image)
    } else {
        option
    }
}

fn reconcile_catalog(
    config: &mut Editors,
    mut catalog: Vec<Editor>,
    preserve_selection: bool,
) -> Vec<Editor> {
    if preserve_selection && let Some(selected) = &config.external_editor {
        // A user can choose another installation with the same display name
        // while discovery is running. Keep that exact application identity.
        catalog.retain(|editor| {
            editor.id() == selected.id() || !editor.label().eq_ignore_ascii_case(selected.label())
        });
    }
    config.reconcile(catalog)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CatalogState {
    Idle,
    Loading,
    Ready,
    Failed,
}

pub struct Changed(pub Editors);
pub struct EditorSettings {
    config: Editors,
    dark: bool,
    pub ssh: Entity<InputState>,
    pub extensions: Entity<InputState>,
    pub select: SelectModel<String>,
    catalog: Vec<Editor>,
    catalog_state: CatalogState,
    user_selected: bool,
    error: Option<String>,
    validation_error: Option<String>,
    _events: Vec<Subscription>,
}
impl EventEmitter<Changed> for EditorSettings {}
impl EditorSettings {
    pub fn new(config: Editors, dark: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        let ssh = cx.new(|cx| InputState::new(window, cx).default_value(&config.ssh_editor));
        let extensions = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.text_extensions.join(", "))
                .placeholder("txt, md, json, yaml, sh, py")
        });
        let catalog: Vec<_> = config.external_editor.iter().cloned().collect();
        let select = SelectModel::new(
            catalog.iter().map(option).collect(),
            config.external_editor.as_ref().map(Editor::id),
            window,
            cx,
        );
        let events = vec![
            cx.subscribe(&ssh, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let mut candidate = this.config.clone();
                    candidate.ssh_editor = input.read(cx).value().trim().into();
                    this.commit(candidate, cx);
                }
            }),
            cx.subscribe_in(
                &extensions,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        let mut candidate = this.config.clone();
                        candidate.text_extensions = parse_extensions(&input.read(cx).value());
                        input.update(cx, |input, cx| {
                            input.set_value(candidate.text_extensions.join(", "), window, cx)
                        });
                        this.commit(candidate, cx);
                    }
                },
            ),
            cx.subscribe(&select.state, |this, _, event: &SelectEvent<String>, cx| {
                if let SelectEvent::Confirm(Some(Some(id))) = event
                    && let Some(editor) = this.catalog.iter().find(|editor| editor.id() == *id)
                {
                    let mut candidate = this.config.clone();
                    candidate.external_editor = Some(editor.clone());
                    this.user_selected = true;
                    this.commit(candidate, cx);
                }
            }),
        ];
        Self {
            config,
            dark,
            ssh,
            extensions,
            select,
            catalog,
            catalog_state: CatalogState::Idle,
            user_selected: false,
            error: None,
            validation_error: None,
            _events: events,
        }
    }
    fn commit(&mut self, candidate: Editors, cx: &mut Context<Self>) {
        if let Err(error) = candidate.validate() {
            self.validation_error = Some(error.to_string());
            cx.notify();
            return;
        }
        self.validation_error = None;
        if candidate != self.config {
            self.config = candidate;
            cx.emit(Changed(self.config.clone()));
        }
        cx.notify();
    }
    /// A settings window can close without another field receiving focus.
    /// Its owner publishes this validated final draft before removing the UI.
    pub fn finish_editing(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Editors> {
        let mut candidate = self.config.clone();
        candidate.ssh_editor = self.ssh.read(cx).value().trim().into();
        candidate.text_extensions = parse_extensions(&self.extensions.read(cx).value());
        candidate.validate()?;
        self.extensions.update(cx, |input, cx| {
            input.set_value(candidate.text_extensions.join(", "), window, cx)
        });
        self.config = candidate.clone();
        cx.notify();
        Ok(candidate)
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.dark != dark {
            self.dark = dark;
            cx.notify();
        }
    }
    pub fn set_config(&mut self, config: Editors, window: &mut Window, cx: &mut Context<Self>) {
        self.user_selected = false;
        if self.catalog_state == CatalogState::Loading {
            self.catalog_state = CatalogState::Idle;
        }
        self.config = config;
        self.error = None;
        self.validation_error = None;
        self.ssh.update(cx, |input, cx| {
            input.set_value(&self.config.ssh_editor, window, cx)
        });
        self.extensions.update(cx, |input, cx| {
            input.set_value(self.config.text_extensions.join(", "), window, cx)
        });
        // Options retain image snapshots. Refresh them as well as the selected
        // ID when the owner reapplies settings after icon/catalog loading.
        self.select.reconcile(
            self.catalog.iter().map(option).collect(),
            self.config.external_editor.as_ref().map(Editor::id),
            window,
            cx,
        );
        cx.notify();
    }
    pub fn detecting(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.catalog_state = CatalogState::Loading;
        cx.notify();
    }
    pub fn catalog_ready(&self) -> bool {
        self.catalog_state == CatalogState::Ready
    }
    pub fn refresh_icons(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.select.reconcile(
            self.catalog.iter().map(option).collect(),
            self.config.external_editor.as_ref().map(Editor::id),
            window,
            cx,
        );
        cx.notify();
    }
    pub fn detected(
        &mut self,
        result: Result<Vec<Editor>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(catalog) => {
                self.catalog_state = CatalogState::Ready;
                let mut candidate = self.config.clone();
                self.catalog = reconcile_catalog(&mut candidate, catalog, self.user_selected);
                self.select.reconcile(
                    self.catalog.iter().map(option).collect(),
                    candidate.external_editor.as_ref().map(Editor::id),
                    window,
                    cx,
                );
                self.commit(candidate, cx);
            }
            Err(error) => {
                self.catalog_state = CatalogState::Failed;
                self.error = Some(format!(
                    "Unable to detect editors: {error}. The saved editor was kept."
                ))
            }
        }
        cx.notify();
    }
    fn row(
        &self,
        title: &'static str,
        description: &'static str,
        control: impl IntoElement,
    ) -> AnyElement {
        div()
            .id(title)
            .w_full()
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
                            .child(crate::localization::view(description)),
                    ),
            )
            .child(div().flex_1().min_w_0().child(control))
            .into_any_element()
    }
}
impl Render for EditorSettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor_label = if self.config.external_editor.is_some() {
            "External Editor"
        } else {
            match self.catalog_state {
                CatalogState::Loading => "Loading applications…",
                CatalogState::Ready if self.catalog.is_empty() => {
                    "No recommended text applications found"
                }
                CatalogState::Failed if self.catalog.is_empty() => "Unable to load applications",
                _ => "External Editor",
            }
        };
        div()
            .id("sftp-editor-settings")
            .w_full()
            .flex()
            .flex_col()
            .gap(px(18.))
            .child(self.row(
                "SSH Editor",
                "Terminal editor for remote files (e.g. vim, nano).",
                Input::new(&self.ssh).aria_label(crate::localization::text(cx, "SSH Editor")),
            ))
            .children(crate::components::validation_message(
                self.validation_error.as_deref(),
                cx,
            ))
            .child(
                self.row(
                    "External Editor",
                    "Editor used for configured text file extensions.",
                    self.select
                        .control_disabled(editor_label, self.catalog.is_empty()),
                ),
            )
            .child(
                self.row(
                    "Text File Extensions",
                    "Extensions that show the configured external editor.",
                    Input::new(&self.extensions)
                        .aria_label(crate::localization::text(cx, "Text File Extensions")),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::reconcile_catalog;
    use crate::editor_config::{Editor, Editors};
    use std::path::PathBuf;

    fn editor(path: &str) -> Editor {
        Editor::Command {
            label: "Text editor".into(),
            executable: PathBuf::from(path),
            arguments: vec![],
        }
    }

    #[test]
    fn late_discovery_keeps_the_exact_editor_the_user_just_chose() {
        let selected = editor("custom-editor");
        let mut config = Editors {
            external_editor: Some(selected.clone()),
            ..Default::default()
        };
        let catalog = reconcile_catalog(&mut config, vec![editor("detected-editor")], true);
        assert_eq!(config.external_editor, Some(selected.clone()));
        assert_eq!(catalog, vec![selected]);
    }

    #[test]
    fn saved_selection_can_still_resolve_its_current_installation() {
        let current = editor("detected-editor");
        let mut config = Editors {
            external_editor: Some(editor("old-editor")),
            ..Default::default()
        };
        let catalog = reconcile_catalog(&mut config, vec![current.clone()], false);
        assert_eq!(config.external_editor, Some(current.clone()));
        assert_eq!(catalog, vec![current]);
    }
}
