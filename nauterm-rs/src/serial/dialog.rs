use super::{Config, Flow, Parity, Port, default_port};
use crate::{
    components::{Button, DialogColors, DialogFrame, SelectModel, SelectOption},
    native::Core,
    palette::Palette,
};
use gpui_kit::{
    assets::IconName,
    base::input::{InputEvent, InputState},
    component::{ActiveTheme, Disableable, Sizable},
    prelude::*,
    *,
};
use std::sync::Arc;

pub enum Event {
    Connect(Config),
}
pub struct Connection {
    core: Option<Arc<Core>>,
    path: Entity<InputState>,
    baud: Entity<InputState>,
    devices: SelectModel<String>,
    rates: SelectModel<u32>,
    data_bits: SelectModel<u32>,
    stop_bits: SelectModel<u32>,
    parity: SelectModel<Parity>,
    flow: SelectModel<Flow>,
    ports: Vec<Port>,
    scanning: bool,
    scan_error: Option<String>,
    error: Option<String>,
    validation_error: Option<String>,
    _inputs: Vec<Subscription>,
}
impl EventEmitter<Event> for Connection {}
impl Focusable for Connection {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.path.read(cx).focus_handle(cx)
    }
}
impl Connection {
    pub fn new(core: Option<Arc<Core>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        crate::components::message::watch(window, cx, |view: &Self| {
            view.scan_error
                .clone()
                .map(crate::components::message::Message::error)
        });
        let path = cx.new(|cx| InputState::new(window, cx).default_value(default_port()));
        let baud = cx.new(|cx| InputState::new(window, cx).default_value("115200"));
        let devices = SelectModel::editable(vec![], None, &path, window, cx);
        let rates = SelectModel::editable(
            [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400]
                .into_iter()
                .map(|rate| SelectOption::new(rate, rate.to_string()))
                .collect(),
            Some(115200),
            &baud,
            window,
            cx,
        );
        let data_bits = SelectModel::new(
            (5..=8)
                .map(|bits| SelectOption::new(bits, bits.to_string()))
                .collect(),
            Some(8),
            window,
            cx,
        );
        let stop_bits = SelectModel::new(
            vec![
                SelectOption::localized(1, "1"),
                SelectOption::localized(2, "2"),
            ],
            Some(1),
            window,
            cx,
        );
        let parity = SelectModel::new(
            vec![
                SelectOption::localized(Parity::None, "None"),
                SelectOption::localized(Parity::Even, "Even"),
                SelectOption::localized(Parity::Odd, "Odd"),
            ],
            Some(Parity::None),
            window,
            cx,
        );
        let flow = SelectModel::new(
            vec![
                SelectOption::localized(Flow::None, "None"),
                SelectOption::localized(Flow::Software, "Software"),
                SelectOption::localized(Flow::Hardware, "Hardware"),
            ],
            Some(Flow::None),
            window,
            cx,
        );
        let inputs = [&path, &baud]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |this: &mut Self, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.error = None;
                        this.validation_error = None;
                    }
                    cx.notify();
                })
            })
            .collect();
        let mut view = Self {
            core,
            path,
            baud,
            devices,
            rates,
            data_bits,
            stop_bits,
            parity,
            flow,
            ports: vec![],
            scanning: false,
            scan_error: None,
            error: None,
            validation_error: None,
            _inputs: inputs,
        };
        view.scan(window, cx);
        view
    }
    pub fn core(&self) -> Option<Arc<Core>> {
        self.core.clone()
    }
    pub fn failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.error = Some(message);
        self.validation_error = None;
        cx.notify();
    }
    fn scan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        self.scan_error = None;
        let core = self.core.clone();
        cx.spawn_in(window, async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    let core = core.map(Ok).unwrap_or_else(Core::discover)?;
                    let ports = core.serial_ports()?;
                    Ok::<_, anyhow::Error>((core, ports))
                })
                .await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.scanning = false;
                match result {
                    Ok((core, ports)) => {
                        view.core = Some(core);
                        let current = view.path.read(cx).value().trim().to_owned();
                        let chosen = if current.is_empty() || current == default_port() {
                            ports
                                .first()
                                .map(|port| port.path.clone())
                                .unwrap_or(current)
                        } else {
                            current
                        };
                        // Editable options use the path itself, never a display
                        // label that could accidentally be opened as a device.
                        let selected = ports
                            .iter()
                            .any(|port| port.path == chosen)
                            .then_some(chosen);
                        let options = ports
                            .iter()
                            .map(|port| SelectOption::new(port.path.clone(), port.path.clone()))
                            .collect();
                        view.ports = ports;
                        view.devices.reconcile(options, selected, window, cx);
                    }
                    Err(error) => {
                        view.scan_error =
                            Some(format!("Unable to enumerate serial devices: {error}"))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn submit(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.validation_error = None;
        let result = (|| {
            let config = Config {
                path: self.path.read(cx).value().trim().to_owned(),
                baud: self.baud.read(cx).value().trim().parse().map_err(|_| {
                    anyhow::anyhow!("Baud rate must be a positive number up to {}.", u32::MAX)
                })?,
                data_bits: selected(&self.data_bits, 8, cx),
                stop_bits: selected(&self.stop_bits, 1, cx),
                parity: selected(&self.parity, Parity::None, cx),
                flow: selected(&self.flow, Flow::None, cx),
            };
            config.validate()?;
            Ok::<_, anyhow::Error>(config)
        })();
        match result {
            Ok(config) => cx.emit(Event::Connect(config)),
            Err(error) => self.validation_error = Some(error.to_string()),
        }
        cx.notify();
    }
}
fn selected<V: Copy + PartialEq + 'static>(model: &SelectModel<V>, default: V, cx: &App) -> V {
    model
        .state
        .read(cx)
        .selected_value()
        .copied()
        .flatten()
        .unwrap_or(default)
}
impl Render for Connection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let p = Palette::new(dark);
        let path = self.path.read(cx).value();
        let status = if self.scanning {
            "Scanning serial devices…".into()
        } else if self.ports.is_empty() {
            "No serial devices found. Enter a path manually.".into()
        } else {
            self.ports
                .iter()
                .find(|port| port.path == path.trim())
                .map(Port::label)
                .unwrap_or_default()
        };
        DialogFrame::new("serial-connection-dialog", "Serial connection")
            .width(460.)
            .colors(DialogColors::workspace(dark))
            .content(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(self.devices.field("Serial port", false)),
                            )
                            .child(
                                Button::new("refresh-serial-devices")
                                    .icon(IconName::RefreshCw)
                                    .large()
                                    .disabled(self.scanning)
                                    .loading(self.scanning)
                                    .tooltip("Refresh serial devices")
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.scan(window, cx)),
                                    ),
                            ),
                    )
                    .when(!status.is_empty(), |view| {
                        view.child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(crate::localization::message(cx, &status)),
                        )
                    })
                    .child(self.rates.digits_field("Baud rate", false))
                    .child(
                        div()
                            .flex()
                            .gap(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(self.data_bits.required_field("Data bits", false)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(self.stop_bits.required_field("Stop bits", false)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(self.parity.required_field("Parity", false)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(self.flow.required_field("Flow control", false)),
                            ),
                    )
                    .when_some(
                        crate::components::validation_message(self.validation_error.as_deref(), cx),
                        |view, error| view.child(error),
                    ),
            )
            .action(
                Button::new("serial-cancel")
                    .ghost()
                    .label("Cancel")
                    .on_click(|_, window, cx| crate::components::Modal::dismiss(window, cx)),
            )
            .action(
                Button::new("serial-connect")
                    .primary()
                    .label("Connect")
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
    }
}
