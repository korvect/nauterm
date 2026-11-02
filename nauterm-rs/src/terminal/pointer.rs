//! Protocol routing over the mouse-mode bits in the native snapshot.
use super::Terminal;
use crate::terminal_search::Selection;
use gpui_kit::{
    App, Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta,
    ScrollEvent, TouchPhase, Window,
};
use gpui_kit::{Modifiers, MouseButton};

const CLICK: u32 = 1 << 4;
const DRAG: u32 = 1 << 5;
const MOTION: u32 = 1 << 6;
const SGR: u32 = 1 << 7;

pub fn reporting(mode: u32) -> bool {
    mode & (CLICK | DRAG | MOTION) != 0
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Press,
    Release,
    Motion,
}
pub fn button_code(button: MouseButton) -> Option<u8> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Middle => Some(1),
        MouseButton::Right => Some(2),
        MouseButton::Navigate(_) => None,
    }
}

/// Coordinates are zero-based viewport cells. Legacy coordinates beyond 223
/// are unrepresentable and must not wrap or turn into a different cell.
pub fn encode(
    mode: u32,
    action: Action,
    button: Option<u8>,
    cell: (u32, u32),
    modifiers: Modifiers,
) -> Option<Vec<u8>> {
    if !reporting(mode)
        || (action == Action::Motion
            && mode & MOTION == 0
            && (mode & DRAG == 0 || button.is_none()))
        || (action != Action::Motion && button.is_none())
    {
        return None;
    }
    let sgr = mode & SGR != 0;
    let mut code = if action == Action::Release && !sgr {
        3
    } else {
        button.unwrap_or(3)
    };
    code += 4 * u8::from(modifiers.shift)
        + 8 * u8::from(modifiers.alt)
        + 16 * u8::from(modifiers.control)
        + 32 * u8::from(action == Action::Motion);
    if sgr {
        Some(
            format!(
                "\x1b[<{code};{};{}{}",
                cell.0 + 1,
                cell.1 + 1,
                if action == Action::Release { 'm' } else { 'M' }
            )
            .into_bytes(),
        )
    } else if cell.0 <= 222 && cell.1 <= 222 {
        Some(vec![
            0x1b,
            b'[',
            b'M',
            code + 32,
            cell.0 as u8 + 33,
            cell.1 as u8 + 33,
        ])
    } else {
        None
    }
}

#[derive(Default)]
pub struct Reporting {
    pub pressed: Option<MouseButton>,
    pub last_cell: Option<(u32, u32)>,
}
impl Reporting {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy, Default)]
pub struct Drag {
    pub anchor: i64,
    pub initial: Option<Selection>,
    pub single_click: bool,
    pub had_focus: bool,
    pub moved: bool,
    pub previous_block: Option<Selection>,
}
impl Drag {
    pub fn extend(self, head: i64) -> Option<Selection> {
        if head == self.anchor && !self.moved {
            self.initial
        } else {
            Some(Selection::between_cells(self.anchor, head))
        }
    }
    pub fn should_select_block(self, enabled: bool) -> bool {
        enabled && self.single_click && self.had_focus && !self.moved
    }
}

impl Terminal {
    pub(super) fn viewport_cell(&self, position: Point<Pixels>) -> (u32, u32) {
        let column = ((position.x - self.bounds.left()) / self.cell_width)
            .floor()
            .max(0.) as u32;
        let row = ((position.y - self.bounds.top()) / self.cell_height)
            .floor()
            .max(0.) as u32;
        (
            column.min(self.frame.columns.saturating_sub(1)),
            row.min(self.frame.rows.saturating_sub(1)),
        )
    }
    pub(super) fn reports_mouse(&self) -> bool {
        self.appearance.report_mouse_events && reporting(self.frame.keyboard_mode)
    }
    fn send_mouse(&mut self, bytes: Option<Vec<u8>>, cx: &mut Context<Self>) {
        if let Some(bytes) = bytes
            && let Err(error) = self.session.send(&bytes)
        {
            self.error = Some(error.to_string());
            cx.notify();
        }
    }
    pub(super) fn cancel_mouse(&mut self, cx: &mut Context<Self>) {
        self.prompt_click = None;
        if let Some(button) = self.mouse.pressed
            && self.reports_mouse()
            && let Some(cell) = self.mouse.last_cell
        {
            self.send_mouse(
                encode(
                    self.frame.keyboard_mode,
                    Action::Release,
                    button_code(button),
                    cell,
                    Modifiers::default(),
                ),
                cx,
            );
        }
        self.mouse.clear();
    }
    pub(super) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.frame.columns == 0 || self.frame.rows == 0 {
            return;
        }
        self.prompt_click = None;
        let had_focus = self.focus.is_focused(window);
        let previous_block = self.command_block.as_ref().map(|block| block.selection);
        self.focus.focus(window, cx);
        self.selecting = None;
        self.selection_drag = None;
        if self.press_link(event) {
            window.prevent_default();
            return;
        }
        if !self.session.is_replay() && self.press_prompt_click(event) {
            window.prevent_default();
            return;
        }
        if self.reports_mouse() && button_code(event.button).is_some() {
            let cell = self.viewport_cell(event.position);
            self.mouse.pressed = Some(event.button);
            self.mouse.last_cell = Some(cell);
            self.clear_selection();
            self.send_mouse(
                encode(
                    self.frame.keyboard_mode,
                    Action::Press,
                    button_code(event.button),
                    cell,
                    event.modifiers,
                ),
                cx,
            );
        } else if event.button == MouseButton::Left {
            let anchor = self.offset(event.position);
            let count = event.click_count.saturating_sub(1) % 3;
            if count != 0 || had_focus {
                self.command_block = None;
                self.selection = match count {
                    1 => match self.session.word_selection_at(
                        anchor,
                        &self.appearance.word_boundaries,
                        &self.frame,
                    ) {
                        Ok(selection) => selection,
                        Err(error) => {
                            self.error = Some(error.to_string());
                            None
                        }
                    },
                    2 => {
                        let columns = i64::from(self.frame.columns);
                        let start = anchor.div_euclid(columns) * columns;
                        Some(Selection {
                            start,
                            end: start + columns,
                        })
                    }
                    _ => None,
                };
            }
            self.selecting = Some(Drag {
                anchor,
                initial: self.selection,
                single_click: count == 0,
                had_focus,
                previous_block,
                moved: false,
            });
            self.start_selection_drag(window, cx);
        }
        window.prevent_default();
        // Parent pane activation still needs the gesture. Default prevention
        // keeps ancestor context menus from claiming an already handled click.
        cx.notify();
    }
    pub(super) fn mouse_up(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        if self.release_link(event, cx) {
            return;
        }
        if self.release_prompt_click(event, cx) {
            return;
        }
        if self.mouse.pressed == Some(event.button) {
            if self.reports_mouse() {
                self.send_mouse(
                    encode(
                        self.frame.keyboard_mode,
                        Action::Release,
                        button_code(event.button),
                        self.viewport_cell(event.position),
                        event.modifiers,
                    ),
                    cx,
                );
            }
            self.mouse.clear();
        }
        if event.button == MouseButton::Left
            && let Some(drag) = self.selecting.take()
        {
            self.finish_selection(drag, event, cx);
        }
    }
    pub(super) fn report_motion(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.reports_mouse() {
            self.mouse.clear();
            return;
        }
        if self.mouse.pressed.is_some() && event.pressed_button != self.mouse.pressed {
            self.cancel_mouse(cx);
            return;
        }
        // Another control's press must not turn into a terminal drag when its
        // pointer crosses the terminal. Idle motion is handled only by its hitbox.
        if event.pressed_button.is_some() && self.mouse.pressed.is_none() {
            return;
        }
        let cell = self.viewport_cell(event.position);
        if self.mouse.last_cell == Some(cell) {
            return;
        }
        let bytes = encode(
            self.frame.keyboard_mode,
            Action::Motion,
            self.mouse.pressed.and_then(button_code),
            cell,
            event.modifiers,
        );
        if bytes.is_some() {
            self.mouse.last_cell = Some(cell);
            self.send_mouse(bytes, cx);
        }
    }
    pub(super) fn mouse_wheel(
        &mut self,
        event: &ScrollEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.touch_phase == TouchPhase::Started {
            self.wheel_remainder = 0.;
        }
        if self.reports_mouse() {
            let lines = match event.delta {
                ScrollDelta::Lines(delta) => {
                    self.wheel_remainder = 0.;
                    if delta.y.is_finite() && delta.y != 0. {
                        delta.y.signum() as i32
                    } else {
                        0
                    }
                }
                delta => {
                    super::wheel_scroll_lines(delta, self.cell_height, &mut self.wheel_remainder)
                }
            };
            let button = if lines > 0 { 64 } else { 65 };
            let cell = self.viewport_cell(event.position);
            for _ in 0..lines.unsigned_abs().min(100) {
                self.send_mouse(
                    encode(
                        self.frame.keyboard_mode,
                        Action::Press,
                        Some(button),
                        cell,
                        event.modifiers,
                    ),
                    cx,
                );
            }
        } else {
            let before = self.frame.display_offset;
            let lines =
                super::wheel_scroll_lines(event.delta, self.cell_height, &mut self.wheel_remainder);
            if lines != 0 {
                self.session.scroll(lines);
                self.refresh(cx);
                if self.selecting.is_some() {
                    self.extend_selection(event.position, cx);
                }
            }
            crate::scroll_trace::record(
                "terminal",
                event,
                format_args!(
                    "cell_height={:?} requested_lines={lines} display_offset={before}->{}",
                    self.cell_height, self.frame.display_offset,
                ),
            );
        }
        cx.stop_propagation();
    }
    pub(super) fn copy_selection(&self, cx: &mut App) {
        if let Some(selection) = self.effective_selection() {
            let (start, end) =
                selection.native_offsets(self.frame.columns, self.frame.history_lines);
            if let Some(text) = self.session.selected_text(start, end)
                && !text.is_empty()
            {
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mouse_modes_route_clicks_drag_motion_release_and_wheel() {
        let mods = Modifiers::default();
        let cell = (8, 3);
        assert_eq!(encode(0, Action::Press, Some(0), cell, mods), None);
        assert_eq!(encode(CLICK, Action::Motion, Some(0), cell, mods), None);
        assert_eq!(encode(DRAG, Action::Motion, None, cell, mods), None);
        assert_eq!(
            encode(DRAG | SGR, Action::Motion, Some(0), cell, mods),
            Some(b"\x1b[<32;9;4M".to_vec())
        );
        assert_eq!(
            encode(MOTION | SGR, Action::Motion, None, cell, mods),
            Some(b"\x1b[<35;9;4M".to_vec())
        );
        assert_eq!(
            encode(CLICK | SGR, Action::Release, Some(2), cell, mods),
            Some(b"\x1b[<2;9;4m".to_vec())
        );
        assert_eq!(
            encode(CLICK, Action::Release, Some(2), (0, 0), mods),
            Some(b"\x1b[M#!!".to_vec())
        );
        assert_eq!(
            encode(CLICK | SGR, Action::Press, Some(64), cell, mods),
            Some(b"\x1b[<64;9;4M".to_vec())
        );
        assert_eq!(encode(CLICK, Action::Press, Some(0), (223, 0), mods), None);
        assert_eq!(
            encode(CLICK, Action::Press, Some(0), (222, 0), mods).unwrap()[4],
            255
        );
        assert_eq!(
            encode(
                CLICK | SGR,
                Action::Press,
                Some(0),
                cell,
                Modifiers {
                    shift: true,
                    alt: true,
                    control: true,
                    ..mods
                }
            ),
            Some(b"\x1b[<28;9;4M".to_vec())
        );
    }
    #[test]
    fn stationary_multi_click_retains_native_selection_and_drag_is_bidirectional() {
        let selection = Selection { start: 20, end: 28 };
        let drag = Drag {
            anchor: 23,
            initial: Some(selection),
            ..Default::default()
        };
        assert_eq!(drag.extend(23), Some(selection));
        assert_eq!(drag.extend(18), Some(Selection { start: 18, end: 24 }));
        assert_eq!(
            Drag {
                anchor: 23,
                initial: None,
                ..Default::default()
            }
            .extend(23),
            None
        );
        let single = Drag {
            single_click: true,
            had_focus: true,
            ..Default::default()
        };
        assert!(single.should_select_block(true));
        assert!(!single.should_select_block(false));
        assert!(
            !Drag {
                had_focus: false,
                ..single
            }
            .should_select_block(true)
        );
        assert!(
            !Drag {
                single_click: false,
                ..single
            }
            .should_select_block(true)
        );
        let moved = Drag {
            moved: true,
            ..single
        };
        assert!(!moved.should_select_block(true));
        assert_eq!(
            moved.extend(moved.anchor),
            Some(Selection {
                start: moved.anchor,
                end: moved.anchor + 1
            })
        );
    }
}
