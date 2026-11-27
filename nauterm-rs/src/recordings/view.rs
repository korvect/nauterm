use super::{Catalog, Entry, Event, Service, presentation};
use crate::{
    components::{
        Button, ButtonCustomVariant, DialogColors, DialogFrame, Modal,
        menu::{ContextMenuExt, DropdownMenu, Menu, MenuItem},
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
    div().w(px(156.)).flex_shrink_0()
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
    catalog_error: Option<(String, bool)>,
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

fn capture_export_menu(menu: Menu, owner: &WeakEntity<Logs>, entry: &Entry) -> Menu {
    let text = owner.clone();
    let raw = owner.clone();
    let text_entry = entry.clone();
    let raw_entry = entry.clone();
    menu.width(232.)
        .item(
            MenuItem::localized("workspace.label.readableTextTxt")
                .icon(IconName::FileText)
                .on_click(move |_, window, cx| {
                    let _ = text.update(cx, |view, cx| {
                        view.export_text(text_entry.clone(), window, cx)
                    });
                }),
        )
        .item(
            MenuItem::localized("workspace.label.rawTerminalStreamAnsi")
                .icon(IconName::Code)
                .on_click(move |_, window, cx| {
                    let _ = raw.update(cx, |view, cx| {
                        view.export_raw(raw_entry.clone(), window, cx)
                    });
                }),
        )
}

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
            catalog_error: None,
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
        self.catalog_error = None;
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
                    Err(error) => view.catalog_error = Some((error.to_string(), false)),
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
            || self.catalog_error.is_some()
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
                    Err(error) => view.catalog_error = Some((error.to_string(), true)),
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
                                "workspace.logs.delete.noneFinished",
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
            let active_selected = self
                .catalog
                .active
                .iter()
                .any(|id| all || self.selection.contains(id));
            self.notice = Some(crate::components::message::Message::info(
                if active_selected {
                    "workspace.logs.delete.closeSessionsFirst"
                } else {
                    "workspace.logs.delete.noneFinished"
                },
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
    fn export_raw(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.catalog.active.contains(&entry.id) {
            return;
        }
        self.busy = true;
        self.notice = None;
        self.error = None;
        let suggested = format!("nauterm-{}.ansi", entry.id);
        let picker =
            cx.prompt_for_new_path(&crate::file_dialog::export_directory(), Some(&suggested));
        let service = self.service.clone();
        cx.spawn_in(window, async move |view, cx| {
            let result = match picker.await {
                Ok(Ok(Some(path))) => service.export_raw(entry, path).await.map(|_| true),
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
                            "workspace.logs.export.raw.success",
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
                            "workspace.logs.export.readable.success",
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
            .text_size(px(12.))
            .font_weight(FontWeights::MEDIUM)
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
                            .tooltip("workspace.label.sortByDate")
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
                            .tooltip("workspace.logs.delete.action")
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
        let can_read = !live && !entry.capture_file.is_empty() && entry.capture_sha256.is_some();
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
            .accessibility_label(crate::localization::format(
                cx,
                "workspace.logs.row.accessibility",
                &[("name", &entry.title)],
            ))
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
                    .line_height(px(15.))
                    .child(div().text_size(px(12.)).child(day))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .truncate()
                            .child(range),
                    ),
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
                            .tooltip("common.action.replay")
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
                            .tooltip("workspace.label.exportTerminalCapture")
                            .disabled(!can_read || self.busy)
                            .dropdown_menu(move |menu, _, _| {
                                capture_export_menu(menu, &owner, &entry)
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("delete-log-{}", remove.id)))
                            .toolbar()
                            .ghost()
                            .icon(IconName::Trash)
                            .tooltip("workspace.label.deleteLogAndCapture")
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
            .context_menu(move |menu, _, cx| {
                let replay = menu_owner.clone();
                let details = menu_owner.clone();
                let delete = menu_owner.clone();
                let details_entry = context_entry.clone();
                let replay_entry = context_entry.clone();
                let (blocked, delete_count, multiple) =
                    menu_owner.upgrade().map_or((true, 0, false), |owner| {
                        let view = owner.read(cx);
                        let count = view
                            .selection
                            .iter()
                            .filter(|id| !view.catalog.active.contains(*id))
                            .count();
                        (
                            view.busy || view.loading || view.loading_more,
                            count,
                            view.selection.len() > 1,
                        )
                    });
                menu.item(
                    MenuItem::localized("common.action.replay")
                        .icon(IconName::Play)
                        .disabled(!can_read || blocked)
                        .on_click(move |_, window, cx| {
                            let _ = replay.update(cx, |view, cx| {
                                view.preview(replay_entry.clone(), window, cx)
                            });
                        }),
                )
                .item(
                    MenuItem::localized("workspace.label.exportTerminalCapture")
                        .icon(IconName::Download)
                        .disabled(!can_read || blocked)
                        .submenu(capture_export_menu(
                            Menu::default(),
                            &menu_owner,
                            &context_entry,
                        )),
                )
                .item(
                    MenuItem::localized("workspace.logs.action.sessionDetails")
                        .icon(IconName::Info)
                        .on_click(move |_, _, cx| {
                            let _ =
                                details.update(cx, |view, cx| view.show(details_entry.clone(), cx));
                        }),
                )
                .separator()
                .item(
                    MenuItem::localized(if multiple {
                        "workspace.logs.action.deleteFinished"
                    } else {
                        "workspace.label.deleteLogAndCapture"
                    })
                    .icon(IconName::Trash)
                    .disabled(blocked || delete_count == 0)
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
        let can_load = self.catalog.has_more
            && !self.loading
            && !self.loading_more
            && self.catalog_error.is_none();
        let extra_row = self.loading_more;
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
                            return Some({
                                div()
                                    .h(px(61.))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(crate::components::Spinner::new())
                                    .into_any_element()
                            });
                        }
                        let entry = catalog.entries.get(*visible.get(index)?)?.clone();
                        Some(owner.update(cx, |view, cx| view.row(entry, cx)))
                    })
                    .collect()
            },
        )
        .track_scroll(&self.scroll)
        .size_full();
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(p.background)
            .text_color(p.text)
            .text_size(px(12.))
            .track_focus(&self.focus)
            .key_context("Nauterm")
            .on_action(cx.listener(|view, _: &SelectCards, window, cx| {
                if !crate::components::overlay::is_modal_open(window, cx)
                    && !view.selection.is_empty()
                {
                    view.selection = view
                        .visible
                        .iter()
                        .filter_map(|index| view.catalog.entries.get(*index))
                        .map(|entry| entry.id.clone())
                        .collect();
                    cx.notify();
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|view, _: &DeleteCards, window, cx| {
                if !crate::components::overlay::is_modal_open(window, cx)
                    && !view.selection.is_empty()
                {
                    view.request_delete(false, cx);
                }
                cx.stop_propagation();
            }))
            .child(self.header(cx))
            .child(
                div()
                    .id("logs-table")
                    .relative()
                    .mx(px(TABLE_MARGIN))
                    .flex_1()
                    .min_h_0()
                    .rounded(px(7.))
                    .bg(p.surface)
                    .child(TableFrame::new(
                        div()
                            .size_full()
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .when_some(self.catalog_error.clone(), |view, (error, next_page)| {
                                view.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap(px(6.))
                                        .p(px(12.))
                                        .child(crate::localization::view(
                                            "workspace.logs.load.unavailable",
                                        ))
                                        .child(error)
                                        .child(
                                            Button::new("retry-log-catalog")
                                                .ghost()
                                                .label("workspace.logs.load.retry")
                                                .on_click(cx.listener(move |view, _, _, cx| {
                                                    view.catalog_error = None;
                                                    if next_page {
                                                        view.load_more(cx);
                                                    } else {
                                                        view.refresh(cx);
                                                    }
                                                })),
                                        ),
                                )
                            })
                            .when(
                                count > 0 || extra_row || self.catalog_error.is_none(),
                                |view| {
                                    view.child(if count > 0 || extra_row {
                                        crate::components::uniform_scrollbar(
                                            "logs-table-scrollbar",
                                            list,
                                            &self.scroll,
                                        )
                                        .into_any_element()
                                    } else {
                                        crate::components::EmptyState::new(
                                            IconName::Clock,
                                            if self.loading {
                                                "workspace.logs.load.loading"
                                            } else {
                                                "workspace.label.noTerminalSessionsRecorded"
                                            },
                                            self.dark,
                                        )
                                        .into_any_element()
                                    })
                                },
                            ),
                        p,
                    )),
            )
            .child(
                div()
                    .h(px(30.))
                    .px(px(20.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .child(Icon::new(IconName::HardDrive).with_size(px(14.)))
                    .child(crate::localization::format(
                        cx,
                        "workspace.logs.storage.size",
                        &[("size", &total)],
                    )),
            )
            .when_some(self.detail.as_ref(), |view, entry| {
                use crate::localization::{format_for, language, text_for};
                let language = language(cx);
                let title = "workspace.logs.action.sessionDetails";
                let ended = entry.ended_at.map_or_else(
                    || text_for(language, "workspace.logs.details.live"),
                    |time| {
                        format_for(
                            language,
                            "workspace.logs.details.ended",
                            &[("time", &date(time))],
                        )
                    },
                );
                let content = div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .font_weight(FontWeights::SEMIBOLD)
                            .child(entry.title.clone()),
                    )
                    .child(if entry.host.is_some() {
                        entry.target().into()
                    } else {
                        text_for(language, "workspace.logs.details.localTerminal")
                    })
                    .child(format_for(
                        language,
                        "workspace.logs.details.started",
                        &[("time", &date(entry.started_at))],
                    ))
                    .child(ended)
                    .when_some(entry.cwd.as_deref(), |view, cwd| {
                        view.child(format_for(
                            language,
                            "workspace.logs.details.directory",
                            &[("cwd", cwd)],
                        ))
                    })
                    .when(self.detail_loading, |view| {
                        view.child(text_for(language, "workspace.logs.details.loadingEvents"))
                    })
                    .children(self.events.iter().map(|event| {
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .child(date(event.timestamp)),
                            )
                            .child(event.message.clone())
                    }));
                view.child(
                    Modal::new("log-details", title)
                        .return_focus(&self.focus)
                        .on_dismiss(cx.listener(|view, _, _, cx| {
                            view.detail = None;
                            view.revision += 1;
                            cx.notify();
                        }))
                        .content(
                            DialogFrame::new("log-details-frame", title)
                                .width(560.)
                                .colors(DialogColors::workspace(self.dark))
                                .content(content)
                                .action(
                                    Button::new("done-log-details")
                                        .primary()
                                        .label("common.action.done")
                                        .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                                ),
                        ),
                )
            })
            .when_some(self.deletion.as_ref(), |view, entries| {
                use crate::localization::{format_for, language, text};
                let title = text(
                    cx,
                    if entries.len() == 1 {
                        "workspace.dialog.deleteTerminalHistory"
                    } else {
                        "workspace.dialog.deleteFinishedLogs"
                    },
                );
                view.child(
                    crate::components::Dialog::new("delete-logs-dialog", title.clone())
                        .return_focus(&self.focus)
                        .dismissible(!self.busy)
                        .on_dismiss(cx.listener(|view, _, _, cx| {
                            view.deletion = None;
                            if view.refresh_pending {
                                view.refresh(cx);
                            }
                            cx.notify();
                        }))
                        .content(
                            DialogFrame::new("delete-logs-frame", title)
                                .width(400.)
                                .colors(DialogColors::workspace(self.dark))
                                .content(format_for(
                                    language(cx),
                                    "workspace.dialog.deleteFinishedLogs.description",
                                    &[("count", &entries.len().to_string())],
                                ))
                                .action(
                                    Button::new("cancel-delete-logs")
                                        .ghost()
                                        .label("common.action.cancel")
                                        .disabled(self.busy)
                                        .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                                )
                                .action(
                                    Button::new("confirm-delete-logs")
                                        .danger()
                                        .label("common.action.delete")
                                        .disabled(self.busy)
                                        .on_click(cx.listener(|view, _, _, cx| view.delete(cx))),
                                ),
                        ),
                )
            })
    }
}
