use crate::components::ScrollbarsExt;
use crate::components::menu::{ContextMenuExt, DropdownMenu, MenuItem};
use crate::{
    components::{Button, Field, SelectEvent, Spinner},
    key_export::{ExportClosed, KeyExport},
    keygen::{self, KeyType, Parameters},
    layout,
    model::{AppState, CredentialRef},
    native::Core,
    palette::Palette,
    typography::FontWeights,
    ui::{CloseEditor, DeleteCards, NewItem, SelectCards},
    vault::{IdentityRecord, Inventory, KeyRecord, Vault},
};
use gpui_kit::base::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{
    assets::IconName,
    component::{Disableable, Icon, Sizable},
    prelude::*,
    *,
};
use std::{collections::BTreeSet, path::PathBuf};
mod creation;
mod fido2;
mod files;
mod import;
mod navigation;
mod related;
pub(crate) mod selector;
use creation::Method;
pub use files::Message;
pub use related::RelatedEvent;
use selector::{KeyKind, KeySelector};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Key,
    Generate,
    Certificate,
    Identity,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Item {
    key: bool,
    id: i64,
}
struct Editor {
    scroll: ScrollHandle,
    kind: Kind,
    fido: Option<fido2::Draft>,
    id: Option<i64>,
    name: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    passphrase: Entity<InputState>,
    private: Entity<TextareaState>,
    public: Entity<TextareaState>,
    certificate: Entity<TextareaState>,
    key_id: Option<i64>,
    error: Option<String>,
    name_error: Option<SharedString>,
    private_error: Option<SharedString>,
    username_error: Option<SharedString>,
    generation: Parameters,
    automatic_name: bool,
    method: Method,
    importing: bool,
    picking: bool,
    import_generation: u64,
    _name: Subscription,
    _username: Subscription,
    _materials: Vec<Subscription>,
    key_select: Option<KeySelector>,
    _key_selection: Option<Subscription>,
    _key_creation: Option<Subscription>,
    related: Option<related::Nested>,
}
impl Editor {
    fn is_generating(&self) -> bool {
        self.kind == Kind::Generate
            || (self.id.is_none() && self.kind != Kind::Identity && self.method == Method::Generate)
    }
    fn can_import(&self, related: bool) -> bool {
        self.id.is_none()
            && self.kind != Kind::Identity
            && !self.is_generating()
            && (!related || self.method == Method::Import)
    }
    fn item(&self) -> Option<Item> {
        self.id.map(|id| Item {
            id,
            key: self.kind != Kind::Identity,
        })
    }
}

pub struct Changed;
type ExportDrawer = (Item, Entity<KeyExport>, Subscription);
enum RetiredDrawer {
    Editor(Box<Editor>),
    Export(ExportDrawer),
}
pub struct Keychain {
    known_hosts_path: PathBuf,
    database_path: PathBuf,
    vault: Option<Vault>,
    inventory: Inventory,
    search: Entity<InputState>,
    dark: bool,
    focus: FocusHandle,
    editor: Option<Editor>,
    export: Option<ExportDrawer>,
    drawer_motion: crate::components::panel::Presence<RetiredDrawer>,
    parent_width: Option<f32>,
    resources: AppState,
    selection: BTreeSet<Item>,
    navigation: crate::management::navigation::Navigation<Item>,
    loading: bool,
    saving: bool,
    exporting_file: bool,
    error: Option<String>,
    revision: u64,
    refresh_generation: u64,
    related: bool,
    inventory_owner: Option<WeakEntity<Keychain>>,
    list: bool,
    sort: crate::management::SortOrder,
    search_open: bool,
    delete_pending: bool,
    _menu: Option<Subscription>,
    _search: Subscription,
}

impl Keychain {
    pub(crate) fn feedback(&self) -> Vec<(Option<EntityId>, crate::components::message::Message)> {
        let mut feedback = vec![];
        if let Some(error) = &self.error {
            feedback.push((
                None,
                crate::components::message::Message::error(error.clone()),
            ));
        }
        if let Some(editor) = &self.editor
            && let Some(error) = &editor.error
        {
            feedback.push((
                Some(editor.name.entity_id()),
                crate::components::message::Message::error(error.clone()),
            ));
        }
        feedback
    }
    pub(crate) fn drawer_open(&self) -> bool {
        self.editor.is_some() || self.export.is_some()
    }
    pub(crate) fn finish_hidden_drawer(&mut self) {
        if self.editor.is_none() && self.export.is_none() {
            self.drawer_motion
                .sample(false, std::time::Instant::now(), true);
        }
    }
    pub(crate) fn set_parent_width(&mut self, width: f32, cx: &mut Context<Self>) {
        if self.parent_width != Some(width) {
            self.parent_width = Some(width);
            cx.notify();
        }
    }
    fn retire_editor(&mut self) {
        if let Some(editor) = self.editor.take() {
            self.drawer_motion
                .retire(RetiredDrawer::Editor(Box::new(editor)));
        }
    }
    fn retire_export(&mut self) {
        if let Some(export) = self.export.take() {
            self.drawer_motion.retire(RetiredDrawer::Export(export));
        }
    }
    fn display_drawer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.editor.is_some() {
            self.drawer_motion.take_retired();
            return self.drawer(window, cx);
        }
        if let Some((_, view, _)) = &self.export {
            self.drawer_motion.take_retired();
            return view.clone().into_any_element();
        }
        match self.drawer_motion.take_retired() {
            Some(RetiredDrawer::Editor(editor)) => {
                self.editor = Some(*editor);
                let content = self.drawer(window, cx);
                self.retire_editor();
                content
            }
            Some(RetiredDrawer::Export(export)) => {
                let content = export.1.clone().into_any_element();
                self.drawer_motion.retire(RetiredDrawer::Export(export));
                content
            }
            None => div().into_any_element(),
        }
    }
}
impl EventEmitter<Changed> for Keychain {}
fn icon(name: IconName) -> Icon {
    Icon::new(name).with_size(px(16.))
}

impl Keychain {
    pub fn has_resource_edit(&self, cx: &App) -> bool {
        self.editor.is_some()
            || self.export.is_some()
            || self.delete_pending
            || self.loading
            || self.busy(cx)
    }
    pub fn new(
        known_hosts_path: PathBuf,
        database_path: PathBuf,
        search: Entity<InputState>,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::empty(known_hosts_path, database_path, search, dark, cx);
        this.retry(cx);
        this
    }
    fn empty(
        known_hosts_path: PathBuf,
        database_path: PathBuf,
        search: Entity<InputState>,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_subscription = cx.subscribe(
            &search,
            |_, _, _: &gpui_kit::base::input::InputEvent, cx| cx.notify(),
        );
        Self {
            known_hosts_path,
            database_path,
            vault: None,
            inventory: Inventory::default(),
            search,
            dark,
            focus: cx.focus_handle(),
            editor: None,
            export: None,
            drawer_motion: Default::default(),
            parent_width: None,
            resources: AppState::default(),
            selection: BTreeSet::new(),
            navigation: Default::default(),
            loading: false,
            saving: false,
            exporting_file: false,
            error: None,
            revision: 0,
            refresh_generation: 0,
            related: false,
            inventory_owner: None,
            list: false,
            sort: Default::default(),
            search_open: false,
            delete_pending: false,
            _menu: None,
            _search: search_subscription,
        }
    }
    pub fn vault(&self) -> Option<Vault> {
        self.vault.clone()
    }
    pub fn inventory(&self) -> Inventory {
        self.inventory.clone()
    }
    pub fn inventory_ref(&self) -> &Inventory {
        &self.inventory
    }
    pub fn set_resources(&mut self, resources: AppState, cx: &mut Context<Self>) {
        if self.resources.hosts == resources.hosts
            && self.resources.groups == resources.groups
            && self.resources.dark == resources.dark
        {
            return;
        }
        self.resources = resources;
        let data = self.export_resources();
        if let Some((_, view, _)) = &self.export {
            view.update(cx, |view, cx| view.set_data(data, cx));
        }
        cx.notify();
    }
    pub fn has_connection(&self, cx: &App) -> bool {
        self.export
            .as_ref()
            .is_some_and(|(_, view, _)| view.read(cx).has_connection())
    }
    fn busy(&self, cx: &App) -> bool {
        self.saving
            || self
                .editor
                .as_ref()
                .and_then(|editor| editor.fido.as_ref())
                .is_some_and(|draft| draft.view.read(cx).busy())
            || self.exporting_file
            || self
                .export
                .as_ref()
                .is_some_and(|(_, view, _)| view.read(cx).running())
    }
    fn active_item(&self) -> Option<Item> {
        self.export
            .as_ref()
            .map(|(item, _, _)| *item)
            .or_else(|| self.editor.as_ref().and_then(Editor::item))
    }
    fn export_resources(&self) -> AppState {
        let mut resources = self.resources.clone();
        resources.dark = self.dark;
        for host in &mut resources.hosts {
            if let Some(CredentialRef::Identity(id)) = host.credential
                && let Some(identity) = self
                    .inventory
                    .identities
                    .iter()
                    .find(|identity| identity.id == id)
            {
                host.username = identity.username.clone();
            }
        }
        resources
    }
    fn start_export(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let Some(key) = self
            .inventory
            .keys
            .iter()
            .find(|key| key.id == item.id && item.key)
            .cloned()
        else {
            return;
        };
        if key.public_key.is_none() {
            crate::components::message::show(
                window,
                crate::components::message::Message::error("A valid public key is required."),
                cx,
            );
            return;
        }
        let Some(vault) = self.vault.clone() else {
            return;
        };
        let resources = self.export_resources();
        let known_hosts_path = self.known_hosts_path.clone();
        let owner = cx.weak_entity();
        let view = cx.new(|cx| {
            KeyExport::new(key, resources, vault, known_hosts_path, window, cx).with_keychain(owner)
        });
        let events = cx.subscribe_in(&view, window, |this, _, _: &ExportClosed, window, cx| {
            this.close_editor(window, cx)
        });
        self.revision += 1;
        self.retire_editor();
        self.selection = BTreeSet::from([item]);
        self.export = Some((item, view, events));
        cx.notify();
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.dark != dark {
            self.dark = dark;
            let data = self.export_resources();
            if let Some((_, view, _)) = &self.export {
                view.update(cx, |view, cx| view.set_data(data, cx));
            }
            cx.notify();
        }
    }
    fn retry(&mut self, cx: &mut Context<Self>) {
        match Core::discover().and_then(|core| Vault::open(core, self.database_path.clone())) {
            Ok(vault) => {
                self.vault = Some(vault);
                self.refresh(cx);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }
    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(vault) = self.vault.clone() else {
            return;
        };
        self.loading = true;
        self.error = None;
        self.refresh_generation += 1;
        let generation = self.refresh_generation;
        cx.spawn(async move |this, cx| {
            let result = vault.inventory().await;
            let _ = this.update(cx, |this, cx| {
                if this.refresh_generation != generation {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(inventory) => {
                        this.inventory = inventory;
                        let data = this.export_resources();
                        if let Some((_, view, _)) = &this.export {
                            view.update(cx, |view, cx| view.set_data(data, cx));
                        }
                        this.error = None;
                        cx.emit(Changed);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn back_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(draft) = self.editor.as_ref().and_then(|editor| editor.fido.as_ref()) {
            draft.view.update(cx, |view, cx| view.request_close(cx));
            return;
        }
        if self.busy(cx) {
            return;
        }
        if let Some(nested) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.related.as_ref())
        {
            nested
                .view
                .update(cx, |view, cx| view.back_editor(window, cx));
            return;
        }
        self.close_editor(window, cx);
    }
    pub fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        self.revision += 1;
        self.retire_editor();
        self.retire_export();
        self.selection.clear();
        self.delete_pending = false;
        self.focus.focus(window, cx);
        if self.related {
            cx.emit(RelatedEvent::Cancelled);
        }
        cx.notify();
    }
    pub fn create(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        if self.vault.is_none() || self.busy(cx) {
            return;
        }
        self.revision += 1;
        self.selection.clear();
        self.install_editor(kind, None, None, window, cx);
        self.focus_editor(window, cx);
    }
    fn focus_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &self.editor {
            editor.name.update(cx, |field, cx| field.focus(window, cx));
        }
    }
    fn install_editor(
        &mut self,
        kind: Kind,
        key: Option<KeyRecord>,
        identity: Option<IdentityRecord>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.retire_export();
        let input = |value: String, masked: bool, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value)
                    .masked(masked)
            })
        };
        let text = |value: String, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(4, 8)
                    .default_value(value)
            })
        };
        let id = key
            .as_ref()
            .and_then(|k| k.id)
            .or_else(|| identity.as_ref().and_then(|i| i.id));
        let name = key
            .as_ref()
            .map(|k| k.name.clone())
            .or_else(|| identity.as_ref().map(|i| i.name.clone()))
            .unwrap_or_default();
        let name = input(name, false, window, cx);
        let name_subscription = cx.subscribe(&name, |this, state, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change)
                && let Some(editor) = &mut this.editor
                && editor.name.entity_id() == state.entity_id()
            {
                // Change reports user edits; programmatic defaults use set_value.
                editor.automatic_name = false;
                editor.name_error = None;
                cx.notify();
            }
        });
        let private = text(
            key.as_ref()
                .and_then(|k| k.private_key.clone())
                .unwrap_or_default(),
            window,
            cx,
        );
        let public = text(
            key.as_ref()
                .and_then(|k| k.public_key.clone())
                .unwrap_or_default(),
            window,
            cx,
        );
        let certificate = text(
            key.as_ref()
                .and_then(|k| k.certificate.clone())
                .unwrap_or_default(),
            window,
            cx,
        );
        let materials = [&private, &public, &certificate]
            .into_iter()
            .map(|field| {
                cx.subscribe_in(
                    field,
                    window,
                    |this, field, event: &InputEvent, window, cx| {
                        if matches!(event, InputEvent::Change)
                            && this.editor.as_ref().is_some_and(|editor| {
                                editor.private.entity_id() == field.entity_id()
                                    || editor.public.entity_id() == field.entity_id()
                                    || editor.certificate.entity_id() == field.entity_id()
                            })
                        {
                            if let Some(editor) = &mut this.editor
                                && (editor.importing || editor.picking)
                            {
                                editor.import_generation = editor.import_generation.wrapping_add(1);
                                editor.importing = false;
                                editor.picking = false;
                            }
                            if let Some(editor) = &mut this.editor
                                && editor.private_error.is_some()
                                && (!editor.private.read(cx).value().trim().is_empty()
                                    || editor.id.is_some()
                                        && (!editor.public.read(cx).value().trim().is_empty()
                                            || !editor
                                                .certificate
                                                .read(cx)
                                                .value()
                                                .trim()
                                                .is_empty()))
                            {
                                editor.private_error = None;
                            }
                            this.refresh_automatic_name(window, cx);
                            cx.notify();
                        }
                    },
                )
            })
            .collect();
        let key_id = identity.as_ref().and_then(|item| item.key_id);
        let username = input(
            identity
                .as_ref()
                .and_then(|i| i.username.clone())
                .unwrap_or_default(),
            false,
            window,
            cx,
        );
        let username_subscription = cx.subscribe_in(
            &username,
            window,
            |this, state, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(editor) = this
                        .editor
                        .as_mut()
                        .filter(|editor| editor.username.entity_id() == state.entity_id())
                {
                    if !editor.username.read(cx).value().trim().is_empty() {
                        editor.username_error = None;
                    }
                    this.refresh_automatic_name(window, cx);
                    cx.notify();
                }
            },
        );
        let key_select =
            (kind == Kind::Identity).then(|| KeySelector::new(&self.inventory, key_id, window, cx));
        let key_selection = key_select.as_ref().map(|select| {
            cx.subscribe_in(
                &select.select.state,
                window,
                |this, state, event: &SelectEvent<i64>, window, cx| {
                    if this.saving {
                        return;
                    }
                    if let Some(editor) = &mut this.editor
                        && editor
                            .key_select
                            .as_ref()
                            .is_some_and(|select| select.select.state == *state)
                    {
                        let SelectEvent::Confirm(value) = event;
                        editor.key_id = value.flatten();
                        if editor.key_id.is_none() {
                            editor.key_select.as_mut().unwrap().clear();
                            editor.username.read(cx).focus_handle(cx).focus(window, cx);
                        }
                        this.refresh_automatic_name(window, cx);
                        cx.notify();
                    }
                },
            )
        });
        let key_creation = key_select.as_ref().map(|select| {
            cx.subscribe_in(
                &select.select.state,
                window,
                |this, state, event: &crate::components::SelectCreate, window, cx| {
                    if !this
                        .editor
                        .as_ref()
                        .and_then(|editor| editor.key_select.as_ref())
                        .is_some_and(|select| select.select.state == *state)
                    {
                        return;
                    }
                    let kind = match event.action {
                        "key" => Kind::Key,
                        "certificate" => Kind::Certificate,
                        _ => return,
                    };
                    this.open_related_key(kind, &event.text, window, cx);
                },
            )
        });
        let automatic_name = id.is_none() && name.read(cx).value().trim().is_empty();
        let editor = Editor {
            scroll: ScrollHandle::new(),
            kind,
            fido: None,
            id,
            name,
            username,
            password: input(
                identity
                    .as_ref()
                    .and_then(|i| i.password.clone())
                    .unwrap_or_default(),
                true,
                window,
                cx,
            ),
            passphrase: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(
                        key.as_ref()
                            .and_then(|k| k.passphrase.clone())
                            .unwrap_or_default(),
                    )
                    .placeholder("Passphrase")
                    .masked(true)
            }),
            private,
            public,
            certificate,
            key_id,
            error: None,
            name_error: None,
            private_error: None,
            username_error: None,
            generation: Parameters::default(),
            automatic_name,
            method: if kind == Kind::Generate {
                Method::Generate
            } else {
                Method::Paste
            },
            importing: false,
            picking: false,
            import_generation: 0,
            _name: name_subscription,
            _username: username_subscription,
            _materials: materials,
            key_select,
            _key_selection: key_selection,
            related: None,
            _key_creation: key_creation,
        };
        if let Some(item) = editor.item() {
            self.selection = BTreeSet::from([item]);
        }
        self.editor = Some(editor);
        self.refresh_automatic_name(window, cx);
        cx.notify();
    }
    fn edit(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_with_focus(item, false, window, cx);
    }
    fn edit_with_focus(
        &mut self,
        item: Item,
        keep_focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(vault) = self.vault.clone() else {
            return;
        };
        if self.busy(cx) {
            return;
        }
        self.retire_export();
        self.revision += 1;
        let revision = self.revision;
        self.selection = BTreeSet::from([item]);
        cx.spawn_in(window, async move |this, cx| {
            let result = if item.key {
                vault.key(item.id).await.map(|key| (Some(key), None))
            } else {
                vault
                    .identity(item.id)
                    .await
                    .map(|identity| (None, Some(identity)))
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if this.revision != revision || this.selection != BTreeSet::from([item]) {
                    return;
                }
                match result {
                    Ok((key, identity)) => {
                        let kind = if !item.key {
                            Kind::Identity
                        } else if key.as_ref().is_some_and(|k| k.certificate.is_some()) {
                            Kind::Certificate
                        } else {
                            Kind::Key
                        };
                        this.install_editor(kind, key, identity, window, cx);
                        if !keep_focus {
                            this.focus_editor(window, cx);
                        }
                    }
                    Err(error) => {
                        this.error = Some(error.to_string());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(draft) = self.editor.as_ref().and_then(|editor| editor.fido.as_ref()) {
            draft.view.update(cx, |view, cx| view.primary(window, cx));
            return;
        }
        if self.editor.as_ref().is_some_and(Editor::is_generating) {
            self.generate(window, cx);
            return;
        }
        if let Some(editor) = &mut self.editor {
            editor.name_error = None;
            editor.username_error = None;
            editor.private_error = None;
        }
        let (Some(editor), Some(vault)) = (&self.editor, self.vault.clone()) else {
            return;
        };
        if self.saving || self.exporting_file || editor.importing || editor.picking {
            return;
        }
        let name = editor.name.read(cx).value().trim().to_owned();
        let username = editor.username.read(cx).value().trim().to_owned();
        if editor.kind == Kind::Identity && username.is_empty() {
            let editor = self.editor.as_mut().unwrap();
            editor.username_error = Some("Username is required.".into());
            editor.error = None;
            cx.notify();
            return;
        }
        if name.is_empty() {
            self.editor.as_mut().unwrap().name_error = Some("Name is required.".into());
            cx.notify();
            return;
        }
        let optional = |text: String| (!text.is_empty()).then_some(text);
        let identity = IdentityRecord {
            id: editor.id,
            name: name.clone(),
            username: Some(username),
            password: optional(editor.password.read(cx).value().to_string()),
            key_id: editor.key_id,
        };
        let key = KeyRecord {
            id: editor.id,
            name,
            private_key: optional(editor.private.read(cx).value().to_string()),
            public_key: optional(editor.public.read(cx).value().trim().to_owned()),
            certificate: optional(editor.certificate.read(cx).value().trim().to_owned()),
            // Paste/Import preserve only an existing record's stored passphrase.
            // A new generation draft's passphrase must not follow a method switch
            // into a new pasted/imported record (the reference keeps it transient).
            passphrase: editor
                .id
                .and_then(|_| optional(editor.passphrase.read(cx).value().to_string())),
        };
        let is_identity = editor.kind == Kind::Identity;
        if !is_identity && let Err(error) = key.validate() {
            let editor = self.editor.as_mut().unwrap();
            editor.private_error = Some(error.to_string().into());
            editor.error = None;
            cx.notify();
            return;
        }
        let revision = self.revision;
        let inventory_owner = self.inventory_owner.clone();
        self.saving = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = if is_identity {
                vault.save_identity(identity).await
            } else {
                vault.save_key(key).await
            };
            if result.is_ok()
                && let Some(owner) = inventory_owner
            {
                let _ = owner.update(cx, |view, cx| view.refresh(cx));
            }
            let _ = this.update_in(cx, |this, window, cx| {
                this.saving = false;
                match result {
                    Ok(id) => {
                        if this.revision == revision {
                            this.finish_save(id, is_identity, window, cx);
                        }
                        if !this.related {
                            this.refresh(cx);
                        }
                    }
                    Err(error) => {
                        if this.revision == revision {
                            if let Some(editor) = &mut this.editor {
                                editor.error = Some(error.to_string());
                            }
                        } else {
                            this.error = Some(error.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn generate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(editor), Some(vault)) = (&mut self.editor, self.vault.clone()) else {
            return;
        };
        if self.saving
            || self.exporting_file
            || !editor.is_generating()
            || editor.importing
            || editor.picking
        {
            return;
        }
        let parameters = editor.generation;
        let name = editor.name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            editor.name_error = Some("Name is required.".into());
            editor.error = None;
            cx.notify();
            return;
        }
        editor.name_error = None;
        let passphrase = zeroize::Zeroizing::new(editor.passphrase.read(cx).value().to_string());
        let certificate = editor.certificate.read(cx).value().trim().to_owned();
        editor.error = None;
        self.saving = true;
        let revision = self.revision;
        let inventory_owner = self.inventory_owner.clone();
        cx.spawn_in(window, async move |this, cx| {
            let generated = cx
                .background_spawn(async move { keygen::generate(parameters, name, passphrase) })
                .await;
            let result = match generated {
                Ok(mut key) => {
                    key.certificate = (!certificate.is_empty()).then_some(certificate);
                    vault.save_key(key).await
                }
                Err(error) => Err(error),
            };
            if result.is_ok()
                && let Some(owner) = inventory_owner
            {
                let _ = owner.update(cx, |view, cx| view.refresh(cx));
            }
            let _ = this.update_in(cx, |this, window, cx| {
                this.saving = false;
                match result {
                    Ok(id) => {
                        if this.revision == revision {
                            this.finish_save(id, false, window, cx);
                        }
                        if !this.related {
                            this.refresh(cx);
                        }
                    }
                    Err(error) => {
                        if this.revision == revision {
                            if let Some(editor) = &mut this.editor {
                                editor.error = Some(error.to_string());
                            }
                        } else {
                            this.error = Some(error.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn set_key_type(&mut self, kind: KeyType, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if let Some(editor) = &mut self.editor {
            editor.generation.kind = kind;
            if editor.automatic_name {
                editor.name.update(cx, |field, cx| {
                    field.set_value(kind.suggested_name(), window, cx)
                });
            }
            editor.error = None;
            cx.notify();
        }
    }
    fn select_identity_key_kind(
        &mut self,
        kind: KeyKind,
        token: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving || self.exporting_file {
            return;
        }
        let Some(editor) = self.editor.as_mut().filter(|editor| {
            editor.name.entity_id() == token
                && editor.kind == Kind::Identity
                && editor.related.is_none()
        }) else {
            return;
        };
        if editor.key_select.as_mut().unwrap().choose_kind(kind, cx) {
            editor.key_id = None;
        }
        self.refresh_automatic_name(window, cx);
        cx.notify();
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let Some(item) = self.selection.first() else {
            return;
        };
        let key = item.key;
        self.selection = self
            .visible(cx)
            .into_iter()
            .filter(|(item, _, _)| item.key == key)
            .map(|(item, _, _)| item)
            .collect();
        cx.notify();
    }
    fn visible(&self, cx: &App) -> Vec<(Item, String, String)> {
        let query = self.search.read(cx).value().to_lowercase();
        let mut values: Vec<_> = self
            .inventory
            .keys
            .iter()
            .map(|key| {
                (
                    Item {
                        key: true,
                        id: key.id,
                    },
                    key.name.clone(),
                    key.algorithm.clone(),
                )
            })
            .chain(self.inventory.identities.iter().map(|identity| {
                (
                    Item {
                        key: false,
                        id: identity.id,
                    },
                    identity.name.clone(),
                    format!(
                        "{}{}",
                        identity.username,
                        identity
                            .key_id
                            .and_then(|id| self.inventory.keys.iter().find(|key| key.id == id))
                            .map(|key| format!(", {}", key.name))
                            .unwrap_or_default()
                    ),
                )
            }))
            .filter(|(_, name, detail)| format!("{name} {detail}").to_lowercase().contains(&query))
            .collect();
        values.sort_by(|a, b| {
            (!a.0.key).cmp(&(!b.0.key)).then_with(|| {
                self.sort
                    .compare((&a.1, a.0.id as u64), (&b.1, b.0.id as u64))
            })
        });
        values
    }
    fn duplicate(&mut self, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let (Some(first), Some(vault)) = (self.selection.first().copied(), self.vault.clone())
        else {
            return;
        };
        let ids = self
            .selection
            .iter()
            .filter(|item| item.key == first.key)
            .map(|item| item.id)
            .collect();
        self.saving = true;
        cx.spawn(async move |this, cx| {
            let result = vault.duplicate(first.key, ids).await;
            // A failed rollback can still leave a copy; always refresh instead
            // of continuing to show an inventory known to be stale.
            let inventory = vault.inventory().await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                let refresh_error = match inventory {
                    Ok(inventory) => {
                        this.inventory = inventory;
                        cx.emit(Changed);
                        None
                    }
                    Err(error) => Some(format!("Could not refresh keychain: {error}")),
                };
                match result {
                    Ok(ids) => {
                        this.revision += 1;
                        this.retire_editor();
                        this.retire_export();
                        this.selection = ids
                            .into_iter()
                            .map(|id| Item { key: first.key, id })
                            .collect();
                        this.error = refresh_error;
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let (Some(first), Some(vault)) = (self.selection.first().copied(), self.vault.clone())
        else {
            return;
        };
        let ids = self
            .selection
            .iter()
            .filter(|item| item.key == first.key)
            .map(|item| item.id)
            .collect();
        self.saving = true;
        cx.spawn(async move |this, cx| {
            let result = vault.delete(first.key, ids).await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                this.delete_pending = false;
                match result {
                    Ok(()) => {
                        this.revision += 1;
                        this.selection.clear();
                        this.retire_editor();
                        this.refresh(cx);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn card(&self, item: Item, name: String, detail: String, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.selection.contains(&item) || self.active_item() == Some(item);
        let can_edit = !self.selection.contains(&item) || self.selection.len() <= 1;
        let owner = cx.entity().downgrade();
        crate::components::item_card(
            SharedString::from(format!("credential-{}-{}", item.key, item.id)),
            self.dark,
            selected,
            self.list,
        )
        .list_menu_hint(self.list)
        .accessibility_label(name.clone())
        .reveal_on_layout(self.navigation.take_reveal(item))
        .child(
            div()
                .size(px(40.))
                .flex_shrink_0()
                .rounded(px(10.))
                .bg(rgb(0x00649d))
                .text_color(rgb(0xffffff))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(if item.key {
                    if self
                        .inventory
                        .keys
                        .iter()
                        .any(|key| key.id == item.id && key.certificate)
                    {
                        IconName::BadgeCheck
                    } else {
                        IconName::KeyRound
                    }
                } else {
                    IconName::Contact
                })),
        )
        .child(crate::components::card_text(name, detail, self.dark))
        .when(can_edit, |card| {
            card.hover_action(
                crate::components::card_edit_button(
                    SharedString::from(format!("edit-credential-{}-{}", item.key, item.id)),
                    self.dark,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.edit(item, window, cx);
                })),
            )
        })
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            cx.stop_propagation();
            this.focus.focus(window, cx);
            let modifiers = event.modifiers();
            let multiple = modifiers.platform || modifiers.control || modifiers.shift;
            if !multiple
                && this.selection.len() > 1
                && this.selection.contains(&item)
                && event.click_count() < 2
            {
                return;
            }
            if event.click_count() == 2
                || !multiple && (this.editor.is_some() || this.export.is_some())
            {
                this.edit(item, window, cx);
            } else {
                this.select_card(item, modifiers, cx);
            }
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, _, window, cx| {
                if this.busy(cx) {
                    return;
                }
                if !this.selection.contains(&item) {
                    this.selection = BTreeSet::from([item]);
                }
                this.focus.focus(window, cx);
                cx.notify();
            }),
        )
        .context_menu(move |menu, _, cx| {
            let shortcuts = crate::shortcuts::current(cx);
            let dismiss_owner = owner.clone();
            let menu_entity = cx.entity();
            let _ = owner.update(cx, |this, cx| {
                this._menu = Some(cx.subscribe(
                    &menu_entity,
                    move |this, _, _: &gpui_kit::DismissEvent, cx| {
                        if !this.delete_pending {
                            this.selection = this.active_item().into_iter().collect();
                            cx.notify();
                        }
                    },
                ));
            });
            let edit_owner = dismiss_owner.clone();
            let export_owner = dismiss_owner.clone();
            let file_owner = dismiss_owner.clone();
            let duplicate_owner = dismiss_owner.clone();
            let single = owner
                .upgrade()
                .is_some_and(|view| view.read(cx).selection.len() == 1);
            let public_key = owner.upgrade().and_then(|owner| {
                owner
                    .read(cx)
                    .inventory
                    .keys
                    .iter()
                    .find(|key| item.key && key.id == item.id)
                    .and_then(|key| key.public_key.clone())
            });
            let busy = owner.update(cx, |this, cx| this.busy(cx)).unwrap_or(true);
            menu.card_menu()
                .when(item.key && single, |menu| {
                    menu.item(
                        MenuItem::localized("Export to Host")
                            .icon(IconName::Upload)
                            .disabled(busy)
                            .on_click(move |_, window, cx| {
                                let _ = export_owner
                                    .update(cx, |this, cx| this.start_export(item, window, cx));
                            }),
                    )
                    .item(
                        MenuItem::localized("rs.keychain.action.exportToFile")
                            .icon(IconName::Download)
                            .disabled(busy)
                            .on_click(move |_, window, cx| {
                                let _ = file_owner
                                    .update(cx, |this, cx| this.export_file(item, window, cx));
                            }),
                    )
                    .item(
                        MenuItem::localized("rs.keychain.action.copyPublicKey")
                            .icon(IconName::Copy)
                            .disabled(busy || public_key.is_none())
                            .on_click(move |_, _, cx| {
                                if let Some(text) = &public_key {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                                }
                            }),
                    )
                })
                .when(single, |menu| {
                    menu.item(
                        shortcuts
                            .menu(
                                MenuItem::localized(if item.key {
                                    "Edit Key"
                                } else {
                                    "Edit Identity"
                                }),
                                crate::shortcuts::Command::Edit,
                            )
                            .icon(IconName::Pencil)
                            .disabled(busy)
                            .on_click(move |_, window, cx| {
                                let _ =
                                    edit_owner.update(cx, |this, cx| this.edit(item, window, cx));
                            }),
                    )
                })
                .item(
                    shortcuts
                        .menu(
                            MenuItem::localized("Duplicate"),
                            crate::shortcuts::Command::Duplicate,
                        )
                        .icon(IconName::Copy)
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let _ = duplicate_owner.update(cx, |this, cx| this.duplicate(cx));
                        }),
                )
                .separator()
                .item(
                    MenuItem::localized(if item.key {
                        "Delete Key"
                    } else {
                        "Delete Identity"
                    })
                    .destructive(true)
                    .icon(IconName::Trash)
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        let _ = dismiss_owner.update(cx, |this, cx| {
                            this.delete_pending = true;
                            cx.notify();
                        });
                    }),
                )
        })
        .into_any_element()
    }
    fn field(&self, label: &'static str, state: &Entity<InputState>, masked: bool) -> AnyElement {
        Field::input(state, label)
            .secret(masked)
            .disabled(self.saving)
            .into_any_element()
    }
    fn text_field(
        &self,
        label: &'static str,
        state: &Entity<TextareaState>,
        error: Option<SharedString>,
    ) -> AnyElement {
        Field::multiline_auto(state, label)
            .disabled(self.saving)
            .error(error)
            .into_any_element()
    }

    fn generation_controls(&self, editor: &Editor, cx: &mut Context<Self>) -> AnyElement {
        let parameters = editor.generation;
        let sizes = match parameters.kind {
            KeyType::Ecdsa => [521, 384, 256],
            KeyType::Rsa => [4096, 2048, 1024],
            KeyType::Ed25519 => [0; 3],
        };
        let selected_size = if parameters.kind == KeyType::Ecdsa {
            parameters.ecdsa_bits
        } else {
            parameters.rsa_bits
        };
        div()
            .flex()
            .flex_col()
            .gap(px(7.))
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeights::MEDIUM)
                    .child(crate::localization::text(cx, "workspace.keys.field.type")),
            )
            .child(
                crate::components::Segmented::new(
                    "generate-type",
                    parameters.kind,
                    KeyType::ALL.map(|kind| (kind, kind.label().into())),
                )
                .height(26.)
                .disabled(self.saving)
                .on_change(
                    cx.listener(|this, kind, window, cx| this.set_key_type(*kind, window, cx)),
                ),
            )
            .when(parameters.kind != KeyType::Ed25519, |view| {
                view.child(
                    div()
                        .mt(px(1.))
                        .text_size(px(12.))
                        .font_weight(FontWeights::MEDIUM)
                        .child(crate::localization::view(
                            if parameters.kind == KeyType::Ecdsa {
                                "Elliptic curve size (bits)"
                            } else {
                                "Key size (bits)"
                            },
                        )),
                )
                .child(
                    crate::components::Segmented::new(
                        "generate-bits",
                        selected_size,
                        sizes.map(|bits| (bits, bits.to_string().into())),
                    )
                    .height(26.)
                    .disabled(self.saving)
                    .on_change(cx.listener(|this, bits, _, cx| {
                        if this.saving {
                            return;
                        }
                        if let Some(editor) = &mut this.editor {
                            if editor.generation.kind == KeyType::Ecdsa {
                                editor.generation.ecdsa_bits = *bits;
                            } else {
                                editor.generation.rsa_bits = *bits;
                            }
                            editor.error = None;
                        }
                        cx.notify();
                    })),
                )
            })
            .into_any_element()
    }

    fn drawer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if let Some(draft) = self.editor.as_ref().and_then(|editor| editor.fido.as_ref()) {
            let view = draft.view.clone();
            view.update(cx, |view, cx| view.set_dark(self.dark, cx));
            return view.into_any_element();
        }
        self.refresh_automatic_name(window, cx);
        if let Some(nested) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.related.as_ref())
        {
            let view = nested.view.clone();
            view.update(cx, |view, cx| view.set_dark(self.dark, cx));
            return view.into_any_element();
        }
        if let Some(editor) = &mut self.editor
            && let Some(select) = &mut editor.key_select
        {
            select.reconcile(&self.inventory, editor.key_id, window, cx);
        }
        let Some(editor) = &self.editor else {
            return div().into_any_element();
        };
        let p = Palette::new(self.dark);
        let key_kind_owner = cx.weak_entity();
        let editor_token = editor.name.entity_id();
        let consolidated = self.related && editor.id.is_none() && editor.kind != Kind::Identity;
        let import_method = consolidated && editor.method == Method::Import;
        let kind_label = match editor.kind {
            Kind::Key => "Key",
            Kind::Generate => "Key",
            Kind::Certificate => "Key",
            Kind::Identity => "Identity",
        };
        let section = div()
            .rounded(px(10.))
            .bg(p.surface)
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(div().font_weight(FontWeights::MEDIUM).child(kind_label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .when(editor.kind != Kind::Identity, |view| {
                        view.child(
                            div()
                                .size(px(40.))
                                .rounded(px(8.))
                                .bg(rgb(0x075e92))
                                .text_color(rgb(0xffffff))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon(IconName::KeyRound)),
                        )
                    })
                    .child(
                        div().flex_1().min_w_0().child(
                            Field::input(&editor.name, "Name")
                                .disabled(self.saving)
                                .error(editor.name_error.clone()),
                        ),
                    ),
            )
            .when(editor.kind == Kind::Identity, |view| {
                view.child(
                    Field::input(&editor.username, "Username *")
                        .disabled(self.saving)
                        .error(editor.username_error.clone()),
                )
                .child(self.field("Password", &editor.password, true))
                .child(
                    editor
                        .key_select
                        .as_ref()
                        .expect("Identity editor owns its key selector")
                        .view(
                            self.saving,
                            move |kind, window, cx| {
                                let _ = key_kind_owner.update(cx, |view, cx| {
                                    view.select_identity_key_kind(kind, editor_token, window, cx)
                                });
                            },
                            cx,
                        ),
                )
            })
            .when(
                editor.kind != Kind::Identity && !editor.is_generating(),
                |view| {
                    let show_certificate = editor.kind == Kind::Certificate
                        || editor.id.is_some()
                        || !editor.certificate.read(cx).value().trim().is_empty();
                    let view = view.when(import_method && show_certificate, |view| {
                        view.child(self.text_field("Certificate", &editor.certificate, None))
                    });
                    let view = if import_method && editor.private.read(cx).value().trim().is_empty()
                    {
                        view.child(self.import_panel(editor, cx))
                    } else {
                        view.child(self.text_field(
                            if editor.id.is_some() {
                                "Private key"
                            } else {
                                "Private key *"
                            },
                            &editor.private,
                            editor.private_error.clone(),
                        ))
                    };
                    view.when(
                        !import_method && (!consolidated || editor.kind != Kind::Certificate)
                            || import_method
                                && editor.kind == Kind::Certificate
                                && !editor.private.read(cx).value().trim().is_empty(),
                        |view| view.child(self.text_field("Public key", &editor.public, None)),
                    )
                    .when(!import_method && show_certificate, |view| {
                        view.child(self.text_field("Certificate", &editor.certificate, None))
                    })
                },
            )
            .when(editor.is_generating(), |view| {
                view.child(self.generation_controls(editor, cx))
            });
        let title = if editor.kind == Kind::Generate {
            "Generate Key".into()
        } else {
            format!(
                "{} {kind_label}",
                if editor.id.is_some() { "Edit" } else { "New" }
            )
        };
        let mut sections = vec![];
        if consolidated {
            sections.push(self.creation_methods(editor, cx));
        }
        sections.push(section.into_any_element());
        if editor.can_import(self.related) && !consolidated {
            sections.push(
                div()
                    .p(px(16.))
                    .rounded(px(10.))
                    .bg(p.surface)
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(crate::localization::text(cx, "common.action.import"))
                    .child(self.import_panel(editor, cx))
                    .into_any_element(),
            );
        }
        if editor.is_generating() {
            sections.push(
                div()
                    .p(px(16.))
                    .rounded(px(10.))
                    .bg(p.surface)
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(crate::localization::text(
                        cx,
                        "workspace.keys.section.security",
                    ))
                    .child(self.field("Passphrase", &editor.passphrase, true))
                    .into_any_element(),
            );
        }
        crate::components::DrawerFrame::new("credential-editor", title, self.dark)
            .scroll_handle(&editor.scroll)
            .actions(self.editor_actions(editor, cx))
            .when(self.related, |frame| {
                frame.leading(
                    Button::new("back-related-credential")
                        .toolbar()
                        .ghost()
                        .icon(IconName::ArrowLeft)
                        .tooltip("Back")
                        .disabled(self.saving)
                        .on_click(cx.listener(|this, _, window, cx| this.close_editor(window, cx))),
                )
            })
            .when(!self.related, |frame| {
                frame.action(
                    Button::new("close-credential-editor")
                        .toolbar()
                        .ghost()
                        .icon(IconName::ArrowRightToLine)
                        .tooltip("Close editor")
                        .disabled(self.saving)
                        .on_click(cx.listener(|this, _, window, cx| this.close_editor(window, cx))),
                )
            })
            .sections(sections)
            .footer(
                Button::new("save-credential")
                    .accent()
                    .loading(self.saving)
                    .w_full()
                    .large()
                    .disabled(
                        self.saving || self.exporting_file || editor.importing || editor.picking,
                    )
                    .label(match (editor.is_generating(), self.saving) {
                        (true, true) => "Generating…",
                        (true, false) => "Generate & save",
                        (false, true) => "Saving…",
                        (false, false) => "Save",
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
            )
            .into_any_element()
    }
}

impl Render for Keychain {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.related {
            return div()
                .h_full()
                .min_h_0()
                .track_focus(&self.focus)
                .key_context("Nauterm")
                .on_action(cx.listener(|this, _: &CloseEditor, window, cx| {
                    this.back_editor(window, cx);
                    cx.stop_propagation();
                }))
                .on_action(|_: &NewItem, _, cx| cx.stop_propagation())
                .on_action(|_: &DeleteCards, _, cx| cx.stop_propagation())
                .on_action(|_: &SelectCards, _, cx| cx.stop_propagation())
                .child(self.display_drawer(window, cx))
                .into_any_element();
        }
        let open = self.drawer_open();
        let amount = self.drawer_motion.frame(open, window, cx);
        let drawer = (open || amount > 0.).then(|| {
            let content = self.display_drawer(window, cx);
            crate::components::panel::slide(
                "keychain-drawer-slide",
                layout::DRAWER_WIDTH,
                amount,
                open,
                content,
            )
        });
        let viewport = f32::from(window.viewport_size().width);
        let available = self
            .parent_width
            .unwrap_or_else(|| viewport - layout::sidebar_width(viewport))
            - layout::DRAWER_WIDTH * amount;
        let entries = self.visible(cx);
        let show_keys = entries.iter().any(|(item, _, _)| item.key);
        let show_identities = entries.iter().any(|(item, _, _)| !item.key);
        let count = entries
            .iter()
            .filter(|(item, _, _)| item.key)
            .count()
            .max(entries.iter().filter(|(item, _, _)| !item.key).count());
        let grid = layout::CardGrid::new(available - 52., count, self.list);
        let mut keys = crate::components::card_grid(grid);
        let mut identities = crate::components::card_grid(grid);
        for (item, name, detail) in entries {
            if item.key {
                keys = keys.child(self.card(item, name, detail, cx));
            } else {
                identities = identities.child(self.card(item, name, detail, cx));
            }
        }
        let query = self.search.read(cx).value().to_lowercase();
        let search_open = self.search_open || !query.is_empty();
        let collapse_labels = crate::components::management::toolbar_labels_collapsed(
            available,
            &[("New key", true), ("Certificate", true), ("FIDO2", true)],
            (if search_open { 3 } else { 4 }) + usize::from(self.error.is_some()),
            search_open,
            window,
        );
        let owner = cx.entity().downgrade();
        let sort_owner = owner.clone();
        let sort = self.sort;
        div().size_full().min_h_0().relative().track_focus(&self.focus).key_context("Nauterm")
            .on_key_down(cx.listener(Self::navigate_cards))
            .on_action(cx.listener(|this, _: &NewItem, window, cx| { if !crate::components::overlay::is_modal_open(window, cx) { this.create(Kind::Key, window, cx); } cx.stop_propagation(); }))
            .on_action(cx.listener(|this, _: &CloseEditor, window, cx| { this.back_editor(window, cx); cx.stop_propagation(); }))
            .on_action(cx.listener(|this, _: &SelectCards, window, cx| { if !crate::components::overlay::is_modal_open(window, cx) { this.select_all(cx); } cx.stop_propagation(); }))
            .on_action(cx.listener(|this, _: &DeleteCards, window, cx| { if !crate::components::overlay::is_modal_open(window, cx) && !this.selection.is_empty() { this.delete_pending = true; cx.notify(); } cx.stop_propagation(); }))
            .flex()
            .child(div().flex_1().min_w_0().flex().flex_col()
                .child(crate::components::management::toolbar(self.dark)
                    .child(crate::components::ButtonGroup::new()
                    .button(Button::new("new-key").toolbar().label("New key").icon(crate::assets::icon(IconName::Plus)).disabled(self.vault.is_none() || self.busy(cx)).on_click(cx.listener(|this, _, window, cx| this.create(Kind::Key, window, cx))))
                    .dropdown(Button::new("new-key-menu").toolbar().icon(IconName::ChevronDown).tooltip("New key actions").disabled(self.vault.is_none() || self.busy(cx)), move |menu, _, _| {
                        let generate_owner = owner.clone();
                        let identity_owner = owner.clone();
                        menu.width(180.).item(MenuItem::localized("Generate key").icon(IconName::WandSparkles).on_click(move |_, window, cx| { let _ = generate_owner.update(cx, |this, cx| this.create(Kind::Generate, window, cx)); }))
                            .item(MenuItem::localized("New identity").icon(IconName::Contact).on_click(move |_, window, cx| { let _ = identity_owner.update(cx, |this, cx| this.create(Kind::Identity, window, cx)); }))
                    }))
                    .child(Button::new("new-certificate").toolbar().ghost().label("Certificate").icon(IconName::BadgeCheck).label_collapsed(collapse_labels).disabled(self.vault.is_none() || self.busy(cx)).on_click(cx.listener(|this, _, window, cx| this.create(Kind::Certificate, window, cx))))
                    .child(Button::new("new-fido2").toolbar().ghost().label("FIDO2").icon(IconName::Usb).label_collapsed(collapse_labels).disabled(self.vault.is_none() || self.busy(cx)).on_click(cx.listener(|this, _, window, cx| this.create_fido2(window, cx))))
                    .child(div().flex_1())
                    .when(self.error.is_some(), |view| view.child(Button::new("reload-keychain").toolbar().ghost().icon(IconName::RefreshCw).tooltip("Reload keychain").disabled(self.loading || self.busy(cx)).on_click(cx.listener(|this, _, _, cx| this.retry(cx)))))
                    .child(Button::new("keychain-sort").toolbar().ghost().icon(sort.icon()).tooltip(sort.label()).dropdown_menu(move |menu, _, _| {
                        let owner = sort_owner.clone();
                        crate::management::sort_menu(menu, sort, move |order, _, cx| {
                            let _ = owner.update(cx, |this, cx| { this.sort = order; cx.notify(); });
                        })
                    }))
                    .child({ let owner = cx.weak_entity(); crate::components::management::toolbar_search("keychain-search", &self.search, self.search_open, move |open, window, cx| { let _ = owner.update(cx, |this, cx| { this.search_open = open; if open { this.search.update(cx, |input, cx| input.focus(window, cx)); } else { this.focus.focus(window, cx); } cx.notify(); }); }, cx) })
                    .child(Button::new("keychain-view").toolbar().ghost().icon(if self.list { IconName::LayoutGrid } else { IconName::List }).tooltip("Switch list/grid view").on_click(cx.listener(|this, _, _, cx| { this.list = !this.list; cx.notify(); }))))
                .child(div().id("keychain-scroll").flex_1().min_h_0().overflow_y_scroll().scrollbars().map(crate::components::management::collection_padding).flex().flex_col().gap(px(crate::components::management::COLLECTION_SECTION_GAP))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_editor(window, cx)))
                    .when(self.loading, |view| view.child(Spinner::new()))
                    .when(show_keys, |view| view.child(crate::components::management::collection_section("Keys", cx).child(keys)))
                    .when(show_identities, |view| view.child(crate::components::management::collection_section("Identities", cx).child(identities)))
                    .when(!self.loading && !show_keys && !show_identities && self.error.is_none(), |view| view.child({
                        let empty = crate::components::EmptyState::new(IconName::KeyRound, if query.is_empty() { "No keychain items yet" } else { "No keychain items found" }, self.dark).description("Add keys and identities once, then reuse them across your saved hosts.");
                        if query.is_empty() { empty.action(Button::new("empty-add-key").primary().icon(crate::assets::icon(IconName::Plus)).label("Add key").on_click(cx.listener(|this, _, window, cx| this.create(Kind::Key, window, cx)))) } else { empty }
                    }))))
            .children(drawer)
            .when_some(self.delete_overlay(cx), |view, overlay| view.child(overlay))
            .into_any_element()
    }
}

impl Keychain {
    fn delete_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.delete_pending {
            return None;
        }
        let title = format!("Delete {} selected item(s)?", self.selection.len());
        Some(crate::components::Dialog::new("credential-delete-modal", title.clone())
            .return_focus(&self.focus).dismissible(!self.saving)
            .on_dismiss(cx.listener(|this, _, _, cx| { this.delete_pending = false; cx.notify(); }))
            .content(crate::components::DialogFrame::new("credential-delete-dialog", title).width(380.)
                .colors(crate::components::DialogColors::workspace(self.dark))
                .content("Saved credential references will be cleared. Active sessions are not disconnected.")
                    .action(Button::new("cancel-credential-delete").ghost().label("Cancel").disabled(self.saving)
                        .on_click(|_, window, cx| crate::components::Modal::dismiss(window, cx)))
                    .action(Button::new("confirm-credential-delete").danger().label("Delete").disabled(self.saving)
                        .on_click(cx.listener(|this, _, _, cx| this.delete(cx)))))
            .into_any_element())
    }
}
