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
    Connect { config: Config, save_as_host: bool },
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
    embedded: bool,
    allow_save: bool,
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
        Self::with_config(core, None, false, window, cx)
    }

    pub fn embedded(config: Option<&Config>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_config(None, config, true, window, cx)
    }

    fn with_config(
        core: Option<Arc<Core>>,
        config: Option<&Config>,
        embedded: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
        let path_value =
            config.map_or_else(|| default_port().to_owned(), |config| config.path.clone());
        let baud_value = config.map_or(115200, |config| config.baud);
        let path = cx.new(|cx| InputState::new(window, cx).default_value(path_value));
        let baud = cx.new(|cx| InputState::new(window, cx).default_value(baud_value.to_string()));
        let devices = SelectModel::editable(vec![], None, &path, window, cx);
        let rates = SelectModel::editable(
            [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400]
                .into_iter()
                .map(|rate| SelectOption::new(rate, rate.to_string()))
                .collect(),
            Some(baud_value),
            &baud,
            window,
            cx,
        );
        let data_bits = SelectModel::new(
            (5..=8)
                .map(|bits| SelectOption::new(bits, bits.to_string()))
                .collect(),
            Some(config.map_or(8, |config| config.data_bits)),
            window,
            cx,
        );
        let stop_bits = SelectModel::new(
            vec![
                SelectOption::localized(1, "1"),
                SelectOption::localized(2, "2"),
            ],
            Some(config.map_or(1, |config| config.stop_bits)),
            window,
            cx,
        );
        let parity = SelectModel::new(
            vec![
                SelectOption::localized(Parity::None, "common.label.none"),
                SelectOption::localized(Parity::Even, "common.label.even"),
                SelectOption::localized(Parity::Odd, "common.label.odd"),
            ],
            Some(config.map_or(Parity::None, |config| config.parity)),
            window,
            cx,
        );
        let flow = SelectModel::new(
            vec![
                SelectOption::localized(Flow::None, "common.label.none"),
                SelectOption::localized(Flow::Software, "workspace.label.software"),
                SelectOption::localized(Flow::Hardware, "common.label.hardware"),
            ],
            Some(config.map_or(Flow::None, |config| config.flow)),
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
            embedded,
            allow_save: false,
            scan_error: None,
            error: None,
            validation_error: None,
            _inputs: inputs,
        };
        if !embedded {
            view.scan(window, cx);
        }
        view
    }
    pub fn allow_save(mut self, allowed: bool) -> Self {
        self.allow_save = allowed;
        self
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
                        let chosen = if current.is_empty()
                            || (!view.embedded && current == default_port())
                        {
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
    pub(crate) fn hash_fields(&self, cx: &App, hash: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        self.path.read(cx).value().hash(hash);
        self.baud.read(cx).value().hash(hash);
        selected(&self.data_bits, 8, cx).hash(hash);
        selected(&self.stop_bits, 1, cx).hash(hash);
        (selected(&self.parity, Parity::None, cx) as u32).hash(hash);
        (selected(&self.flow, Flow::None, cx) as u32).hash(hash);
    }
    pub fn configuration(&self, cx: &App) -> anyhow::Result<Config> {
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
        Ok(config)
    }

    fn submit(&mut self, save_as_host: bool, cx: &mut Context<Self>) {
        self.error = None;
        self.validation_error = None;
        match self.configuration(cx) {
            Ok(config) => cx.emit(Event::Connect {
                config,
                save_as_host,
            }),
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
impl Connection {
    pub(crate) fn device_field(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.devices.field("workspace.label.serialPort", false)),
            )
            .child(
                Button::new("refresh-serial-devices")
                    .icon(IconName::RefreshCw)
                    .large()
                    .disabled(self.scanning)
                    .loading(self.scanning)
                    .tooltip("workspace.label.refreshSerialDevices")
                    .on_click(cx.listener(|this, _, window, cx| this.scan(window, cx))),
            )
            .into_any_element()
    }
}

impl Render for Connection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let p = Palette::new(dark);
        let fields = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .when(!self.embedded, |view| {
                let path = self.path.read(cx).value();
                let status = if self.scanning {
                    "workspace.serial.scanning".into()
                } else if self.ports.is_empty() {
                    "workspace.serial.empty.description".into()
                } else {
                    self.ports
                        .iter()
                        .find(|port| port.path == path.trim())
                        .map(Port::label)
                        .unwrap_or_default()
                };
                view.child(self.device_field(cx))
                    .when(!status.is_empty(), |view| {
                        view.child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(crate::localization::message(cx, &status)),
                        )
                    })
            })
            .child(self.rates.digits_field("workspace.label.baudRate", false))
            .child(
                div()
                    .flex()
                    .gap(px(10.))
                    .child(
                        div().flex_1().min_w_0().child(
                            self.data_bits
                                .required_field("workspace.label.dataBits", false),
                        ),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            self.stop_bits
                                .required_field("workspace.label.stopBits", false),
                        ),
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
                            .child(self.parity.required_field("common.label.parity", false)),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            self.flow
                                .required_field("workspace.label.flowControl", false),
                        ),
                    ),
            )
            .when_some(
                crate::components::validation_message(self.validation_error.as_deref(), cx),
                |view, error| view.child(error),
            );
        if self.embedded {
            return fields.into_any_element();
        }
        DialogFrame::new("serial-connection-dialog", "workspace.serial.dialog.title")
            .width(460.)
            .colors(DialogColors::workspace(dark))
            .content(fields)
            .action(
                Button::new("serial-cancel")
                    .ghost()
                    .label("common.action.cancel")
                    .on_click(|_, window, cx| crate::components::Modal::dismiss(window, cx)),
            )
            .when(self.allow_save, |dialog| {
                dialog.action(
                    Button::new("serial-connect-and-save")
                        .secondary()
                        .label("workspace.label.connectSave")
                        .on_click(cx.listener(|this, _, _, cx| this.submit(true, cx))),
                )
            })
            .action(
                Button::new("serial-connect")
                    .primary()
                    .label("common.action.connect")
                    .on_click(cx.listener(|this, _, _, cx| this.submit(false, cx))),
            )
            .into_any_element()
    }
}
