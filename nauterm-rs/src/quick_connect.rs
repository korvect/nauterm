//! Shared Quick Connect for the title bar and workspace creation/new sessions.
use crate::{
    components::{Button, ControlMetrics, Input, ScrollbarsExt},
    model::{AppState, Host},
    palette::Palette,
    shells::{self, Shell},
    ssh_address::SshAddress,
    typography::FontWeights,
};
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::{
    assets::IconName,
    component::{Icon, Sizable, Size},
    prelude::*,
    *,
};

const PANEL_INSET: f32 = 18.;
const PANEL_PADDING: f32 = 14.;
const SHELL_GAP: f32 = 6.;
const SHELL_HEIGHT: f32 = 36.;

#[derive(Clone)]
pub enum Target {
    Shell(Shell),
    Host(Host),
    SavedTelnet(Host),
    SavedSsh(Host),
    SavedMosh(Host),
    Sftp(Host),
    SftpUri(Host, String),
    Telnet(crate::telnet::Options),
}
impl Target {
    pub fn label(&self) -> &str {
        match self {
            Self::Shell(shell) => &shell.label,
            Self::Host(host)
            | Self::SavedSsh(host)
            | Self::SavedMosh(host)
            | Self::SavedTelnet(host)
            | Self::Sftp(host)
            | Self::SftpUri(host, _) => &host.name,
            Self::Telnet(options) => &options.host,
        }
    }
}
pub enum QuickConnectEvent {
    Dismiss,
    Connect(Box<Target>),
}
impl QuickConnectEvent {
    fn connect(target: Target) -> Self {
        Self::Connect(Box::new(target))
    }
}
pub struct QuickConnect {
    data: AppState,
    search: Entity<InputState>,
    shells: Vec<Shell>,
    show_all: bool,
    options_scroll: ScrollHandle,
    _search: Subscription,
}
impl EventEmitter<QuickConnectEvent> for QuickConnect {}
impl Focusable for QuickConnect {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle(cx)
    }
}
fn hosts<'a>(data: &'a AppState, query: &str) -> Vec<&'a Host> {
    let mut hosts: Vec<_> = data
        .hosts
        .iter()
        .filter(|host| data.host_matches(host, query))
        .collect();
    hosts.sort_by_key(|host| (host.name.to_lowercase(), host.id));
    hosts
}
fn direct(query: &str) -> Option<Host> {
    let args = shlex::split(query)?;
    let mut args = args.iter().peekable();
    let command = args
        .peek()
        .is_some_and(|arg| arg.eq_ignore_ascii_case("ssh"));
    if command {
        args.next();
    }
    let (address, port, username) = command_address(args)?;
    if !command && address.username.is_empty() && port.is_none() && username.is_none() {
        return None;
    }
    let telnet_port = port.unwrap_or(if address.port == 22 { 23 } else { address.port });
    let username = username.unwrap_or(address.username);
    Some(Host {
        connections: crate::model::ConnectionProfiles {
            mosh: !command,
            telnet: (!command).then(|| crate::telnet::Profile {
                port: telnet_port,
                username: username.clone(),
                ..Default::default()
            }),
            ..Default::default()
        },
        name: address.host.clone(),
        address: address.host,
        port: port.unwrap_or(address.port),
        username,
        ..Default::default()
    })
}

fn command_address<'a>(
    args: impl Iterator<Item = &'a String>,
) -> Option<(SshAddress, Option<u16>, Option<String>)> {
    let mut args = args;
    let mut endpoint = None;
    let mut port = None;
    let mut username = None;
    let mut flags = true;
    while let Some(arg) = args.next() {
        if flags && arg == "--" {
            flags = false;
        } else if flags && arg == "-p" {
            port = args
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .filter(|value| *value != 0);
        } else if flags && arg == "-l" {
            username = args.next().cloned().filter(|value| !value.is_empty());
        } else if flags && arg.starts_with('-') {
            continue;
        } else if endpoint.is_none() {
            endpoint = SshAddress::parse(arg).ok();
        } else if port.is_none() {
            port = arg.parse::<u16>().ok().filter(|value| *value != 0);
        }
    }
    Some((endpoint?, port, username))
}

fn telnet_command(query: &str) -> Option<crate::telnet::Options> {
    let args = shlex::split(query)?;
    let mut args = args.iter();
    if !args.next()?.eq_ignore_ascii_case("telnet") {
        return None;
    }
    let (address, port, username) = command_address(args)?;
    Some(crate::telnet::Options {
        host: address.host,
        port: port.unwrap_or(if address.port == 22 { 23 } else { address.port }),
        username: username.unwrap_or(address.username),
        password: Default::default(),
        encoding: Default::default(),
    })
}

pub(crate) fn connection_url(value: &str) -> Option<Target> {
    let value = value.trim();
    let (_, authority_and_path) = value.split_once("://")?;
    let authority = authority_and_path.split(['/', '?', '#']).next()?;
    if authority.ends_with(':')
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return None;
    }
    let uri = url::Url::parse(value).ok()?;
    if uri.query().is_some() || uri.fragment().is_some() {
        return None;
    }
    let host = uri
        .host_str()?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    if host.is_empty() || host.contains(['/', '\\', '@', '#', '?']) {
        return None;
    }
    let username = percent_encoding::percent_decode_str(uri.username())
        .decode_utf8()
        .ok()?
        .into_owned();
    let path = percent_encoding::percent_decode_str(uri.path())
        .decode_utf8()
        .ok()?
        .into_owned();
    if username.chars().chain(path.chars()).any(char::is_control) {
        return None;
    }
    let protocol = uri.scheme();
    if protocol != "sftp" && !matches!(path.as_str(), "" | "/") {
        return None;
    }
    let port = uri
        .port()
        .unwrap_or(if protocol == "telnet" { 23 } else { 22 });
    if port == 0 {
        return None;
    }
    if protocol == "telnet" {
        return Some(Target::Telnet(crate::telnet::Options {
            host,
            port,
            username,
            password: Default::default(),
            encoding: Default::default(),
        }));
    }
    let host = Host {
        name: host.clone(),
        address: host,
        username,
        port,
        ..Default::default()
    };
    match protocol {
        "ssh" => Some(Target::Host(host)),
        "sftp" => Some(Target::SftpUri(host, path)),
        _ => None,
    }
}
fn address_label(host: &Host) -> String {
    let address = if host.address.contains(':') && !host.address.starts_with('[') {
        format!("[{}]", host.address)
    } else {
        host.address.clone()
    };
    let (port, default_port) = if !host.supports_ssh()
        && let Some(profile) = &host.connections.telnet
    {
        (profile.port, 23)
    } else {
        (host.port, 22)
    };
    if port == default_port {
        address
    } else {
        format!("{address}:{port}")
    }
}
fn host_details(data: &AppState, host: &Host) -> String {
    let group = data
        .groups
        .iter()
        .find(|group| Some(group.id) == host.group_id)
        .map(|group| group.name.as_str())
        .unwrap_or_default();
    let username = if !host.supports_ssh() {
        host.connections
            .telnet
            .as_ref()
            .map(|profile| profile.username.as_str())
            .unwrap_or(&host.username)
    } else {
        &host.username
    };
    [username, &address_label(host), group]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("  ")
}
fn shell_columns(available: f32, widest_label: f32) -> usize {
    if available < 360. {
        2
    } else {
        let metrics = ControlMetrics::button(Size::Small);
        let label_width = (available - SHELL_GAP * 3.) / 4. - metrics.padding * 2.;
        if widest_label > label_width { 3 } else { 4 }
    }
}
fn section_label(label: &'static str) -> Div {
    div()
        .text_size(px(12.))
        .font_weight(FontWeights::MEDIUM)
        .child(crate::localization::view(label))
}
fn empty_state(message: &'static str, p: Palette, cx: &App) -> Div {
    div()
        .w_full()
        .h(px(52.))
        .rounded(px(8.))
        .border_1()
        .border_color(p.border)
        .bg(p.surface)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(13.))
        .font_weight(FontWeights::MEDIUM)
        .text_color(p.muted)
        .child(crate::localization::message(cx, message))
}
impl QuickConnect {
    pub fn new(data: AppState, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search hosts or enter a connection command")
        });
        let events = cx.subscribe_in(&search, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.options_scroll.set_offset(point(px(0.), px(0.)));
            }
            if matches!(event, InputEvent::PressEnter { .. }) {
                let query = this.search.read(cx).value();
                if let Some(target) =
                    connection_url(&query).or_else(|| telnet_command(&query).map(Target::Telnet))
                {
                    cx.emit(QuickConnectEvent::connect(target));
                } else if let Some(host) = direct(&query).or_else(|| {
                    hosts(&this.data, &query)
                        .first()
                        .map(|host| (*host).clone())
                }) {
                    cx.emit(QuickConnectEvent::connect(Target::Host(host)));
                }
            }
            cx.notify();
        });
        search.update(cx, |input, cx| input.focus(window, cx));
        Self {
            data,
            search,
            shells: shells::discover(),
            show_all: false,
            options_scroll: ScrollHandle::new(),
            _search: events,
        }
    }
    pub fn set_data(&mut self, data: AppState, cx: &mut Context<Self>) {
        if self.data.hosts != data.hosts
            || self.data.groups != data.groups
            || self.data.dark != data.dark
        {
            self.data = data;
            cx.notify();
        }
    }
    fn host_row(&self, host: Host, first: bool, last: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.data.dark);
        let details = host_details(&self.data, &host);
        let target = host.clone();
        let ssh_target = host.clone();
        let sftp_target = host.clone();
        let telnet_target = host.clone();
        let mosh_target = host.clone();
        let mosh_enabled = host.is_remote() && host.connections.mosh;
        let ssh_enabled = host.supports_ssh();
        let telnet_enabled = host.is_remote() && host.connections.telnet.is_some();
        Button::new(SharedString::from(format!(
            "quick-host-{}-{}",
            host.id, host.address
        )))
        .ghost()
        .h(px(52.))
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .content_start()
        .content_gap(px(9.))
        .pl(px(10.))
        .pr(px(8.))
        .rounded_none()
        .when(first, |row| row.rounded_t(px(7.)))
        .when(last, |row| row.rounded_b(px(7.)))
        .localized_accessibility_label(format!("Connect to {}", host.name))
        .on_click(cx.listener(move |_, _, _, cx| {
            cx.emit(QuickConnectEvent::connect(Target::Host(target.clone())))
        }))
        .child(
            crate::avatar::host(&host.name)
                .custom_icon(host.icon.as_deref())
                .platform(host.platform.clone())
                .dimension(36.),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(2.))
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .font_weight(FontWeights::MEDIUM)
                        .text_size(px(13.))
                        .line_height(relative(1.35))
                        .child(host.name),
                )
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .text_color(p.muted)
                        .text_size(px(12.))
                        .line_height(relative(1.35))
                        .font_weight(FontWeight::NORMAL)
                        .child(details),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .gap(px(6.))
                .when(ssh_enabled, |view| {
                    view.child(
                        Button::new(SharedString::from(format!("quick-host-{}-ssh", host.id)))
                            .xsmall()
                            .ghost()
                            .outline()
                            .flex_shrink_0()
                            .px(px(11.))
                            .label("SSH")
                            .tooltip("Connect with SSH")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(QuickConnectEvent::connect(Target::SavedSsh(
                                    ssh_target.clone(),
                                )))
                            })),
                    )
                })
                .when(mosh_enabled, |view| {
                    view.child(
                        Button::new(SharedString::from(format!(
                            "quick-host-{}-mosh",
                            mosh_target.id
                        )))
                        .xsmall()
                        .ghost()
                        .outline()
                        .flex_shrink_0()
                        .px(px(11.))
                        .label("Mosh")
                        .tooltip("Connect with Mosh")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(QuickConnectEvent::connect(Target::SavedMosh(
                                mosh_target.clone(),
                            )))
                        })),
                    )
                })
                .when(telnet_enabled, |view| {
                    view.child(
                        Button::new(SharedString::from(format!(
                            "quick-host-{}-telnet",
                            telnet_target.id
                        )))
                        .xsmall()
                        .ghost()
                        .outline()
                        .flex_shrink_0()
                        .px(px(11.))
                        .label("Telnet")
                        .tooltip("Connect with Telnet")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(QuickConnectEvent::connect(Target::SavedTelnet(
                                telnet_target.clone(),
                            )))
                        })),
                    )
                })
                .when(ssh_enabled, |view| {
                    view.child(
                        Button::new(SharedString::from(format!("quick-host-{}-sftp", host.id)))
                            .xsmall()
                            .ghost()
                            .outline()
                            .flex_shrink_0()
                            .px(px(11.))
                            .label("SFTP")
                            .tooltip("Connect with SFTP")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(QuickConnectEvent::connect(Target::Sftp(
                                    sftp_target.clone(),
                                )))
                            })),
                    )
                }),
        )
        .into_any_element()
    }
    fn telnet_row(&self, options: crate::telnet::Options, cx: &mut Context<Self>) -> AnyElement {
        Button::new("quick-direct-telnet")
            .ghost()
            .outline()
            .w_full()
            .min_w_0()
            .h(px(52.))
            .content_start()
            .icon(IconName::SquareTerminal)
            .label(format!("Telnet {}", options.endpoint()))
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(QuickConnectEvent::connect(Target::Telnet(options.clone())))
            }))
            .into_any_element()
    }
    fn direct_row(&self, host: Host, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.data.dark);
        let label = if host.username.is_empty() {
            address_label(&host)
        } else {
            format!("{}@{}", host.username, address_label(&host))
        };
        let action = if host.connections.mosh {
            "Connect"
        } else {
            "SSH"
        };
        div()
            .id("quick-connect-direct-row")
            .w_full()
            .min_w_0()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .pl(px(10.))
            .pr(px(8.))
            .py(px(8.))
            .mb(px(6.))
            .flex()
            .items_center()
            .gap(px(9.))
            .child(
                div()
                    .size(px(30.))
                    .flex_shrink_0()
                    .rounded(px(7.))
                    .bg(p.surface.blend(p.accent.alpha(0.16)))
                    .text_color(p.accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Zap).with_size(px(17.))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeights::MEDIUM)
                    .child(label.clone()),
            )
            .child(
                Button::new("quick-direct-ssh")
                    .xsmall()
                    .primary()
                    .flex_shrink_0()
                    .px(px(12.))
                    .label(action)
                    .localized_accessibility_label(format!("Connect to {label}"))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(QuickConnectEvent::connect(Target::Host(host.clone())))
                    })),
            )
            .into_any_element()
    }
}
impl Render for QuickConnect {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.data.dark);
        let query = self.search.read(cx).value();
        let url = connection_url(&query);
        let telnet = telnet_command(&query);
        let direct = if url.is_some() || telnet.is_some() {
            None
        } else {
            direct(&query)
        };
        let width = (f32::from(window.viewport_size().width) - PANEL_INSET * 2.).clamp(0., 500.);
        let height = (f32::from(window.viewport_size().height) - PANEL_INSET * 2.).clamp(0., 560.);
        // Account for the frame's border as well as its content padding. The
        // grid itself uses flex slots so partial last rows keep the same widths.
        let available = (width - 2. - PANEL_PADDING * 2.).max(0.);
        let mut text_style = window.text_style();
        text_style.font_weight = FontWeights::MEDIUM;
        let metrics = ControlMetrics::button(Size::Small);
        let widest_label = self
            .shells
            .iter()
            .map(|shell| {
                let run = text_style.to_run(shell.label.len());
                f32::from(
                    window
                        .text_system()
                        .shape_line(shell.label.clone().into(), px(metrics.font), &[run], None)
                        .width,
                )
            })
            .fold(0., f32::max);
        let columns = shell_columns(available, widest_label);
        let shell_width = ((available - SHELL_GAP * (columns - 1) as f32) / columns as f32).max(0.);
        let limit = if self.show_all {
            self.shells.len()
        } else {
            columns * 2
        };
        let shells: Vec<_> = self.shells.iter().take(limit).cloned().collect();
        let matching: Vec<_> = hosts(&self.data, &query).into_iter().cloned().collect();
        let host_count = matching.len();
        let mut host_list = div()
            .id("quick-connect-host-list")
            .w_full()
            .min_w_0()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .flex()
            .flex_col()
            .overflow_hidden();
        for (index, host) in matching.into_iter().enumerate() {
            let last = index + 1 == host_count;
            host_list = host_list.child(self.host_row(host, index == 0, last, cx));
            if !last {
                host_list = host_list.child(div().h(px(1.)).w_full().bg(p.border));
            }
        }
        div()
            .id("quick-connect-panel")
            .w(px(width))
            .max_h(px(height))
            .min_w_0()
            .min_h_0()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.background)
            .text_color(p.text)
            .text_size(px(13.))
            .font_weight(FontWeight::NORMAL)
            .line_height(relative(1.35))
            .flex()
            .flex_col()
            .overflow_hidden()
            // Modal owns the outside-click/Escape scope, including child input
            // menus. A second unscoped outside handler would bypass that guard.
            .child(
                div()
                    .id("quick-connect-header")
                    .flex_shrink_0()
                    .min_w_0()
                    .px(px(PANEL_PADDING))
                    .py(px(12.))
                    .bg(p.surface)
                    .rounded_t(px(9.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .min_w_0()
                            .mb(px(9.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(15.))
                                    .font_weight(FontWeights::MEDIUM)
                                    .child(crate::localization::text(
                                        cx,
                                        "common.label.quickConnect",
                                    )),
                            )
                            .child(
                                Button::new("dismiss-quick-connect")
                                    .xsmall()
                                    .ghost()
                                    .icon(crate::assets::icon(IconName::X))
                                    .tooltip("Close")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(QuickConnectEvent::Dismiss)
                                    })),
                            ),
                    )
                    .child(
                        Input::new(&self.search)
                            .h(px(36.))
                            .text_size(px(13.))
                            .line_height(relative(18. / 13.))
                            .pl(px(10.))
                            .pr(px(8.))
                            .gap(px(8.))
                            .cleanable(true)
                            .prefix(
                                Icon::new(IconName::Search)
                                    .with_size(px(16.))
                                    .text_color(p.muted),
                            )
                            .aria_label(crate::localization::text(cx, "Quick Connect search")),
                    ),
            )
            .child(
                div()
                    .id("quick-connect-options")
                    .min_w_0()
                    .min_h_0()
                    .flex_shrink_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.options_scroll)
                    .scrollbars_with(&self.options_scroll)
                    .px(px(PANEL_PADDING))
                    .pt(px(10.))
                    .pb(px(14.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .mb(px(6.))
                            .child(section_label("Shells").flex_1())
                            .when(self.shells.len() > columns * 2, |view| {
                                view.child(
                                    Button::new("quick-shell-show-all")
                                        .xsmall()
                                        .ghost()
                                        .icon(if self.show_all {
                                            IconName::ChevronUp
                                        } else {
                                            IconName::ChevronDown
                                        })
                                        .label(if self.show_all {
                                            "Show Less"
                                        } else {
                                            "Show All"
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.show_all = !this.show_all;
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .when(shells.is_empty(), |view| {
                        view.child(empty_state("No local shells found.", p, cx))
                    })
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(SHELL_GAP))
                            .children(shells.chunks(columns).enumerate().map(|(row, shells)| {
                                div()
                                    .id(("quick-shell-row", row))
                                    .w_full()
                                    .min_w_0()
                                    .flex()
                                    .gap(px(SHELL_GAP))
                                    .children(shells.iter().map(|shell| {
                                        let shell = shell.clone();
                                        let selected = shell.clone();
                                        Button::new(SharedString::from(format!(
                                            "quick-shell-{}",
                                            shell.path
                                        )))
                                        .small()
                                        .ghost()
                                        .outline()
                                        .bg(p.surface)
                                        .w(px(shell_width))
                                        .flex_shrink_0()
                                        .min_w_0()
                                        .h(px(SHELL_HEIGHT))
                                        .raw_label(shell.label)
                                        .raw_tooltip(shell.path)
                                        .on_click(
                                            cx.listener(move |_, _, _, cx| {
                                                cx.emit(QuickConnectEvent::connect(Target::Shell(
                                                    selected.clone(),
                                                )))
                                            }),
                                        )
                                    }))
                                    .children(
                                        (shells.len()..columns)
                                            .map(|_| div().w(px(shell_width)).flex_shrink_0()),
                                    )
                            })),
                    )
                    .child(section_label("Hosts").mt(px(14.)).mb(px(6.)))
                    .when_some(url, |view, target| match target {
                        Target::Telnet(options) => view.child(self.telnet_row(options, cx)),
                        Target::Host(host) => view.child(self.direct_row(host, cx)),
                        Target::SftpUri(host, path) => view.child(
                            Button::new("quick-direct-sftp")
                                .outline()
                                .h(px(52.))
                                .label(format!("SFTP {}", address_label(&host)))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(QuickConnectEvent::connect(Target::SftpUri(
                                        host.clone(),
                                        path.clone(),
                                    )));
                                })),
                        ),
                        _ => view,
                    })
                    .when_some(telnet, |view, options| {
                        view.child(self.telnet_row(options, cx))
                    })
                    .when_some(direct, |view, host| view.child(self.direct_row(host, cx)))
                    .when(host_count == 0, |view| {
                        view.child(empty_state("No hosts found.", p, cx))
                    })
                    .when(host_count > 0, |view| view.child(host_list)),
            )
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn quick_connect_parses_commands_and_protocol_choice() {
        let ssh = super::direct("ssh -p 2022 -l 'admin' -- 'example.test'").unwrap();
        assert_eq!(
            (ssh.address.as_str(), ssh.port, ssh.username.as_str()),
            ("example.test", 2022, "admin")
        );
        assert!(!ssh.connections.mosh);
        assert!(ssh.connections.telnet.is_none());
        let host = super::direct("user@example.test -p 8023").unwrap();
        assert!(host.connections.mosh);
        assert_eq!(host.port, 8023);
        assert_eq!(host.connections.telnet.unwrap().port, 8023);
        assert_eq!(
            super::direct("user@example.test")
                .unwrap()
                .connections
                .telnet
                .unwrap()
                .port,
            23
        );
        let telnet = super::telnet_command("telnet example.test 2323").unwrap();
        assert_eq!((telnet.host.as_str(), telnet.port), ("example.test", 2323));
    }

    #[test]
    fn connection_urls_provide_endpoint_and_discard_password() {
        let Some(super::Target::Telnet(options)) =
            super::connection_url("telnet://qa:discard@[::1]:2323")
        else {
            panic!("a Telnet URL should produce a Telnet target");
        };
        assert_eq!(options.host, "::1");
        assert_eq!(options.port, 2323);
        assert_eq!(options.username, "qa");
        assert!(options.password.is_empty());
        let Some(super::Target::SftpUri(host, path)) =
            super::connection_url("sftp://qa@example.test/work%20files")
        else {
            panic!("an SFTP URL should produce an SFTP target");
        };
        assert_eq!(host.port, 22);
        assert_eq!(path, "/work files");
        for invalid in [
            "ssh://localhost:0",
            "ssh://localhost/path",
            "telnet://example.test?query",
            "ssh://localhost:",
        ] {
            assert!(super::connection_url(invalid).is_none(), "{invalid}");
        }
    }

    use super::{address_label, direct, host_details, hosts, shell_columns};
    use crate::model::{AppState, CredentialRef, Group};
    #[test]
    fn shell_grid_uses_two_columns_when_narrow_and_measured_labels_otherwise() {
        assert_eq!(shell_columns(240., 180.), 2);
        assert_eq!(shell_columns(359., 20.), 2);
        assert_eq!(shell_columns(470., 60.), 4);
        assert_eq!(shell_columns(470., 120.), 3);
        // 360px: (360 - 3 * 6) / 4 - 2 * 9 = 67.5px per label.
        assert_eq!(shell_columns(360., 67.5), 4);
        assert_eq!(shell_columns(360., 68.), 3);
    }
    #[test]
    fn host_metadata_shows_group_and_only_nondefault_ports_with_unambiguous_ipv6() {
        let mut data = AppState::default();
        data.groups.push(Group {
            mosh: None,
            telnet: None,
            uuid: None,
            ssh: None,
            id: 7,
            name: "QA".into(),
            parent: None,
            proxy_id: None,
        });
        let mut host = direct("qa@[::1]:2022").unwrap();
        host.group_id = Some(7);
        assert_eq!(address_label(&host), "[::1]:2022");
        assert_eq!(host_details(&data, &host), "qa  [::1]:2022  QA");
        host.port = 22;
        host.username.clear();
        host.group_id = Some(99);
        assert_eq!(host_details(&data, &host), "[::1]");
        host.address = "example.test".into();
        assert_eq!(address_label(&host), "example.test");
        host.address = "[::1]".into();
        assert_eq!(address_label(&host), "[::1]");
    }
    #[test]
    fn direct_requests_preserve_ipv6_port_and_never_inherit_saved_credentials() {
        let host = direct("qa@[::1]:2022").unwrap();
        assert_eq!(
            (host.address.as_str(), host.port, host.username.as_str()),
            ("::1", 2022, "qa")
        );
        assert_eq!(host.credential, None);
        assert!(direct("qa:password@host").is_none());
        assert!(direct("host:example").is_none());
        let mut data = AppState::default();
        let mut host = direct("qa@localhost").unwrap();
        host.name = "B".into();
        host.credential = Some(CredentialRef::Identity(7));
        data.hosts.push(host);
        assert_eq!(
            hosts(&data, "user:qa")[0].credential,
            Some(CredentialRef::Identity(7))
        );
        assert!(hosts(&data, "tag:missing").is_empty());
    }
}
