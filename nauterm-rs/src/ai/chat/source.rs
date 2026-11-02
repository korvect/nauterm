use super::*;
use crate::ai::history::Scope;

/// A UI binding, never serialized into a conversation or workspace snapshot.
/// A terminal can be absent while its connection page is still visible.
#[derive(Clone)]
pub enum Source {
    Terminal {
        view: Option<WeakEntity<Terminal>>,
        host_uuid: Option<String>,
        colors: TerminalPalette,
    },
    Workspace {
        dark: bool,
    },
}
impl Source {
    pub(super) fn owner(&self, cx: &App) -> Option<crate::native::CommandOwner> {
        match self {
            Self::Terminal {
                view: Some(view), ..
            } => view.upgrade().and_then(|view| view.read(cx).ai_owner()),
            _ => None,
        }
    }
    pub(super) fn prepare(
        &self,
        owner: &crate::native::CommandOwner,
        cx: &mut App,
    ) -> anyhow::Result<crate::native::CommandTarget> {
        if let Self::Terminal {
            view: Some(view), ..
        } = self
        {
            return view
                .upgrade()
                .ok_or_else(|| anyhow::anyhow!("The terminal session is closed."))?
                .update(cx, |view, cx| view.prepare_ai_command(owner, cx));
        }
        anyhow::bail!("This conversation is not bound to a terminal.")
    }
    pub fn terminal(
        view: Option<&Entity<Terminal>>,
        host_uuid: Option<String>,
        colors: TerminalPalette,
    ) -> Self {
        Self::Terminal {
            view: view.map(Entity::downgrade),
            host_uuid,
            colors,
        }
    }
    pub fn workspace(dark: bool) -> Self {
        Self::Workspace { dark }
    }
    pub(super) fn scope(&self) -> Scope {
        match self {
            Self::Terminal { .. } => Scope::Terminal,
            Self::Workspace { .. } => Scope::Workspace,
        }
    }
    pub(super) fn host_uuid(&self) -> Option<String> {
        match self {
            Self::Terminal { host_uuid, .. } => host_uuid.clone(),
            Self::Workspace { .. } => None,
        }
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Workspace { dark }, Self::Workspace { dark: next }) => dark == next,
            (
                Self::Terminal {
                    view,
                    host_uuid,
                    colors,
                },
                Self::Terminal {
                    view: next,
                    host_uuid: next_host,
                    colors: next_colors,
                },
            ) => {
                view.as_ref().map(|view| view.entity_id())
                    == next.as_ref().map(|view| view.entity_id())
                    && host_uuid == next_host
                    && colors == next_colors
            }
            _ => false,
        }
    }
    pub(super) fn set_terminal_theme(&mut self, next: TerminalPalette) -> bool {
        if let Self::Terminal { colors, .. } = self
            && *colors != next
        {
            *colors = next;
            true
        } else {
            false
        }
    }
    pub(super) fn palette(&self) -> Palette {
        match self {
            Self::Terminal { colors, .. } => {
                let mut palette = Palette::terminal(*colors);
                palette.background = palette.sidebar;
                palette
            }
            Self::Workspace { dark } => {
                let mut palette = Palette::new(*dark);
                palette.background = palette.surface;
                palette
            }
        }
    }
    pub(super) fn available(&self, cx: &App) -> bool {
        match self {
            Self::Terminal {
                view: Some(view), ..
            } => view
                .upgrade()
                .is_some_and(|view| view.read(cx).ai_context_available()),
            _ => false,
        }
    }
    fn context_kinds(&self, cx: &App) -> (Option<&'static str>, bool) {
        if let Self::Terminal {
            view: Some(view), ..
        } = self
            && let Some(view) = view.upgrade()
        {
            return view.read(cx).ai_context_kinds();
        }
        (None, false)
    }
    pub(super) fn capture(&self, preferences: &Preferences, cx: &App) -> String {
        if self.available(cx)
            && let Self::Terminal {
                view: Some(view), ..
            } = self
        {
            return view
                .upgrade()
                .map(|view| view.read(cx).ai_context(preferences))
                .unwrap_or_default();
        }
        String::new()
    }
    pub(super) fn placeholder(&self) -> &'static str {
        match self {
            Self::Terminal { .. } => "Ask about this terminal...",
            Self::Workspace { .. } => "Ask about this workspace...",
        }
    }
}
impl Chat {
    pub(super) fn observe_source(&mut self, cx: &mut Context<Self>) {
        self._source_events = None;
        if let Source::Terminal {
            view: Some(view), ..
        } = &self.source
            && let Some(view) = view.upgrade()
        {
            let mut previous = view.read(cx).ai_context_kinds();
            self._source_events = Some(cx.observe(&view, move |_, view, cx| {
                let next = view.read(cx).ai_context_kinds();
                if next != previous {
                    previous = next;
                    cx.notify();
                }
            }));
        }
    }
    pub(super) fn context_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (selection, recent) = self.source.context_kinds(cx);
        let selection = selection.filter(|_| self.context.include_selection);
        let recent = recent && self.context.include_recent_output;
        if selection.is_none() && !recent {
            return None;
        }
        let p = self.palette();
        let chip = |id, label| {
            Button::new(id)
                .ghost()
                .custom(ButtonCustomVariant::for_surface(p))
                .h(px(27.))
                .px(px(7.))
                .rounded(px(6.))
                .border_1()
                .border_color(p.border)
                .bg(p.background)
                .text_size(px(11.))
                .content_gap(px(5.))
                .icon_size(px(12.))
                .icon(crate::assets::icon(IconName::Terminal).text_color(p.accent))
                .raw_label(label)
                .trailing_icon(crate::assets::icon(IconName::X).text_color(p.muted))
        };
        Some(
            div()
                .px(px(8.))
                .pt(px(8.))
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .when_some(selection, |chips, label| {
                    chips.child(
                        chip("ai-context-selection", label)
                            .tooltip("Remove terminal selection from context")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.context.include_selection = false;
                                cx.notify();
                            })),
                    )
                })
                .when(recent, |chips| {
                    chips.child(
                        chip("ai-context-output", "Recent output")
                            .tooltip("Remove recent output from context")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.context.include_recent_output = false;
                                cx.notify();
                            })),
                    )
                })
                .into_any_element(),
        )
    }
    pub(super) fn restore_context_button(&self, cx: &mut Context<Self>) -> Option<Button> {
        let (selection, recent) = self.source.context_kinds(cx);
        ((selection.is_some() && !self.context.include_selection)
            || (recent && !self.context.include_recent_output))
            .then(|| {
                Button::new("ai-restore-context")
                    .ghost()
                    .custom(ButtonCustomVariant::for_surface(self.palette()))
                    .size(px(30.))
                    .rounded_full()
                    .icon_size(px(17.))
                    .icon(IconName::Link2)
                    .tooltip("Include terminal context")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.context.include_selection = true;
                        this.context.include_recent_output = true;
                        cx.notify();
                    }))
            })
    }
}
