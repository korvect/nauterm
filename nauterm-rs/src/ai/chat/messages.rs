use super::*;
use crate::components::menu::ContextMenuExt;
use crate::components::{motion::Transition, selectable_text};
use gpui_kit::base::TextSelection;
use std::time::Instant;

pub(super) struct Document {
    markdown: MarkdownDocument,
    _changed: Subscription,
}
pub(super) struct Editor {
    uuid: String,
    original: String,
    input: Entity<TextareaState>,
    has_attachments: bool,
    _events: Subscription,
}

impl Chat {
    pub(super) fn select_all_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selecting_messages = true;
        self.selection_pending = true;
        self.follow = false;
        cx.notify();
        let owner = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            window.defer(cx, move |window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.selection_pending = false;
                    if this.active {
                        TextSelection::select_all(window, cx);
                    }
                    cx.notify();
                });
            });
        });
    }
    pub(super) fn message_text_focused(&self, window: &Window, cx: &App) -> bool {
        self.selection_focus.is_focused(window)
            || self.documents.values().any(|document| {
                document
                    .markdown
                    .state()
                    .read(cx)
                    .focus_handle()
                    .is_focused(window)
            })
    }
    pub(super) fn copy_text(&mut self, key: String, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.copied_text = Some(key);
        self.copy_reset = Some(cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1400))
                .await;
            let _ = view.update(cx, |view, cx| {
                view.copied_text = None;
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn markdown(
        &mut self,
        key: String,
        source: &str,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let document = self.documents.entry(key).or_insert_with(|| {
            let markdown = MarkdownDocument::new(cx);
            let changed = cx.observe(markdown.state(), |_, _, cx| {
                // Parsed Markdown can grow after the stream notification. Its
                // final layout must also honor the user's follow preference.
                cx.notify();
            });
            Document {
                markdown,
                _changed: changed,
            }
        });
        document.markdown.set_source(source, cx);
        document.markdown.view(palette)
    }
    fn begin_edit(&mut self, uuid: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.history_loading
            || self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.uuid == uuid)
        {
            return;
        }
        let Some(session) = &self.session else {
            return;
        };
        if !session.can_edit() {
            return;
        }
        let view = session.view();
        let Some((index, message)) = view.entry.messages.iter().enumerate().find(|(_, message)| {
            message.role == "user" && message.uuid.as_deref() == Some(uuid.as_str())
        }) else {
            return;
        };
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(message.content.clone())
                .auto_grow(1, 8)
                .submit_on_enter(true)
        });
        let events = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
            if matches!(
                event,
                InputEvent::PressEnter {
                    secondary: false,
                    shift: false
                }
            ) {
                this.save_edit(window, cx);
            }
            cx.notify();
        });
        input.read(cx).focus_handle(cx).focus(window, cx);
        self.editor = Some(Editor {
            uuid,
            original: message.content.clone(),
            input,
            has_attachments: !message.attachments.is_empty(),
            _events: events,
        });
        self.follow = false;
        let item = view.entry.messages[..index]
            .iter()
            .filter(|message| message.role != "tool")
            .count();
        self.scroll.scroll_to_item(item);
        self.error = None;
        cx.notify();
    }
    fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.history_loading || self.busy() {
            return;
        }
        self.editor = None;
        self.input.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }
    fn save_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.history_loading || self.busy() {
            return;
        }
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        let content = editor.input.read(cx).value().trim().to_owned();
        if content.is_empty() && !editor.has_attachments {
            return;
        }
        if content == editor.original {
            self.cancel_edit(window, cx);
            return;
        }
        let uuid = editor.uuid.clone();
        let provider = self.provider.clone().unwrap_or_default();
        let model = self.model.read(cx).value().trim().to_owned();
        let target = self.command_owner(cx);
        self.history_loading = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = session.edit(uuid, content, provider, model, target).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.history_loading = false;
                match result {
                    Ok(()) => {
                        this.editor = None;
                        this.error = None;
                        this.follow = true;
                        this.scroll_motion.clear();
                        this.input.read(cx).focus_handle(cx).focus(window, cx);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn editor_view(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let editor = self
            .editor
            .as_ref()
            .expect("rendering an active message editor");
        let busy = self.busy() || self.history_loading;
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .on_action(cx.listener(|this, _: &crate::ui::CloseEditor, window, cx| {
                this.cancel_edit(window, cx);
                cx.stop_propagation();
            }))
            .child(
                Input::multiline(&editor.input)
                    .palette(palette)
                    .aria_label(crate::localization::text(cx, "Edit message"))
                    .readonly(busy),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(
                        Button::new("ai-cancel-edit")
                            .small()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(palette))
                            .label("Cancel")
                            .disabled(busy)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.cancel_edit(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("ai-submit-edit")
                            .small()
                            .primary()
                            .label("Save and Resend")
                            .disabled(
                                busy || (editor.input.read(cx).value().trim().is_empty()
                                    && !editor.has_attachments),
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.save_edit(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn messages(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.sync_command_clock(cx);
        let palette = self.palette();
        let view = self.session.as_ref().map(|session| session.view());
        if view
            .as_ref()
            .is_none_or(|view| view.entry.messages.is_empty())
        {
            return div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .p(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .text_color(palette.muted)
                .font_weight(crate::typography::FontWeights::MEDIUM)
                .child(crate::localization::text(
                    cx,
                    "workspace.label.startAConversation",
                ))
                .into_any_element();
        }
        let busy = view.as_ref().is_some_and(|view| view.busy);
        let generating = view
            .as_ref()
            .is_some_and(|view| view.busy && view.active_command.is_none() && !view.stopping);
        let editable = !self.history_loading
            && self
                .session
                .as_ref()
                .is_some_and(|session| session.can_edit());
        let alive: std::collections::BTreeSet<_> = view
            .iter()
            .flat_map(|view| view.entry.messages.iter())
            .filter(|message| message.role == "assistant")
            .map(|message| {
                message
                    .uuid
                    .clone()
                    .unwrap_or_else(|| format!("assistant-{}", message.sequence))
            })
            .collect();
        self.documents.retain(|key, _| alive.contains(key));
        if !self.selection_drag
            && !self.selection_pending
            && !TextSelection::has_selection(window, cx)
        {
            self.selecting_messages = false;
        }
        let row_ids = view
            .iter()
            .flat_map(|view| view.entry.messages.iter())
            .filter(|message| message.role != "tool")
            .map(|message| message.sequence)
            .collect();
        self.message_rows.retain(&row_ids);
        let start = -f32::from(self.scroll.offset().y);
        let end = start + f32::from(self.scroll.bounds().size.height).max(600.);
        let width = (f32::from(self.scroll.bounds().size.width) - 24.).max(120.);
        let mut row_top = 0.;
        let observer = cx.weak_entity();
        let mut messages = div()
            .track_focus(&self.selection_focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if TextSelection::is_selectable_at(event.position, window, cx) {
                        this.selecting_messages = true;
                        this.selection_drag = true;
                        this.follow = false;
                        cx.notify();
                        this.selection_focus.focus(window, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.selection_drag = false;
                    let owner = cx.weak_entity();
                    window.defer(cx, move |window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if !TextSelection::has_selection(window, cx) {
                                this.selecting_messages = false;
                                if this.at_bottom() {
                                    this.follow = true;
                                    this.scroll_motion.clear();
                                }
                            }
                            cx.notify();
                        });
                    });
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) && this.selection_drag {
                    this.selection_drag = false;
                    if !TextSelection::has_selection(window, cx) {
                        this.selecting_messages = false;
                    }
                    cx.notify();
                }
            }))
            .on_children_prepainted(move |_, window, cx| {
                let observer = observer.clone();
                window.defer(cx, move |_, cx| {
                    let _ = observer.update(cx, |this, cx| {
                        let previous = this.follow;
                        this.observe_scroll();
                        if previous != this.follow {
                            cx.notify();
                        }
                    });
                });
            })
            .id("ai-messages")
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_x_hidden()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .scrollbars_with(&self.scroll)
            .hover_only()
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                let delta = event.delta.pixel_delta(px(1.)).y;
                if delta == px(0.) {
                    return;
                }
                let resume_at_bottom = delta < px(0.);
                let owner = cx.weak_entity();
                // Cancel before the next token notification can schedule a
                // follow frame. Only a downward user scroll may resume it.
                this.follow = false;
                this.scroll_motion.clear();
                window.defer(cx, move |_, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.follow = resume_at_bottom && this.at_bottom();
                        this.last_offset = this.scroll.offset().y;
                        cx.notify();
                    });
                });
            }));
        if let Some(view) = &view {
            let current = self.command_owner(cx);
            self.command_scroll.retain(|id, _| {
                view.entry
                    .command_blocks
                    .iter()
                    .any(|block| &block.tool_call_id == id)
            });
            for (index, message) in view.entry.messages.iter().enumerate() {
                if message.role == "tool" {
                    continue;
                }
                let assistant = message.role == "assistant";
                let editing = self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| message.uuid.as_ref() == Some(&editor.uuid));
                use std::hash::{Hash, Hasher};
                let next = view
                    .entry
                    .messages
                    .get(index + 1)
                    .map_or(i64::MAX, |message| message.sequence);
                let blocks = view.entry.commands_between(message.sequence, next);
                let mut digest = std::collections::hash_map::DefaultHasher::new();
                (std::sync::Arc::as_ptr(message) as usize).hash(&mut digest);
                for block in blocks {
                    (std::sync::Arc::as_ptr(block) as usize).hash(&mut digest);
                }
                editing.hash(&mut digest);
                let revision = digest.finish();
                let estimate = self.message_rows.estimate(message.sequence, || {
                    windowed::estimate(&message.content, width, message.attachments.len())
                        + blocks
                            .iter()
                            .map(|block| {
                                100. + windowed::estimate(
                                    block.output.as_deref().unwrap_or_default(),
                                    width,
                                    0,
                                )
                            })
                            .sum::<f32>()
                });
                let (height, materialize) = self.message_rows.slot(
                    message.sequence,
                    revision,
                    estimate,
                    row_top,
                    start..end,
                    self.selecting_messages || editing,
                );
                row_top += height + 14.;
                if !materialize {
                    messages = messages.child(
                        div()
                            .id(SharedString::from(format!(
                                "ai-message-slot-{}",
                                message.sequence
                            )))
                            .w_full()
                            .h(px(height))
                            .flex_shrink_0(),
                    );
                    continue;
                }
                let content = if editing {
                    self.editor_view(palette, cx)
                } else if message.content.is_empty() && busy && assistant {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_color(palette.muted)
                        .text_size(px(12.))
                        .child(Spinner::new())
                        .child(crate::localization::text(cx, "rs.ui.copy.thinking"))
                        .into_any_element()
                } else if assistant {
                    self.markdown(
                        message
                            .uuid
                            .clone()
                            .unwrap_or_else(|| format!("assistant-{}", message.sequence)),
                        &message.content,
                        palette,
                        cx,
                    )
                } else {
                    div()
                        .text_size(px(13.))
                        .line_height(relative(1.45))
                        .child(selectable_text(
                            SharedString::from(format!("ai-message-text-{}", message.sequence)),
                            message.content.clone(),
                            palette,
                        ))
                        .into_any_element()
                };
                let copy = message.content.clone();
                let sequence = message.sequence;
                let copy_id = format!("message-{sequence}");
                let copied = self.copied_text.as_ref() == Some(&copy_id);
                let visible = assistant || self.hovered_message == Some(sequence);
                let state = window.use_keyed_state(
                    SharedString::from(format!("ai-message-actions-{sequence}")),
                    cx,
                    |_, _| Transition::new(f32::from(assistant)),
                );
                let reduced = cx.reduce_motion();
                let (amount, moving) = state.update(cx, |state, _| {
                    state.set(
                        f32::from(visible),
                        Instant::now(),
                        Duration::from_millis(100),
                        reduced,
                    )
                });
                if moving {
                    window.request_animation_frame();
                }
                let mut actions = div()
                    .h(px(24.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .text_color(palette.muted)
                    .font_weight(crate::typography::FontWeights::REGULAR)
                    .opacity(amount)
                    .when(amount == 0., |bar| bar.invisible())
                    .when_some(
                        message
                            .created_at
                            .and_then(chrono::DateTime::from_timestamp_millis),
                        |bar, time| {
                            bar.child(
                                div()
                                    .text_size(px(10.))
                                    .line_height(px(14.))
                                    .mr(px(3.))
                                    .child(
                                        time.with_timezone(&chrono::Local)
                                            .format("%H:%M")
                                            .to_string(),
                                    ),
                            )
                        },
                    )
                    .child(
                        Button::new(("ai-copy", index))
                            .xsmall()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(palette))
                            .size(px(20.))
                            .p_0()
                            .icon_size(px(13.))
                            .text_color(if copied {
                                palette.accent
                            } else {
                                palette.muted
                            })
                            .icon(if copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .tooltip(if copied { "Copied" } else { "Copy message" })
                            .focusable(visible)
                            .disabled(copy.is_empty() || !visible)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.copy_text(copy_id.clone(), copy.clone(), cx)
                            })),
                    );
                if !assistant
                    && message.role == "user"
                    && !editing
                    && let Some(uuid) = &message.uuid
                {
                    let uuid = uuid.clone();
                    actions = actions.child(
                        Button::new(("ai-edit", index))
                            .xsmall()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(palette))
                            .size(px(20.))
                            .p_0()
                            .icon_size(px(13.))
                            .text_color(palette.muted)
                            .icon(IconName::Pencil)
                            .tooltip("Edit message")
                            .focusable(visible)
                            .disabled(!editable || !visible)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.begin_edit(uuid.clone(), window, cx)
                            })),
                    );
                }
                let mut group = div()
                    .w_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .id(SharedString::from(format!("ai-message-{sequence}")))
                            .w_full()
                            .min_w_0()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .on_hover(cx.listener(move |this, hovered, _, cx| {
                                let next = if *hovered {
                                    Some(sequence)
                                } else {
                                    this.hovered_message.filter(|current| *current != sequence)
                                };
                                if this.hovered_message != next {
                                    this.hovered_message = next;
                                    cx.notify();
                                }
                            }))
                            .when(assistant || editing, |row| row.items_start())
                            .when(!assistant && !editing, |row| row.items_end())
                            .child(
                                div()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .when(assistant || editing, |body| body.w_full())
                                    .when(!assistant && !editing, |body| {
                                        body.max_w(relative(0.88))
                                            .rounded(px(16.))
                                            .px(px(12.))
                                            .py(px(9.))
                                            .bg(palette.control)
                                    })
                                    .when(!message.attachments.is_empty(), |body| {
                                        body.child(
                                            div()
                                                .min_w_0()
                                                .flex()
                                                .flex_wrap()
                                                .gap(px(4.))
                                                .children(message.attachments.iter().map(
                                                    |attachment| {
                                                        super::attachments::badge(
                                                            attachment, palette,
                                                        )
                                                    },
                                                )),
                                        )
                                    })
                                    .child(content),
                            )
                            .when(!editing, |row| row.child(actions)),
                    );
                for block in blocks {
                    group = group.child(self.command_card(block, view, current.as_ref(), cx));
                }
                let key = message.sequence;
                let owner = cx.weak_entity();
                messages =
                    messages.child(group.on_children_prepainted(move |bounds, window, cx| {
                        let Some((first, last)) = bounds.first().zip(bounds.last()) else {
                            return;
                        };
                        let measured_width = f32::from(first.size.width);
                        let measured_height = f32::from(last.bottom() - first.top());
                        let owner = owner.clone();
                        window.defer(cx, move |_, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                if let Some(delta) = this.message_rows.measure(
                                    key,
                                    revision,
                                    measured_width,
                                    measured_height,
                                ) {
                                    if !this.follow && delta.abs() >= 0.5 {
                                        let mut offset = this.scroll.offset();
                                        offset.y -= px(delta);
                                        this.scroll.set_offset(offset);
                                    }
                                    cx.notify();
                                }
                            });
                        });
                    }));
            }
            if view.stopped && view.error.is_none() {
                messages = messages.child(
                    div()
                        .text_size(px(10.))
                        .text_color(palette.muted)
                        .child(crate::localization::text(cx, "common.label.stopped")),
                );
            }
        }
        let selection_owner = cx.weak_entity();
        let messages = messages.context_menu(move |menu, window, cx| {
            let selection_owner = selection_owner.clone();
            let text = TextSelection::selected_text(window, cx);
            menu.palette(palette)
                .item(
                    MenuItem::localized("Copy")
                        .icon(IconName::Copy)
                        .disabled(text.is_empty())
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                        }),
                )
                .separator()
                .item(
                    MenuItem::localized("Select all").on_click(move |_, window, cx| {
                        let _ = selection_owner
                            .update(cx, |this, cx| this.select_all_messages(window, cx));
                    }),
                )
        });
        div()
            .flex_1()
            .min_h_0()
            .relative()
            .child(messages)
            .when(!self.follow, |body| {
                body.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(12.))
                        .flex()
                        .justify_center()
                        .child(
                            Button::new("ai-jump-bottom")
                                .ghost()
                                .custom(ButtonCustomVariant::for_surface(palette))
                                .size(px(26.))
                                .rounded_full()
                                .border_1()
                                .border_color(palette.border)
                                .bg(palette.surface)
                                .tooltip("Back to bottom")
                                .when(generating, |button| {
                                    button.child(typing_dots(palette, cx.reduce_motion()))
                                })
                                .when(!generating, |button| {
                                    button.icon(IconName::ArrowDown).icon_size(px(13.))
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.jump_to_bottom(window, cx)
                                })),
                        ),
                )
            })
            .into_any_element()
    }
}
