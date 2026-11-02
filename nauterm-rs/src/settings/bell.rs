//! Bell preferences share the reference client's fields and Settings controls.
use super::*;

impl Settings {
    pub(super) fn bell_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let bell = self.values.terminal.bell;
        self.section(
            Section::Bell,
            IconName::Bell,
            div()
                .flex()
                .flex_col()
                .gap(px(18.))
                .child(self.row(
                    "settings.terminal.bell.sound.label",
                    "settings.terminal.bell.sound.description",
                    Switch::new("terminal-bell-sound", "Sound", bell.sound).on_change(cx.listener(
                        |this, value, window, cx| {
                            this.values.terminal.bell.sound = *value;
                            this.changed(window, cx);
                        },
                    )),
                ))
                .child(self.row(
                    "settings.terminal.bell.visual.label",
                    "settings.terminal.bell.visual.description",
                    Switch::new("terminal-bell-visual", "Visual Bell", bell.visual).on_change(
                        cx.listener(|this, value, window, cx| {
                            this.values.terminal.bell.visual = *value;
                            this.changed(window, cx);
                        }),
                    ),
                ))
                .child(
                    self.row(
                        "settings.terminal.bell.tabIndicator.label",
                        "settings.terminal.bell.tabIndicator.description",
                        Switch::new("terminal-bell-tab", "Show Bell in Tab", bell.tab_indicator)
                            .on_change(cx.listener(|this, value, window, cx| {
                                this.values.terminal.bell.tab_indicator = *value;
                                this.changed(window, cx);
                            })),
                    ),
                ),
        )
    }
}
