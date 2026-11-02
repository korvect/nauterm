use super::{Document, Entry};
use crate::{
    components::{
        Button, DialogColors, DialogFrame, Modal, ScrollbarsExt,
        menu::{ContextMenuExt, DropdownMenu, MenuItem},
    },
    management::{CollectionView, sort_menu},
    palette::Palette,
    ui::{DeleteCards, SelectCards},
};
use gpui_kit::{
    assets::IconName,
    base::input::{InputEvent, InputState},
    component::{Disableable, Icon, Sizable},
    prelude::*,
    *,
};
use std::{collections::BTreeSet, path::PathBuf};

pub struct ConvertToHost(pub String, pub u16);
pub struct KnownHosts {
    path: PathBuf,
    document: Document,
    search: Entity<InputState>,
    selection: BTreeSet<usize>,
    navigation: crate::management::navigation::Navigation<usize>,
    preferences: CollectionView,
    focus: FocusHandle,
    loading: bool,
    busy: bool,
    dark: bool,
    parent_width: Option<f32>,
    error: Option<String>,
    notice: Option<String>,
    deletion: Option<Vec<Entry>>,
    _search: Subscription,
}
impl EventEmitter<ConvertToHost> for KnownHosts {}
impl KnownHosts {
    pub(crate) fn feedback(&self) -> Vec<(&'static str, crate::components::message::Message)> {
        let mut feedback = vec![];
        if let Some(notice) = &self.notice {
            feedback.push((
                "notice",
                crate::components::message::Message::info(notice.clone()),
            ));
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
        path: PathBuf,
        search: Entity<InputState>,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        let mut view = Self {
            path,
            search,
            dark,
            parent_width: None,
            document: Document::default(),
            selection: BTreeSet::new(),
            navigation: Default::default(),
            preferences: CollectionView::default(),
            focus: cx.focus_handle(),
            loading: false,
            busy: false,
            error: None,
            notice: None,
            deletion: None,
            _search: subscription,
        };
        view.refresh(cx);
        view
    }
    pub(crate) fn set_parent_width(&mut self, width: f32, cx: &mut Context<Self>) {
        if self.parent_width != Some(width) {
            self.parent_width = Some(width);
            cx.notify();
        }
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.dark != dark {
            self.dark = dark;
            cx.notify();
        }
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.loading || self.deletion.is_some() {
            return;
        }
        self.loading = true;
        self.error = None;
        let path = self.path.clone();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move { Document::read(&path) })
                .await;
            let _ = view.update(cx, |view, cx| {
                view.loading = false;
                match result {
                    Ok(document) => {
                        view.document = document;
                        view.selection.clear();
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn visible(&self, cx: &App) -> Vec<Entry> {
        let query = self.search.read(cx).value();
        let mut entries: Vec<_> = self
            .document
            .entries
            .iter()
            .filter(|entry| entry.matches(&query))
            .cloned()
            .collect();
        entries.sort_by(|a, b| {
            self.preferences
                .sort
                .compare((&a.pattern, a.index as u64), (&b.pattern, b.index as u64))
        });
        entries
    }
    fn navigate_cards(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::management::navigation::{Key, Section};
        if !self.focus.is_focused(window)
            || self.loading
            || self.busy
            || crate::components::overlay::is_modal_open(window, cx)
        {
            return;
        }
        let Some(Key::Move(direction)) = Key::read(&event.keystroke) else {
            return;
        };
        let visible: Vec<_> = self.visible(cx).iter().map(|entry| entry.index).collect();
        let width = f32::from(window.viewport_size().width);
        let columns = crate::layout::CardGrid::new(
            self.parent_width
                .unwrap_or_else(|| width - crate::layout::sidebar_width(width))
                - 52.,
            visible.len(),
            self.preferences.list,
        )
        .columns as usize;
        if self
            .navigation
            .navigate(
                &mut self.selection,
                &[Section::new(visible, columns)],
                direction,
                event.keystroke.modifiers,
            )
            .is_some()
        {
            cx.stop_propagation();
            cx.notify();
        }
    }
    fn request_delete(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.loading {
            return;
        }
        let selected: Vec<_> = self
            .document
            .entries
            .iter()
            .filter(|entry| self.selection.contains(&entry.index))
            .cloned()
            .collect();
        if !selected.is_empty() {
            self.deletion = Some(selected);
            self.error = None;
            cx.notify();
        }
    }
    fn delete(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(selected) = self.deletion.clone() else {
            return;
        };
        let path = self.path.clone();
        self.busy = true;
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    let previous = Document::read(&path)?;
                    let next = previous.remove(&selected)?;
                    next.publish(&previous, &path)?;
                    anyhow::Ok(next)
                })
                .await;
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(document) => {
                        view.document = document;
                        view.selection.clear();
                        view.deletion = None;
                        view.notice = Some("Known hosts deleted.".into());
                        view.error = None;
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.loading {
            return;
        }
        self.busy = true;
        self.error = None;
        let path = self.path.clone();
        let picker = crate::file_dialog::open(
            PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Import known_hosts".into()),
            },
            Some(crate::file_dialog::InitialDirectory::SshImport),
            window,
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let picked = match picker.await {
                Ok(Ok(Some(mut paths))) => paths.pop(),
                Ok(Ok(None)) => None,
                _ => {
                    let _ = view.update(cx, |view, cx| {
                        view.busy = false;
                        view.error = Some("Could not open the import file picker.".into());
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(picked) = picked else {
                let _ = view.update(cx, |view, cx| {
                    view.busy = false;
                    cx.notify();
                });
                return;
            };
            let result = cx
                .background_spawn(async move {
                    anyhow::ensure!(picked.is_file(), "Choose a regular known_hosts file.");
                    let imported = Document::read(&picked)?;
                    let previous = Document::read(&path)?;
                    let (next, added) = previous.merge(&imported)?;
                    if added > 0 {
                        next.publish(&previous, &path)?;
                    }
                    anyhow::Ok((next, added))
                })
                .await;
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok((document, added)) => {
                        view.document = document;
                        view.selection.clear();
                        view.notice = Some(if added == 0 {
                            "No new known hosts to import.".into()
                        } else {
                            format!("Imported {added} known host(s).")
                        });
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn card(&self, entry: Entry, cx: &mut Context<Self>) -> AnyElement {
        let index = entry.index;
        let owner = cx.weak_entity();
        let endpoint = entry.endpoint.clone();
        let detail = entry.detail();
        crate::components::item_card(
            ("known-host", index),
            self.dark,
            self.selection.contains(&index),
            self.preferences.list,
        )
        .list_menu_hint(self.preferences.list)
        .accessibility_label(entry.pattern.clone())
        .reveal_on_layout(self.navigation.take_reveal(index))
        .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
            cx.stop_propagation();
            view.focus.focus(window, cx);
            let visible: Vec<_> = view.visible(cx).iter().map(|entry| entry.index).collect();
            view.navigation
                .select(&mut view.selection, &visible, index, event.modifiers());
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |view, _, window, cx| {
                view.focus.focus(window, cx);
                if !view.selection.contains(&index) {
                    view.selection = BTreeSet::from([index]);
                }
                cx.notify();
            }),
        )
        .child(
            div()
                .size(px(40.))
                .flex_shrink_0()
                .rounded(px(10.))
                .bg(rgb(0x096b9e))
                .text_color(rgb(0xffffff))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(IconName::FingerprintPattern).with_size(px(20.))),
        )
        .child(crate::components::card_text(
            entry.pattern,
            detail,
            self.dark,
        ))
        .context_menu(move |menu, _, cx| {
            let single = owner
                .upgrade()
                .is_some_and(|view| view.read(cx).selection.len() == 1);
            let convert = owner.clone();
            let delete = owner.clone();
            let endpoint = endpoint.clone();
            menu.card_menu()
                .when(single, |menu| {
                    menu.item(
                        MenuItem::localized("Convert to Host")
                            .icon(IconName::Server)
                            .disabled(!single || endpoint.is_none())
                            .on_click(move |_, _, cx| {
                                if let Some((host, port)) = &endpoint {
                                    let _ = convert.update(cx, |_, cx| {
                                        cx.emit(ConvertToHost(host.clone(), *port))
                                    });
                                }
                            }),
                    )
                })
                .separator()
                .item(
                    MenuItem::localized("Delete")
                        .destructive(true)
                        .icon(IconName::Trash)
                        .on_click(move |_, _, cx| {
                            let _ = delete.update(cx, |view, cx| view.request_delete(cx));
                        }),
                )
        })
        .into_any_element()
    }
}
impl Render for KnownHosts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.dark);
        let entries = self.visible(cx);
        let width = f32::from(window.viewport_size().width);
        let grid = crate::layout::CardGrid::new(
            self.parent_width
                .unwrap_or_else(|| width - crate::layout::sidebar_width(width))
                - 52.,
            entries.len(),
            self.preferences.list,
        );
        let sort = self.preferences.sort;
        let search_open = self.preferences.search_open || !self.search.read(cx).value().is_empty();
        let collapse_labels = crate::components::management::toolbar_labels_collapsed(
            self.parent_width
                .unwrap_or_else(|| width - crate::layout::sidebar_width(width)),
            &[("Import", false)],
            if search_open { 2 } else { 3 },
            search_open,
            window,
        );
        let owner = cx.weak_entity();
        div().size_full().min_h_0().relative().flex().flex_col().track_focus(&self.focus).key_context("Nauterm")
            .on_key_down(cx.listener(Self::navigate_cards))
            .on_action(cx.listener(|view, _: &SelectCards, window, cx| {
                if !crate::components::overlay::is_modal_open(window, cx) && !view.selection.is_empty() {
                    view.selection = view.visible(cx).iter().map(|entry| entry.index).collect(); cx.notify();
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|view, _: &DeleteCards, window, cx| {
                if !crate::components::overlay::is_modal_open(window, cx) { view.request_delete(cx); }
                cx.stop_propagation();
            }))
            .child(crate::components::management::toolbar(self.dark)
                .child(Button::new("import-known-hosts").toolbar().label("Import").when(collapse_labels, |button| button.icon(IconName::Upload).label_collapsed(true)).disabled(self.loading || self.busy).on_click(cx.listener(|view, _, window, cx| view.import(window, cx))))
                .child(div().flex_1())
                .child(Button::new("sort-known-hosts").toolbar().ghost().icon(sort.icon()).tooltip(sort.label()).dropdown_menu(move |menu, _, _| {
                    let owner = owner.clone();
                    sort_menu(menu, sort, move |sort, _, cx| { let _ = owner.update(cx, |view, cx| { view.preferences.sort = sort; cx.notify(); }); })
                }))
                .child({ let owner = cx.weak_entity(); crate::components::management::toolbar_search("known-hosts-search", &self.search, self.preferences.search_open, move |open, window, cx| { let _ = owner.update(cx, |view, cx| { view.preferences.search_open = open; if open { view.search.update(cx, |input, cx| input.focus(window, cx)); } else { view.focus.focus(window, cx); } cx.notify(); }); }, cx) })
                .child(Button::new("known-hosts-view").toolbar().ghost().icon(if self.preferences.list { IconName::LayoutGrid } else { IconName::List }).tooltip("Switch list/grid view").on_click(cx.listener(|view, _, _, cx| { view.preferences.list = !view.preferences.list; cx.notify(); }))))
            .when(self.document.invalid_lines > 0, |view| view.child(div().px(px(26.)).text_size(px(12.)).text_color(p.muted).child(crate::localization::view(format!("{} unrecognized line(s) are preserved in the file.", self.document.invalid_lines)))))
            .child(div().id("known-hosts-scroll").flex_1().min_h_0().overflow_y_scroll().scrollbars().map(crate::components::management::collection_padding).flex().flex_col()
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| { view.selection.clear(); view.focus.focus(window, cx); cx.notify(); }))
                .when(!entries.is_empty(), |view| view.child(crate::components::management::section_title("Known Hosts", cx).mb(px(crate::components::management::COLLECTION_TITLE_GAP))))
                .child(crate::components::card_grid(grid).children(entries.iter().cloned().map(|entry| self.card(entry, cx))))
                .when(entries.is_empty(), |view| view.child({
                    let empty = crate::components::EmptyState::new(IconName::FingerprintPattern, if self.loading { "Loading known hosts…" } else if self.search.read(cx).value().is_empty() { "No known hosts yet" } else { "No known hosts found" }, self.dark)
                        .description("Saved host keys let you verify the identity of your SSH servers.");
                    if !self.loading && self.search.read(cx).value().is_empty() { empty.action(Button::new("empty-import-known-hosts").primary().icon(crate::assets::icon(IconName::Plus)).label("Import known hosts").disabled(self.busy).on_click(cx.listener(|view, _, window, cx| view.import(window, cx)))) } else { empty }
                })))
            .when_some(self.deletion.as_ref(), |view, selected| {
                let title = format!("Delete {} known host(s)?", selected.len());
                view.child(crate::components::Dialog::new("delete-known-hosts-modal", title.clone()).return_focus(&self.focus).dismissible(!self.busy)
                    .on_dismiss(cx.listener(|view, _, _, cx| { view.deletion = None; cx.notify(); }))
                    .content(DialogFrame::new("delete-known-hosts-dialog", title).width(380.).colors(DialogColors::workspace(self.dark))
                        .content("Future connections will need to verify these host keys again. Existing sessions are not closed.")
                        .action(Button::new("cancel-delete-known-hosts").ghost().label("Cancel").disabled(self.busy).on_click(|_, window, cx| Modal::dismiss(window, cx)))
                        .action(Button::new("confirm-delete-known-hosts").danger().label("Delete").disabled(self.busy).on_click(cx.listener(|view, _, _, cx| view.delete(cx))))))
            })
    }
}
