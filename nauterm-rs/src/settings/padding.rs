use super::*;
use crate::terminal_padding::Padding;

pub(super) struct Control {
    input: Entity<InputState>,
    _events: Subscription,
}
impl Settings {
    pub(super) fn init_padding_control(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.padding_control.is_some() {
            return;
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.values.terminal.padding.format())
                .placeholder("all · vertical, horizontal · top, right, bottom, left")
        });
        let events = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                && this.finish_padding_editing(window, cx)
            {
                this.changed(window, cx);
            }
        });
        self.padding_control = Some(Control {
            input,
            _events: events,
        });
    }
    pub(super) fn finish_padding_editing(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(control) = &self.padding_control else {
            return false;
        };
        let input = control.input.clone();
        match Padding::parse(&input.read(cx).value()) {
            Ok(padding) => {
                let changed = padding != self.values.terminal.padding;
                self.values.terminal.padding = padding;
                let formatted = padding.format();
                if input.read(cx).value() != formatted {
                    input.update(cx, |input, cx| input.set_value(formatted, window, cx));
                }
                changed
            }
            Err(_) => {
                // Match the reference: invalid shorthand reverts on commit.
                input.update(cx, |input, cx| {
                    input.set_value(self.values.terminal.padding.format(), window, cx)
                });
                false
            }
        }
    }
    pub(super) fn padding_row(&self, cx: &App) -> AnyElement {
        self.row(
            "settings.terminal.padding.label",
            "rs.settings.terminal.padding.description",
            Input::new(
                &self
                    .padding_control
                    .as_ref()
                    .expect("padding control initialized")
                    .input,
            )
            .aria_label(crate::localization::text(cx, "Terminal Padding")),
        )
    }
}
