//! Host/key import uses one content Modal and the existing form/selection controls.
use super::*;
use crate::typography::FontWeights;
use crate::{
    components::{Checkbox, DialogColors, DialogFrame, Modal},
    host_import::{KeyBatch, KeyCandidate, Source},
    host_transfer::{self, Candidate},
};
use gpui_kit::{PathPromptOptions, base::input::InputEvent};
use std::{collections::BTreeMap, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Hosts,
    Keys,
}
#[derive(Clone, Copy)]
enum Preview {
    Header(Section),
    Host(usize),
    Key(usize),
}
pub(super) struct Import {
    filter: Entity<InputState>,
    _filter: Subscription,
    source: Option<Source>,
    rows: Vec<Candidate>,
    keys: Vec<KeyCandidate>,
    visible: Rc<Vec<Preview>>,
    selected: BTreeSet<usize>,
    selected_keys: BTreeSet<usize>,
    expanded: [bool; 2],
    file: Option<String>,
    skipped_files: usize,
    loading: bool,
    saving: bool,
    pub(super) error: Option<String>,
    scroll: UniformListScrollHandle,
}
impl Import {
    fn indices(&self, section: Section, query: &str) -> Vec<usize> {
        match section {
            Section::Hosts => self
                .rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row.matches(query))
                .map(|(index, _)| index)
                .collect(),
            Section::Keys => self
                .keys
                .iter()
                .enumerate()
                .filter(|(_, row)| row.matches(query))
                .map(|(index, _)| index)
                .collect(),
        }
    }
    fn refilter(&mut self, query: &str) {
        let mut visible = vec![];
        for (index, section) in [Section::Hosts, Section::Keys].into_iter().enumerate() {
            let rows = self.indices(section, query);
            if rows.is_empty() {
                continue;
            }
            visible.push(Preview::Header(section));
            if self.expanded[index] {
                visible.extend(rows.into_iter().map(|row| match section {
                    Section::Hosts => Preview::Host(row),
                    Section::Keys => Preview::Key(row),
                }));
            }
        }
        self.visible = Rc::new(visible);
    }
    fn selection_count(&self) -> usize {
        self.selected.len() + self.selected_keys.len()
    }
}
impl Nauterm {
    pub(super) fn open_host_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.host_import.as_ref().is_some_and(|state| state.saving) {
            return;
        }
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Search Hosts and keys"));
        let event = cx.subscribe(&filter, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change)
                && let Some(state) = this
                    .host_import
                    .as_mut()
                    .filter(|state| state.filter.entity_id() == input.entity_id())
            {
                state.refilter(&input.read(cx).value());
                state.scroll.scroll_to_item(0, ScrollStrategy::Nearest);
                cx.notify();
            }
        });
        self.host_import = Some(Import {
            filter,
            _filter: event,
            source: None,
            rows: vec![],
            keys: vec![],
            visible: Rc::new(vec![]),
            selected: BTreeSet::new(),
            selected_keys: BTreeSet::new(),
            expanded: [true, true],
            file: None,
            skipped_files: 0,
            loading: false,
            saving: false,
            error: None,
            scroll: UniformListScrollHandle::new(),
        });
        cx.notify();
    }
    fn pick_host_source(&mut self, source: Source, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self
            .host_import
            .as_mut()
            .filter(|state| !state.saving && !state.loading)
        else {
            return;
        };
        state.loading = true;
        state.error = None;
        let token = state.filter.entity_id();
        let picker = crate::file_dialog::open(
            PathPromptOptions {
                files: !source.directory(),
                directories: source.directory(),
                multiple: false,
                prompt: Some(source.detail().into()),
            },
            matches!(source, Source::OpenSsh)
                .then_some(crate::file_dialog::InitialDirectory::SshImport),
            window,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let result = match picker.await {
                Ok(Ok(Some(mut paths))) => match paths.pop() {
                    Some(path) => {
                        cx.background_spawn(async move {
                            let name = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned();
                            crate::host_import::load(source, &path)
                                .map(|bundle| Some((name, bundle)))
                        })
                        .await
                    }
                    None => Ok(None),
                },
                Ok(Ok(None)) => Ok(None),
                _ => Err(anyhow::anyhow!("Could not open the import picker.")),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(state) = this
                    .host_import
                    .as_mut()
                    .filter(|state| state.filter.entity_id() == token)
                else {
                    return;
                };
                state.loading = false;
                match result {
                    Ok(Some((name, bundle))) => {
                        state.source = Some(source);
                        state.file = Some(name);
                        state.rows = bundle.hosts;
                        state.keys = bundle.keys;
                        state.skipped_files = bundle.skipped_files;
                        state.selected = state
                            .rows
                            .iter()
                            .enumerate()
                            .filter(|(_, row)| row.issue.is_none())
                            .map(|(index, _)| index)
                            .collect();
                        state.selected_keys = (0..state.keys.len()).collect();
                        state.expanded = [true, true];
                        state
                            .filter
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        state.refilter("");
                        state.scroll.scroll_to_item(0, ScrollStrategy::Nearest);
                        if state.rows.is_empty() && state.keys.is_empty() {
                            state.error = Some("No importable Hosts or keys were found.".into());
                        }
                    }
                    Ok(None) => {}
                    Err(error) => state.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn confirm_host_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.resources_ready() {
            return;
        }
        let Some(state) = self
            .host_import
            .as_ref()
            .filter(|state| !state.saving && !state.loading && state.selection_count() > 0)
        else {
            return;
        };
        let token = state.filter.entity_id();
        let rows: Vec<_> = state
            .selected
            .iter()
            .filter_map(|index| state.rows.get(*index))
            .cloned()
            .collect();
        let keys: Vec<_> = state
            .selected_keys
            .iter()
            .filter_map(|index| state.keys.get(*index))
            .cloned()
            .collect();
        let password_rows: Vec<_> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| !row.password.is_empty())
            .map(|(index, _)| index)
            .collect();
        let vault = if password_rows.is_empty() {
            None
        } else {
            let Some(vault) = self.ensure_keychain(cx).read(cx).vault() else {
                self.host_import.as_mut().unwrap().error =
                    Some("The encrypted database is not ready.".into());
                cx.notify();
                return;
            };
            Some(vault)
        };
        let state = self.host_import.as_mut().unwrap();
        state.saving = true;
        state.error = None;
        cx.spawn_in(window, async move |this, cx| {
            let saved = if let Some(vault) = &vault {
                vault.save_host_passwords(password_rows.iter().map(|index| rows[*index].password.clone()).collect()).await
            } else {
                Ok(vec![])
            };
            let created = saved.as_ref().cloned().unwrap_or_default();
            let _ = this.update_in(cx, |this, _, cx| {
                if !this.host_import.as_ref().is_some_and(|state| state.filter.entity_id() == token) {
                    return;
                }
                let result = (|| -> anyhow::Result<_> {
                    let saved = saved?;
                    let mut references = vec![None; rows.len()];
                    for (index, id) in password_rows.into_iter().zip(saved) {
                        references[index] = Some(id);
                    }
                    anyhow::ensure!(this.resources_ready(), "Workspace storage is unavailable.");
                    let mut imported = host_transfer::merge(&this.data, &rows, &references)?;
                    let report = this.store.publish_host_import(&mut imported.data, KeyBatch {
                        keys,
                        bindings: std::mem::take(&mut imported.key_bindings),
                    })?;
                    Ok((imported, report))
                })();
                match result {
                    Ok((imported, report)) => {
                        let count = imported.ids.len();
                        this.data = imported.data;
                        this.host_import = None;
                        let unmatched = if report.unmatched_hosts > 0 {
                            format!(" {} Host key references remain unconfigured.", report.unmatched_hosts)
                        } else {
                            String::new()
                        };
                        this.notice = Some(format!("Imported {count} Hosts and {} keys. {} duplicate Hosts skipped; {} keys reused.{unmatched}", report.created, imported.skipped, report.reused));
                        if let Some(keychain) = &this.keychain {
                            keychain.update(cx, |view, cx| view.refresh(cx));
                        }
                    }
                    Err(error) => {
                        let state = this.host_import.as_mut().unwrap();
                        state.saving = false;
                        state.error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
            if !created.is_empty()
                && let Some(vault) = vault
                && let Err(error) = vault.remove_host_passwords(created).await
            {
                let _ = this.update(cx, |this, cx| {
                    this.notice = Some(format!("Encrypted import cleanup failed: {error}"));
                    cx.notify();
                });
            }
        }).detach();
        cx.notify();
    }
}

impl Nauterm {
    pub(super) fn export_host_csv(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.host_exporting {
            return;
        }
        let export = host_transfer::Export::new(&self.data);
        if export.hosts.is_empty() {
            self.notice = Some("No remote hosts to export.".into());
            cx.notify();
            return;
        }
        let ids = export.password_ids();
        let vault = if ids.is_empty() {
            None
        } else {
            let Some(vault) = self.ensure_keychain(cx).read(cx).vault() else {
                self.notice = Some("The encrypted database is not ready.".into());
                cx.notify();
                return;
            };
            Some(vault)
        };
        self.host_exporting = true;
        let picker = cx.prompt_for_new_path(
            &crate::file_dialog::export_directory(),
            Some("nauterm-hosts.csv"),
        );
        cx.spawn_in(window, async move |this, cx| {
            let result = async {
                let path = match picker.await {
                    Ok(Ok(Some(path))) => path,
                    Ok(Ok(None)) => return Ok(None),
                    _ => anyhow::bail!("Could not open the CSV export picker."),
                };
                let path = if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
                {
                    path
                } else {
                    let mut path = path.into_os_string();
                    path.push(".csv");
                    std::path::PathBuf::from(path)
                };
                let passwords = if let Some(vault) = vault {
                    vault.host_passwords(ids).await?
                } else {
                    BTreeMap::new()
                };
                cx.background_spawn(async move {
                    let count = export.hosts.len();
                    let csv = host_transfer::csv(&export, &passwords)?;
                    host_transfer::write(&path, &csv)?;
                    Ok(Some(count))
                })
                .await
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.host_exporting = false;
                match result {
                    Ok(Some(count)) => {
                        this.notice = Some(format!("Exported {count} hosts to CSV."))
                    }
                    Ok(None) => {}
                    Err(error) => this.notice = Some(format!("Host export failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn import_preview_row(
        &self,
        preview: Preview,
        token: EntityId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.host_import.as_ref().unwrap();
        let p = Palette::new(self.data.dark);
        let busy = state.loading || state.saving;
        match preview {
            Preview::Header(section) => {
                let index = usize::from(section == Section::Keys);
                let expanded = state.expanded[index];
                let query = state.filter.read(cx).value();
                let available: Vec<_> = state
                    .indices(section, &query)
                    .into_iter()
                    .filter(|row| section == Section::Keys || state.rows[*row].issue.is_none())
                    .collect();
                let selected = if section == Section::Hosts {
                    &state.selected
                } else {
                    &state.selected_keys
                };
                let checked =
                    !available.is_empty() && available.iter().all(|row| selected.contains(row));
                let title = if section == Section::Hosts {
                    format!("Hosts ({})", state.rows.len())
                } else {
                    format!("Keys ({})", state.keys.len())
                };
                div()
                    .h(px(52.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        Button::new(("import-expand", index))
                            .toolbar()
                            .ghost()
                            .icon(if expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .disabled(busy)
                            .tooltip(if expanded { "Collapse" } else { "Expand" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(state) = this.host_import.as_mut().filter(|state| {
                                    state.filter.entity_id() == token
                                        && !state.saving
                                        && !state.loading
                                }) {
                                    state.expanded[index] = !state.expanded[index];
                                    state.refilter(&state.filter.read(cx).value());
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Checkbox::new(("import-group", index), title, checked)
                                .list_row()
                                .disabled(busy || available.is_empty())
                                .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                                    if let Some(state) = this.host_import.as_mut().filter(|state| {
                                        state.filter.entity_id() == token
                                            && !state.saving
                                            && !state.loading
                                    }) {
                                        let selected = if section == Section::Hosts {
                                            &mut state.selected
                                        } else {
                                            &mut state.selected_keys
                                        };
                                        for row in &available {
                                            if *checked {
                                                selected.insert(*row);
                                            } else {
                                                selected.remove(row);
                                            }
                                        }
                                        cx.notify();
                                    }
                                })),
                        ),
                    )
                    .into_any_element()
            }
            Preview::Host(index) | Preview::Key(index) => {
                let key = matches!(preview, Preview::Key(_));
                let (name, detail, selected, unavailable) = if key {
                    let row = &state.keys[index];
                    (
                        row.name.clone(),
                        row.detail().to_owned(),
                        state.selected_keys.contains(&index),
                        false,
                    )
                } else {
                    let row = &state.rows[index];
                    let detail = row.issue.clone().unwrap_or_else(|| {
                        format!(
                            "{}  {}@{}:{}  {}{}",
                            row.protocol.to_uppercase(),
                            row.username,
                            row.address,
                            row.port,
                            row.groups.join(" / "),
                            row.identity_file
                                .as_ref()
                                .map(|path| {
                                    if state.keys.iter().enumerate().any(|(index, key)| {
                                        state.selected_keys.contains(&index)
                                            && key.paths.contains(path)
                                    }) {
                                        " · matching key selected"
                                    } else {
                                        " · key file not selected"
                                    }
                                })
                                .unwrap_or("")
                        )
                    });
                    (
                        row.name.clone(),
                        detail,
                        state.selected.contains(&index),
                        row.issue.is_some(),
                    )
                };
                div()
                    .h(px(52.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        Checkbox::new(
                            (if key { "import-key" } else { "import-host" }, index),
                            name,
                            selected,
                        )
                        .list_row()
                        .description(detail)
                        .disabled(busy || unavailable)
                        .on_change(cx.listener(
                            move |this, checked: &bool, _, cx| {
                                if let Some(state) = this.host_import.as_mut().filter(|state| {
                                    state.filter.entity_id() == token
                                        && !state.saving
                                        && !state.loading
                                }) {
                                    let selected = if key {
                                        &mut state.selected_keys
                                    } else {
                                        &mut state.selected
                                    };
                                    if *checked {
                                        selected.insert(index);
                                    } else {
                                        selected.remove(&index);
                                    }
                                    cx.notify();
                                }
                            },
                        )),
                    )
                    .into_any_element()
            }
        }
    }
    pub(super) fn host_import_modal(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.host_import.as_ref()?;
        let token = state.filter.entity_id();
        let p = Palette::new(self.data.dark);
        let title = state
            .source
            .map(|source| format!("Import from {}", source.label()))
            .unwrap_or_else(|| "Import to vault".into());
        let visible = state.visible.clone();
        let owner = cx.weak_entity();
        let list = uniform_list("host-import-list", visible.len(), move |range, _, cx| {
            let Some(owner) = owner.upgrade() else {
                return vec![];
            };
            range
                .filter_map(|row| {
                    let preview = *visible.get(row)?;
                    Some(owner.update(cx, |this, cx| this.import_preview_row(preview, token, cx)))
                })
                .collect()
        })
        .track_scroll(&state.scroll)
        .w_full()
        .h(px(
            (f32::from(window.viewport_size().height) - 350.).clamp(150., 420.)
        ));
        let mut body = div().w_full().flex().flex_col().gap(px(12.));
        if state.source.is_none() {
            body = body.child(div().text_color(p.muted).child(crate::localization::text(
                cx,
                "rs.ui.copy.chooseImportSource",
            )));
            let mut sources = div().w_full().min_w_0().grid().grid_cols(2).gap(px(10.));
            for source in Source::ALL {
                let source_icon = match source {
                    Source::Csv => IconName::FileText,
                    Source::OpenSsh => IconName::Folder,
                    Source::Putty => IconName::Terminal,
                    Source::MobaXterm => IconName::Server,
                    Source::SecureCrt => IconName::Shield,
                };
                sources = sources.child(
                    Button::new(SharedString::from(format!(
                        "import-source-{}",
                        source.label()
                    )))
                    .custom(
                        crate::components::ButtonCustomVariant::for_surface(p)
                            .color(p.surface.into()),
                    )
                    .outline()
                    .w_full()
                    .min_w_0()
                    .h(px(100.))
                    .px(px(15.))
                    .rounded(px(9.))
                    .content_start()
                    .content_gap(px(12.))
                    .disabled(state.loading)
                    .child(
                        div()
                            .size(px(36.))
                            .flex_shrink_0()
                            .rounded(px(8.))
                            .bg(p.accent.alpha(0.12))
                            .text_color(p.accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(source_icon).with_size(px(19.))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .items_start()
                            .gap(px(3.))
                            .child(
                                div()
                                    .font_weight(FontWeights::SEMIBOLD)
                                    .child(crate::localization::view(source.label())),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(source.detail()),
                            ),
                    )
                    .trailing_icon(
                        Icon::new(IconName::ChevronRight)
                            .with_size(px(18.))
                            .text_color(p.muted),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.pick_host_source(source, window, cx)
                    })),
                );
            }
            body = body.child(sources);
        } else {
            body = body
                .child(
                    div()
                        .text_color(p.muted)
                        .child(state.file.clone().unwrap_or_default()),
                )
                .child(
                    Input::new(&state.filter)
                        .prefix(icon(IconName::Search))
                        .disabled(state.saving || state.loading),
                )
                .child(crate::components::uniform_scrollbar(
                    "host-import-scrollbar",
                    list,
                    &state.scroll,
                ))
                .when(state.visible.is_empty(), |body| {
                    body.child(div().text_color(p.muted).child(crate::localization::text(
                        cx,
                        "rs.ui.copy.noMatchingHostsOrKeys",
                    )))
                })
                .child(div().text_size(px(12.)).text_color(p.muted).child(
                    crate::localization::text(cx, "rs.ui.copy.existingImportEntries"),
                ))
                .when(state.skipped_files > 0, |body| {
                    body.child(div().text_size(px(12.)).text_color(p.muted).child(
                        crate::localization::view(format!(
                            "{} unreadable or oversized files were skipped.",
                            state.skipped_files
                        )),
                    ))
                });
        }
        body = body.when(state.loading, |body| {
            body.child(
                div()
                    .text_color(p.muted)
                    .child(crate::localization::text(cx, "rs.ui.copy.loadingImport")),
            )
        });
        let mut frame = DialogFrame::new("host-import-frame", title.clone())
            .width(760.)
            .colors(DialogColors::workspace(self.data.dark))
            .content(body);
        if state.source.is_some() {
            frame = frame.action(
                Button::new("back-host-import")
                    .ghost()
                    .label("Back")
                    .disabled(state.loading || state.saving)
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(state) = this
                            .host_import
                            .as_mut()
                            .filter(|state| !state.loading && !state.saving)
                        {
                            state.source = None;
                            state.rows.clear();
                            state.keys.clear();
                            state.selected.clear();
                            state.selected_keys.clear();
                            state.file = None;
                            state.error = None;
                            state
                                .filter
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            state.refilter("");
                            cx.notify();
                        }
                    })),
            );
        }
        frame = frame.action(
            Button::new("cancel-host-import")
                .ghost()
                .label(if state.source.is_some() {
                    "Choose again"
                } else {
                    "Cancel"
                })
                .disabled(state.loading || state.saving)
                .on_click(cx.listener(|this, _, window, cx| {
                    if let Some(source) = this.host_import.as_ref().and_then(|state| state.source) {
                        this.pick_host_source(source, window, cx);
                    } else {
                        Modal::dismiss(window, cx);
                    }
                })),
        );
        if state.source.is_some() {
            frame = frame.action(
                Button::new("confirm-host-import")
                    .primary()
                    .label(if state.saving {
                        "Importing…".into()
                    } else {
                        format!("Import {}", state.selection_count())
                    })
                    .disabled(
                        state.loading
                            || state.saving
                            || state.selection_count() == 0
                            || !self.resources_ready(),
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.confirm_host_import(window, cx)),
                    ),
            );
        }
        Some(
            Modal::new("host-import-modal", title)
                .return_focus(&self.focus)
                .dismissible(!state.saving)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.host_import = None;
                    cx.notify();
                }))
                .on_confirm(cx.listener(|this, _, window, cx| this.confirm_host_import(window, cx)))
                .content(frame)
                .into_any_element(),
        )
    }
}
