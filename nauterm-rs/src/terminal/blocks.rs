//! Transient native command-block selection and its passive focus painting.
use super::*;
use anyhow::{Result, ensure};

#[derive(serde::Deserialize)]
pub(crate) struct RawBlock {
    #[serde(default)]
    pub id: u64,
    pub start: i64,
    pub end: i64,
    pub working_directory: Option<String>,
    #[serde(default)]
    pub shell_integrated: bool,
}
#[derive(Clone)]
pub(crate) struct Block {
    pub selection: Selection,
    pub directory: Option<String>,
    pub prompt: String,
    id: u64,
    integrated: bool,
}
impl RawBlock {
    pub fn into_block(self, absolute: i64, frame: &Frame, prompt: String) -> Result<Block> {
        let selection = Selection::from_native(
            (self.start, self.end),
            frame.columns,
            frame.rows,
            frame.history_lines,
        )?;
        ensure!(
            selection.contains(absolute),
            "Command context missed its reference cell"
        );
        Ok(Block {
            selection,
            id: self.id,
            integrated: self.shell_integrated,
            prompt,
            directory: self
                .working_directory
                .filter(|value| crate::recovery::safe_directory(value)),
        })
    }
}
impl Block {
    fn same_origin(&self, other: &Self) -> bool {
        if self.integrated && other.integrated {
            self.id == other.id
        } else {
            !self.integrated
                && !other.integrated
                && !self.prompt.is_empty()
                && self.prompt == other.prompt
                && self.selection.start == other.selection.start
        }
    }
}
impl Terminal {
    pub(super) fn effective_selection(&self) -> Option<Selection> {
        self.selection
            .or_else(|| self.command_block.as_ref().map(|block| block.selection))
    }
    pub(super) fn clear_selection(&mut self) {
        self.selection = None;
        self.command_block = None;
        self.block_revision = self.block_revision.wrapping_add(1);
        self.selecting = None;
        self.selection_drag = None;
        self.prompt_click = None;
        self.pressed_link = None;
    }
    pub(super) fn finish_selection(
        &mut self,
        drag: super::pointer::Drag,
        event: &MouseUpEvent,
        cx: &mut Context<Self>,
    ) {
        self.selection_drag = None;
        if drag.single_click && !drag.moved && !drag.had_focus {
            return;
        }
        if drag.should_select_block(self.appearance.select_command_block_on_click)
            && self.bounds.contains(&event.position)
            && !self.frame.alternate_screen
            && !self.reports_mouse()
        {
            match self
                .session
                .command_block_at(self.offset(event.position), &self.frame)
            {
                Ok(block) => {
                    self.selection = None;
                    self.command_block =
                        block.filter(|block| Some(block.selection) != drag.previous_block);
                    self.block_revision = self.block_revision.wrapping_add(1);
                    cx.notify();
                }
                Err(error) => {
                    self.error = Some(error.to_string());
                    cx.notify();
                }
            }
        }
        if self.appearance.copy_on_select {
            self.copy_selection(cx);
        }
    }
    pub(super) fn refresh_command_block(&mut self, cx: &mut Context<Self>) {
        let Some(previous) = self.command_block.clone() else {
            return;
        };
        if self.frame.alternate_screen || !self.appearance.select_command_block_on_click {
            self.command_block = None;
            return;
        }
        // One in-flight read per terminal; newer output is coalesced and read
        // again after completion, without a polling timer or a queue of probes.
        if self.block_refresh.is_some() {
            return;
        }
        let frame = self.frame.clone();
        let revision = self.block_revision;
        let request = self
            .session
            .command_block_request(previous.selection.start, frame.clone());
        self.block_refresh = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { request.read() }).await;
            let _ = this.update(cx, |this, cx| {
                this.block_refresh = None;
                let current_frame = Arc::ptr_eq(&this.frame, &frame);
                let current_selection = this.block_revision == revision;
                if current_frame && current_selection && this.command_block.is_some() {
                    this.command_block = result
                        .ok()
                        .flatten()
                        .filter(|block| previous.same_origin(block));
                    cx.notify();
                }
                if this.command_block.is_some() && (!current_frame || !current_selection) {
                    this.refresh_command_block(cx);
                }
            });
        }));
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FocusRows {
    top: u32,
    bottom: u32,
    top_border: bool,
    bottom_border: bool,
}
fn focus_rows(selection: Selection, frame: &Frame) -> Option<FocusRows> {
    if frame.columns == 0 || frame.rows == 0 || selection.start >= selection.end {
        return None;
    }
    let columns = i64::from(frame.columns);
    let viewport = crate::terminal_search::viewport_start(
        frame.columns,
        frame.history_lines,
        frame.display_offset,
    );
    let length = columns * i64::from(frame.rows);
    let start = (selection.start - viewport).clamp(0, length);
    let end = (selection.end - viewport).clamp(0, length);
    Some(FocusRows {
        top: (start / columns) as u32,
        bottom: ((end + columns - 1) / columns) as u32,
        top_border: selection.start >= viewport && selection.start < viewport + length,
        bottom_border: selection.end > viewport && selection.end <= viewport + length,
    })
}
fn luminance(color: u32) -> f32 {
    let channel = |shift: u32| {
        let value = ((color >> shift) & 255_u32) as f32 / 255.;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}
pub(super) fn paint_focus(
    selection: Selection,
    frame: &Frame,
    bounds: Bounds<Pixels>,
    height: Pixels,
    background: u32,
    accent: u32,
    window: &mut Window,
) {
    let Some(rows) = focus_rows(selection, frame) else {
        return;
    };
    // Match Flutter's grid-sized focus painting. The viewport can have spare
    // pixels below its last complete row; that padding is outside the terminal.
    let bounds = Bounds::new(
        bounds.origin,
        size(
            bounds.size.width,
            bounds.size.height.min(height * frame.rows as f32),
        ),
    );
    let top = (height * rows.top as f32).min(bounds.size.height);
    let bottom = (height * rows.bottom as f32).min(bounds.size.height);
    let mut dim = rgb(0);
    dim.a = if luminance(background) > 0.5 {
        0.20
    } else {
        0.30
    };
    if top > px(0.) {
        window.paint_quad(fill(
            Bounds::new(bounds.origin, size(bounds.size.width, top)),
            dim,
        ));
    }
    if bottom < bounds.size.height {
        window.paint_quad(fill(
            Bounds::new(
                point(bounds.left(), bounds.top() + bottom),
                size(bounds.size.width, bounds.size.height - bottom),
            ),
            dim,
        ));
    }
    if top >= bottom {
        return;
    }
    let mut border = rgb(accent);
    border.a = 0.55;
    if rows.top_border {
        window.paint_quad(fill(
            Bounds::new(
                point(bounds.left(), bounds.top() + top),
                size(bounds.size.width, px(1.)),
            ),
            border,
        ));
    }
    if rows.bottom_border {
        window.paint_quad(fill(
            Bounds::new(
                point(bounds.left(), bounds.top() + bottom - px(1.)),
                size(bounds.size.width, px(1.)),
            ),
            border,
        ));
    }
    for x in [bounds.left(), bounds.right() - px(1.)] {
        window.paint_quad(fill(
            Bounds::new(point(x, bounds.top() + top), size(px(1.), bottom - top)),
            border,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn native_block_identity_and_viewport_clipping_do_not_follow_unrelated_output() -> Result<()> {
        let frame = Frame {
            columns: 10,
            rows: 4,
            history_lines: 20,
            display_offset: 10,
            ..Default::default()
        };
        let raw = |id, start, end| RawBlock {
            id,
            start,
            end,
            shell_integrated: true,
            working_directory: None,
        };
        let block = raw(7, -100, -75).into_block(105, &frame, "$ old".into())?;
        assert_eq!(
            focus_rows(block.selection, &frame),
            Some(FocusRows {
                top: 0,
                bottom: 3,
                top_border: true,
                bottom_border: true
            })
        );
        assert!(block.same_origin(&raw(7, -100, -70).into_block(105, &frame, "$ old".into())?));
        assert!(!block.same_origin(&raw(8, -100, -75).into_block(
            105,
            &frame,
            "$ other".into()
        )?));
        assert_eq!(
            focus_rows(
                Selection {
                    start: 90,
                    end: 125
                },
                &frame
            ),
            Some(FocusRows {
                top: 0,
                bottom: 3,
                top_border: false,
                bottom_border: true
            })
        );
        assert_eq!(
            focus_rows(
                Selection {
                    start: 125,
                    end: 150
                },
                &frame
            ),
            Some(FocusRows {
                top: 2,
                bottom: 4,
                top_border: true,
                bottom_border: false
            })
        );
        assert_eq!(
            focus_rows(Selection { start: 0, end: 50 }, &frame),
            Some(FocusRows {
                top: 0,
                bottom: 0,
                top_border: false,
                bottom_border: false
            })
        );
        assert!(luminance(0xfbfbf8) > 0.5);
        assert!(luminance(0x242728) < 0.5);
        Ok(())
    }
}
