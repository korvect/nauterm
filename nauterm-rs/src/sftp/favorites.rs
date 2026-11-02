//! Remote path favorites use the original shared table, scoped to a saved Host.
use super::*;
use crate::resource_reference::{Kind, Protocol, Reference};
use crate::typography::FontWeights;

#[derive(Default)]
pub(super) struct Favorites {
    paths: Vec<String>,
    pub(super) open: bool,
    loading: bool,
    busy: bool,
    error: Option<String>,
    task: Option<Task<()>>,
}
fn unique_paths(paths: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    paths
        .into_iter()
        .map(|path| path.trim().to_owned())
        .filter(|path| !path.is_empty() && seen.insert(path.clone()))
        .collect()
}
fn read(
    database: &nauterm_core::NautermDatabase,
    source: &Reference,
) -> anyhow::Result<Vec<String>> {
    // ID reuse/deletion must not redirect a pending favorite operation.
    let _host = source.load(database)?;
    Ok(unique_paths(
        database
            .list_sftp_favorite_paths("remote".into(), Some(source.id))?
            .into_iter()
            .map(|favorite| favorite.path),
    ))
}
impl FilePane {
    fn favorite_source(&self) -> Option<Reference> {
        let id = self.remote.as_ref()?.host_id?;
        let host = self.data.hosts.iter().find(|host| host.id == id)?;
        let uuid = host
            .uuid
            .clone()
            .filter(|uuid| crate::resource_reference::valid_uuid(uuid))?;
        Some(Reference {
            kind: Kind::Host,
            id: i64::try_from(id).ok()?,
            uuid,
            protocol: Protocol::Ssh,
        })
    }
    pub(super) fn load_favorites(&mut self, cx: &mut Context<Self>) {
        if self.favorites.loading || self.favorites.busy {
            return;
        }
        let (Some(source), Some(vault)) = (self.favorite_source(), self.vault.clone()) else {
            return;
        };
        let token = self.endpoint_token;
        self.favorites.loading = true;
        self.favorites.error = None;
        self.notice = None;
        self.favorites.task = Some(cx.spawn(async move |this, cx| {
            let result = vault.dispatch(move |db| read(&db.inner, &source)).await;
            let _ = this.update(cx, |this, cx| {
                if this.endpoint_token != token {
                    return;
                }
                this.favorites.loading = false;
                this.favorites.task = None;
                match result {
                    Ok(paths) => {
                        this.favorites.paths = paths;
                        this.favorites.error = None;
                    }
                    Err(error) => {
                        this.favorites.error = Some(error.to_string());
                        this.notice = this.favorites.error.clone().map(Message::error);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn toggle_favorite(&mut self, path: String, cx: &mut Context<Self>) {
        if self.favorites.busy
            || self.favorites.loading
            || self.remote.is_none()
            || self.stage != Stage::Files
        {
            return;
        }
        let path = path.trim().to_owned();
        if path.is_empty() {
            return;
        }
        let remove = self.favorites.paths.contains(&path);
        let source = self.favorite_source();
        if self
            .remote
            .as_ref()
            .is_some_and(|remote| remote.host_id.is_some())
            && source.is_none()
        {
            self.favorites.error =
                Some("Reload the saved Host before changing its favorites.".into());
            self.notice = self.favorites.error.clone().map(Message::error);
            cx.notify();
            return;
        }
        let Some(source) = source else {
            // Quick Connect has no saved Host. Its favorites last for this pane.
            self.favorites.paths.retain(|old| old != &path);
            if !remove {
                self.favorites.paths.insert(0, path);
            }
            cx.notify();
            return;
        };
        let Some(vault) = self.vault.clone() else {
            self.favorites.error = Some("The shared database is not ready.".into());
            self.notice = self.favorites.error.clone().map(Message::error);
            cx.notify();
            return;
        };
        let token = self.endpoint_token;
        self.favorites.busy = true;
        self.favorites.error = None;
        self.notice = None;
        self.favorites.task = Some(cx.spawn(async move |this, cx| {
            let result = vault.dispatch(move |db| {
                db.inner.atomic(|database| -> anyhow::Result<Vec<String>> {
                    let _host = source.load(database)?;
                    if remove {
                        database.delete_sftp_favorite_path_by_target("remote".into(), Some(source.id), path)?;
                    } else {
                        let favorite = serde_json::from_value(serde_json::json!({
                            "scope":"remote", "host_id":source.id, "host_uuid":source.uuid, "path":path,
                        }))?;
                        database.save_sftp_favorite_path(&favorite)?;
                    }
                    read(database, &source)
                })
            }).await;
            let _ = this.update(cx, |this, cx| {
                if this.endpoint_token != token { return; }
                this.favorites.busy = false;
                this.favorites.task = None;
                match result {
                    Ok(paths) => this.favorites.paths = paths,
                    Err(error) => { this.favorites.error = Some(error.to_string()); this.notice = this.favorites.error.clone().map(Message::error); }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn favorite_controls(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = self.palette();
        let current = self.history.path.clone();
        let selected = self.favorites.paths.contains(&current);
        let owner = cx.weak_entity();
        let star = Button::new("sftp-favorite-current")
            .toolbar()
            .w(px(24.))
            .h(px(26.))
            .p_0()
            .ghost()
            .custom(
                crate::components::ButtonCustomVariant::for_surface(p)
                    .foreground(if selected { p.accent } else { p.text }.into()),
            )
            .icon(IconName::Star)
            .tooltip(if selected {
                "Remove path favorite"
            } else {
                "Favorite current path"
            })
            .disabled(self.favorites.loading || self.favorites.busy || current.trim().is_empty())
            .on_click(move |_, _, cx| {
                let _ = owner.update(cx, |this, cx| this.toggle_favorite(current.clone(), cx));
            });
        if self.compact {
            let open = self.favorites.open;
            let toggle = self
                .tool_button(
                    "sftp-favorites-toggle",
                    IconName::Bookmark,
                    "Favorite paths",
                    open,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.favorites.open = !this.favorites.open;
                    this.tasks_open = false;
                    if this.favorites.open {
                        this.load_favorites(cx);
                    }
                    cx.notify();
                }));
            return vec![
                star.size(px(29.))
                    .custom(
                        crate::components::ButtonCustomVariant::for_surface(p)
                            .foreground(if selected { p.accent } else { p.muted }.into())
                            .hover(p.control.into())
                            .highlight(p.accent.alpha(0.10).into())
                            .active(p.accent.alpha(0.16).into()),
                    )
                    .icon(Icon::new(IconName::Star).size(px(15.)))
                    .into_any_element(),
                toggle.into_any_element(),
            ];
        }
        let owner = cx.weak_entity();
        let count = self.favorites.paths.len();
        let open = self.favorites.open;
        let panel = Overlay::new("sftp-favorites-overlay")
            .anchor(Anchor::TopRight)
            .p_0()
            .bg(p.surface)
            .border_color(p.border)
            .rounded(px(8.))
            .open(open)
            .on_open_change(cx.listener(|this, open, _, cx| {
                this.favorites.open = *open;
                if *open {
                    this.load_favorites(cx);
                }
                cx.notify();
            }))
            .trigger(
                Button::new("sftp-favorites-toggle")
                    .toolbar()
                    .w(px(24.))
                    .h(px(26.))
                    .p_0()
                    .ghost()
                    .custom(
                        crate::components::ButtonCustomVariant::for_surface(p)
                            .foreground(if open { p.accent } else { p.text }.into()),
                    )
                    .icon(IconName::Bookmark)
                    .tooltip(if open {
                        "Hide favorite paths"
                    } else {
                        "Show favorite paths"
                    })
                    .when(count > 0, |button| {
                        button.child(
                            div()
                                .absolute()
                                .top(px(-2.))
                                .right(px(-2.))
                                .min_w(px(12.))
                                .h(px(12.))
                                .px(px(2.))
                                .rounded_full()
                                .bg(p.accent)
                                .text_color(rgb(0xffffff))
                                .text_size(px(8.))
                                .line_height(px(12.))
                                .text_center()
                                .child(if count > 99 {
                                    "99+".into()
                                } else {
                                    count.to_string()
                                }),
                        )
                    }),
            )
            .content(move |_, cx| {
                owner
                    .upgrade()
                    .map(|pane| pane.update(cx, |pane, cx| pane.favorite_panel(cx)))
                    .unwrap_or_else(|| div().into_any_element())
            });
        vec![star.into_any_element(), panel.into_any_element()]
    }
    pub(super) fn favorite_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.compact {
            return self.tool_favorite_panel(cx);
        }
        let p = self.palette();
        div()
            .w(px(280.))
            .max_w(relative(1.))
            .h(px(246.))
            .flex()
            .flex_col()
            .text_color(p.text)
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(13.))
                    .pt(px(13.))
                    .pb(px(7.))
                    .text_size(px(13.))
                    .font_weight(FontWeights::SEMIBOLD)
                    .child(crate::localization::text(cx, "common.label.favorites")),
            )
            .when_some(self.favorites.error.clone(), |body, error| {
                body.child(
                    div()
                        .px(px(13.))
                        .text_size(px(11.))
                        .text_color(rgb(0xef4444))
                        .child(crate::localization::message(cx, &error)),
                )
            })
            .child(
                div()
                    .id("sftp-favorite-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .scrollbars()
                    .when(self.favorites.paths.is_empty(), |body| {
                        body.flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .child(crate::localization::message(
                                cx,
                                if self.favorites.loading {
                                    "Loading favorites…"
                                } else {
                                    "No favorite paths"
                                },
                            ))
                    })
                    .children(
                        self.favorites
                            .paths
                            .iter()
                            .enumerate()
                            .map(|(index, path)| {
                                let target = path.clone();
                                Button::new(("sftp-favorite-path", index))
                                    .ghost()
                                    .w_full()
                                    .h(px(40.))
                                    .custom(crate::components::ButtonCustomVariant::for_surface(p))
                                    .content_start()
                                    .px(px(13.))
                                    .text_size(px(12.))
                                    .line_height(relative(1.5))
                                    .icon(
                                        Icon::new(IconName::Folder)
                                            .size(px(16.))
                                            .text_color(rgb(0x5ec4f2)),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(path.clone()),
                                    )
                                    .raw_tooltip(path.clone())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.favorites.open = false;
                                        this.navigate(
                                            target.clone(),
                                            Navigation::Visit,
                                            window,
                                            cx,
                                        );
                                    }))
                            }),
                    ),
            )
            .into_any_element()
    }
    fn tool_favorite_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let body_height = if self.favorites.paths.is_empty() {
            54.
        } else {
            (self.favorites.paths.len() as f32 * 34. + 8.).min(210.)
        };
        div()
            .w_full()
            .max_h(px(250.))
            .rounded(px(9.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .text_color(p.text)
            .child(
                div()
                    .h(px(38.))
                    .flex_shrink_0()
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(p.border)
                    .text_size(px(11.5))
                    .line_height(px(17.))
                    .font_weight(FontWeights::SEMIBOLD)
                    .child(crate::localization::text(
                        cx,
                        "workspace.label.favoritePaths",
                    )),
            )
            .when_some(self.favorites.error.clone(), |view, error| {
                view.child(
                    div()
                        .px(px(10.))
                        .py(px(4.))
                        .text_size(px(11.))
                        .line_height(px(16.))
                        .text_color(rgb(0xd54b3f))
                        .child(crate::localization::message(cx, &error)),
                )
            })
            .child(
                div()
                    .id("tool-sftp-favorite-list")
                    .h(px(body_height))
                    .min_h_0()
                    .overflow_y_scroll()
                    .scrollbars()
                    .py(px(4.))
                    .when(self.favorites.paths.is_empty(), |view| {
                        view.flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.))
                            .line_height(px(16.))
                            .text_color(p.muted)
                            .child(crate::localization::view(if self.favorites.loading {
                                "Loading favorites…"
                            } else {
                                "No favorite paths"
                            }))
                    })
                    .children(
                        self.favorites
                            .paths
                            .iter()
                            .enumerate()
                            .map(|(index, path)| {
                                let target = path.clone();
                                Button::new(("tool-sftp-favorite", index))
                                    .ghost()
                                    .w_full()
                                    .h(px(34.))
                                    .px(px(10.))
                                    .rounded(px(0.))
                                    .content_start()
                                    .custom(
                                        crate::components::ButtonCustomVariant::for_surface(p)
                                            .hover(p.control.into())
                                            .highlight(p.accent.alpha(0.10).into())
                                            .active(p.accent.alpha(0.16).into()),
                                    )
                                    .icon(
                                        Icon::new(IconName::Folder)
                                            .size(px(14.))
                                            .text_color(p.accent),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .font_family("monospace")
                                            .text_size(px(10.5))
                                            .line_height(px(16.))
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(path.clone()),
                                    )
                                    .raw_tooltip(path.clone())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.favorites.open = false;
                                        this.navigate(
                                            target.clone(),
                                            Navigation::Visit,
                                            window,
                                            cx,
                                        );
                                    }))
                            }),
                    ),
            )
            .into_any_element()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn stored_favorites_keep_recent_order_without_empty_or_duplicate_paths() {
        assert_eq!(
            unique_paths([" /srv ", "/home", "/srv", ""].map(str::to_owned)),
            ["/srv", "/home"]
        );
    }
}
