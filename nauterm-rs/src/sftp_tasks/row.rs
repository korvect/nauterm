use super::*;

impl TaskQueue {
    pub(super) fn task_row(
        &self,
        job: &Snapshot,
        p: Palette,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = job.id;
        let status = job.status;
        let inset = if compact { 10. } else { 19. };
        let label = crate::localization::text(cx, job.kind.label());
        let text = format!("{label} {}", presentation::primary_path(job));
        let mut highlights = vec![(
            0..label.len(),
            HighlightStyle {
                font_weight: Some(if compact {
                    FontWeights::MEDIUM
                } else {
                    FontWeights::SEMIBOLD
                }),
                ..Default::default()
            },
        )];
        if status == Status::Cancelled {
            highlights.push((
                label.len() + 1..text.len(),
                HighlightStyle {
                    strikethrough: Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: Some(p.muted.into()),
                    }),
                    ..Default::default()
                },
            ));
        }
        let button = |name: &'static str, icon: IconName, tooltip: &'static str, danger: bool| {
            let color = if danger { rgb(0xd54b3f) } else { p.muted };
            Button::new((name, id))
                .ghost()
                .xsmall()
                .size(px(26.))
                .p_0()
                .icon(crate::assets::icon(icon).size(px(if compact { 14. } else { 13. })))
                .custom(
                    crate::components::ButtonCustomVariant::for_surface(p)
                        .foreground(color.into())
                        .hover(if danger { color.alpha(0.12) } else { p.control }.into())
                        .highlight(if danger { color } else { p.accent }.alpha(0.10).into())
                        .active(if danger { color } else { p.accent }.alpha(0.16).into()),
                )
                .tooltip(tooltip)
        };
        let retryable = job.pausable && job.resumable && !job.permission_retry;
        let mut actions = div()
            .absolute()
            .right(px(if compact { 4. } else { 11. }))
            .top(px(2.))
            .flex()
            .items_center()
            .gap(px(0.));
        if retryable
            && matches!(
                status,
                Status::Queued
                    | Status::Running
                    | Status::Pausing
                    | Status::Cancelling
                    | Status::Paused
                    | Status::Failed
            )
        {
            actions = actions.child(
                button(
                    "sftp-pause-task",
                    if matches!(status, Status::Paused | Status::Failed) {
                        IconName::Play
                    } else {
                        IconName::Pause
                    },
                    match status {
                        Status::Failed => "Retry transfer",
                        Status::Paused => "Resume transfer",
                        _ => "Pause transfer",
                    },
                    false,
                )
                .disabled(matches!(status, Status::Pausing | Status::Cancelling))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.service.pause_toggle(id);
                    this.sync(cx);
                })),
            );
        }
        if job.permission_retry {
            let attempt = job.attempt;
            actions = actions.child(
                button(
                    "sftp-sudo-task",
                    IconName::RefreshCw,
                    "Retry with sudo",
                    true,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(ticket) = this.service.permission_ticket(id, attempt) {
                        cx.emit(TaskAuthorizationRequested(ticket));
                    }
                })),
            );
        }
        actions = actions.child(
            button(
                "sftp-cancel-task",
                IconName::X,
                if status.finished() {
                    "Delete record"
                } else {
                    "Cancel task"
                },
                false,
            )
            .disabled(status == Status::Cancelling)
            .on_click(cx.listener(move |this, _, _, cx| {
                if status.finished() {
                    this.dismiss(id, cx);
                } else {
                    this.service.cancel(id);
                }
                this.sync(cx);
            })),
        );
        let fraction = if job.progress.total == 0 {
            0.
        } else {
            (job.progress.bytes as f32 / job.progress.total as f32).clamp(0., 1.)
        };
        let progress_color = if status == Status::Paused {
            rgb(0xd18b22)
        } else {
            p.accent
        };
        let progress = if job.progress.total == 0 {
            let stripe = div()
                .relative()
                .h_full()
                .w(relative(0.35))
                .bg(progress_color);
            if status.running() && !cx.reduce_motion() {
                stripe
                    .with_animation(
                        ("sftp-task-progress", id),
                        Animation::new(std::time::Duration::from_millis(1500)).repeat(),
                        |stripe, t| stripe.left(relative(-0.35 + t * 1.35)),
                    )
                    .into_any_element()
            } else {
                stripe.into_any_element()
            }
        } else {
            div()
                .h_full()
                .w(relative(fraction))
                .bg(progress_color)
                .into_any_element()
        };
        div()
            .id(("sftp-task-row", id))
            .aria_label(format!(
                "{}: {} · {}",
                job.label,
                presentation::primary_path(job),
                status.label()
            ))
            .relative()
            .w_full()
            .min_w_0()
            .h(px(if compact { 44. } else { 45. }))
            .flex_shrink_0()
            .px(px(inset))
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(2.))
            .child(
                div()
                    .pr(px(48.))
                    .min_w_0()
                    .text_size(px(if compact { 10.5 } else { 11. }))
                    .line_height(px(if compact { 16. } else { 14. }))
                    .font_weight(FontWeights::MEDIUM)
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(StyledText::new(text).with_highlights(highlights)),
            )
            .child(
                div()
                    .min_w_0()
                    .text_size(px(if compact { 9. } else { 11. }))
                    .line_height(px(if compact { 12. } else { 14. }))
                    .font_weight(FontWeights::MEDIUM)
                    .text_color(if status == Status::Failed {
                        rgb(0xd54b3f)
                    } else {
                        p.muted
                    })
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(presentation::subtitle(
                        job,
                        if compact {
                            None
                        } else {
                            self.rows.queue_position(id)
                        },
                        crate::localization::language(cx),
                    )),
            )
            .child(actions)
            .child(
                div()
                    .absolute()
                    .left(px(inset))
                    .right(px(inset))
                    .bottom_0()
                    .h(px(1.))
                    .bg(p.border),
            )
            .when(status.running() || status == Status::Paused, |view| {
                view.child(
                    div()
                        .absolute()
                        .left(px(inset))
                        .right(px(inset))
                        .bottom_0()
                        .h(px(if compact { 1.5 } else { 2. }))
                        .rounded_full()
                        .bg(p.control)
                        .overflow_hidden()
                        .child(progress),
                )
            })
            .into_any_element()
    }
}
