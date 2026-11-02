use super::*;
impl Panel {
    pub(super) fn open_licenses(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.licenses_open {
            return;
        }
        self.licenses_open = true;
        self.license_loading = true;
        self.license_error = None;
        self.catalog = None;
        self.filtered.clear();
        self.select_license(None, window, cx);
        self.query
            .update(cx, |query, cx| query.set_value("", window, cx));
        self.license_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(model::licenses::bundled).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.license_task = None;
                this.license_loading = false;
                if !this.licenses_open {
                    return;
                }
                match result {
                    Ok(catalog) => {
                        this.filtered = (0..catalog.packages.len()).collect();
                        this.catalog = Some(catalog);
                        this.select_license(Some(0), window, cx);
                    }
                    Err(error) => this.license_error = Some(error),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn select_license(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let package = index.and_then(|index| self.catalog.as_ref()?.packages.get(index));
        let value = package.map(|package| package.text()).unwrap_or_default();
        self.selected = index.filter(|_| package.is_some());
        self.text
            .update(cx, |text, cx| text.set_value(value, window, cx));
        cx.notify();
    }
    pub(super) fn filter_licenses(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.query.read(cx).value();
        self.filtered = self
            .catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .packages
                    .iter()
                    .enumerate()
                    .filter_map(|(index, package)| package.matches(&query).then_some(index))
                    .collect()
            })
            .unwrap_or_default();
        if self
            .selected
            .is_some_and(|index| !self.filtered.contains(&index))
        {
            self.select_license(None, window, cx);
        }
        self.list_scroll.scroll_to_item(0, ScrollStrategy::Nearest);
        cx.notify();
    }
    pub(super) fn source_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.source_open {
            return None;
        }
        Some(
            Modal::new("about-source-license-modal", "Source License")
                .focus_scope(&self.source_focus)
                .return_focus(&self.focus)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.source_open = false;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("about-source-license-frame", "MIT + Commons Clause")
                        .colors(self.colors())
                        .width(760.)
                        .content(
                            Input::multiline(&self.source)
                                .readonly(true)
                                .h(px(350.))
                                .aria_label(crate::localization::text(
                                    cx,
                                    "Nauterm source license",
                                )),
                        )
                        .action(
                            Button::new("copy-source-license")
                                .outline()
                                .label("Copy")
                                .on_click(|_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        model::SOURCE_LICENSE.into(),
                                    ))
                                }),
                        )
                        .action(
                            Button::new("close-source-license")
                                .label("Done")
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        ),
                )
                .into_any_element(),
        )
    }
    pub(super) fn licenses_modal(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.licenses_open {
            return None;
        }
        let p = Palette::settings(self.dark);
        let compact = window.viewport_size().width < px(740.);
        let mut body =
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(div().text_size(px(12.)).text_color(p.muted).child(
                    crate::localization::view(format!(
                        "Nauterm RS {} · Dependencies across all supported platforms.",
                        model::VERSION
                    )),
                ))
                .child(
                    Input::new(&self.query)
                        .aria_label(crate::localization::text(cx, "Search third-party packages")),
                );
        if self.license_loading {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Spinner::new())
                    .child(crate::localization::text(cx, "rs.ui.copy.loadingLicenses")),
            );
        }
        if let Some(catalog) = &self.catalog {
            let filtered = self.filtered.clone();
            let owner = cx.weak_entity();
            let list = uniform_list(
                "about-license-package-list",
                filtered.len(),
                move |range, _, cx| {
                    let Some(owner) = owner.upgrade() else {
                        return Vec::new();
                    };
                    range
                        .filter_map(|row| {
                            let index = *filtered.get(row)?;
                            Some(owner.update(cx, |this, cx| {
                                let label = this
                                    .catalog
                                    .as_ref()
                                    .and_then(|catalog| catalog.packages.get(index))
                                    .map(|package| package.label())
                                    .unwrap_or_default();
                                Button::new(("about-license-package", index))
                                    .ghost()
                                    .w_full()
                                    .h(px(36.))
                                    .content_start()
                                    .label(label)
                                    .when(this.selected == Some(index), |button| {
                                        button.bg(p.hover).text_color(p.accent)
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select_license(Some(index), window, cx)
                                    }))
                                    .into_any_element()
                            }))
                        })
                        .collect()
                },
            )
            .track_scroll(&self.list_scroll)
            .size_full();
            let list = div()
                .min_w_0()
                .h(px(if compact { 144. } else { 340. }))
                .when(!compact, |view| view.w(px(235.)).flex_shrink_0())
                .when(self.filtered.is_empty(), |view| {
                    view.child(div().p(px(12.)).text_color(p.muted).child(
                        crate::localization::text(cx, "settings.about.licenses.noMatches"),
                    ))
                })
                .when(!self.filtered.is_empty(), |view| {
                    view.child(crate::components::uniform_scrollbar(
                        "about-license-list-scrollbar",
                        list,
                        &self.list_scroll,
                    ))
                });
            let selected = self.selected.and_then(|index| catalog.packages.get(index));
            let mut detail = div().flex_1().min_w_0().flex().flex_col().gap(px(8.));
            if let Some(package) = selected {
                detail = detail
                    .child(
                        div()
                            .font_weight(FontWeights::MEDIUM)
                            .child(package.label()),
                    )
                    .child(
                        div().text_size(px(11.)).text_color(p.muted).child(
                            package
                                .components
                                .iter()
                                .copied()
                                .collect::<Vec<_>>()
                                .join(" · "),
                        ),
                    )
                    .child(
                        Input::multiline(&self.text)
                            .readonly(true)
                            .h(px(if compact { 230. } else { 290. }))
                            .aria_label(crate::localization::text(cx, "Third-party license text")),
                    );
            } else {
                detail = detail.child(div().p(px(12.)).text_color(p.muted).child(
                    crate::localization::text(cx, "rs.ui.copy.selectPackageLicenses"),
                ));
            }
            body = body.child(
                div()
                    .flex()
                    .gap(px(16.))
                    .when(compact, |view| view.flex_col())
                    .child(list)
                    .child(detail),
            );
        }
        Some(
            Modal::new("about-third-party-modal", "Third-party Licenses")
                .focus_scope(&self.license_focus)
                .return_focus(&self.focus)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.licenses_open = false;
                    this.license_task = None;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("about-third-party-frame", "Third-party Licenses")
                        .colors(self.colors())
                        .width(870.)
                        .content(body)
                        .action(
                            Button::new("copy-third-party-license")
                                .outline()
                                .label("Copy")
                                .disabled(self.selected.is_none())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(package) = this.selected.and_then(|index| {
                                        this.catalog.as_ref()?.packages.get(index)
                                    }) {
                                        cx.write_to_clipboard(ClipboardItem::new_string(format!(
                                            "{}\n\n{}",
                                            package.label(),
                                            package.text()
                                        )));
                                    }
                                })),
                        )
                        .action(
                            Button::new("close-third-party-licenses")
                                .label("Done")
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        ),
                )
                .into_any_element(),
        )
    }
}
