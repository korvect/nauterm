//! Native prompt geometry owns cursor movement; the UI owns the click gesture.
use super::*;
use anyhow::{Result, ensure};

#[derive(Clone, Copy, serde::Deserialize)]
pub(crate) struct Movement {
    pub left: usize,
    pub right: usize,
}
impl Movement {
    pub fn encode(self, mode: u32, max_cells: usize) -> Result<Vec<u8>> {
        ensure!(
            self.left == 0 || self.right == 0,
            "Invalid prompt cursor movement"
        );
        let count = self
            .left
            .checked_add(self.right)
            .filter(|count| *count <= max_cells)
            .ok_or_else(|| anyhow::anyhow!("Prompt cursor movement exceeded its limit"))?;
        if count == 0 {
            return Ok(vec![]);
        }
        let key = Keystroke::parse(if self.left > 0 { "left" } else { "right" })
            .expect("static terminal cursor key");
        Ok(super::encode_key(&key, mode)
            .expect("terminal cursor key encoding")
            .repeat(count))
    }
}

pub(super) struct Gesture {
    start: (u32, u32),
    eligible: bool,
    dragged: bool,
}
impl Gesture {
    fn drag_to(&mut self, cell: (u32, u32), inside: bool) {
        self.dragged |= cell != self.start || !inside;
    }
}

impl Terminal {
    fn can_prompt_click(&self) -> bool {
        self.phase() == Phase::Completing
            && self.frame.cursor_visible
            && !self.frame.alternate_screen
            && self.frame.display_offset == 0
            && !super::pointer::reporting(self.frame.keyboard_mode)
            && self.effective_selection().is_none()
            && self.marked.is_empty()
            && !self.composer_sensitive()
    }
    pub(super) fn press_prompt_click(&mut self, event: &MouseDownEvent) -> bool {
        self.prompt_click = None;
        if !self.appearance.option_click_moves_cursor
            || event.button != MouseButton::Left
            || !event.modifiers.alt
            || event.modifiers.control
            || event.modifiers.platform
        {
            return false;
        }
        self.prompt_click = Some(Gesture {
            start: self.viewport_cell(event.position),
            eligible: self.can_prompt_click(),
            dragged: false,
        });
        // An ineligible Option gesture is consumed, as in the reference. It
        // must not become a selection or a mouse report for a TUI/password prompt.
        true
    }
    pub(super) fn drag_prompt_click(&mut self, event: &MouseMoveEvent) -> bool {
        let cell = self.viewport_cell(event.position);
        let inside = self.bounds.contains(&event.position);
        let Some(gesture) = self.prompt_click.as_mut() else {
            return false;
        };
        if event.pressed_button == Some(MouseButton::Left) {
            gesture.drag_to(cell, inside);
        } else {
            self.prompt_click = None;
        }
        true
    }
    pub(super) fn release_prompt_click(
        &mut self,
        event: &MouseUpEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        let Some(gesture) = self.prompt_click.take() else {
            return false;
        };
        if !gesture.eligible
            || gesture.dragged
            || !self.bounds.contains(&event.position)
            || !self.appearance.option_click_moves_cursor
            || !self.can_prompt_click()
        {
            return true;
        }
        let result = self
            .session
            .prompt_click_move(self.offset(event.position), &self.frame)
            .and_then(|movement| {
                movement
                    .map(|movement| {
                        movement.encode(
                            self.frame.keyboard_mode,
                            self.frame.columns as usize * self.frame.rows as usize,
                        )
                    })
                    .transpose()
            });
        match result {
            Ok(Some(bytes)) if !bytes.is_empty() => self.send(&bytes, cx),
            Ok(_) => {}
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn native_movements_encode_only_bounded_cursor_keys_in_both_modes() -> Result<()> {
        let left: Option<Movement> = serde_json::from_str(r#"{"left":2,"right":0}"#)?;
        let left = left.unwrap();
        assert_eq!(left.encode(0, 500)?, b"\x1b[D\x1b[D");
        assert_eq!(left.encode(1, 500)?, b"\x1bOD\x1bOD");
        assert_eq!(Movement { left: 0, right: 1 }.encode(0, 500)?, b"\x1b[C");
        assert_eq!(Movement { left: 0, right: 1 }.encode(1, 500)?, b"\x1bOC");
        assert!(Movement { left: 0, right: 0 }.encode(0, 500)?.is_empty());
        assert!(Movement { left: 1, right: 1 }.encode(0, 500).is_err());
        assert_eq!(
            Movement {
                left: 1000,
                right: 0
            }
            .encode(0, 2000)?
            .len(),
            3000
        );
        assert!(
            Movement {
                left: 501,
                right: 0
            }
            .encode(0, 500)
            .is_err()
        );
        assert!(
            Movement {
                left: usize::MAX,
                right: 0
            }
            .encode(0, 500)
            .is_err()
        );
        assert!(serde_json::from_str::<Option<Movement>>("null")?.is_none());
        Ok(())
    }
    #[test]
    fn a_drag_out_and_back_cannot_become_a_cursor_click() {
        let mut gesture = Gesture {
            start: (4, 2),
            eligible: true,
            dragged: false,
        };
        gesture.drag_to((4, 2), true);
        assert!(!gesture.dragged);
        gesture.drag_to((5, 2), true);
        gesture.drag_to((4, 2), true);
        assert!(gesture.dragged);
        let mut outside = Gesture {
            start: (4, 2),
            eligible: true,
            dragged: false,
        };
        outside.drag_to((4, 2), false);
        assert!(outside.dragged);
    }
}
