//! Mosh is an SSH-card option, sharing SSH credentials and terminal appearance.
use super::*;
pub(super) struct Draft {
    pub enabled: bool,
    pub configured: bool,
    pub command: Entity<InputState>,
    select: SelectModel<bool>,
    _event: Subscription,
}
fn options() -> Vec<SelectOption<bool>> {
    vec![
        SelectOption::localized(false, "Disabled"),
        SelectOption::localized(true, "Enabled"),
    ]
}
impl Draft {
    pub fn new(host: Option<&Host>, window: &mut Window, cx: &mut Context<Nauterm>) -> Self {
        let enabled = host.is_some_and(|host| host.connections.mosh);
        let command = cx.new(|cx| {
            InputState::new(window, cx).default_value(
                host.map(|host| host.connections.mosh_command.clone())
                    .unwrap_or_else(|| crate::mosh::DEFAULT_COMMAND.into()),
            )
        });
        let select = SelectModel::new(options(), Some(enabled), window, cx);
        let event = cx.subscribe(
            &select.state,
            |this, state, event: &SelectEvent<bool>, cx| {
                if let Some(draft) = this
                    .editor
                    .as_mut()
                    .and_then(|editor| editor.host.as_mut())
                    .filter(|draft| !draft.busy)
                    && draft.mosh.select.state.entity_id() == state.entity_id()
                    && let SelectEvent::Confirm(Some(Some(enabled))) = event
                {
                    draft.mosh.enabled = *enabled;
                    draft.mosh.configured = true;
                    cx.notify();
                }
            },
        );
        Self {
            enabled,
            configured: host.is_some(),
            command,
            select,
            _event: event,
        }
    }
    pub fn value(&self, cx: &App) -> anyhow::Result<Option<crate::mosh::Config>> {
        self.enabled
            .then(|| crate::mosh::Config::new(Some(&self.command.read(cx).value())))
            .transpose()
    }
    pub fn reconcile(&mut self, window: &mut Window, cx: &mut App) {
        self.select
            .reconcile(options(), Some(self.enabled), window, cx);
    }
    pub fn inherit(&mut self, parent: &crate::mosh::Config, window: &mut Window, cx: &mut App) {
        if !self.configured {
            self.enabled = true;
            self.configured = true;
        }
        let value = self.command.read(cx).value();
        if value.trim().is_empty() || value.trim() == crate::mosh::DEFAULT_COMMAND {
            self.command.update(cx, |input, cx| {
                input.set_value(parent.command.clone(), window, cx)
            });
        }
    }
    pub fn fields(&self, busy: bool) -> Vec<AnyElement> {
        let mut fields = vec![self.select.field("Mosh", busy)];
        if self.enabled {
            fields.push(
                Field::input(&self.command, "Mosh server command")
                    .disabled(busy)
                    .into_any_element(),
            );
        }
        fields
    }
}
