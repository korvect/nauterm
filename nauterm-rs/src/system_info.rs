//! Terminal-bound, transient system monitoring. Native I/O stays off the UI.
pub(crate) mod data;
use crate::{
    components::{
        Button, ButtonCustomVariant, ScrollbarsExt, Spinner,
        menu::{DropdownMenu, MenuItem},
    },
    native::SshOptions,
    palette::Palette,
    terminal::Terminal,
    terminal_appearance::{Palette as TerminalPalette, platform_family},
    typography::FontWeights,
};
use data::{Samples, Snapshot, Sort, bytes, usage, used_total};
use gpui_kit::{
    assets::IconName,
    component::{Disableable, Sizable},
    prelude::*,
    *,
};
use std::time::{Duration, Instant};

pub struct SystemTool {
    source: WeakEntity<Terminal>,
    options: Box<SshOptions>,
    colors: TerminalPalette,
    active: bool,
    loading: bool,
    generation: u64,
    samples: Samples,
    poll: Option<Task<()>>,
    scroll: ScrollHandle,
}
impl SystemTool {
    pub fn new(terminal: &Entity<Terminal>, options: SshOptions, colors: TerminalPalette) -> Self {
        Self {
            source: terminal.downgrade(),
            options: Box::new(options.for_new_connection()),
            colors,
            active: false,
            loading: false,
            generation: 0,
            samples: Samples::default(),
            poll: None,
            scroll: ScrollHandle::new(),
        }
    }
    pub fn belongs_to(&self, terminal: &Entity<Terminal>) -> bool {
        self.source.entity_id() == terminal.entity_id()
    }
    pub fn set_theme(&mut self, colors: TerminalPalette, cx: &mut Context<Self>) {
        if self.colors != colors {
            self.colors = colors;
            cx.notify();
        }
    }
    pub fn sync(&mut self, active: bool, colors: TerminalPalette, cx: &mut Context<Self>) {
        self.set_theme(colors, cx);
        if self.active == active {
            return;
        }
        self.active = active;
        self.generation = self.generation.wrapping_add(1);
        self.poll = None;
        self.loading = false;
        self.samples.pause();
        if active {
            self.refresh(cx);
        }
        cx.notify();
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.loading {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.poll = Some(cx.spawn(async move |this, cx| {
            loop {
                let request = this.update(cx, |this, cx| {
                    if !this.active || this.generation != generation {
                        return None;
                    }
                    this.loading = true;
                    cx.notify();
                    Some(
                        this.source
                            .update(cx, |terminal, _| {
                                terminal.system_info_request(&this.options)
                            })
                            .and_then(|request| request),
                    )
                });
                let Ok(Some(request)) = request else {
                    break;
                };
                let result = cx
                    .background_spawn(async move { request.and_then(|request| request.read()) })
                    .await;
                let keep_polling = this.update(cx, |this, cx| {
                    if !this.active || this.generation != generation {
                        return false;
                    }
                    let sample = result.unwrap_or_else(|error| {
                        Some(Snapshot {
                            error: Some(error.to_string()),
                            ..Default::default()
                        })
                    });
                    if let Some(mut info) = sample {
                        info.latency_ms = this
                            .source
                            .upgrade()
                            .and_then(|terminal| terminal.read(cx).ssh_latency_ms());
                        this.samples.apply(info, Instant::now());
                    }
                    this.loading = false;
                    cx.notify();
                    true
                });
                if !matches!(keep_polling, Ok(true)) {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(5)).await;
            }
        }));
    }
    fn palette(&self) -> Palette {
        let mut p = Palette::terminal(self.colors);
        p.background = p.sidebar;
        p.surface = p.sidebar;
        p
    }
    fn has_live_session(&self, cx: &App) -> bool {
        self.source.upgrade().is_some_and(|terminal| {
            terminal.read(cx).phase() == crate::connection_state::Phase::Completing
        })
    }
    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let Some(info) = &self.samples.info else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .when(self.active, |v| v.child(Spinner::new().with_size(px(18.))))
                .when(!self.active, |v| {
                    v.child(crate::localization::view(if self.has_live_session(cx) {
                        "Updates paused"
                    } else {
                        "No live SSH session"
                    }))
                })
                .into_any_element();
        };
        if !info.has_data() {
            return div()
                .size_full()
                .p(px(16.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .child(crate::localization::text(
                    cx,
                    "workspace.label.unableToLoadSystemInformation",
                ))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .text_center()
                        .child(
                            info.error.clone().unwrap_or_else(|| {
                                "The host returned no system information.".into()
                            }),
                        ),
                )
                .child(
                    Button::new("retry-system-info")
                        .small()
                        .ghost()
                        .custom(ButtonCustomVariant::for_surface(p))
                        .label("Retry")
                        .disabled(!self.active || self.loading)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                )
                .into_any_element();
        }
        let cpu = info
            .cpu_usage_percent
            .filter(|v| v.is_finite())
            .map(|v| (v / 100.).clamp(0., 1.) as f32);
        let memory = usage(info.memory_used_bytes, info.memory_total_bytes);
        let swap = usage(info.swap_used_bytes, info.swap_total_bytes);
        let details = [
            ("Operating system", info.os_name.clone()),
            ("Kernel", info.kernel.clone()),
            ("Architecture", info.architecture.clone()),
            ("Uptime", info.uptime_seconds.map(data::uptime)),
            (
                "Load average",
                info.load_average.map(|_| {
                    [info.load_average, info.load_average_5, info.load_average_15]
                        .into_iter()
                        .flatten()
                        .map(|v| format!("{v:.2}"))
                        .collect::<Vec<_>>()
                        .join("  ")
                }),
            ),
        ];
        let mut body = div()
            .id("system-info-scroll")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(px(12.))
            .pb(px(18.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(metric(
                "CPU",
                info.cpu_usage_percent
                    .map(|v| format!("{v:.1}%"))
                    .unwrap_or_else(|| "—".into()),
                [
                    info.cpu_count
                        .map(|v| crate::localization::text(cx, &format!("{v} cores")).to_string()),
                    info.load_average.map(|v| {
                        crate::localization::text(cx, &format!("load {v:.2}")).to_string()
                    }),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · "),
                cpu,
                p,
            ))
            .child(metric(
                "Memory",
                percent(memory),
                used_total(info.memory_used_bytes, info.memory_total_bytes),
                memory,
                p,
            ))
            .child(metric(
                "Swap",
                percent(swap),
                used_total(info.swap_used_bytes, info.swap_total_bytes),
                swap,
                p,
            ))
            .child(
                section("System", p).children(details.into_iter().filter_map(|(label, value)| {
                    value.map(|value| {
                        div()
                            .px(px(10.))
                            .py(px(6.))
                            .flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .w(px(92.))
                                    .flex_shrink_0()
                                    .text_size(px(10.))
                                    .text_color(p.muted)
                                    .child(crate::localization::view(label)),
                            )
                            .child(div().flex_1().min_w_0().text_size(px(10.5)).child(value))
                    })
                })),
            );
        if !info.processes.is_empty() {
            let sort = self.samples.sort;
            let ascending = self.samples.ascending;
            body = body.child(
                section("Top processes", p)
                    .child(
                        div().px(px(5.)).flex().children(
                            [
                                ("Memory", Sort::Memory, 0.3),
                                ("CPU", Sort::Cpu, 0.2),
                                ("Command", Sort::Command, 0.5),
                            ]
                            .map(|(label, value, width)| {
                                Button::new(label)
                                    .ghost()
                                    .custom(ButtonCustomVariant::for_surface(p))
                                    .w(relative(width))
                                    .h(px(29.))
                                    .px(px(4.))
                                    .text_size(px(9.5))
                                    .content_start()
                                    .label(label)
                                    .text_color(if sort == value { p.text } else { p.muted })
                                    .when(sort == value, |b| {
                                        b.trailing_icon(if ascending {
                                            IconName::ChevronUp
                                        } else {
                                            IconName::ChevronDown
                                        })
                                        .icon_size(px(11.))
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.samples.select_sort(value);
                                        cx.notify();
                                    }))
                            }),
                        ),
                    )
                    .children(self.samples.processes().iter().map(|process| {
                        table_row(
                            [
                                bytes(process.memory_bytes),
                                format!("{:.1}%", process.cpu_usage_percent),
                                process.command.clone(),
                            ],
                            p,
                        )
                    })),
            );
        }
        if !info.network_interfaces.is_empty() {
            let interfaces = info.network_interfaces.clone();
            let selected = self.samples.interface.clone().unwrap_or_default();
            let owner = cx.weak_entity();
            let menu_selected = selected.clone();
            body = body.child(
                section_with(
                    "Network",
                    p,
                    Button::new("system-interface")
                        .xsmall()
                        .ghost()
                        .custom(ButtonCustomVariant::for_surface(p))
                        .w(px(104.))
                        .raw_label(selected)
                        .trailing_icon(IconName::ChevronDown)
                        .icon_size(px(12.))
                        .dropdown_menu(move |menu, _, _| {
                            interfaces.iter().fold(
                                menu.width(180.).palette(p),
                                |menu, interface| {
                                    let name = interface.name.clone();
                                    let owner = owner.clone();
                                    menu.item(
                                        MenuItem::new(name.clone())
                                            .checked(name == menu_selected)
                                            .on_click(move |_, _, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    this.samples.select_interface(name.clone());
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                },
                            )
                        })
                        .into_any_element(),
                )
                .child(
                    div()
                        .px(px(10.))
                        .py(px(8.))
                        .flex()
                        .gap(px(12.))
                        .child(div().text_color(p.accent).child(format!(
                            "↓ {}/s",
                            bytes(self.samples.download.round().max(0.) as u64)
                        )))
                        .child(div().text_color(p.muted).child(format!(
                            "↑ {}/s",
                            bytes(self.samples.upload.round().max(0.) as u64)
                        ))),
                )
                .child(div().px(px(10.)).pb(px(10.)).child(trend(
                    self.samples.downloads.clone(),
                    self.samples.uploads.clone(),
                    58.,
                    p,
                ))),
            );
        }
        if let Some(latency) = info.latency_ms {
            body = body.child(
                section_with(
                    "Latency",
                    p,
                    div()
                        .text_color(p.accent)
                        .child(format!("{latency:.1} ms"))
                        .into_any_element(),
                )
                .child(div().px(px(10.)).py(px(8.)).child(trend(
                    self.samples.latencies.clone(),
                    vec![],
                    48.,
                    p,
                ))),
            );
        }
        if !info.filesystems.is_empty() {
            body = body.child(section("Filesystems", p).children(
                info.filesystems.iter().take(12).map(|fs| {
                    div()
                        .px(px(10.))
                        .py(px(7.))
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .font_family(platform_family())
                                        .child(fs.path.clone()),
                                )
                                .child(div().text_color(p.muted).text_size(px(10.)).child(
                                    crate::localization::view(format!(
                                        "{} free / {}",
                                        bytes(fs.total_bytes.saturating_sub(fs.used_bytes)),
                                        bytes(fs.total_bytes)
                                    )),
                                )),
                        )
                        .child(progress(
                            usage(Some(fs.used_bytes), Some(fs.total_bytes)),
                            p,
                        ))
                }),
            ));
        }
        body.when_some(info.error.clone(), |v, error| {
            v.child(
                div()
                    .text_color(p.muted)
                    .text_size(px(10.5))
                    .child(crate::localization::message(cx, &error)),
            )
        })
        .scrollbars()
        .into_any_element()
    }
}
impl Render for SystemTool {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let target = self.options.host.clone();
        let copy = target.clone();
        let hostname = self
            .samples
            .info
            .as_ref()
            .and_then(|info| info.hostname.clone());
        let subtitle = hostname
            .filter(|hostname| hostname != &target)
            .map(|h| format!("{h} · {target}"))
            .unwrap_or(target);
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .text_color(p.text)
            .text_size(px(10.5))
            .child(
                div()
                    .h(px(54.))
                    .flex_shrink_0()
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeights::MEDIUM)
                                    .child(crate::localization::text(
                                        cx,
                                        "workspace.label.systemInformation",
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(p.muted)
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(subtitle),
                            ),
                    )
                    .child(
                        Button::new("copy-system-host")
                            .xsmall()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(p))
                            .icon(IconName::Copy)
                            .tooltip("Copy host")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                            }),
                    )
                    .child(
                        Button::new("refresh-system-info")
                            .xsmall()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(p))
                            .icon(IconName::RefreshCw)
                            .loading(self.loading)
                            .disabled(self.loading || !self.active)
                            .tooltip("Refresh system information")
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .child(div().h(px(1.)).flex_shrink_0().bg(p.border))
            .when(!self.active, |v| {
                v.child(div().px(px(12.)).py(px(6.)).text_color(p.muted).child(
                    crate::localization::message(
                        cx,
                        if self.has_live_session(cx) {
                            "Updates paused"
                        } else {
                            "SSH session is not active"
                        },
                    ),
                ))
            })
            .child(div().flex_1().min_h_0().child(self.body(cx)))
    }
}
fn percent(value: Option<f32>) -> String {
    value
        .map(|v| format!("{:.0}%", v * 100.))
        .unwrap_or_else(|| "—".into())
}
fn progress(value: Option<f32>, p: Palette) -> impl IntoElement {
    div()
        .w_full()
        .h(px(4.))
        .rounded(px(2.))
        .overflow_hidden()
        .bg(p.border)
        .when_some(value, |v, value| {
            v.child(div().w(relative(value)).h_full().bg(p.accent))
        })
}
fn metric(
    label: &'static str,
    value: String,
    detail: String,
    ratio: Option<f32>,
    p: Palette,
) -> impl IntoElement {
    div()
        .w_full()
        .flex_shrink_0()
        .rounded(px(8.))
        .border_1()
        .border_color(p.border)
        .bg(p.control)
        .p(px(10.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeights::MEDIUM)
                        .child(crate::localization::view(label)),
                )
                .child(
                    div()
                        .text_color(p.accent)
                        .child(crate::localization::view(value)),
                ),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(p.muted)
                .child(crate::localization::view(detail)),
        )
        .child(progress(ratio, p))
}
fn section(title: &'static str, p: Palette) -> Div {
    section_with(title, p, div().into_any_element())
}
fn section_with(title: &'static str, p: Palette, trailing: AnyElement) -> Div {
    div()
        .w_full()
        .flex_shrink_0()
        .mt(px(6.))
        .rounded(px(8.))
        .border_1()
        .border_color(p.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(33.))
                .flex_shrink_0()
                .px(px(10.))
                .bg(p.control)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeights::MEDIUM)
                        .child(crate::localization::view(title)),
                )
                .child(trailing),
        )
}
fn table_row(cells: [String; 3], p: Palette) -> impl IntoElement {
    div()
        .px(px(9.))
        .py(px(6.))
        .flex()
        .text_color(p.text)
        .children(cells.into_iter().zip([0.3, 0.2, 0.5]).enumerate().map(
            |(index, (value, width))| {
                div()
                    .w(relative(width))
                    .min_w_0()
                    .pr(px(4.))
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .when(index == 2, |v| v.font_family(platform_family()))
                    .child(value)
            },
        ))
}
fn trend(primary: Vec<f64>, secondary: Vec<f64>, height: f32, p: Palette) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            for fraction in [0.5, 1.] {
                window.paint_quad(fill(
                    Bounds::new(
                        point(
                            bounds.left(),
                            bounds.top() + bounds.size.height * fraction - px(0.5),
                        ),
                        size(bounds.size.width, px(0.5)),
                    ),
                    p.border,
                ));
            }
            let maximum = primary
                .iter()
                .chain(&secondary)
                .copied()
                .filter(|v| v.is_finite())
                .fold(1_f64, f64::max);
            for (values, color) in [(&primary, p.accent), (&secondary, p.muted)] {
                let mut path = PathBuilder::stroke(px(1.4));
                for (index, value) in values.iter().enumerate() {
                    let x = if values.len() <= 1 {
                        1.
                    } else {
                        index as f32 / (values.len() - 1) as f32
                    };
                    let point = point(
                        bounds.left() + bounds.size.width * x,
                        bounds.bottom()
                            - bounds.size.height * (*value / maximum).clamp(0., 1.) as f32,
                    );
                    if index == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                if values.len() > 1
                    && let Ok(path) = path.build()
                {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .w_full()
    .h(px(height))
}
