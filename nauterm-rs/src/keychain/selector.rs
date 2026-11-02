//! The reference's optional SSH Key/Certificate control, shared by Host, Group
//! and Identity forms. Only public key summaries enter this presentation.
use crate::{
    components::menu::{DropdownMenu, MenuItem},
    components::{Button, ButtonCustomVariant, SelectAction, SelectModel, SelectOption},
    palette::Palette,
    vault::Inventory,
};
use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme, Disableable, Sizable},
    prelude::*,
    *,
};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyKind {
    Key,
    Certificate,
}
impl KeyKind {
    fn label(self) -> &'static str {
        match self {
            Self::Key => "Key",
            Self::Certificate => "Certificate",
        }
    }
    fn icon(self) -> IconName {
        match self {
            Self::Key => IconName::KeyRound,
            Self::Certificate => IconName::BadgeCheck,
        }
    }
    fn action(self) -> SelectAction {
        match self {
            Self::Key => SelectAction::new("key", "Create key", self.icon()),
            Self::Certificate => {
                SelectAction::new("certificate", "Create certificate", self.icon())
            }
        }
    }
}
fn kind_for(inventory: &Inventory, selected: Option<i64>) -> Option<KeyKind> {
    selected.map(|id| {
        if inventory
            .keys
            .iter()
            .any(|key| key.id == id && key.certificate)
        {
            KeyKind::Certificate
        } else {
            KeyKind::Key
        }
    })
}
fn options(inventory: &Inventory, kind: Option<KeyKind>) -> Vec<SelectOption<i64>> {
    inventory
        .keys
        .iter()
        .filter(|key| kind != Some(KeyKind::Certificate) || key.certificate)
        .map(|key| SelectOption::new(key.id, key.name.clone()))
        .collect()
}
pub(crate) struct KeySelector {
    pub select: SelectModel<i64>,
    kind: Option<KeyKind>,
    configured: bool,
}
impl KeySelector {
    pub fn new(
        inventory: &Inventory,
        selected: Option<i64>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let kind = kind_for(inventory, selected);
        Self {
            select: SelectModel::new(options(inventory, kind), selected, window, cx),
            kind,
            configured: false,
        }
    }
    pub fn reconcile(
        &mut self,
        inventory: &Inventory,
        selected: Option<i64>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // An async initial inventory may reveal that the saved key is a certificate.
        // Once the user chooses a view, keep it (Key intentionally includes certs).
        if !self.configured || (self.kind.is_none() && selected.is_some()) {
            self.kind = kind_for(inventory, selected);
        }
        self.select
            .reconcile(options(inventory, self.kind), selected, window, cx);
    }
    /// The owner clears its selected key only when changing to another kind.
    pub fn choose_kind(&mut self, kind: KeyKind, cx: &mut App) -> bool {
        let changed = self.kind != Some(kind);
        self.kind = Some(kind);
        self.configured = true;
        self.select.open(cx);
        changed
    }
    pub fn clear(&mut self) {
        self.kind = None;
        self.configured = true;
    }
    pub fn view(
        &self,
        disabled: bool,
        on_kind: impl Fn(KeyKind, &mut Window, &mut App) + 'static,
        cx: &App,
    ) -> AnyElement {
        let p = Palette::new(cx.theme().is_dark());
        let selected_kind = self.kind;
        let on_kind = Rc::new(on_kind);
        let mut content = div().flex().flex_col().gap(px(12.));
        if let Some(kind) = selected_kind {
            content = content.child(self.select.field_with_actions(
                kind.label(),
                disabled,
                vec![kind.action()],
            ));
        }
        content
            .child(
                div().flex().child(
                    Button::new(("ssh-add-credential", self.select.state.entity_id()))
                        .small()
                        .ghost()
                        .icon(crate::assets::icon(IconName::Plus))
                        .label("Key or Certificate")
                        .disabled(disabled)
                        .custom(ButtonCustomVariant::new(cx).foreground(p.muted.into()))
                        .dropdown_menu(move |mut menu, _, _| {
                            menu = menu.width(190.);
                            for kind in [KeyKind::Key, KeyKind::Certificate] {
                                if selected_kind == Some(kind) {
                                    continue;
                                }
                                let on_kind = on_kind.clone();
                                menu = menu.item(
                                    MenuItem::localized(kind.label())
                                        .icon(kind.icon())
                                        .on_click(move |_, window, cx| on_kind(kind, window, cx)),
                                );
                            }
                            menu
                        }),
                ),
            )
            .into_any_element()
    }
}
