//! A shared host-selection page for connection drawers and SFTP consumers.
use crate::components::menu::DropdownMenu;
use crate::management::SortOrder as Order;
use crate::{
    avatar,
    components::{Button, Input, ScrollbarsExt},
    model::{AppState, Host, Id},
    palette::Palette,
    typography::FontWeights,
};
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::{
    assets::IconName,
    component::{Icon, Sizable},
    prelude::*,
    *,
};
use std::collections::BTreeSet;

pub enum PickerEvent {
    Back,
    Selected(Id),
    UseLocal,
}
pub struct HostPicker {
    data: AppState,
    search: Entity<InputState>,
    group: Option<Id>,
    tags: BTreeSet<String>,
    tag_filter: Entity<crate::tags::picker::Picker>,
    order: Order,
    dark: bool,
    focus: FocusHandle,
    allow_local: bool,
    _search: Subscription,
    _tags: Subscription,
}
impl EventEmitter<PickerEvent> for HostPicker {}
impl HostPicker {
    pub fn new(mut data: AppState, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::tags::materialize(&mut data);
        let tag_filter = cx.new(|cx| {
            crate::tags::picker::Picker::new(
                crate::tags::picker::Mode::Filter,
                data.tags.clone(),
                BTreeSet::new(),
                window,
                cx,
            )
            .filter_only()
        });
        let tag_events = cx.subscribe(
            &tag_filter,
            |this: &mut Self, _, event: &crate::tags::picker::Event, cx| {
                if let crate::tags::picker::Event::Selection(ids) = event {
                    this.tags = crate::tags::names(&this.data.tags, ids)
                        .into_iter()
                        .collect();
                    cx.notify();
                }
            },
        );
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search hosts or tags"));
        let subscription = cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. })
                && let Some(host) = this.hosts(cx).first()
            {
                cx.emit(PickerEvent::Selected(host.id));
            }
            cx.notify();
        });
        Self {
            dark: data.dark,
            focus: cx.focus_handle(),
            allow_local: false,
            data,
            search,
            group: None,
            tags: BTreeSet::new(),
            tag_filter,
            order: Order::NameAscending,
            _search: subscription,
            _tags: tag_events,
        }
    }
    pub fn contains_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
    }
    pub fn allow_local(mut self) -> Self {
        self.allow_local = true;
        self
    }
    pub fn set_allow_local(&mut self, allow: bool, cx: &mut Context<Self>) {
        self.allow_local = allow;
        cx.notify();
    }
    pub fn set_data(&mut self, mut data: AppState, cx: &mut Context<Self>) {
        crate::tags::materialize(&mut data);
        if self.data.hosts == data.hosts
            && self.data.groups == data.groups
            && self.data.tags == data.tags
            && self.dark == data.dark
        {
            return;
        }
        self.dark = data.dark;
        self.data = data;
        self.tag_filter.update(cx, |picker, cx| {
            picker.set_catalog(&self.data.tags, false, cx)
        });
        self.tags = crate::tags::names(&self.data.tags, self.tag_filter.read(cx).selected())
            .into_iter()
            .collect();
        if self
            .group
            .is_some_and(|id| !self.data.groups.iter().any(|group| group.id == id))
        {
            self.group = None;
        }
        cx.notify();
    }
    fn back(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.group {
            self.group = self
                .data
                .groups
                .iter()
                .find(|group| group.id == id)
                .and_then(|group| group.parent);
            cx.notify();
        } else {
            cx.emit(PickerEvent::Back);
        }
    }
    fn hosts(&self, cx: &App) -> Vec<&Host> {
        matching_hosts(
            &self.data,
            &self.search.read(cx).value(),
            self.group,
            &self.tags,
            self.order,
        )
    }
    fn path(&self) -> String {
        let mut names = vec![];
        let mut current = self.group;
        let mut seen = BTreeSet::new();
        while let Some(id) = current.filter(|id| seen.insert(*id)) {
            let Some(group) = self.data.groups.iter().find(|group| group.id == id) else {
                break;
            };
            names.push(group.name.as_str());
            current = group.parent;
        }
        names.reverse();
        names.insert(0, "Vaults");
        names.join(" / ")
    }
    fn row(&self, id: String, name: String, detail: String, leading: AnyElement) -> Button {
        Button::new(SharedString::from(id))
            .ghost()
            .h(px(52.))
            .mb(px(4.))
            .w_full()
            .px(px(8.))
            .accessibility_label(name.clone())
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(leading)
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
                                    .w_full()
                                    .truncate()
                                    .text_size(px(13.))
                                    .font_weight(FontWeights::MEDIUM)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .truncate()
                                    .text_size(px(12.))
                                    .text_color(Palette::new(self.dark).muted)
                                    .child(detail),
                            ),
                    ),
            )
    }
}

fn matching_hosts<'a>(
    data: &'a AppState,
    query: &str,
    group: Option<Id>,
    tags: &BTreeSet<String>,
    order: Order,
) -> Vec<&'a Host> {
    let mut hosts: Vec<_> = data
        .hosts
        .iter()
        .filter(|host| {
            host.supports_ssh()
                && group.is_none_or(|id| host.group_id == Some(id))
                && (tags.is_empty() || host.tags.iter().any(|tag| tags.contains(tag)))
                && data.host_matches(host, query)
        })
        .collect();
    hosts.sort_by(|a, b| order.compare((&a.name, a.id), (&b.name, b.id)));
    hosts
}
impl Render for HostPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.dark);
        let query = self.search.read(cx).value().to_string();
        let mut groups: Vec<_> = self
            .data
            .groups
            .iter()
            .filter(|group| {
                group.parent == self.group
                    && !query
                        .split_whitespace()
                        .any(|token| token.contains(':') || token.contains('@'))
                    && group.name.to_lowercase().contains(&query.to_lowercase())
            })
            .cloned()
            .collect();
        groups.sort_by(|a, b| self.order.compare((&a.name, a.id), (&b.name, b.id)));
        let has_groups = !groups.is_empty();
        let hosts: Vec<_> = self.hosts(cx).into_iter().cloned().collect();
        let sort_owner = cx.entity().downgrade();
        let no_results = groups.is_empty() && hosts.is_empty();
        div()
            .size_full()
            .track_focus(&self.focus)
            .min_h_0()
            .bg(p.background)
            .text_color(p.text)
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(58.))
                    .flex_shrink_0()
                    .bg(p.surface)
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        Button::new("picker-back")
                            .toolbar()
                            .ghost()
                            .icon(IconName::ArrowLeft)
                            .tooltip("Back")
                            .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeights::MEDIUM)
                                    .child(crate::localization::text(
                                        cx,
                                        "common.label.selectHost",
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(self.path()),
                            ),
                    )
                    .when(self.allow_local, |view| {
                        view.child(
                            Button::new("picker-use-local")
                                .toolbar()
                                .accent()
                                .label("Local")
                                .icon(IconName::Folder)
                                .on_click(
                                    cx.listener(|_, _, _, cx| cx.emit(PickerEvent::UseLocal)),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(42.))
                    .flex_shrink_0()
                    .px(px(10.))
                    .pb(px(8.))
                    .bg(p.surface)
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.search)
                                .appearance(false)
                                .text_size(px(12.))
                                .prefix(Icon::new(IconName::Search).with_size(px(16.)))
                                .aria_label(crate::localization::text(cx, "Search hosts or tags"))
                                .cleanable(true),
                        ),
                    )
                    .child(self.tag_filter.clone())
                    .child(
                        Button::new("picker-sort")
                            .toolbar()
                            .ghost()
                            .icon(self.order.icon())
                            .tooltip(self.order.label())
                            .dropdown_menu(move |menu, _, cx| {
                                let order = sort_owner
                                    .upgrade()
                                    .map(|view| view.read(cx).order)
                                    .unwrap_or_default();
                                let owner = sort_owner.clone();
                                crate::management::sort_menu(menu, order, move |order, _, cx| {
                                    let _ = owner.update(cx, |this, cx| {
                                        this.order = order;
                                        cx.notify();
                                    });
                                })
                            }),
                    ),
            )
            .child(
                div()
                    .id("host-picker-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .scrollbars()
                    .px(px(30.))
                    .pt(px(20.))
                    .pb(px(24.))
                    .when(!groups.is_empty(), |view| {
                        view.child(
                            div()
                                .mb(px(16.))
                                .font_weight(FontWeights::MEDIUM)
                                .child(crate::localization::text(cx, "common.label.groups")),
                        )
                    })
                    .children(groups.into_iter().map(|group| {
                        let id = group.id;
                        let count = self
                            .data
                            .hosts
                            .iter()
                            .filter(|host| host.supports_ssh() && host.group_id == Some(id))
                            .count();
                        let leading = crate::avatar::group().into_any_element();
                        self.row(
                            format!("pick-group-{id}"),
                            group.name,
                            format!("{count} Hosts"),
                            leading,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.group = Some(id);
                            cx.notify();
                        }))
                    }))
                    .when(!hosts.is_empty(), |view| {
                        view.child(
                            div()
                                .mt(px(if has_groups { 26. } else { 0. }))
                                .mb(px(16.))
                                .font_weight(FontWeights::MEDIUM)
                                .child(crate::localization::text(cx, "common.label.hosts")),
                        )
                    })
                    .children(hosts.into_iter().map(|host| {
                        let id = host.id;
                        self.row(
                            format!("pick-host-{id}"),
                            host.name.clone(),
                            format!("{}:{}", host.address, host.port),
                            avatar::host(&host.name)
                                .platform(host.platform.clone())
                                .into_any_element(),
                        )
                        .on_click(
                            cx.listener(move |_, _, _, cx| cx.emit(PickerEvent::Selected(id))),
                        )
                    }))
                    .when(no_results, |view| {
                        view.child(div().mt(px(50.)).text_color(p.muted).text_center().child(
                            crate::localization::text(cx, "common.label.noMatchingHosts"),
                        ))
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{AppState, BTreeSet, Host, Order, matching_hosts};
    #[test]
    fn ssh_consumers_never_offer_saved_local_hosts() {
        let local = Host {
            id: 2,
            name: "Local fixture".into(),
            kind: crate::model::HostKind::Local,
            ..Default::default()
        };
        let remote = Host {
            id: 3,
            name: "Remote fixture".into(),
            address: "fixture.invalid".into(),
            port: 22,
            ..Default::default()
        };
        let data = AppState {
            hosts: vec![local, remote],
            ..Default::default()
        };
        let result = matching_hosts(
            &data,
            "fixture",
            None,
            &BTreeSet::new(),
            Order::NameAscending,
        );
        assert_eq!(result.iter().map(|host| host.id).collect::<Vec<_>>(), [3]);
        assert!(
            matching_hosts(
                &data,
                "type:local",
                None,
                &BTreeSet::new(),
                Order::NameAscending
            )
            .is_empty()
        );
    }
    #[test]
    fn picker_scope_search_tags_and_sort_follow_reference() {
        let data = AppState {
            hosts: vec![
                Host {
                    connections: Default::default(),
                    uuid: None,
                    platform: Default::default(),
                    kind: Default::default(),
                    shell_path: None,
                    work_dir: None,
                    ssh: Default::default(),
                    id: 2,
                    name: "Zebra".into(),
                    address: "one.test".into(),
                    port: 22,
                    username: "fixture".into(),
                    group_id: Some(4),
                    tags: vec!["lab".into()],
                    credential: None,
                    proxy_id: None,
                    theme: None,
                },
                Host {
                    connections: Default::default(),
                    uuid: None,
                    platform: Default::default(),
                    kind: Default::default(),
                    shell_path: None,
                    work_dir: None,
                    ssh: Default::default(),
                    id: 3,
                    name: "Alpha".into(),
                    address: "two.test".into(),
                    port: 22,
                    username: "other".into(),
                    group_id: None,
                    tags: vec![],
                    credential: None,
                    proxy_id: None,
                    theme: None,
                },
            ],
            ..Default::default()
        };
        let tags = BTreeSet::new();
        assert_eq!(
            matching_hosts(&data, "", None, &tags, Order::NameAscending)
                .iter()
                .map(|host| host.id)
                .collect::<Vec<_>>(),
            vec![3, 2]
        );
        assert_eq!(
            matching_hosts(&data, "", Some(4), &tags, Order::NameAscending).len(),
            1
        );
        assert_eq!(
            matching_hosts(
                &data,
                "user:fixture tag:lab",
                None,
                &tags,
                Order::NameAscending
            )[0]
            .id,
            2
        );
        assert_eq!(
            matching_hosts(
                &data,
                "",
                None,
                &BTreeSet::from(["lab".into()]),
                Order::NameAscending
            )[0]
            .id,
            2
        );
        assert_eq!(
            matching_hosts(&data, "", None, &tags, Order::NameDescending)[0].id,
            2
        );
    }
}
