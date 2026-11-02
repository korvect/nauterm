//! Host protocol selection shares the connection page without opening a transport.
use super::*;
use crate::model::{Host, Id};
use crate::typography::FontWeights;
use gpui_kit::component::ActiveTheme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Ssh,
    Mosh,
    Telnet,
}
impl Protocol {
    fn label(self) -> &'static str {
        match self {
            Self::Ssh => "SSH",
            Self::Mosh => "Mosh",
            Self::Telnet => "Telnet",
        }
    }
    fn command(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Mosh => "mosh",
            Self::Telnet => "telnet",
        }
    }
    fn color(self, dark: bool) -> u32 {
        match self {
            Self::Ssh => {
                if dark {
                    0x30d158
                } else {
                    0x35d394
                }
            }
            Self::Mosh => 0x0ea5a8,
            Self::Telnet => 0x7b61ff,
        }
    }
}
pub struct ProtocolSelected {
    pub host_id: Id,
    pub uuid: Option<String>,
    pub protocol: Protocol,
    pub port: u16,
    pub command: Option<String>,
}
impl EventEmitter<ProtocolSelected> for Connection {}

pub(super) struct Choice {
    host_id: Id,
    uuid: Option<String>,
    pub address: String,
    selected: Protocol,
    ports: Vec<(Protocol, Entity<InputState>)>,
    command: Option<Entity<InputState>>,
    starting: bool,
    error: Option<String>,
}
impl Choice {
    fn new(host: Host, window: &mut Window, cx: &mut Context<Connection>) -> Self {
        let mut ports = vec![];
        if host.supports_ssh() {
            ports.push((Protocol::Ssh, host.port));
        }
        if host.is_remote() && host.connections.mosh {
            ports.push((Protocol::Mosh, host.port));
        }
        if host.is_remote()
            && let Some(profile) = &host.connections.telnet
        {
            ports.push((Protocol::Telnet, profile.port));
        }
        let selected = ports
            .first()
            .map(|(protocol, _)| *protocol)
            .unwrap_or(Protocol::Ssh);
        let command = (host.is_remote() && host.connections.mosh).then(|| {
            cx.new(|cx| {
                InputState::new(window, cx).default_value(host.connections.mosh_command.clone())
            })
        });
        Self {
            host_id: host.id,
            uuid: host.uuid,
            address: host.address,
            selected,
            command,
            starting: false,
            error: None,
            ports: ports
                .into_iter()
                .map(|(protocol, port)| {
                    let input =
                        cx.new(|cx| InputState::new(window, cx).default_value(port.to_string()));
                    (protocol, input)
                })
                .collect(),
        }
    }
}
impl Connection {
    pub fn new_host_choice(
        core: Arc<Core>,
        host: Host,
        terminal: TerminalPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let platform = host.platform.clone();
        let choice = Choice::new(host, window, cx);
        let mut this = Self::create_target(
            core,
            Target::Choice(Box::new(choice)),
            terminal.title,
            terminal.dark,
            Presentation {
                attached: None,
                directory: None,
                auto_focus: false,
                terminal_palette: terminal.palette,
                recording: None,
            },
            window,
            cx,
        )
        .with_host_platform(platform);
        let Target::Choice(choice) = &this.target else {
            unreachable!()
        };
        let inputs: Vec<_> = choice
            .ports
            .iter()
            .cloned()
            .chain(
                choice
                    .command
                    .iter()
                    .cloned()
                    .map(|input| (Protocol::Mosh, input)),
            )
            .collect();
        for (protocol, input) in inputs {
            this._inputs.push(cx.subscribe_in(
                &input,
                window,
                move |this, _, event: &InputEvent, _, cx| {
                    if let Target::Choice(choice) = &mut this.target
                        && !choice.starting
                    {
                        match event {
                            InputEvent::Change | InputEvent::Focus => {
                                choice.selected = protocol;
                                choice.error = None;
                                cx.notify();
                            }
                            InputEvent::PressEnter { .. } => {
                                choice.selected = protocol;
                                this.confirm_protocol(cx);
                            }
                            InputEvent::Blur => cx.notify(),
                        }
                    }
                },
            ));
        }
        this
    }
    pub(super) fn confirming_protocol(&self) -> bool {
        matches!(&self.target, Target::Choice(choice) if choice.starting)
    }
    pub fn protocol_choice_error(&mut self, error: String, cx: &mut Context<Self>) {
        if let Target::Choice(choice) = &mut self.target {
            choice.starting = false;
            choice.error = Some(error);
            cx.notify();
        }
    }
    pub(super) fn confirm_protocol(&mut self, cx: &mut Context<Self>) {
        let Target::Choice(choice) = &mut self.target else {
            return;
        };
        if choice.starting {
            return;
        }
        let port = choice
            .ports
            .iter()
            .find(|(protocol, _)| *protocol == choice.selected)
            .and_then(|(_, input)| parse_port(&input.read(cx).value()));
        let Some(port) = port else {
            choice.error = Some("Port must be between 1 and 65535.".into());
            cx.notify();
            return;
        };
        let command = if choice.selected == Protocol::Mosh {
            let value = choice
                .command
                .as_ref()
                .map(|input| input.read(cx).value().to_string());
            match crate::mosh::Config::new(value.as_deref()) {
                Ok(config) => Some(config.command),
                Err(error) => {
                    choice.error = Some(error.to_string());
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        choice.starting = true;
        choice.error = None;
        cx.emit(ProtocolSelected {
            host_id: choice.host_id,
            uuid: choice.uuid.clone(),
            protocol: choice.selected,
            port,
            command,
        });
        cx.notify();
    }
    pub(super) fn protocol_body(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Target::Choice(choice) = &self.target else {
            return div().into_any_element();
        };
        let p = Palette::new(self.dark);
        div()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .font_weight(FontWeights::MEDIUM)
                    .child(crate::localization::text(cx, "common.label.protocol")),
            )
            .child(
                div()
                    .id("connection-protocol-scrollable")
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .children(choice.ports.iter().map(|(protocol, input)| {
                        let protocol = *protocol;
                        let color = rgb(protocol.color(self.dark));
                        let selected = choice.selected == protocol;
                        let command = div()
                            .min_w_0()
                            .truncate()
                            .italic()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .font_weight(FontWeights::REGULAR)
                            .text_color(p.muted)
                            .child(format!("{} {}", protocol.command(), choice.address));
                        let mosh = protocol == Protocol::Mosh;
                        let background = if selected {
                            p.surface
                        } else {
                            p.background
                                .blend(p.surface.alpha(if self.dark { 0.72 } else { 0.86 }))
                        };
                        Button::new(SharedString::from(format!(
                            "connection-protocol-{}",
                            protocol.command()
                        )))
                        .ghost()
                        .custom(
                            ButtonCustomVariant::for_surface(p)
                                .color(background.into())
                                .hover(background.blend(color.alpha(0.04)).into())
                                .highlight(color.alpha(0.06).into())
                                .active(color.alpha(0.12).into()),
                        )
                        // The card selects a protocol; its editors own focus
                        // and keyboard input, including Space and Enter.
                        .focusable(false)
                        .tab_stop(false)
                        .w_full()
                        .h_auto()
                        .min_h(px(62.))
                        .px(px(14.))
                        .py(px(10.))
                        .content_start()
                        .rounded(px(9.))
                        .border_0()
                        .when(selected, |card| card.selection_outline(color, px(2.)))
                        .disabled(choice.starting)
                        .localized_accessibility_label(format!("Select {}", protocol.label()))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Target::Choice(choice) = &mut this.target
                                && !choice.starting
                            {
                                choice.selected = protocol;
                                choice.error = None;
                                this.focus.focus(window, cx);
                                cx.notify();
                            }
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .w(px(4.))
                                        .h(px(40.))
                                        .flex_shrink_0()
                                        .rounded_full()
                                        .bg(color),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(3.))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(5.))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .text_size(px(13.))
                                                        .font_weight(FontWeights::SEMIBOLD)
                                                        .child(protocol.label()),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(13.))
                                                        .font_weight(FontWeights::REGULAR)
                                                        .text_color(p.muted)
                                                        .child(crate::localization::text(
                                                            cx,
                                                            "rs.ui.copy.portLabel",
                                                        )),
                                                )
                                                .child(
                                                    protocol_input(
                                                        input,
                                                        format!("{} port", protocol.label()),
                                                        color,
                                                        choice.starting,
                                                        true,
                                                        window,
                                                        cx,
                                                    )
                                                    .w(px(52.)),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(10.))
                                                .min_w_0()
                                                .child(
                                                    command
                                                        .when(mosh, |v| {
                                                            v.max_w(px(150.)).flex_shrink(1.)
                                                        })
                                                        .when(!mosh, |v| v.flex_1()),
                                                )
                                                .when(mosh, |v| {
                                                    v.child(
                                                        protocol_input(
                                                            choice.command.as_ref().unwrap(),
                                                            "Mosh server command".into(),
                                                            color,
                                                            choice.starting,
                                                            false,
                                                            window,
                                                            cx,
                                                        )
                                                        .flex_1()
                                                        .min_w_0(),
                                                    )
                                                }),
                                        ),
                                ),
                        )
                    }))
                    .when_some(choice.error.clone(), |view, error| {
                        view.child(
                            div()
                                .text_color(rgb(0xe05545))
                                .child(crate::localization::message(cx, &error)),
                        )
                    })
                    .scrollbars(),
            )
            .into_any_element()
    }
}
fn protocol_input(
    input: &Entity<InputState>,
    label: String,
    color: Rgba,
    disabled: bool,
    digits: bool,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    let palette = Palette::new(cx.theme().is_dark());
    let focused = input.read(cx).focus_handle(cx).is_focused(window);
    div()
        .id(SharedString::from(label.clone()))
        .h(px(24.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
        .border_b_1()
        .border_color(if focused { color } else { palette.border })
        .child(
            Input::new(input)
                .appearance(false)
                .h(px(24.))
                .pb(px(4.))
                .text_size(px(13.))
                .line_height(px(18.))
                .caret_color(color)
                .disabled(disabled)
                .aria_label(label)
                .when(digits, |v| v.digits_only()),
        )
}
fn parse_port(value: &str) -> Option<u16> {
    let value = value.trim();
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok().filter(|port| *port != 0)
}
#[cfg(test)]
mod tests {
    use super::parse_port;
    #[test]
    fn attempt_ports_are_bounded_and_do_not_accept_signed_or_partial_input() {
        assert_eq!(parse_port(" 2323 "), Some(2323));
        assert_eq!(parse_port("65535"), Some(65535));
        for invalid in ["", "0", "65536", "-1", "+22", "22.0", "2x"] {
            assert!(parse_port(invalid).is_none());
        }
    }
}
