//! Reference reconnect countdown, kept separate from the initial connection page.
use super::*;
use crate::{
    components::ButtonCustomVariant,
    connection::{Connection, ConnectionEvent},
    typography::FontWeights,
};

pub struct Requested;
impl EventEmitter<Requested> for Terminal {}
pub struct Changed;
impl EventEmitter<Changed> for Terminal {}
pub struct ExitRequested;
impl EventEmitter<ExitRequested> for Terminal {}

#[derive(Default)]
struct Countdown {
    attempt: u32,
    deadline: Option<Instant>,
    pending: bool,
    dismissed: bool,
    boundary: bool,
    exit_requested: bool,
    exit_reported: bool,
    input: zeroize::Zeroizing<String>,
}
impl Countdown {
    fn delay(&self) -> Duration {
        Duration::from_secs(3 << self.attempt.min(2))
    }
    fn sync(&mut self, phase: Phase, now: Instant) {
        if matches!(phase, Phase::Failed | Phase::Exited) && !self.exit_requested && !self.pending {
            let delay = self.delay();
            self.deadline.get_or_insert(now + delay);
        } else {
            self.deadline = None;
        }
        if phase == Phase::Completing {
            self.attempt = 0;
            self.exit_reported = false;
            self.boundary = false;
            self.pending = false;
        }
    }
    fn request(&mut self) -> bool {
        if self.pending || self.exit_requested {
            return false;
        }
        self.attempt = self.attempt.saturating_add(1);
        self.deadline = None;
        self.pending = true;
        self.dismissed = false;
        true
    }
    fn report_exit(&mut self, exited: bool, local: bool) -> bool {
        if !exited || self.exit_reported || !(local || self.exit_requested) {
            return false;
        }
        self.exit_reported = true;
        true
    }
    fn command(&mut self, command: &str) {
        self.input.clear();
        self.exit_requested = active_exit_command(command);
    }
    fn input(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.exit_requested = false;
        for ch in text.chars() {
            match ch {
                '\u{4}' => {
                    self.exit_requested = true;
                    self.input.clear();
                }
                '\u{8}' | '\u{7f}' => {
                    self.input.pop();
                }
                '\u{3}' => {
                    self.input.clear();
                    self.exit_requested = false;
                }
                '\u{15}' => self.input.clear(),
                '\r' | '\n' => {
                    self.exit_requested |= active_exit_command(&self.input);
                    self.input.clear();
                }
                ch if !ch.is_control() && self.input.len() < 4096 => self.input.push(ch),
                _ => {}
            }
        }
    }
}
fn active_exit_command(command: &str) -> bool {
    let mut words = command.split_whitespace();
    match (words.next(), words.next(), words.next()) {
        (Some("exit" | "logout"), None, None) => true,
        (Some("exit"), Some(status), None) => status.parse::<i64>().is_ok(),
        _ => false,
    }
}

pub(super) struct Reconnect {
    pub profile: Option<crate::native::SshOptions>,
    model: Countdown,
    timer: Option<Task<()>>,
    pub login: Option<Entity<Connection>>,
    intervened: bool,
    completion_hold: bool,
    spinner_epoch: Instant,
    pub focus_allowed: bool,
    events: Vec<Subscription>,
}
impl Reconnect {
    pub fn new(profile: Option<crate::native::SshOptions>) -> Self {
        Self {
            profile,
            model: Countdown::default(),
            timer: None,
            login: None,
            intervened: false,
            completion_hold: false,
            spinner_epoch: Instant::now(),
            focus_allowed: false,
            events: vec![],
        }
    }
}
impl Terminal {
    pub(crate) fn set_reconnect_focus(&mut self, allowed: bool) {
        self.reconnect.focus_allowed = allowed;
    }
    pub(super) fn reconnect_shows_login(&self) -> bool {
        self.reconnect.login.is_some()
            && (self.reconnect.intervened || self.reconnect.completion_hold)
    }
    pub(crate) fn reconnect_login(&self) -> Option<Entity<Connection>> {
        self.reconnect.login.clone()
    }
    pub(crate) fn reconnect_profile(&self) -> Option<crate::native::SshOptions> {
        self.reconnect
            .profile
            .as_ref()
            .map(crate::native::SshOptions::for_new_connection)
    }
    pub(crate) fn reconnect_mosh(&self) -> Option<crate::mosh::Config> {
        self.session.mosh_config()
    }
    pub(crate) fn reconnect_snapshot(&self) -> ConnectionState {
        let mut state = self.connection.clone();
        state.phase = self.phase();
        state
    }
    pub(crate) fn reconnect_in_place(
        &mut self,
        options: &crate::native::SshOptions,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.reset_prediction(cx);
        self.reconnect.timer = None;
        self.reconnect.model.deadline = None;
        self.reconnect.model.pending = false;
        self.reconnect.model.exit_requested = false;
        self.reconnect.model.input.clear();
        if !self.reconnect.model.boundary {
            self.session.reconnect_boundary()?;
            self.reconnect.model.boundary = true;
        }
        self.connection.begin_attempt();
        if self.working_directory.reset() {
            cx.emit(TerminalDirectoryChanged(None));
            cx.emit(TerminalMetadataChanged);
        }
        self.session.reconnect_remote(options)?;
        self.prompt_click = None;
        self.cancel_mouse(cx);
        self.clear_selection();
        self.error = None;
        self.refresh(cx);
        Ok(())
    }
    pub(crate) fn reconnect_failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.reconnect.model.pending = false;
        self.connection.fail(message);
        self.sync_reconnect(cx);
        cx.emit(Changed);
        cx.notify();
    }
    pub(crate) fn defer_reconnect(&mut self, cx: &mut Context<Self>) {
        self.reconnect.model.pending = false;
        self.sync_reconnect(cx);
    }
    pub(crate) fn install_reconnect(
        &mut self,
        connection: Entity<Connection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reconnect.events = vec![
            cx.observe(&connection, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &connection,
                window,
                |this, connection, event: &ConnectionEvent, window, cx| match event {
                    ConnectionEvent::Ready => {
                        let focused = connection.read(cx).contains_focus(window, cx);
                        if let Some(profile) = connection.read(cx).duplicate_options() {
                            this.paths =
                                paths::Paths::new(this.session.path_source(Some(profile.clone())));
                            this.reconnect.profile = Some(profile);
                        }
                        this.reconnect.login = None;
                        this.reconnect.completion_hold = false;
                        this.reconnect.intervened = false;
                        this.reconnect.events.clear();
                        if focused {
                            this.focus.focus(window, cx);
                        }
                        cx.emit(TerminalConnected);
                        cx.notify();
                    }
                    ConnectionEvent::Close => cx.emit(TerminalCommand::Close),
                },
            ),
        ];
        self.reconnect.login = Some(connection);
        cx.notify();
    }
    pub(super) fn note_reconnect_input(&mut self, text: &str) {
        if self.reconnect.profile.is_some()
            && self.phase() == Phase::Completing
            && !self.frame.alternate_screen
            && !self.composer_sensitive()
        {
            self.reconnect.model.input(text);
        }
    }
    pub(super) fn note_reconnect_command(&mut self, command: &str) {
        if self.reconnect.profile.is_some() {
            self.reconnect.model.command(command);
        }
    }
    pub(super) fn sync_reconnect(&mut self, cx: &mut Context<Self>) {
        if self.session.is_replay() {
            return;
        }
        if self
            .reconnect
            .model
            .report_exit(self.frame.exited, self.session.is_local())
        {
            cx.emit(ExitRequested);
        }
        if self.reconnect.profile.is_none() {
            return;
        }
        if matches!(self.phase(), Phase::HostKey | Phase::Authentication)
            || self.connection.changed_key
        {
            self.reconnect.intervened = true;
        }
        if self.phase() == Phase::Completing {
            self.reconnect.completion_hold |= self.reconnect.intervened;
            self.reconnect.intervened = false;
        } else if matches!(self.phase(), Phase::Failed | Phase::Exited)
            && !self.reconnect.intervened
        {
            self.reconnect.completion_hold = false;
        }
        self.reconnect.model.sync(self.phase(), Instant::now());
        if self.reconnect.intervened {
            self.reconnect.model.deadline = None;
        }
        if let Some(deadline) = self.reconnect.model.deadline {
            if self.reconnect.timer.is_none() {
                self.reconnect.timer = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(Duration::from_secs(1)).await;
                        let running = this
                            .update(cx, |this, cx| {
                                if this.reconnect.model.deadline != Some(deadline) {
                                    return false;
                                }
                                if Instant::now() >= deadline {
                                    this.reconnect.timer = None;
                                    this.request_reconnect(cx);
                                    return false;
                                }
                                cx.notify();
                                true
                            })
                            .unwrap_or(false);
                        if !running {
                            break;
                        }
                    }
                }));
            }
        } else {
            self.reconnect.timer = None;
        }
    }
    fn request_reconnect(&mut self, cx: &mut Context<Self>) {
        if self.reconnect.model.request() {
            self.reconnect.timer = None;
            cx.emit(Requested);
            cx.notify();
        }
    }
    pub(super) fn reconnect_overlay(
        &self,
        bottom_inset: Pixels,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.reconnect.profile.is_none()
            || self.reconnect.model.dismissed
            || self.reconnect.model.exit_requested
            || !matches!(
                self.phase(),
                Phase::Connecting | Phase::Failed | Phase::Exited
            )
        {
            return None;
        }
        let palette = self.appearance.palette();
        let mut p = crate::palette::Palette::terminal(palette);
        p.surface = p
            .background
            .blend(p.text.alpha(if palette.dark { 0.09 } else { 0.07 }));
        p.muted = crate::components::motion::mix(p.surface, p.text, 0.64);
        p.border = crate::components::motion::mix(
            p.surface,
            p.text,
            if palette.dark { 0.18 } else { 0.13 },
        );
        let danger = rgb(palette.ansi[1]);
        let remaining = self.reconnect.model.deadline.map(|deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .as_secs_f64()
                .ceil()
                .max(1.) as u64
        });
        let detail = if let Some(seconds) = remaining {
            format!("Reconnecting in {seconds}s...")
        } else if self.phase() == Phase::Connecting || self.reconnect.model.pending {
            "Reconnecting...".into()
        } else {
            self.connection.message.clone()
        };
        Some(
            div()
                .absolute()
                .left(px(12.))
                .right(px(12.))
                .bottom(bottom_inset + px(12.))
                .child(
                    div()
                        .id("terminal-reconnect-card")
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .w_full()
                        .max_w(px(244.))
                        .pl(px(10.))
                        .pr(px(7.))
                        .py(px(9.))
                        .rounded(px(9.))
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border)
                        .text_color(p.text)
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .shadow(vec![BoxShadow {
                            color: black().alpha(if palette.dark { 0.34 } else { 0.14 }),
                            offset: point(px(0.), px(7.)),
                            blur_radius: px(18.),
                            spread_radius: px(-6.),
                            inset: false,
                        }])
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(9.))
                                .child(progress(
                                    remaining,
                                    self.reconnect.model.delay(),
                                    danger,
                                    p.muted.alpha(0.24),
                                    self.reconnect.spinner_epoch,
                                ))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(div().font_weight(FontWeights::SEMIBOLD).child(
                                            if matches!(self.phase(), Phase::Failed | Phase::Exited)
                                            {
                                                "Connection lost"
                                            } else {
                                                "Reconnecting"
                                            },
                                        ))
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(p.muted)
                                                .text_ellipsis()
                                                .child(crate::localization::message(cx, &detail)),
                                        ),
                                )
                                .child(
                                    Button::new("dismiss-reconnect")
                                        .ghost()
                                        .custom(ButtonCustomVariant::for_surface(p))
                                        .size(px(30.))
                                        .icon(crate::assets::icon(IconName::X))
                                        .icon_size(px(17.))
                                        .tooltip("Dismiss reconnect status")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.reconnect.model.dismissed = true;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .mt(px(8.))
                                .flex()
                                .gap(px(6.))
                                .child(
                                    Button::new("reconnect-now")
                                        .small()
                                        .h(px(26.))
                                        .px(px(7.))
                                        .rounded(px(6.))
                                        .text_size(px(12.))
                                        .font_weight(FontWeights::SEMIBOLD)
                                        .border_1()
                                        .border_color(p.accent)
                                        .label("Reconnect")
                                        .custom(
                                            ButtonCustomVariant::for_surface(p)
                                                .color(p.accent.into())
                                                .foreground(p.background.into()),
                                        )
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.request_reconnect(cx)
                                            }),
                                        ),
                                )
                                .child(
                                    Button::new("close-disconnected-terminal")
                                        .small()
                                        .h(px(26.))
                                        .px(px(7.))
                                        .rounded(px(6.))
                                        .text_size(px(12.))
                                        .font_weight(FontWeights::SEMIBOLD)
                                        .border_1()
                                        .border_color(danger.alpha(0.36))
                                        .label("Close terminal")
                                        .custom(
                                            ButtonCustomVariant::for_surface(p)
                                                .color(danger.alpha(0.08).into())
                                                .foreground(danger.into()),
                                        )
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            cx.emit(TerminalCommand::Close)
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}

fn progress(
    remaining: Option<u64>,
    delay: Duration,
    color: Rgba,
    track: Rgba,
    epoch: Instant,
) -> AnyElement {
    let fraction = remaining
        .map(|value| value as f32 / delay.as_secs_f32())
        .unwrap_or(0.72)
        .clamp(0., 1.);
    let arc = div().absolute().inset_0().child(
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let rotation = if remaining.is_none() {
                    window.request_animation_frame();
                    epoch.elapsed().as_secs_f32() * std::f32::consts::TAU / 0.9
                } else {
                    0.
                };
                let center = bounds.center();
                let radius = bounds.size.width / 2. - px(1.5);
                for (amount, color) in [(1., track), (fraction, color)] {
                    let mut path = PathBuilder::stroke(px(2.5));
                    let steps = (80. * amount).ceil() as usize;
                    if steps == 0 {
                        continue;
                    }
                    for index in 0..=steps {
                        let angle = rotation - std::f32::consts::FRAC_PI_2
                            + std::f32::consts::TAU * amount * index as f32 / steps as f32;
                        let position = point(
                            center.x + radius * angle.cos(),
                            center.y + radius * angle.sin(),
                        );
                        if index == 0 {
                            path.move_to(position);
                        } else {
                            path.line_to(position);
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .size_full(),
    );
    let arc = arc.into_any_element();
    div()
        .size(px(32.))
        .flex_shrink_0()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .text_color(color)
        .text_size(px(14.))
        .font_weight(FontWeights::SEMIBOLD)
        .child(arc)
        .when_some(remaining, |view, value| view.child(value.to_string()))
        .when(remaining.is_none(), |view| {
            view.child(crate::assets::icon(IconName::RotateCw).size(px(15.)))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::Countdown;
    use crate::connection_state::Phase;
    use std::time::{Duration, Instant};
    #[test]
    fn countdown_retries_with_reference_backoff_and_dismiss_does_not_cancel() {
        let now = Instant::now();
        let mut state = Countdown::default();
        for seconds in [3, 6, 12, 12] {
            state.sync(Phase::Exited, now);
            assert_eq!(state.deadline, Some(now + Duration::from_secs(seconds)));
            state.dismissed = true;
            assert!(state.request());
            assert!(!state.dismissed);
            assert!(!state.request());
            state.pending = false;
        }
        state.sync(Phase::Authentication, now);
        assert!(state.deadline.is_none());
        state.sync(Phase::Completing, now);
        assert!(!state.boundary);
        assert_eq!(state.delay(), Duration::from_secs(3));
    }
    #[test]
    fn explicit_exit_input_suppresses_reconnect_but_subsequent_input_restores_it() {
        for text in ["exit\r", "logout\r", "exit 2\r", "exit -1\r", "\u{4}"] {
            let mut state = Countdown::default();
            state.input(text);
            assert!(state.exit_requested, "{text:?}");
            state.sync(Phase::Exited, Instant::now());
            assert!(state.deadline.is_none());
            state.input("ls\r");
            assert!(!state.exit_requested);
        }
        let mut state = Countdown::default();
        state.input("exitx\u{7f}\r");
        assert!(state.exit_requested);
        state.input("printf exit\r");
        assert!(!state.exit_requested);
    }
    #[test]
    fn close_requests_wait_for_native_exit_and_are_emitted_once() {
        let mut local = Countdown::default();
        assert!(!local.report_exit(false, true));
        assert!(local.report_exit(true, true));
        assert!(!local.report_exit(true, true));
        let mut remote = Countdown::default();
        assert!(!remote.report_exit(true, false));
        remote.input("exit\r");
        assert!(!remote.report_exit(false, false));
        assert!(remote.report_exit(true, false));
        assert!(!remote.report_exit(true, false));
    }
    #[test]
    fn reported_commands_and_replaced_prompt_input_resolve_exit_intent() {
        let mut state = Countdown::default();
        state.input("old prompt input");
        state.input("\u{15}exit 0\r");
        assert!(state.exit_requested);
        state.command("printf exit");
        assert!(!state.exit_requested);
        state.input("\x1b[A\r"); // History recall is identified by shell metadata.
        state.command("exit");
        assert!(state.exit_requested);
        assert!(state.report_exit(true, false));
        let mut interrupted = Countdown::default();
        interrupted.input("exit\u{3}\r");
        assert!(!interrupted.exit_requested);
    }
}
