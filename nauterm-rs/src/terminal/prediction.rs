//! Local predictive echo is paint-only; SSH and Mosh reconcile independently.
use super::*;
use crate::terminal_prediction::{Adaptive, Position, Prediction, cell_width};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub(super) struct State {
    model: Prediction,
    mosh: crate::mosh::prediction::Prediction,
    adaptive: Adaptive,
    expiry: Option<Task<()>>,
    deadline: Option<Instant>,
}
impl State {
    pub fn new(logs: &[LogEntry]) -> Self {
        let mut state = Self::default();
        // Connection-page reveal may have consumed the initial echo/latency
        // events. Replay their transient state before accepting terminal input.
        for log in logs {
            state.mosh.event(&log.event);
        }
        state
    }
    pub fn event(&mut self, event: &crate::native::SessionEvent) -> bool {
        self.mosh.event(event)
    }
}
impl Terminal {
    fn prediction_enabled(&mut self) -> bool {
        let adaptive = self.prediction.adaptive.update(self.connection.latency_ms);
        self.reconnect.profile.is_some()
            && !self.session.is_mosh()
            && !self.session.is_replay()
            && self.phase() == Phase::Completing
            && self.appearance.ssh_prediction.enabled(adaptive)
            && !self.composer_sensitive()
    }
    pub(super) fn reset_prediction(&mut self, cx: &mut Context<Self>) {
        let changed = !self.prediction.model.is_empty() || !self.prediction.mosh.is_empty();
        self.prediction = State::default();
        if changed {
            cx.notify();
        }
    }
    pub(super) fn suspend_prediction(&mut self, cx: &mut Context<Self>) {
        let changed = self.prediction.model.suspend() | self.prediction.mosh.suspend();
        self.prediction.expiry = None;
        self.prediction.deadline = None;
        if changed {
            cx.notify();
        }
    }
    pub(super) fn prediction_input(
        &mut self,
        bytes: &[u8],
        accepted: bool,
        sensitive: bool,
        cx: &mut Context<Self>,
    ) {
        if !accepted || sensitive {
            self.suspend_prediction(cx);
            return;
        }
        if self.session.is_mosh() {
            let changed = if self.phase() == Phase::Completing
                && !self.session.is_replay()
                && !self.composer_sensitive()
            {
                match std::str::from_utf8(bytes) {
                    Ok(text) => self.prediction.mosh.input(text, &self.frame),
                    Err(_) => self.prediction.mosh.suspend(),
                }
            } else {
                self.prediction.mosh.clear()
            };
            if changed {
                cx.notify();
            }
            return;
        }
        if !self.prediction_enabled() {
            if self.prediction.model.clear() {
                cx.notify();
            }
            self.prediction_expiry(cx);
            return;
        }
        let changed = match std::str::from_utf8(bytes) {
            Ok(text) => self.prediction.model.add(text, &self.frame),
            Err(_) => self.prediction.model.suspend(),
        };
        self.prediction_expiry(cx);
        if changed {
            cx.notify();
        }
    }
    pub(super) fn reconcile_prediction(&mut self, screen_changed: bool, cx: &mut Context<Self>) {
        if self.session.is_mosh() {
            let changed = if self.phase() == Phase::Completing
                && !self.session.is_replay()
                && !self.composer_sensitive()
            {
                self.prediction.mosh.reconcile(&self.frame, screen_changed)
            } else {
                self.prediction.mosh.clear()
            };
            if changed {
                cx.notify();
            }
            return;
        }
        let changed = if !self.prediction_enabled() {
            self.prediction.model.clear()
        } else if screen_changed {
            self.prediction.model.reconcile(&self.frame)
        } else {
            false
        };
        if changed || self.prediction.model.is_empty() {
            self.prediction_expiry(cx);
        }
        if changed {
            cx.notify();
        }
    }
    fn prediction_expiry(&mut self, cx: &mut Context<Self>) {
        self.prediction.expiry = None;
        self.prediction.deadline = None;
        if self.prediction.model.is_empty() {
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        self.prediction.deadline = Some(deadline);
        self.prediction.expiry = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(deadline.saturating_duration_since(Instant::now()))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.prediction.deadline == Some(deadline) {
                    this.prediction.model.clear();
                    this.prediction.expiry = None;
                    this.prediction.deadline = None;
                    cx.notify();
                }
            });
        }));
    }
    pub(super) fn prediction_display(&self) -> Display {
        if self.session.is_mosh() {
            return Display {
                text: if Prediction::allows(&self.frame) {
                    self.prediction
                        .mosh
                        .units()
                        .map(|unit| unit.text.as_str())
                        .collect()
                } else {
                    String::new()
                },
                start: self
                    .prediction
                    .mosh
                    .start()
                    .unwrap_or_else(|| Position::cursor(&self.frame)),
            };
        }
        Display {
            text: if Prediction::allows(&self.frame) {
                self.prediction
                    .model
                    .units()
                    .map(|unit| unit.text.as_str())
                    .collect()
            } else {
                String::new()
            },
            start: self
                .prediction
                .model
                .start()
                .unwrap_or_else(|| Position::cursor(&self.frame)),
        }
    }
}

pub(super) struct Display {
    pub text: String,
    pub start: Position,
}
pub(super) struct Paint<'a> {
    pub text: &'a str,
    pub frame: &'a Frame,
    pub appearance: &'a Appearance,
    pub font: &'a Font,
    pub cell: Size<Pixels>,
    pub focused: bool,
    pub start: Position,
}
impl Paint<'_> {
    pub fn paint(self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if self.text.is_empty() {
            return;
        }
        let metrics = decorations::Metrics::measure(
            self.font,
            self.appearance.size,
            self.cell.height,
            window,
        );
        let foreground = rgb(self.appearance.palette().base.foreground);
        let mut position = self.start;
        for text in self.text.graphemes(true) {
            let width = cell_width(text);
            if width == 0 {
                continue;
            }
            position = position.fit(width, self.frame.columns);
            if position.row >= self.frame.rows {
                break;
            }
            let origin = bounds.origin
                + point(
                    self.cell.width * position.column as f32,
                    self.cell.height * position.row as f32,
                );
            let text: SharedString = text.to_owned().into();
            let run = TextRun {
                len: text.len(),
                font: self.font.clone(),
                color: foreground.alpha(0.72).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line =
                window
                    .text_system()
                    .shape_line(text, px(self.appearance.size), &[run], None);
            let _ = line.paint(origin, self.cell.height, TextAlign::Left, None, window, cx);
            window.paint_quad(fill(
                Bounds::new(
                    point(origin.x, origin.y + metrics.underline),
                    size(self.cell.width * width as f32, metrics.thickness),
                ),
                foreground.alpha(0.42),
            ));
            position = position.after(width, self.frame.columns);
        }
        if self.focused && position.row < self.frame.rows {
            window.paint_quad(fill(
                Bounds::new(
                    bounds.origin
                        + point(
                            self.cell.width * position.column as f32,
                            self.cell.height * position.row as f32 + px(2.),
                        ),
                    size(metrics.thickness, (self.cell.height - px(4.)).max(px(0.))),
                ),
                rgb(self.frame.cursor_color).alpha(0.9),
            ));
        }
    }
}
