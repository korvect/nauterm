use super::*;
use crate::terminal_appearance::ThemeId;

pub(super) struct FontControls {
    session: u64,
    family: SelectModel<String>,
    size: SelectModel<String>,
    family_value: String,
    size_value: f32,
    loaded: bool,
    _events: Vec<Subscription>,
}
enum FontChange {
    Family(String),
    Size(String),
}

impl Nauterm {
    pub(super) fn refresh_terminal_fonts(
        &mut self,
        visible: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = self.active_terminal();
        let showing = visible
            && self.terminals.iter().any(|tab| {
                Some(tab.id) == selected
                    && tools_visible(tab.tools.open, tab.show_sftp)
                    && tab.tools.mode == ToolMode::Themes
            });
        if !showing {
            return;
        }
        if self.font_options.is_none() && self.font_loading.is_none() {
            let text = cx.text_system().clone();
            self.font_loading = Some(cx.spawn(async move |this, cx| {
                let options = cx
                    .background_spawn(async move { crate::fonts::options(&text) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.font_options = Some(options);
                    this.font_loading = None;
                    cx.notify();
                });
            }));
        }
        let loaded = self.font_options.is_some();
        if self
            .terminals
            .iter()
            .find(|tab| Some(tab.id) == selected)
            .is_some_and(|tab| {
                let appearance = tab.active().appearance(&self.data);
                tab.tools.fonts.as_ref().is_some_and(|controls| {
                    controls.session == tab.active().id
                        && controls.loaded == loaded
                        && controls.family_value == appearance.family
                        && controls.size_value == appearance.size
                })
            })
        {
            return;
        }
        let fonts = self.font_options.clone().unwrap_or_default();
        for tab in self
            .terminals
            .iter_mut()
            .filter(|tab| Some(tab.id) == selected)
        {
            let appearance = tab.active().appearance(&self.data);
            let session = tab.active().id;
            if let Some(controls) = &mut tab.tools.fonts
                && controls.session == session
            {
                if controls.loaded != loaded || controls.family_value != appearance.family {
                    controls.family.reconcile(
                        crate::fonts::primary_options(&fonts, &appearance.family),
                        Some(appearance.family.clone()),
                        window,
                        cx,
                    );
                }
                if controls.size_value != appearance.size {
                    controls.size.reconcile(
                        crate::fonts::size_options(appearance.size),
                        Some(appearance.size.to_string()),
                        window,
                        cx,
                    );
                }
                controls.family_value = appearance.family;
                controls.size_value = appearance.size;
                controls.loaded = loaded;
                continue;
            }
            let family = SelectModel::new(
                crate::fonts::primary_options(&fonts, &appearance.family),
                Some(appearance.family.clone()),
                window,
                cx,
            )
            .allow_custom_values(str::to_owned, cx)
            .retain_selection_on_search(cx);
            let size = SelectModel::new(
                crate::fonts::size_options(appearance.size),
                Some(appearance.size.to_string()),
                window,
                cx,
            );
            let tab_id = tab.id;
            let family_event = cx.subscribe_in(
                &family.state,
                window,
                move |this, _, event: &SelectEvent<String>, window, cx| {
                    let SelectEvent::Confirm(value) = event;
                    let Some(Some(value)) = value else {
                        return;
                    };
                    let value = value.clone();
                    this.select_tool_font(tab_id, session, FontChange::Family(value), window, cx);
                },
            );
            let size_event = cx.subscribe_in(
                &size.state,
                window,
                move |this, _, event: &SelectEvent<String>, window, cx| {
                    let SelectEvent::Confirm(value) = event;
                    if let Some(Some(value)) = value {
                        this.select_tool_font(
                            tab_id,
                            session,
                            FontChange::Size(value.clone()),
                            window,
                            cx,
                        );
                    }
                },
            );
            tab.tools.fonts = Some(FontControls {
                session,
                family,
                size,
                family_value: appearance.family,
                size_value: appearance.size,
                loaded,
                _events: vec![family_event, size_event],
            });
        }
    }

    fn select_tool_font(
        &mut self,
        tab_id: u64,
        session: u64,
        change: FontChange,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page != Page::Sessions || self.overview || self.active_terminal() != Some(tab_id) {
            return;
        }
        let Some(tab) = self.terminals.iter_mut().find(|tab| {
            tab.id == tab_id
                && tab.active().id == session
                && tab.tools.mode == ToolMode::Themes
                && tools_visible(tab.tools.open, tab.show_sftp)
        }) else {
            return;
        };
        // A rejected edit must restore the committed value on the next render,
        // not leave an unsaved choice shown.
        if let Some(controls) = &mut tab.tools.fonts {
            controls.family_value.clear();
            controls.size_value = f32::NAN;
        }
        let mut next = self.data.clone();
        let result = (|| -> anyhow::Result<()> {
            match change {
                FontChange::Family(family) => next.terminal.family = family.trim().to_owned(),
                FontChange::Size(size) => next.terminal.size = size.parse()?,
            }
            next.terminal.validate()?;
            if let Some(error) = &self.storage_error {
                anyhow::bail!(error.clone());
            }
            if self.storage_loading {
                anyhow::bail!("Saved settings are still loading. Please retry.");
            }
            if next.terminal != self.data.terminal {
                self.store.save(&mut next)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.notice = Some(format!("Could not save terminal font: {error}"));
            cx.notify();
            return;
        }
        self.data = next;
        self.apply_tool_appearance(cx);
        if let Some(settings) = &self.settings_window {
            let view = settings._view.clone();
            let family = self.data.terminal.family.clone();
            let size = self.data.terminal.size;
            let _ = settings.handle.update(cx, |_, window, cx| {
                view.update(cx, |settings, cx| {
                    settings.external_font(&family, size, window, cx)
                });
            });
        }
        cx.notify();
    }

    pub(super) fn init_tool_search(
        tools: &mut ToolsState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (slot, label, scroll) in [
            (
                &mut tools.history_search,
                "Search commands",
                tools.history_scroll.clone(),
            ),
            (
                &mut tools.theme_search,
                "Search themes",
                tools.theme_scroll.clone(),
            ),
        ] {
            if slot.is_some() {
                continue;
            }
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(label));
            tools
                ._tool_events
                .push(cx.subscribe(&input, move |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        scroll.scroll_to_item(0, ScrollStrategy::Nearest);
                        cx.notify();
                    }
                }));
            *slot = Some(input);
        }
    }

    pub(super) fn history_tool(&self, tab: &TerminalTab, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::terminal(tab.active().appearance(&self.data).palette());
        let Some(search) = tab.tools.history_search.as_ref() else {
            return div().into_any_element();
        };
        let TabContent::Terminal(terminal) = &tab.active().content else {
            return div()
                .p(px(12.))
                .text_color(p.muted)
                .child(crate::localization::text(
                    cx,
                    "rs.ui.copy.shellHistoryAfterConnect",
                ))
                .into_any_element();
        };
        let query = search.read(cx).value().to_lowercase();
        let entries = std::rc::Rc::new(
            terminal
                .read(cx)
                .history_entries()
                .into_iter()
                .filter(|entry| entry.command.to_lowercase().contains(&query))
                .collect::<Vec<_>>(),
        );
        let count = entries.len();
        let target = terminal.clone();
        let can_run = target.read(cx).can_run_history();
        let feedback = cx.weak_entity();
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .text_color(p.text)
            .child(
                div()
                    .flex_shrink_0()
                    .p(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(crate::localization::text(cx, "workspace.label.shellHistory"))
                    .child(
                        Input::new(search)
                            .palette(p)
                            .bg(p.control)
                            .prefix(icon(IconName::Search)),
                    ),
            )
            .when(count == 0, |v| {
                v.child(
                    div()
                        .p(px(16.))
                        .text_color(p.muted)
                        .child(crate::localization::view(if query.is_empty() {
                            "No shell history yet"
                        } else {
                            "No matching commands"
                        })),
                )
            })
            .when(count > 0, |v| {
                v.child(crate::components::uniform_scrollbar(
                    "history-tools-scrollbar",
                    uniform_list("history-tools-list", count, move |range, _, _| {
                        range
                            .map(|index| {
                                let entry = &entries[index];
                                let command = entry.command.clone();
                                let copy = command.clone();
                                let run = command.clone();
                                let terminal = target.clone();
                                let feedback = feedback.clone();
                                let stamp = entry.time()
                                    .map(|time| {
                                        time.with_timezone(&chrono::Local)
                                            .format("%Y-%m-%d %H:%M")
                                            .to_string()
                                    });
                                div().h(px(60.)).px(px(8.)).pb(px(4.)).child(
                                    div()
                                        .size_full()
                                        .rounded(px(7.))
                                        .bg(p.control)
                                        .px(px(9.))
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .flex()
                                                .flex_col()
                                                .gap(px(3.))
                                                .child(
                                                    div()
                                                        .text_size(px(12.))
                                                        .line_clamp(2)
                                                        .child(command),
                                                )
                                                .when_some(stamp, |v, stamp| {
                                                    v.child(
                                                        div()
                                                            .text_size(px(10.))
                                                            .text_color(p.muted)
                                                            .child(stamp),
                                                    )
                                                }),
                                        )
                                        .child(
                                            Button::new(("history-copy", index))
                                                .xsmall()
                                                .ghost().custom(crate::components::ButtonCustomVariant::for_surface(p))
                                                .icon_size(px(13.))
                                                .icon(IconName::Copy)
                                                .tooltip("Copy command")
                                                .on_click(move |_, _, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(copy.clone()),
                                                    )
                                                }),
                                        )
                                        .child(
                                            Button::new(("history-run", index))
                                                .xsmall()
                                                .ghost().custom(crate::components::ButtonCustomVariant::for_surface(p))
                                                .icon_size(px(14.))
                                                .icon(IconName::Play)
                                                .text_color(p.accent)
                                                .disabled(!can_run)
                                                .tooltip("Run command")
                                                .on_click(move |_, window, cx| {
                                                    let result = terminal.update(cx, |terminal, cx| {
                                                        terminal.run_history(&run, window, cx)
                                                    });
                                                    if let Err(error) = result {
                                                        let _ = feedback.update(cx, |this, cx| { this.notice = Some(error.to_string()); cx.notify(); });
                                                    }
                                                }),
                                        ),
                                )
                            })
                            .collect()
                    })
                    .track_scroll(&tab.tools.history_scroll)
                    .size_full(),
                    &tab.tools.history_scroll,
                ))
            })
            .into_any_element()
    }

    pub(super) fn themes_tool(
        &self,
        tab: &TerminalTab,
        width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        const INSET: f32 = 12.;
        let appearance = tab.active().appearance(&self.data);
        let p = Palette::terminal(appearance.palette());
        let Some(search) = tab.tools.theme_search.as_ref() else {
            return div().into_any_element();
        };
        let themes = std::rc::Rc::new(crate::theme_catalog::filter(&search.read(cx).value()));
        let selected = appearance
            .custom_palette
            .is_none()
            .then_some(appearance.theme);
        // Account for the tool panel's own right inset and border, then use the
        // same horizontal gutter for controls and virtualized preview rows.
        let columns = if width - 10. - 2. * INSET >= 520. {
            2
        } else {
            1
        };
        let count = themes.len().div_ceil(columns);
        let tab_id = tab.id;
        let owner = cx.weak_entity();
        let header = div()
            .p(px(INSET))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeights::MEDIUM)
                    .child(crate::localization::text(
                        cx,
                        "workspace.label.terminalAppearance",
                    )),
            )
            .when_some(tab.tools.fonts.as_ref(), |header, controls| {
                let field = |label, control| crate::components::terminal_field(label, control, p);
                header.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(div().flex_1().min_w_0().child(field(
                            "Font",
                            controls.family.terminal_control("Font", p, true),
                        )))
                        .child(div().w(px(82.)).flex_shrink_0().child(field(
                            "Size",
                            controls.size.terminal_control("Font size", p, false),
                        ))),
                )
            })
            .child(
                Input::new(search)
                    .palette(p)
                    .bg(p.control)
                    .h(px(34.))
                    .aria_label(crate::localization::text(cx, "Search themes"))
                    .prefix(icon(IconName::Search).size(px(14.))),
            );
        let mut body = div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .text_color(p.text)
            .child(header);
        if count == 0 {
            return body
                .child(
                    div()
                        .p(px(INSET))
                        .text_color(p.muted)
                        .child(crate::localization::text(cx, "rs.ui.copy.noMatchingThemes")),
                )
                .into_any_element();
        }
        let previews = uniform_list("theme-tools-list", count, move |range, _, _| {
            range
                .map(|row| {
                    div()
                        .w_full()
                        .h(px(118.))
                        .px(px(INSET))
                        .pb(px(10.))
                        .flex()
                        .gap(px(8.))
                        .children(
                            themes
                                .iter()
                                .skip(row * columns)
                                .take(columns)
                                .copied()
                                .map(|theme| {
                                    let palette = crate::terminal_appearance::Palette::new(theme);
                                    let owner = owner.clone();
                                    Button::new(SharedString::from(format!(
                                        "tools-theme-{}",
                                        theme.id()
                                    )))
                                    .ghost()
                                    .custom(crate::components::ButtonCustomVariant::for_surface(p))
                                    .p_0()
                                    .h_full()
                                    .flex_1()
                                    .min_w_0()
                                    .rounded(px(8.))
                                    .raw_tooltip(theme.label())
                                    .child(
                                        crate::theme_preview::ThemePreview::new(
                                            theme.label(),
                                            palette,
                                        )
                                        .compact()
                                        .selected(selected == Some(theme)),
                                    )
                                    .on_click(
                                        move |_, window, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.select_tool_theme(tab_id, theme, window, cx)
                                            });
                                        },
                                    )
                                }),
                        )
                        .children(
                            (themes.len().saturating_sub(row * columns).min(columns)..columns)
                                .map(|_| div().flex_1().min_w_0()),
                        )
                })
                .collect()
        })
        .track_scroll(&tab.tools.theme_scroll)
        .size_full();
        body = body.child(
            div()
                .flex_1()
                .min_h_0()
                .child(crate::components::uniform_scrollbar(
                    "theme-tools-scrollbar",
                    previews,
                    &tab.tools.theme_scroll,
                )),
        );
        body.into_any_element()
    }

    fn select_tool_theme(
        &mut self,
        tab_id: u64,
        theme: ThemeId,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.terminals.iter().find(|tab| tab.id == tab_id) else {
            return;
        };
        let host_id = tab
            .active()
            .host_id()
            .filter(|id| self.data.hosts.iter().any(|host| host.id == *id));
        let mut next = self.data.clone();
        if let Some(host) = host_id.and_then(|id| next.hosts.iter_mut().find(|host| host.id == id))
        {
            if matches!(
                tab.active().restore_target,
                Some(RestoreTarget::TelnetHost { .. })
            ) {
                let Some(profile) = &mut host.connections.telnet else {
                    self.notice =
                        Some("The saved Telnet configuration is no longer available.".into());
                    cx.notify();
                    return;
                };
                profile.theme = Some(theme);
            } else {
                host.theme = Some(theme);
            }
        } else {
            next.terminal.select_theme(theme);
        }
        let saved = if let Some(error) = &self.storage_error {
            Err(anyhow::anyhow!(error.clone()))
        } else {
            self.store.save(&mut next)
        };
        if let Err(error) = saved {
            self.notice = Some(format!("Could not save terminal theme: {error}"));
            cx.notify();
            return;
        }
        self.data = next;
        self.apply_tool_appearance(cx);
        if host_id.is_none()
            && let Some(settings) = &self.settings_window
        {
            let view = settings._view.clone();
            let _ = settings.handle.update(cx, |_, window, cx| {
                view.update(cx, |settings, cx| {
                    settings.external_theme(theme, window, cx)
                })
            });
        }
        cx.notify();
    }
    fn apply_tool_appearance(&mut self, cx: &mut Context<Self>) {
        for session in self.terminals.iter().flat_map(|tab| tab.tree.sessions()) {
            let appearance = session.appearance(&self.data);
            match &session.content {
                TabContent::Terminal(terminal) => {
                    if let Err(error) =
                        terminal.update(cx, |terminal, cx| terminal.set_appearance(&appearance, cx))
                    {
                        self.notice = Some(error.to_string());
                    }
                }
                TabContent::Connection(connection) => {
                    if let Err(error) = connection.update(cx, |connection, _| {
                        connection.set_terminal_palette(appearance.palette())
                    }) {
                        self.notice = Some(error.to_string());
                    }
                }
                _ => (),
            }
            for files in [&session.files, &session.tool_files].into_iter().flatten() {
                files.update(cx, |files, cx| files.set_data(self.data.clone(), cx));
            }
        }
    }
}
