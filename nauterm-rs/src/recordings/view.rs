use super::{Catalog, Entry, Event, Service, presentation};
use crate::{
    components::{
        Button, ButtonCustomVariant, DialogColors, DialogFrame, Modal,
        menu::{ContextMenuExt, DropdownMenu, MenuItem},
    },
    management::{CollectionView, SortOrder},
    palette::Palette,
    typography::FontWeights,
    ui::{DeleteCards, SelectCards},
};
use gpui_kit::{
    assets::IconName,
    base::input::{InputEvent, InputState},
    component::{Disableable, Icon, Sizable},
    prelude::*,
    *,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
mod table_frame;
use table_frame::TableFrame;

const TABLE_MARGIN: f32 = 10.;
const TABLE_INSET: f32 = 26.;
const PAGE_SIZE: usize = 40;
fn date_column() -> Div {
    div().w(px(112.)).flex_shrink_0()
}
fn user_column() -> Div {
    div().flex_1().min_w_0().flex_grow(3.)
}
fn host_column() -> Div {
    div().flex_1().min_w_0().flex_grow(4.)
}
fn actions_column() -> Div {
    div()
        .w(px(96.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
}

pub struct Logs {
    service: Service,
    search: Entity<InputState>,
    catalog: Rc<Catalog>,
    visible: Rc<Vec<usize>>,
    host_platforms: BTreeMap<String, crate::avatar::Platform>,
    selection: BTreeSet<String>,
    preferences: CollectionView,
    focus: FocusHandle,
    loading: bool,
    loading_more: bool,
    refresh_pending: bool,
    busy: bool,
    dark: bool,
    error: Option<String>,
    notice: Option<crate::components::message::Message>,
    deletion: Option<Vec<Entry>>,
    detail: Option<Entry>,
    events: Vec<Event>,
    detail_loading: bool,
    revision: u64,
    scroll: UniformListScrollHandle,
    _search: Subscription,
    _updates: Task<()>,
}
pub struct ReplayRequested(pub Entry);
impl EventEmitter<ReplayRequested> for Logs {}
fn date(time: i64) -> String {
    chrono::DateTime::from_timestamp_millis(time)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "Unknown time".into())
}
fn bytes(count: u64) -> String {
    if count >= 1024 * 1024 {
        format!("{:.1} MiB", count as f64 / 1048576.)
    } else if count >= 1024 {
        format!("{:.1} KiB", count as f64 / 1024.)
    } else {
        format!("{count} B")
    }
}
impl Logs {
    pub(crate) fn feedback(&self) -> Vec<(&'static str, crate::components::message::Message)> {
        let mut feedback = vec![];
        if let Some(warning) = self.catalog.warnings.last() {
            feedback.push((
                "catalog",
                crate::components::message::Message::error(warning.clone()),
            ));
        }
        if let Some(notice) = &self.notice {
            feedback.push(("notice", notice.clone()));
        }
        if let Some(error) = &self.error {
            feedback.push((
                "operation",
                crate::components::message::Message::error(error.clone()),
            ));
        }
        feedback
    }
    pub fn new(
        service: Service,
        search: Entity<InputState>,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&search, |view: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.rebuild_visible(cx);
                view.scroll.scroll_to_item(0, ScrollStrategy::Nearest);
                cx.notify();
            }
        });
        let notifications = service.notifications();
        let updates = cx.spawn(async move |view, cx| {
            while notifications.recv().await.is_ok() {
                if view.update(cx, |view, cx| view.refresh(cx)).is_err() {
                    break;
                }
            }
        });
        let mut view = Self {
            service,
            host_platforms: BTreeMap::new(),
            search,
            dark,
            catalog: Rc::new(Catalog {
                entries: vec![],
                active: BTreeSet::new(),
                warnings: vec![],
                storage_bytes: Some(0),
                next_offset: 0,
                has_more: false,
            }),
            visible: Rc::default(),
            selection: BTreeSet::new(),
            preferences: CollectionView {
                list: false,
                sort: SortOrder::NewestFirst,
                search_open: false,
            },
            focus: cx.focus_handle(),
            loading: false,
            loading_more: false,
            refresh_pending: false,
            busy: false,
            error: None,
            notice: None,
            deletion: None,
            detail: None,
            events: vec![],
            detail_loading: false,
            revision: 0,
            scroll: UniformListScrollHandle::new(),
            _search: subscription,
            _updates: updates,
        };
        view.refresh(cx);
        view
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.dark != dark {
            self.dark = dark;
            cx.notify();
        }
    }
    pub fn set_hosts(&mut self, hosts: &[crate::model::Host], cx: &mut Context<Self>) {
        let platforms = hosts
            .iter()
            .filter_map(|host| host.uuid.clone().map(|uuid| (uuid, host.platform.clone())))
            .collect();
        if self.host_platforms != platforms {
            self.host_platforms = platforms;
            cx.notify();
        }
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.loading_more || self.busy || self.deletion.is_some() {
            self.refresh_pending = true;
            return;
        }
        self.refresh_pending = false;
        self.loading = true;
        self.error = None;
        let service = self.service.clone();
        let limit = self.catalog.next_offset.max(PAGE_SIZE);
        cx.spawn(async move |view, cx| {
            let result = service.catalog(0, limit).await;
            let _ = view.update(cx, |view, cx| {
                view.loading = false;
                match result {
                    Ok(catalog) => {
                        view.catalog = Rc::new(catalog);
                        view.rebuild_visible(cx);
                        view.selection
                            .retain(|id| view.catalog.entries.iter().any(|entry| &entry.id == id));
                        if let Some(detail) = view.detail.as_ref() {
                            let entry = view
                                .catalog
                                .entries
                                .iter()
                                .find(|entry| entry.id == detail.id)
                                .cloned();
                            view.detail = entry;
                        }
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                if view.refresh_pending {
                    view.refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn load_more(&mut self, cx: &mut Context<Self>) {
        if self.loading
            || self.loading_more
            || self.busy
            || self.deletion.is_some()
            || !self.catalog.has_more
            || self.error.is_some()
        {
            return;
        }
        self.loading_more = true;
        let service = self.service.clone();
        let offset = self.catalog.next_offset;
        cx.spawn(async move |view, cx| {
            let result = service.catalog(offset, PAGE_SIZE).await;
            let _ = view.update(cx, |view, cx| {
                view.loading_more = false;
                match result {
                    Ok(page) => {
                        let mut catalog = (*view.catalog).clone();
                        catalog.append_page(page);
                        view.catalog = Rc::new(catalog);
                        view.rebuild_visible(cx);
                    }
                    Err(error) => view.error = Some(format!("Could not load more logs: {error}")),
                }
                if view.refresh_pending {
                    view.refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn rebuild_visible(&mut self, cx: &App) {
        let query = self.search.read(cx).value();
        self.visible = Rc::new(presentation::visible_indices(
            &self.catalog.entries,
            &query,
            self.preferences.sort,
        ));
    }
    fn show(&mut self, entry: Entry, cx: &mut Context<Self>) {
        self.revision += 1;
        let revision = self.revision;
        let id = entry.id.clone();
        self.detail = Some(entry);
        self.events.clear();
        self.detail_loading = true;
        let service = self.service.clone();
        cx.spawn(async move |view, cx| {
            let result = service.events(id).await;
            let _ = view.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                view.detail_loading = false;
                match result {
                    Ok(events) => view.events = events,
                    Err(error) => view.error = Some(error.to_string()),
                }
                if view.refresh_pending {
                    view.refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn request_delete(&mut self, all: bool, cx: &mut Context<Self>) {
        if self.loading || self.loading_more || self.busy {
            return;
        }
        if all && self.catalog.has_more {
            self.busy = true;
            self.notice = None;
            let service = self.service.clone();
            cx.spawn(async move |view, cx| {
                let result = service.finished_entries().await;
                let _ = view.update(cx, |view, cx| {
                    view.busy = false;
                    match result {
                        Ok(entries) if !entries.is_empty() => {
                            view.deletion = Some(entries);
                            view.error = None;
                        }
                        Ok(_) => {
                            view.notice = Some(crate::components::message::Message::info(
                                "There are no finished logs to delete.",
                            ))
                        }
                        Err(error) => view.error = Some(error.to_string()),
                    }
                    cx.notify();
                });
            })
            .detach();
            cx.notify();
            return;
        }
        let selected: Vec<_> = self
            .catalog
            .entries
            .iter()
            .filter(|entry| {
                (all || self.selection.contains(&entry.id))
                    && !self.catalog.active.contains(&entry.id)
            })
            .cloned()
            .collect();
        if selected.is_empty() {
            self.notice = Some(crate::components::message::Message::info(
                "There are no finished logs to delete. Close a session before deleting its log.",
            ));
        } else {
            self.deletion = Some(selected);
            self.error = None;
        }
        cx.notify();
    }
    fn delete(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(entries) = self.deletion.clone() else {
            return;
        };
        self.busy = true;
        self.notice = None;
        let service = self.service.clone();
        cx.spawn(async move |view, cx| {
            let result = service.delete(entries).await;
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                view.deletion = None;
                match result {
                    Ok(count) => {
                        view.notice = Some(crate::components::message::Message::info(format!(
                            "Deleted {count} log(s)."
                        )));
                        view.error = None;
                    }
                    Err(error) => {
                        view.notice = Some(crate::components::message::Message::error(
                            error.to_string(),
                        ))
                    }
                }
                // Batch failures can leave completed deletions; always reconcile.
                view.refresh(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn export(&mut self, entry: Entry, output: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.catalog.active.contains(&entry.id) {
            return;
        }
        self.busy = true;
        self.notice = None;
        self.error = None;
        let suggested = format!(
            "nauterm-{}.{}",
            entry.id,
            if output { "ansi" } else { "json" }
        );
        let picker =
            cx.prompt_for_new_path(&crate::file_dialog::export_directory(), Some(&suggested));
        let service = self.service.clone();
        cx.spawn_in(window, async move |view, cx| {
            let result = match picker.await {
                Ok(Ok(Some(path))) => service.export(entry, path, output).await.map(|_| true),
                Ok(Ok(None)) => Ok(false),
                _ => Err(anyhow::anyhow!(
                    "Could not open the export destination picker."
                )),
            };
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(true) => {
                        view.notice =
                            Some(crate::components::message::Message::info("Log exported."))
                    }
                    Ok(false) => {}
                    Err(error) => view.error = Some(error.to_string()),
                }
                if view.refresh_pending {
                    view.refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn preview(&mut self, entry: Entry, _: &mut Window, cx: &mut Context<Self>) {
        if self.catalog.active.contains(&entry.id)
            || self.busy
            || entry.capture_sha256.is_none()
            || entry.capture_file.is_empty()
        {
            return;
        }
        self.revision += 1;
        self.detail = None;
        self.error = None;
        cx.emit(ReplayRequested(entry));
        cx.notify();
    }
    fn export_text(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.catalog.active.contains(&entry.id) {
            return;
        }
        self.busy = true;
        self.notice = None;
        let name = format!("nauterm-{}.txt", entry.id);
        let picker = cx.prompt_for_new_path(&crate::file_dialog::export_directory(), Some(&name));
        let service = self.service.clone();
        cx.spawn_in(window, async move |view, cx| {
            let result = match picker.await {
                Ok(Ok(Some(path))) => service.export_readable(entry, path).await.map(|_| true),
                Ok(Ok(None)) => Ok(false),
                _ => Err(anyhow::anyhow!(
                    "Could not open the export destination picker."
                )),
            };
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(true) => {
                        view.notice = Some(crate::components::message::Message::info(
                            "Readable text exported.",
                        ))
                    }
                    Ok(false) => {}
                    Err(error) => view.error = Some(error.to_string()),
                }
                if view.refresh_pending {
                    view.refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .mx(px(TABLE_MARGIN))
            .px(px(TABLE_INSET))
            .h(px(50.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .child(
                date_column()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(crate::localization::text(cx, "common.label.date"))
                    .child(
                        Button::new("log-date-order")
                            .toolbar()
                            .ghost()
                            .w(px(26.))
                            .h(px(26.))
                            .p_0()
                            .icon_size(px(15.))
                            .tooltip("Sort by date")
                            .icon(if self.preferences.sort == SortOrder::NewestFirst {
                                IconName::ArrowDownWideNarrow
                            } else {
                                IconName::ArrowUpNarrowWide
                            })
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.preferences.sort =
                                    if view.preferences.sort == SortOrder::NewestFirst {
                                        SortOrder::OldestFirst
                                    } else {
                                        SortOrder::NewestFirst
                                    };
                                view.rebuild_visible(cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(user_column().child(crate::localization::text(cx, "common.label.user")))
            .child(host_column().child(crate::localization::text(cx, "common.label.host")))
            .child(
                actions_column()
                    .child(crate::localization::text(cx, "common.label.actions"))
                    .child(
                        Button::new("clear-logs")
                            .toolbar()
                            .ghost()
                            .icon(IconName::Trash)
                            .tooltip("Delete all finished terminal history and captures")
                            .disabled(self.loading || self.busy || self.catalog.entries.is_empty())
                            .on_click(cx.listener(|view, _, _, cx| view.request_delete(true, cx))),
                    ),
            )
            .into_any_element()
    }
    fn row(&self, entry: Entry, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.dark);
        let id = entry.id.clone();
        let row_id = SharedString::from(format!("log-{id}"));
        let clicked = entry.clone();
        let context_entry = entry.clone();
        let right_id = id.clone();
        let play = entry.clone();
        let remove = entry.clone();
        let selected = self.selection.contains(&id);
        let live = self.catalog.active.contains(&id);
        let can_read = !live && entry.capture_sha256.is_some();
        let owner = cx.weak_entity();
        let menu_owner = owner.clone();
        let (day, range) = presentation::date_range(&entry, crate::localization::language(cx));
        let (username, account) = presentation::account(&entry);
        let host_subtitle = presentation::subtitle(&entry);
        let host_icon = match presentation::kind(&entry) {
            presentation::Kind::Remote => crate::avatar::host(entry.title.trim())
                .platform(
                    entry
                        .host_uuid
                        .as_ref()
                        .and_then(|uuid| self.host_platforms.get(uuid))
                        .cloned()
                        .unwrap_or_default(),
                )
                .dimension(24.)
                .into_any_element(),
            kind => div()
                .size(px(24.))
                .rounded(px(6.))
                .bg(rgb(if kind == presentation::Kind::Serial {
                    0x7456c8
                } else {
                    0x075e92
                }))
                .text_color(rgb(0xffffff))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(if kind == presentation::Kind::Serial {
                        IconName::Cable
                    } else {
                        IconName::Terminal
                    })
                    .with_size(px(14.)),
                )
                .into_any_element(),
        };
        let row = Button::new(SharedString::from(format!("log-row-{id}")))
            .h_full()
            .font_weight(FontWeights::MEDIUM)
            .text_size(px(12.))
            .line_height(px(18.))
            .w_full()
            .p_0()
            .px(px(TABLE_INSET))
            .rounded(px(0.))
            .border_0()
            .content_start()
            .content_gap(px(0.))
            .focusable(false)
            .aria_selected(selected)
            .localized_accessibility_label(format!("Session log: {}", entry.title))
            .custom(
                ButtonCustomVariant::for_surface(p)
                    .color(
                        if selected {
                            p.surface
                                .blend(p.accent.alpha(if self.dark { 0.10 } else { 0.055 }))
                        } else {
                            p.surface
                        }
                        .into(),
                    )
                    .hover(rgb(if self.dark { 0x343434 } else { 0xfbfdfd }).into())
                    .highlight(p.accent.alpha(if self.dark { 0.10 } else { 0.06 }).into())
                    .active(p.accent.alpha(if self.dark { 0.18 } else { 0.12 }).into()),
            )
            .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                view.focus.focus(window, cx);
                if event.modifiers().platform || event.modifiers().control {
                    if !view.selection.remove(&id) {
                        view.selection.insert(id.clone());
                    }
                } else {
                    view.selection = BTreeSet::from([id.clone()]);
                }
                if event.click_count() == 2 && can_read {
                    view.preview(clicked.clone(), window, cx);
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |view, _, window, cx| {
                    view.focus.focus(window, cx);
                    if !view.selection.contains(&right_id) {
                        view.selection = BTreeSet::from([right_id.clone()]);
                    }
                    cx.notify();
                }),
            )
            .child(
                date_column()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(div().text_size(px(12.)).child(day))
                    .child(div().text_size(px(11.)).text_color(p.muted).child(range)),
            )
            .child(
                user_column()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .text_color(p.muted)
                    .child(
                        div()
                            .size(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::UserRound).with_size(px(16.))),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(div().text_color(p.text).truncate().child(username))
                            .child(div().text_size(px(11.)).truncate().child(account)),
                    ),
            )
            .child(
                host_column()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(host_icon)
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(div().truncate().child(entry.title.clone()))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(host_subtitle),
                            ),
                    ),
            )
            .child(
                actions_column()
                    .child(
                        Button::new((SharedString::from(format!("view-log-{}", entry.id)), 0usize))
                            .toolbar()
                            .ghost()
                            .icon(IconName::Play)
                            .tooltip("Replay terminal recording")
                            .disabled(!can_read)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                cx.stop_propagation();
                                view.preview(play.clone(), window, cx);
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("export-log-{}", entry.id)))
                            .toolbar()
                            .ghost()
                            .icon(IconName::Download)
                            .tooltip("Export terminal capture")
                            .disabled(!can_read || self.busy)
                            .dropdown_menu(move |menu, _, _| {
                                let text = owner.clone();
                                let raw = owner.clone();
                                let text_entry = entry.clone();
                                let raw_entry = entry.clone();
                                menu.width(232.)
                                    .item(
                                        MenuItem::localized("Readable text (.txt)")
                                            .icon(IconName::FileText)
                                            .on_click(move |_, window, cx| {
                                                let _ = text.update(cx, |view, cx| {
                                                    view.export_text(text_entry.clone(), window, cx)
                                                });
                                            }),
                                    )
                                    .item(
                                        MenuItem::localized("Raw terminal stream (.ansi)")
                                            .icon(IconName::Code)
                                            .on_click(move |_, window, cx| {
                                                let _ = raw.update(cx, |view, cx| {
                                                    view.export(raw_entry.clone(), true, window, cx)
                                                });
                                            }),
                                    )
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("delete-log-{}", remove.id)))
                            .toolbar()
                            .ghost()
                            .icon(IconName::Trash)
                            .tooltip("Delete log and capture")
                            .disabled(live || self.busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                cx.stop_propagation();
                                view.selection = BTreeSet::from([remove.id.clone()]);
                                view.request_delete(false, cx);
                            })),
                    ),
            );
        div()
            .id(row_id)
            .h(px(61.))
            .w_full()
            .flex_shrink_0()
            .relative()
            .border_color(p.border)
            .border_b_1()
            .child(row)
            .when(selected, |view| {
                view.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(12.))
                        .bottom(px(12.))
                        .w(px(2.))
                        .rounded(px(1.))
                        .bg(p.accent),
                )
            })
            .context_menu(move |menu, _, _| {
                let details = menu_owner.clone();
                let history = menu_owner.clone();
                let delete = menu_owner.clone();
                let details_entry = context_entry.clone();
                let history_entry = details_entry.clone();
                menu.item(
                    MenuItem::localized("rs.recordings.action.sessionDetails").on_click(
                        move |_, _, cx| {
                            let _ =
                                details.update(cx, |view, cx| view.show(details_entry.clone(), cx));
                        },
                    ),
                )
                .item(
                    MenuItem::localized("rs.recordings.action.exportHistory")
                        .disabled(live)
                        .on_click(move |_, window, cx| {
                            let _ = history.update(cx, |view, cx| {
                                view.export(history_entry.clone(), false, window, cx)
                            });
                        }),
                )
                .separator()
                .item(
                    MenuItem::localized("rs.recordings.action.deleteFinished")
                        .destructive(true)
                        .on_click(move |_, _, cx| {
                            let _ = delete.update(cx, |view, cx| view.request_delete(false, cx));
                        }),
                )
            })
            .into_any_element()
    }
}

impl Render for Logs {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.dark);
        let visible = self.visible.clone();
        let catalog = self.catalog.clone();
        let count = visible.len();
        let total = self
            .catalog
            .storage_bytes
            .map(bytes)
            .unwrap_or_else(|| "Unavailable".into());
        let owner = cx.weak_entity();
        let can_load =
            self.catalog.has_more && !self.loading && !self.loading_more && self.error.is_none();
        let extra_row = self.loading_more || (self.catalog.has_more && self.error.is_some());
        if count == 0 && can_load {
            let owner = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = owner.update(cx, |view, cx| view.load_more(cx));
            });
        }
        let list = uniform_list(
            "logs-table-list",
            count + usize::from(extra_row),
            move |range, _, cx| {
                let Some(owner) = owner.upgrade() else {
                    return vec![];
                };
                if can_load && range.end.saturating_add(4) >= count {
                    let owner = owner.clone();
                    cx.defer(move |cx| owner.update(cx, |view, cx| view.load_more(cx)));
                }
                range
                    .filter_map(|index| {
                        if index == count {
                            return Some(owner.update(cx, |view, cx| {
                                div()
                                    .h(px(61.))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(view.loading_more, |row| {
                                        row.child(crate::components::Spinner::new())
                                    })
                                    .when(!view.loading_more, |row| {
                                        row.child(
                                            Button::new("retry-log-page")
                                                .ghost()
                                                .label("Retry loading logs")
                                                .on_click(cx.listener(|view, _, _, cx| {
                                                    view.error = None;
                                                    view.load_more(cx);
                                                })),
                                        )
                                    })
                                    .into_any_element()
                            }));
                        }
                        let entry = catalog.entries.get(*visible.get(index)?)?.clone();
                        Some(owner.update(cx, |view, cx| view.row(entry, cx)))
                    })
                    .collect()
            },
        )
        .track_scroll(&self.scroll)
        .size_full();
        div().size_full().min_h_0().flex().flex_col().bg(p.background).text_color(p.text).text_size(px(12.)).track_focus(&self.focus).key_context("Nauterm")
            .on_action(cx.listener(|view, _: &SelectCards, window, cx| { if !crate::components::overlay::is_modal_open(window, cx) && !view.selection.is_empty() { view.selection = view.visible.iter().filter_map(|index| view.catalog.entries.get(*index)).map(|entry| entry.id.clone()).collect(); cx.notify(); } cx.stop_propagation(); }))
            .on_action(cx.listener(|view, _: &DeleteCards, window, cx| { if !crate::components::overlay::is_modal_open(window, cx) && !view.selection.is_empty() { view.request_delete(false, cx); } cx.stop_propagation(); }))
            .child(self.header(cx))
            .when(count > 0 || extra_row, |view| view.child(div().id("logs-table").relative().mx(px(TABLE_MARGIN)).flex_1().min_h_0().rounded(px(7.)).bg(p.surface)
                .child(TableFrame::new(div().size_full().overflow_hidden().child(crate::components::uniform_scrollbar("logs-table-scrollbar", list, &self.scroll)), p))))
            .when(count == 0 && !extra_row, |view| view.child(crate::components::EmptyState::new(IconName::Clock, if self.loading { "Loading logs…" } else { "No logs yet" }, self.dark).description("Session history appears here after opening a terminal. Enable output recording in Settings to save terminal contents.")))
            .child(div().h(px(30.)).px(px(20.)).flex_shrink_0().flex().items_center().gap(px(7.)).text_size(px(11.)).text_color(p.muted).child(Icon::new(IconName::HardDrive).with_size(px(14.))).child(crate::localization::view(format!("Capture storage  {total}"))))
            .when_some(self.detail.as_ref(), |view, entry| {
                let title = entry.title.clone();
                let content = div().flex().flex_col().gap(px(10.)).child(entry.target()).child(crate::localization::view(format!("Started: {}", date(entry.started_at)))).child(crate::localization::view(format!("Ended: {}", entry.ended_at.map(date).unwrap_or_else(|| "Live".into()))))
                    .when_some(entry.cwd.clone(), |view, cwd| view.child(crate::localization::view(format!("Directory: {cwd}"))))
                    .when(self.detail_loading, |view| view.child(crate::localization::text(cx, "rs.ui.copy.loadingEvents")))
                    .children(self.events.iter().map(|event| div().flex().flex_col().gap(px(4.)).child(div().text_size(px(11.)).text_color(p.muted).child(date(event.timestamp))).child(event.message.clone())));
                view.child(Modal::new("log-details", title.clone()).return_focus(&self.focus)
                    .on_dismiss(cx.listener(|view, _, _, cx| { view.detail = None; view.revision += 1; cx.notify(); }))
                    .content(DialogFrame::new("log-details-frame", title).width(560.).colors(DialogColors::workspace(self.dark)).content(content)
                        .action(Button::new("done-log-details").primary().label("Done").on_click(|_, window, cx| Modal::dismiss(window, cx)))))
            })
            .when_some(self.deletion.as_ref(), |view, entries| {
                let title = format!("Delete {} finished log(s)?", entries.len());
                view.child(crate::components::Dialog::new("delete-logs-dialog", title.clone()).return_focus(&self.focus).dismissible(!self.busy)
                    .on_dismiss(cx.listener(|view, _, _, cx| { view.deletion = None; if view.refresh_pending { view.refresh(cx); } cx.notify(); }))
                    .content(DialogFrame::new("delete-logs-frame", title).width(400.).colors(DialogColors::workspace(self.dark))
                        .content("The selected session history and recorded output will be permanently removed. Active sessions are kept.")
                        .action(Button::new("cancel-delete-logs").ghost().label("Cancel").disabled(self.busy).on_click(|_, window, cx| Modal::dismiss(window, cx)))
                        .action(Button::new("confirm-delete-logs").danger().label("Delete").disabled(self.busy).on_click(cx.listener(|view, _, _, cx| view.delete(cx))))))
            })
    }
}
