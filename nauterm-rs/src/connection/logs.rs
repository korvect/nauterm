//! Read-only diagnostics use one Base text-selection participant. Timestamp
//! and message have independent layouts, but copy remains one logical document.
use crate::{
    components::{
        ScrollbarsExt,
        menu::{ContextMenuExt, MenuItem},
    },
    connection_state::{LogEntry, Severity},
    palette::Palette,
};
use gpui_kit::{
    assets::IconName,
    base::{TextSelection, TextSelectionHandle, TextSelectionRegistration, TextSelectionRun},
    prelude::*,
    *,
};
use std::{cell::RefCell, ops::Range, rc::Rc};

pub(super) struct View {
    entries: Vec<LogEntry>,
    revision: Option<(u64, u64, usize)>,
    dark: bool,
    visible: bool,
    focus: FocusHandle,
    document: Rc<DocumentState>,
    scroll: ScrollHandle,
    _selection: Subscription,
}
struct DocumentState {
    selection: TextSelectionHandle,
    geometry: RefCell<Vec<TextSelectionRun>>,
    source: RefCell<String>,
}
impl View {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let selection = TextSelectionHandle::new("", cx);
        let focus = cx.focus_handle();
        let selection_focus = focus.clone();
        selection.focus_with(move |window, cx| selection_focus.focus(window, cx), cx);
        let document = Rc::new(DocumentState {
            selection: selection.clone(),
            geometry: RefCell::default(),
            source: RefCell::default(),
        });
        // Clipboard requests can arrive while View is handling a key. Read a
        // separate document, never re-enter that borrowed GPUI entity. The
        // weak capture also avoids a selection-handle/copy-callback cycle.
        let owner = Rc::downgrade(&document);
        selection.copy_with(
            move |cx| {
                let Some(document) = owner.upgrade() else {
                    return String::new();
                };
                if document.selection.has_local_selection(cx) {
                    return document.source.borrow().clone();
                }
                let runs = document.geometry.borrow();
                let projection = document.selection.update_runs(&runs, cx);
                let text: Vec<_> = runs.iter().map(|run| run.text().clone()).collect();
                selected_text(&text, projection.ranges())
            },
            cx,
        );
        let owner = cx.weak_entity();
        let subscription = selection.subscribe(
            move |_, cx| {
                let _ = owner.update(cx, |_, cx| cx.notify());
            },
            cx,
        );
        Self {
            entries: Vec::new(),
            revision: None,
            dark: false,
            visible: false,
            focus,
            document,
            scroll: ScrollHandle::new(),
            _selection: subscription,
        }
    }
    pub fn configure(
        &mut self,
        entries: &[LogEntry],
        dark: bool,
        visible: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let revision = entries
            .first()
            .zip(entries.last())
            .map(|(first, last)| (first.sequence, last.sequence, entries.len()));
        let head_changed =
            self.revision.map(|revision| revision.0) != revision.map(|revision| revision.0);
        if (self.visible && !visible) || head_changed {
            if self.document.selection.snapshot(cx).is_some()
                || self.document.selection.has_local_selection(cx)
            {
                TextSelection::clear(window, cx);
            }
            self.document.geometry.borrow_mut().clear();
        }
        let changed = self.dark != dark || self.visible != visible || self.revision != revision;
        if self.revision != revision {
            self.entries = entries.to_vec();
            self.revision = revision;
            let source = self.plain_text();
            self.document
                .selection
                .set_fallback_copy_text(source.clone(), cx);
            *self.document.source.borrow_mut() = source;
        }
        self.dark = dark;
        self.visible = visible;
        if changed {
            cx.notify();
        }
    }
    fn plain_text(&self) -> String {
        self.entries
            .iter()
            .map(|entry| {
                let (time, message) = fields(entry);
                format!("{time}{message}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let primary = if cfg!(target_os = "macos") {
            key.modifiers.platform
        } else {
            key.modifiers.control
        };
        if primary && !key.modifiers.alt && !key.modifiers.shift {
            match key.key.as_str() {
                "a" => {
                    self.select_all(window, cx);
                    cx.stop_propagation();
                }
                "c" => {
                    let text = TextSelection::selected_text(window, cx);
                    if !text.is_empty() {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                    cx.stop_propagation();
                }
                _ => {}
            }
        }
    }
    fn select_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        TextSelection::clear(window, cx);
        self.document.selection.set_local_selection(true, cx);
        self.focus.focus(window, cx);
        cx.notify();
    }
}
fn fields(entry: &LogEntry) -> (SharedString, SharedString) {
    (
        format!(
            "{}  ",
            if entry.time.is_empty() {
                "--:--:--"
            } else {
                &entry.time
            }
        )
        .into(),
        format!("{}  {}", entry.category, entry.event.message).into(),
    )
}
fn color(severity: Severity, p: Palette) -> Rgba {
    match severity {
        Severity::Info => p.muted,
        Severity::Success => rgb(0x14a765),
        Severity::Warning => rgb(0xbf7b12),
        Severity::Error => rgb(0xd54a45),
    }
}
fn selected_text(parts: &[SharedString], ranges: &[Option<Range<usize>>]) -> String {
    parts
        .chunks(2)
        .enumerate()
        .filter_map(|(row, fields)| {
            let text: String = fields
                .iter()
                .enumerate()
                .filter_map(|(field, text)| {
                    ranges
                        .get(row * 2 + field)
                        .and_then(Option::as_ref)
                        .and_then(|range| text.get(range.clone()))
                })
                .collect();
            (!text.is_empty()).then_some(text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Render for View {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.dark);
        let mut content = div().w_full().min_w_0().flex().flex_col();
        let mut layouts = Vec::new();
        for entry in &self.entries {
            let (time, message) = fields(entry);
            let time_text = StyledText::new(time.clone());
            let message_text = StyledText::new(message.clone()).with_highlights([(
                0..entry.category.len(),
                HighlightStyle {
                    color: Some(color(entry.severity(), p).into()),
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                },
            )]);
            layouts.push((time, time_text.layout().clone()));
            layouts.push((message, message_text.layout().clone()));
            content = content.child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_start()
                    .pb(px(8.))
                    .child(
                        div()
                            .w(px(68.))
                            .flex_shrink_0()
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .line_height(relative(1.35))
                            .child(time_text),
                    )
                    .child(
                        div()
                            .size(px(7.))
                            .flex_shrink_0()
                            .mt(px(5.))
                            .mr(px(9.))
                            .rounded_full()
                            .bg(color(entry.severity(), p)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .line_height(relative(1.35))
                            .text_color(p.text)
                            .child(message_text),
                    ),
            );
        }
        let document = Document {
            content: content.into_any_element(),
            layouts,
            state: self.document.clone(),
            scroll: self.scroll.clone(),
            selection_color: p.accent.alpha(0.3),
        };
        let owner = cx.weak_entity();
        let empty = self.entries.is_empty();
        div()
            .id("connection-log-scroll")
            .size_full()
            .min_h_0()
            .min_w_0()
            .track_focus(&self.focus)
            .key_context("ConnectionLogs")
            .role(Role::Log)
            .aria_label(crate::localization::text(cx, "Connection diagnostics"))
            .on_key_down(cx.listener(Self::key_down))
            .cursor_text()
            .overflow_y_scroll()
            .scrollbars_with(&self.scroll)
            .when(self.entries.is_empty(), |view| {
                view.child(
                    div()
                        .text_size(px(13.))
                        .line_height(relative(1.35))
                        .text_color(p.muted)
                        .child(crate::localization::text(
                            cx,
                            "workspace.description.waitingForConnectionEvents",
                        )),
                )
            })
            .child(document)
            .context_menu(move |menu, window, cx| {
                let text = TextSelection::selected_text(window, cx);
                let owner = owner.clone();
                menu.item(
                    MenuItem::localized("Copy")
                        .icon(IconName::Copy)
                        .disabled(text.is_empty())
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                        }),
                )
                .separator()
                .item(MenuItem::localized("Select all").disabled(empty).on_click(
                    move |_, window, cx| {
                        let _ = owner.update(cx, |view, cx| view.select_all(window, cx));
                    },
                ))
            })
    }
}

struct Document {
    content: AnyElement,
    layouts: Vec<(SharedString, TextLayout)>,
    state: Rc<DocumentState>,
    scroll: ScrollHandle,
    selection_color: Rgba,
}
impl IntoElement for Document {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Document {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        Some("connection-log-document".into())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.content.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.prepaint(window, cx);
        let viewport = bounds.intersect(&window.content_mask().bounds);
        let hitbox = window.insert_hitbox(viewport, HitboxBehavior::Normal);
        let registration = TextSelectionRegistration::new(hitbox, viewport)
            .with_scroll_offset(self.scroll.offset())
            .with_text_bounds(
                self.layouts
                    .iter()
                    .map(|(_, layout)| layout.bounds())
                    .collect(),
            )
            .with_rendered_element(&self.state.selection, window, cx);
        self.state.selection.register(registration, window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let runs: Vec<_> = self
            .layouts
            .iter()
            .enumerate()
            .map(|(index, (text, layout))| {
                TextSelectionRun::new(text.clone(), layout.clone(), layout.bounds())
                    .with_document_order(index as u64)
            })
            .collect();
        let projection = self.state.selection.update_runs(&runs, cx);
        for (run, range) in runs.iter().zip(projection.ranges()) {
            let range = if self.state.selection.has_local_selection(cx) {
                Some(0..run.text().len())
            } else {
                range.clone()
            };
            if let Some(range) = range {
                paint_selection(run.layout(), range, self.selection_color, window);
            }
        }
        *self.state.geometry.borrow_mut() = runs;
        self.content.paint(window, cx);
    }
}
fn paint_selection(layout: &TextLayout, range: Range<usize>, color: Rgba, window: &mut Window) {
    let (Some(start), Some(end)) = (
        layout.position_for_index(range.start),
        layout.position_for_index(range.end),
    ) else {
        return;
    };
    let bounds = layout.bounds();
    let height = layout.line_height();
    if start.y == end.y {
        window.paint_quad(fill(
            Bounds::from_corners(start, point(end.x, end.y + height)),
            color,
        ));
    } else {
        window.paint_quad(fill(
            Bounds::from_corners(start, point(bounds.right(), start.y + height)),
            color,
        ));
        if end.y > start.y + height {
            window.paint_quad(fill(
                Bounds::from_corners(
                    point(bounds.left(), start.y + height),
                    point(bounds.right(), end.y),
                ),
                color,
            ));
        }
        window.paint_quad(fill(
            Bounds::from_corners(point(bounds.left(), end.y), point(end.x, end.y + height)),
            color,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::selected_text;
    use gpui_kit::SharedString;
    #[test]
    fn copied_rows_keep_inline_fields_and_line_breaks_without_visual_dots() {
        let parts: Vec<SharedString> = [
            "10:41:18  ",
            "transport  Opening SSH",
            "10:41:19  ",
            "auth  密钥已拒绝",
        ]
        .map(Into::into)
        .into();
        let full: Vec<_> = parts.iter().map(|part| Some(0..part.len())).collect();
        assert_eq!(
            selected_text(&parts, &full),
            "10:41:18  transport  Opening SSH\n10:41:19  auth  密钥已拒绝"
        );
        assert_eq!(
            selected_text(&parts, &[None, Some(11..22), Some(0..10), Some(0..12)]),
            "Opening SSH\n10:41:19  auth  密钥"
        );
        assert_eq!(selected_text(&parts, &[None, None, None, None]), "");
    }
}
