use super::{Kind, Record, Summary};
use crate::{
    components::{
        Button, DialogColors, DialogFrame, Field, Modal, ScrollbarsExt, SelectEvent, SelectModel,
        SelectOption,
        menu::{ContextMenuExt, DropdownMenu, MenuItem},
    },
    management::{CollectionView, SortOrder, sort_menu},
    model::Proxy as LegacyProxy,
    native::Core,
    palette::Palette,
    typography::FontWeights,
    ui::{CloseEditor, DeleteCards, NewItem, SelectCards},
    vault::{Inventory, Vault},
};
use gpui_kit::{
    assets::IconName,
    base::input::{InputEvent, InputState},
    component::{Disableable, Icon, Sizable},
    prelude::*,
    *,
};
use std::{collections::BTreeSet, path::PathBuf};
mod related;
pub use related::RelatedEvent;

pub struct MigrationReady;
pub struct Changed;
pub struct ReferencesDeleted(pub super::Deletions);
struct RelatedIdentity {
    view: Entity<crate::keychain::Keychain>,
    _events: Subscription,
}
struct Editor {
    scroll: ScrollHandle,
    id: Option<i64>,
    uuid: Option<String>,
    name: Entity<InputState>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    kind: Kind,
    identity: Option<i64>,
    kinds: SelectModel<Kind>,
    identities: SelectModel<i64>,
    password_identities: SelectModel<i64>,
    error: Option<String>,
    validation_error: Option<String>,
    related: Option<RelatedIdentity>,
    _events: Vec<Subscription>,
}
pub struct Proxies {
    database_path: PathBuf,
    vault: Option<Vault>,
    legacy: Vec<LegacyProxy>,
    ready: bool,
    loading: bool,
    refresh_pending: bool,
    saving: bool,
    revision: u64,
    search: Entity<InputState>,
    dark: bool,
    active: bool,
    focus: FocusHandle,
    entries: Vec<Summary>,
    inventory: Inventory,
    inventory_generation: u64,
    keychain: WeakEntity<crate::keychain::Keychain>,
    owner: Option<WeakEntity<Proxies>>,
    preferences: CollectionView,
    selection: BTreeSet<i64>,
    navigation: crate::management::navigation::Navigation<i64>,
    deletion: Option<Vec<i64>>,
    editor: Option<Editor>,
    drawer_motion: crate::components::panel::Presence<Box<Editor>>,
    parent_width: Option<f32>,
    error: Option<String>,
    reference_error: Option<String>,
    reference_pending: bool,
    _search: Subscription,
    _menu: Option<Subscription>,
}
impl EventEmitter<MigrationReady> for Proxies {}
impl Proxies {
    pub(crate) fn feedback(
        &self,
    ) -> Vec<(
        (&'static str, Option<EntityId>),
        crate::components::message::Message,
    )> {
        let mut feedback = vec![];
        for (source, error) in [
            ("catalog", &self.error),
            ("references", &self.reference_error),
        ] {
            if let Some(error) = error {
                feedback.push((
                    (source, None),
                    crate::components::message::Message::error(error.clone()),
                ));
            }
        }
        if let Some(editor) = &self.editor
            && let Some(error) = &editor.error
        {
            feedback.push((
                ("editor", Some(editor.name.entity_id())),
                crate::components::message::Message::error(error.clone()),
            ));
        }
        feedback
    }
    pub(crate) fn drawer_open(&self) -> bool {
        self.editor.is_some()
    }
    pub(crate) fn finish_hidden_drawer(&mut self) {
        if self.editor.is_none() {
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
            self.drawer_motion.retire(Box::new(editor));
        }
    }
    fn display_drawer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.editor.is_some() {
            self.drawer_motion.take_retired();
            return self.drawer(window, cx);
        }
        let Some(editor) = self.drawer_motion.take_retired() else {
            return div().into_any_element();
        };
        self.editor = Some(*editor);
        let content = self.drawer(window, cx);
        self.retire_editor();
        content
    }
}
impl EventEmitter<Changed> for Proxies {}
impl EventEmitter<ReferencesDeleted> for Proxies {}
fn kinds() -> Vec<SelectOption<Kind>> {
    Kind::ALL
        .into_iter()
        .map(|kind| SelectOption::localized(kind, kind.label()))
        .collect()
}
fn identities(inventory: &Inventory) -> Vec<SelectOption<i64>> {
    inventory
        .identities
        .iter()
        .map(|identity| SelectOption::new(identity.id, identity.name.clone()))
        .collect()
}
impl Proxies {
    pub fn has_resource_edit(&self) -> bool {
        self.editor.is_some()
            || self.deletion.is_some()
            || self.loading
            || self.saving
            || self.reference_pending
    }
    pub fn new(
        database_path: PathBuf,
        vault: Option<Vault>,
        legacy: Vec<LegacyProxy>,
        search: Entity<InputState>,
        dark: bool,
        keychain: WeakEntity<crate::keychain::Keychain>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::empty(database_path, vault, legacy, search, dark, keychain, cx);
        view.refresh(cx);
        view
    }
    fn empty(
        database_path: PathBuf,
        vault: Option<Vault>,
        legacy: Vec<LegacyProxy>,
        search: Entity<InputState>,
        dark: bool,
        keychain: WeakEntity<crate::keychain::Keychain>,
        cx: &mut Context<Self>,
    ) -> Self {
        let events = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        Self {
            database_path,
            vault,
            legacy,
            ready: false,
            loading: false,
            refresh_pending: false,
            saving: false,
            revision: 0,
            search,
            dark,
            active: false,
            focus: cx.focus_handle(),
            entries: vec![],
            inventory: Inventory::default(),
            inventory_generation: 0,
            keychain,
            owner: None,
            preferences: CollectionView::default(),
            selection: BTreeSet::new(),
            navigation: Default::default(),
            deletion: None,
            editor: None,
            drawer_motion: Default::default(),
            parent_width: None,
            error: None,
            reference_error: None,
            reference_pending: false,
            _search: events,
            _menu: None,
        }
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.dark != dark {
            self.dark = dark;
            cx.notify();
        }
    }
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    pub fn set_inventory(&mut self, inventory: Inventory, cx: &mut Context<Self>) {
        if self.inventory != inventory {
            self.inventory_generation += 1;
            self.inventory = inventory;
            cx.notify();
        }
    }
    pub fn summaries(&self) -> Vec<Summary> {
        self.entries.clone()
    }
    pub fn available(&self) -> bool {
        self.ready && !self.loading && !self.reference_pending && self.reference_error.is_none()
    }
    /// A catalog can arrive before its cross-store cleanup has been published.
    /// Connections must wait for both; a failure stays explicit and retryable.
    pub(crate) fn connection_readiness(&self) -> Result<bool, String> {
        if let Some(error) = &self.reference_error {
            return Err(error.clone());
        }
        if self.loading || self.reference_pending {
            return Ok(false);
        }
        if self.ready {
            return Ok(true);
        }
        self.error.clone().map_or(Ok(false), Err)
    }
    fn set_catalog(&mut self, catalog: super::Catalog, cx: &mut Context<Self>) {
        self.entries = catalog.entries;
        if !catalog.deletions.is_empty() {
            self.reference_pending = true;
            cx.emit(ReferencesDeleted(catalog.deletions));
        }
    }
    pub(crate) fn references_result(
        &mut self,
        deleted: super::Deletions,
        result: anyhow::Result<()>,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = result {
            self.reference_error = Some(format!(
                "Proxy deletion completed, but workspace references could not be saved: {error}. Refresh to retry."
            ));
            cx.emit(Changed);
            cx.notify();
            return;
        }
        self.reference_error = None;
        self.reference_pending = false;
        if let Some(vault) = self.vault.clone() {
            cx.spawn(async move |view, cx| {
                if let Err(error) = vault.acknowledge_proxy_deletions(deleted).await {
                    let _ = view.update(cx, |view, cx| { view.error = Some(format!("Workspace references were updated, but deletion cleanup could not be acknowledged: {error}. Refresh to retry.")); cx.notify(); });
                }
            }).detach();
        }
        cx.emit(Changed);
        cx.notify();
    }
    pub fn migration_result(&mut self, result: anyhow::Result<()>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.legacy.clear();
                self.ready = true;
                self.error = None;
            }
            Err(error) => {
                self.ready = false;
                self.error = Some(format!(
                    "Proxies were imported, but migration metadata could not be saved: {error}. Original metadata is preserved; refresh to retry."
                ));
            }
        }
        cx.emit(Changed);
        cx.notify();
    }
    pub fn back_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if let Some(related) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.related.as_ref())
        {
            related
                .view
                .update(cx, |view, cx| view.back_editor(window, cx));
            return;
        }
        self.close_editor(window, cx);
    }
    pub fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.revision += 1;
        self.retire_editor();
        self.selection.clear();
        if self.active {
            self.focus.focus(window, cx);
        }
        if self.owner.is_some() {
            cx.emit(RelatedEvent::Cancelled);
        }
        cx.notify();
    }
    pub(crate) fn request_refresh(&mut self, cx: &mut Context<Self>) {
        self.refresh_pending = true;
        self.resume_refresh(cx);
    }
    pub(crate) fn resume_refresh(&mut self, cx: &mut Context<Self>) {
        if self.refresh_pending && !self.loading && !self.saving && self.deletion.is_none() {
            self.refresh_pending = false;
            self.refresh(cx);
        }
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.saving || self.deletion.is_some() {
            return;
        }
        if self.vault.is_none() {
            match Core::discover().and_then(|core| Vault::open(core, self.database_path.clone())) {
                Ok(vault) => self.vault = Some(vault),
                Err(error) => {
                    self.error = Some(error.to_string());
                    cx.notify();
                    return;
                }
            }
        }
        let vault = self.vault.clone().unwrap();
        let legacy = self.legacy.clone();
        let inventory_generation = self.inventory_generation;
        self.loading = true;
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = async {
                let entries = vault.proxies(legacy).await?;
                let inventory = vault.inventory().await?;
                anyhow::Ok((entries, inventory))
            }
            .await;
            let _ = view.update(cx, |view, cx| {
                view.loading = false;
                match result {
                    Ok((entries, inventory)) => {
                        view.set_catalog(entries, cx);
                        if view.inventory_generation == inventory_generation {
                            view.inventory = inventory;
                        }
                        view.selection
                            .retain(|id| view.entries.iter().any(|entry| entry.id == *id));
                        if view.legacy.is_empty() {
                            view.ready = true;
                        } else {
                            cx.emit(MigrationReady);
                        }
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.emit(Changed);
                view.resume_refresh(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn busy(&self) -> bool {
        self.loading || self.saving || !self.ready
    }
    pub(crate) fn collection_shortcut(
        &mut self,
        command: crate::shortcuts::Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.focus.is_focused(window) {
            return false;
        }
        if self.busy() {
            return true;
        }
        let sections = [crate::management::navigation::Section::new(
            self.visible(cx).iter().map(|entry| entry.id).collect(),
            1,
        )];
        let Some(id) = self.navigation.active(&self.selection, &sections) else {
            return false;
        };
        match command {
            crate::shortcuts::Command::Edit if self.selection.len() == 1 => {
                self.edit(id, window, cx)
            }
            crate::shortcuts::Command::Duplicate => self.mutate(false, cx),
            crate::shortcuts::Command::Edit => {}
            _ => return false,
        }
        true
    }
    fn navigate_cards(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::management::navigation::{Key, Section};
        if !self.focus.is_focused(window)
            || self.busy()
            || crate::components::overlay::is_modal_open(window, cx)
        {
            return;
        }
        let shortcut = crate::shortcuts::current(cx)
            .matching(crate::shortcuts::Scope::Collection, &event.keystroke);
        let command = Key::read(&event.keystroke);
        if shortcut.is_none() && command.is_none() {
            return;
        }
        let visible: Vec<_> = self.visible(cx).iter().map(|entry| entry.id).collect();
        let width = f32::from(window.viewport_size().width);
        let available = self
            .parent_width
            .unwrap_or_else(|| width - crate::layout::sidebar_width(width))
            - 52.
            - crate::layout::DRAWER_WIDTH * self.drawer_motion.amount();
        let columns = crate::layout::CardGrid::new(available, visible.len(), self.preferences.list)
            .columns as usize;
        let sections = [Section::new(visible, columns)];
        let Some(current) = self.navigation.active(&self.selection, &sections) else {
            return;
        };
        if let Some(shortcut) = shortcut {
            if self.collection_shortcut(shortcut, window, cx) {
                cx.stop_propagation();
            }
            return;
        }
        let Some(command) = command else {
            return;
        };
        match command {
            Key::Activate => self.edit(current, window, cx),
            Key::Move(direction) => {
                let Some(id) = self.navigation.navigate(
                    &mut self.selection,
                    &sections,
                    direction,
                    event.keystroke.modifiers,
                ) else {
                    return;
                };
                if self.selection.len() == 1
                    && self.selection.contains(&id)
                    && self
                        .editor
                        .as_ref()
                        .is_some_and(|editor| editor.id != Some(id))
                {
                    self.edit_with_focus(id, true, window, cx);
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
    fn visible(&self, cx: &App) -> Vec<Summary> {
        let query = self.search.read(cx).value();
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| {
                let identity = entry
                    .identity_id
                    .and_then(|id| {
                        self.inventory
                            .identities
                            .iter()
                            .find(|identity| identity.id == id)
                    })
                    .map(|identity| identity.name.as_str())
                    .unwrap_or("");
                entry.matches(&query, identity)
            })
            .cloned()
            .collect();
        entries.sort_by(|a, b| {
            let name = self
                .preferences
                .sort
                .compare((&a.name, a.id as u64), (&b.name, b.id as u64));
            match (self.preferences.sort, a.created_at.zip(b.created_at)) {
                (SortOrder::NewestFirst, Some((a, b))) if a != b => b.cmp(&a),
                (SortOrder::OldestFirst, Some((a, b))) if a != b => a.cmp(&b),
                _ => name,
            }
        });
        entries
    }
    pub fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.revision += 1;
        self.selection.clear();
        self.install_editor(
            Record {
                port: 1080,
                id: None,
                uuid: None,
                name: String::new(),
                host: String::new(),
                kind: Kind::Socks5,
                identity_id: None,
                username: None,
                password: None,
                created_at: None,
            },
            window,
            cx,
        );
        self.focus_editor(window, cx);
    }
    fn focus_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &self.editor {
            editor.name.update(cx, |field, cx| field.focus(window, cx));
        }
    }
    fn edit(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_with_focus(id, false, window, cx);
    }
    fn edit_with_focus(
        &mut self,
        id: i64,
        keep_focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        let Some(vault) = self.vault.clone() else {
            return;
        };
        self.revision += 1;
        let revision = self.revision;
        self.selection = BTreeSet::from([id]);
        cx.spawn_in(window, async move |view, cx| {
            let result = vault.proxy(id).await;
            let _ = view.update_in(cx, |view, window, cx| {
                if view.revision != revision || view.selection != BTreeSet::from([id]) {
                    return;
                }
                match result {
                    Ok(record) => {
                        view.install_editor(record, window, cx);
                        if !keep_focus {
                            view.focus_editor(window, cx);
                        }
                    }
                    Err(error) => {
                        view.error = Some(error.to_string());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }
    fn install_editor(&mut self, record: Record, window: &mut Window, cx: &mut Context<Self>) {
        let field = |text: String, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).default_value(text))
        };
        let name = field(record.name.clone(), window, cx);
        let host = field(record.host.clone(), window, cx);
        let port = field(record.port.to_string(), window, cx);
        let username = field(record.username.clone().unwrap_or_default(), window, cx);
        let password = cx.new(|cx| {
            InputState::new(window, cx).masked(true).default_value(
                if record.identity_id.is_some() {
                    String::new()
                } else {
                    record.password.clone().unwrap_or_default()
                },
            )
        });
        let kinds = SelectModel::new(kinds(), Some(record.kind), window, cx);
        let identities = SelectModel::editable(
            identities(&self.inventory),
            record.identity_id,
            &username,
            window,
            cx,
        );
        let password_identities = SelectModel::editable(
            self::identities(&self.inventory),
            None,
            &password,
            window,
            cx,
        );
        let kind_event = cx.subscribe_in(
            &kinds.state,
            window,
            |view, state, event: &SelectEvent<Kind>, _, cx| {
                if let Some(editor) = &mut view.editor
                    && editor.kinds.state == *state
                {
                    let SelectEvent::Confirm(kind) = event;
                    if let Some(kind) = kind.flatten() {
                        editor.kind = kind;
                    }
                    cx.notify();
                }
            },
        );
        let identity_event = cx.subscribe_in(
            &identities.state,
            window,
            |view, state, event: &SelectEvent<i64>, window, cx| {
                if view
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.identities.state == *state)
                {
                    let SelectEvent::Confirm(identity) = event;
                    view.select_identity(identity.flatten(), false, window, cx);
                }
            },
        );
        let password_event = cx.subscribe_in(
            &password_identities.state,
            window,
            |view, state, event: &SelectEvent<i64>, window, cx| {
                if view
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.password_identities.state == *state)
                {
                    let SelectEvent::Confirm(identity) = event;
                    view.select_identity(identity.flatten(), true, window, cx);
                }
            },
        );
        let identity_edit = cx.subscribe(
            &identities.state,
            |view, state, _: &crate::components::SelectEdited, cx| {
                if let Some(editor) = &mut view.editor
                    && editor.identities.state.entity_id() == state.entity_id()
                {
                    editor.identity = None;
                    cx.notify();
                }
            },
        );
        let password_edit = cx.subscribe(
            &password_identities.state,
            |view, state, _: &crate::components::SelectEdited, cx| {
                if let Some(editor) = &mut view.editor
                    && editor.password_identities.state.entity_id() == state.entity_id()
                {
                    editor.identity = None;
                    cx.notify();
                }
            },
        );
        let identity_create = cx.subscribe_in(
            &identities.state,
            window,
            |view, state, event: &crate::components::SelectCreate, window, cx| {
                if view
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.identities.state == *state)
                    && event.action == "identity"
                {
                    view.create_related_identity(&event.text, window, cx);
                }
            },
        );
        self.editor = Some(Editor {
            scroll: ScrollHandle::new(),
            id: record.id,
            uuid: record.uuid.clone(),
            name,
            host,
            port,
            username,
            password,
            kind: record.kind,
            identity: record.identity_id,
            kinds,
            identities,
            password_identities,
            error: None,
            validation_error: None,
            related: None,
            _events: vec![
                kind_event,
                identity_event,
                password_event,
                identity_edit,
                password_edit,
                identity_create,
            ],
        });
        cx.notify();
    }
    fn create_related_identity(
        &mut self,
        initial_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        let Some(editor) = self
            .editor
            .as_ref()
            .filter(|editor| editor.related.is_none())
        else {
            return;
        };
        let username = if editor.identity.is_some() {
            String::new()
        } else {
            editor.username.read(cx).value().to_string()
        };
        let Some(source) = self
            .keychain
            .update(cx, |view, cx| view.related_source(cx))
            .ok()
            .flatten()
        else {
            self.editor.as_mut().unwrap().error =
                Some("Keychain is unavailable. Open Keychain and retry.".into());
            cx.notify();
            return;
        };
        let view = cx.new(|cx| {
            crate::keychain::Keychain::related(
                source,
                crate::keychain::Kind::Identity,
                initial_name,
                &username,
                window,
                cx,
            )
        });
        let events = cx.subscribe_in(
            &view,
            window,
            |this, view, event: &crate::keychain::RelatedEvent, window, cx| {
                let Some(editor) = this.editor.as_mut().filter(|editor| {
                    editor
                        .related
                        .as_ref()
                        .is_some_and(|related| related.view == *view)
                }) else {
                    return;
                };
                if let crate::keychain::RelatedEvent::Saved(
                    crate::model::CredentialRef::Identity(id),
                ) = event
                {
                    editor.identity = Some(*id);
                    editor
                        .username
                        .update(cx, |field, cx| field.set_value("", window, cx));
                    editor
                        .password
                        .update(cx, |field, cx| field.set_value("", window, cx));
                }
                editor.related = None;
                editor
                    .identities
                    .state
                    .read(cx)
                    .focus_handle(cx)
                    .focus(window, cx);
                // The shared Keychain publishes refreshed summaries independently,
                // including when this parent drawer was closed during the save.
                cx.notify();
            },
        );
        self.editor.as_mut().unwrap().related = Some(RelatedIdentity {
            view,
            _events: events,
        });
        cx.notify();
    }
    fn select_identity(
        &mut self,
        identity: Option<i64>,
        from_password: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = identity
            .and_then(|id| {
                self.inventory
                    .identities
                    .iter()
                    .find(|identity| identity.id == id)
            })
            .map(|identity| identity.name.clone());
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.identity = identity;
        if identity.is_some() {
            editor.username.update(cx, |field, cx| {
                field.set_value(
                    name.unwrap_or_else(|| "Unavailable item".into()),
                    window,
                    cx,
                )
            });
            editor
                .password
                .update(cx, |field, cx| field.set_value("", window, cx));
            if from_password {
                editor
                    .identities
                    .state
                    .read(cx)
                    .focus_handle(cx)
                    .focus(window, cx);
            }
        } else if !from_password {
            editor
                .username
                .update(cx, |field, cx| field.set_value("", window, cx));
        }
        cx.notify();
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let (Some(editor), Some(vault)) = (&self.editor, self.vault.clone()) else {
            return;
        };
        let optional = |value: String| (!value.is_empty()).then_some(value);
        let record = Record {
            id: editor.id,
            uuid: editor.uuid.clone(),
            name: editor.name.read(cx).value().trim().to_owned(),
            host: editor.host.read(cx).value().trim().to_owned(),
            port: editor.port.read(cx).value().parse().unwrap_or(0),
            kind: editor.kind,
            identity_id: editor.identity,
            username: editor
                .identity
                .is_none()
                .then(|| optional(editor.username.read(cx).value().to_string()))
                .flatten(),
            password: editor
                .identity
                .is_none()
                .then(|| optional(editor.password.read(cx).value().to_string()))
                .flatten(),
            created_at: None,
        };
        if let Err(error) = record.validate() {
            self.editor.as_mut().unwrap().validation_error = Some(error.to_string());
            cx.notify();
            return;
        }
        self.saving = true;
        self.editor.as_mut().unwrap().validation_error = None;
        let revision = self.revision;
        let owner = self.owner.clone();
        cx.spawn_in(window, async move |view, cx| {
            let result = vault.save_proxy(record).await;
            let inventory = vault.proxies(vec![]).await;
            if result.is_ok()
                && let Some(owner) = owner
            {
                let _ = owner.update(cx, |view, cx| view.request_refresh(cx));
            }
            let _ = view.update_in(cx, |view, window, cx| {
                view.saving = false;
                match result {
                    Ok(id) => {
                        if view.revision == revision {
                            if view.owner.is_some() {
                                view.retire_editor();
                                view.revision += 1;
                                cx.emit(RelatedEvent::Saved(id));
                            } else {
                                view.close_editor(window, cx);
                                view.selection = BTreeSet::from([id]);
                            }
                        }
                        match inventory {
                            Ok(entries) => {
                                view.set_catalog(entries, cx);
                                view.error = None;
                            }
                            Err(error) => {
                                view.error = Some(format!(
                                    "Proxy saved, but the list could not be refreshed: {error}"
                                ))
                            }
                        }
                    }
                    Err(error) => {
                        if let Some(editor) = &mut view.editor {
                            editor.error = Some(error.to_string());
                        } else {
                            view.error = Some(error.to_string());
                        }
                    }
                }
                cx.emit(Changed);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn request_delete(&mut self, cx: &mut Context<Self>) {
        if !self.busy() && !self.selection.is_empty() {
            self.deletion = Some(self.selection.iter().copied().collect());
            self.error = None;
            cx.notify();
        }
    }
    fn mutate(&mut self, delete: bool, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(vault) = self.vault.clone() else {
            return;
        };
        let ids = if delete {
            self.deletion.clone().unwrap_or_default()
        } else {
            self.selection.iter().copied().collect()
        };
        if ids.is_empty() {
            return;
        }
        self.saving = true;
        cx.spawn(async move |view, cx| {
            let result = if delete {
                vault.delete_proxies(ids).await.map(|_| vec![])
            } else {
                vault.duplicate_proxies(ids).await
            };
            let inventory = vault.proxies(vec![]).await;
            let _ = view.update(cx, |view, cx| {
                view.saving = false;
                if let Ok(entries) = &inventory {
                    view.set_catalog(entries.clone(), cx);
                }
                view.selection
                    .retain(|id| view.entries.iter().any(|entry| entry.id == *id));
                match result {
                    Ok(created) => {
                        view.revision += 1;
                        view.retire_editor();
                        view.deletion = None;
                        view.selection = created.into_iter().collect();
                        view.error = inventory.err().map(|error| {
                            format!("Operation completed, but refresh failed: {error}")
                        });
                    }
                    Err(error) => {
                        view.error = Some(error.to_string());
                        if delete {
                            view.deletion = view
                                .deletion
                                .take()
                                .map(|ids| {
                                    ids.into_iter()
                                        .filter(|id| {
                                            view.entries.iter().any(|entry| entry.id == *id)
                                        })
                                        .collect()
                                })
                                .filter(|ids: &Vec<i64>| !ids.is_empty());
                        }
                    }
                }
                cx.emit(Changed);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn card(&self, entry: Summary, cx: &mut Context<Self>) -> AnyElement {
        let id = entry.id;
        let selected = self.selection.contains(&id)
            || self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.id == Some(id));
        let owner = cx.weak_entity();
        let identity = entry
            .identity_id
            .and_then(|id| {
                self.inventory
                    .identities
                    .iter()
                    .find(|identity| identity.id == id)
            })
            .map(|identity| identity.name.as_str())
            .unwrap_or(&entry.username);
        let detail = format!(
            "{} · {}:{}{}",
            entry.kind.label(),
            entry.host,
            entry.port,
            if identity.is_empty() {
                String::new()
            } else {
                format!(", {identity}")
            }
        );
        crate::components::item_card(
            ("proxy-card", id as u64),
            self.dark,
            selected,
            self.preferences.list,
        )
        .list_menu_hint(self.preferences.list)
        .accessibility_label(entry.name.clone())
        .reveal_on_layout(self.navigation.take_reveal(id))
        .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
            cx.stop_propagation();
            if view.busy() {
                return;
            }
            view.focus.focus(window, cx);
            if event.click_count() == 2 {
                view.edit(id, window, cx);
                return;
            }
            let modifiers = event.modifiers();
            let multiple = modifiers.platform || modifiers.control || modifiers.shift;
            if !multiple && view.selection.len() > 1 && view.selection.contains(&id) {
                return;
            }
            let visible: Vec<_> = view.visible(cx).iter().map(|entry| entry.id).collect();
            view.navigation
                .select(&mut view.selection, &visible, id, modifiers);
            if !multiple && view.editor.is_some() {
                view.edit(id, window, cx);
            }
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |view, _, window, cx| {
                if view.busy() {
                    return;
                }
                if !view.selection.contains(&id) {
                    view.selection = BTreeSet::from([id]);
                }
                view.focus.focus(window, cx);
                cx.notify();
            }),
        )
        .child(
            div()
                .size(px(40.))
                .flex_shrink_0()
                .rounded(px(10.))
                .bg(rgb(0x096b9e))
                .text_color(rgb(0xffffff))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(IconName::Network).with_size(px(20.))),
        )
        .child(crate::components::card_text(entry.name, detail, self.dark))
        .when(
            !self.selection.contains(&id) || self.selection.len() <= 1,
            |card| {
                card.hover_action(
                    crate::components::card_edit_button(("edit-proxy", id as u64), self.dark)
                        .disabled(self.busy())
                        .on_click(cx.listener(move |view, _, window, cx| {
                            cx.stop_propagation();
                            view.edit(id, window, cx);
                        })),
                )
            },
        )
        .context_menu(move |menu, _, cx| {
            let shortcuts = crate::shortcuts::current(cx);
            let menu_entity = cx.entity();
            let (busy, single) = owner
                .update(cx, |view, cx| {
                    view._menu =
                        Some(cx.subscribe(&menu_entity, |view, _, _: &DismissEvent, cx| {
                            if view.deletion.is_none() {
                                view.selection = view
                                    .editor
                                    .as_ref()
                                    .and_then(|editor| editor.id)
                                    .into_iter()
                                    .collect();
                                cx.notify();
                            }
                        }));
                    (view.busy(), view.selection.len() == 1)
                })
                .unwrap_or((true, false));
            let edit = owner.clone();
            let duplicate = owner.clone();
            let delete = owner.clone();
            menu.card_menu()
                .when(single, |menu| {
                    menu.item(
                        shortcuts
                            .menu(
                                MenuItem::localized("Edit Proxy"),
                                crate::shortcuts::Command::Edit,
                            )
                            .icon(IconName::Pencil)
                            .disabled(busy || !single)
                            .on_click(move |_, window, cx| {
                                let _ = edit.update(cx, |view, cx| view.edit(id, window, cx));
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
                            let _ = duplicate.update(cx, |view, cx| view.mutate(false, cx));
                        }),
                )
                .separator()
                .item(
                    MenuItem::localized("Delete Proxy")
                        .icon(IconName::Trash)
                        .destructive(true)
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let _ = delete.update(cx, |view, cx| view.request_delete(cx));
                        }),
                )
        })
        .into_any_element()
    }
    fn drawer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if let Some(related) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.related.as_ref())
        {
            let view = related.view.clone();
            view.update(cx, |view, cx| view.set_dark(self.dark, cx));
            return view.into_any_element();
        }
        let p = Palette::new(self.dark);
        let Some(editor) = &mut self.editor else {
            return div().into_any_element();
        };
        editor
            .kinds
            .reconcile(kinds(), Some(editor.kind), window, cx);
        editor
            .identities
            .reconcile(identities(&self.inventory), editor.identity, window, cx);
        editor
            .password_identities
            .reconcile(identities(&self.inventory), None, window, cx);
        let mut authentication = vec![editor.identities.field_with_actions(
            if editor.identity.is_some() {
                "Identity"
            } else {
                "Username"
            },
            self.saving,
            vec![crate::components::SelectAction::new(
                "identity",
                "Create identity",
                IconName::Contact,
            )],
        )];
        if editor.identity.is_none() {
            authentication.push(editor.password_identities.field("Password", self.saving));
        }
        let section = |title: &str, fields: Vec<AnyElement>| {
            div()
                .rounded(px(10.))
                .bg(p.surface)
                .p(px(16.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .font_weight(FontWeights::MEDIUM)
                        .child(title.to_owned()),
                )
                .children(fields)
        };
        let title = if editor.id.is_some() {
            "Edit Proxy"
        } else {
            "New Proxy"
        };
        let sections = vec![
            section(
                "Proxy",
                vec![
                    Field::input(&editor.name, "Proxy name *")
                        .disabled(self.saving)
                        .into_any_element(),
                    editor.kinds.field("Type", self.saving),
                ],
            )
            .into_any_element(),
            section(
                "Endpoint",
                vec![
                    Field::input(&editor.host, "Host *")
                        .disabled(self.saving)
                        .into_any_element(),
                    Field::input(&editor.port, "Port *")
                        .digits_only()
                        .disabled(self.saving)
                        .into_any_element(),
                    crate::components::validation_message(editor.validation_error.as_deref(), cx)
                        .unwrap_or_else(|| div().into_any_element()),
                ],
            )
            .into_any_element(),
            section("Authentication", authentication).into_any_element(),
        ];
        crate::components::DrawerFrame::new("proxy-editor", title, self.dark)
            .scroll_handle(&editor.scroll)
            .when(self.owner.is_some(), |frame| {
                frame.leading(
                    Button::new("back-related-proxy")
                        .toolbar()
                        .ghost()
                        .icon(IconName::ArrowLeft)
                        .tooltip("Back")
                        .disabled(self.saving)
                        .on_click(cx.listener(|view, _, window, cx| view.back_editor(window, cx))),
                )
            })
            .when(self.owner.is_none(), |frame| {
                frame.action(
                    Button::new("close-proxy-editor")
                        .toolbar()
                        .ghost()
                        .icon(IconName::ArrowRightToLine)
                        .tooltip("Close editor")
                        .disabled(self.saving)
                        .on_click(cx.listener(|view, _, window, cx| view.close_editor(window, cx))),
                )
            })
            .sections(sections)
            .footer(
                Button::new("save-proxy")
                    .large()
                    .accent()
                    .label("Save")
                    .w_full()
                    .disabled(self.saving || !self.ready)
                    .on_click(cx.listener(|view, _, window, cx| view.save(window, cx))),
            )
            .into_any_element()
    }
}
impl Render for Proxies {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.resume_refresh(cx);
        if self.owner.is_some() {
            return div()
                .h_full()
                .min_h_0()
                .track_focus(&self.focus)
                .key_context("Nauterm")
                .on_action(cx.listener(|view, _: &CloseEditor, window, cx| {
                    view.back_editor(window, cx);
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
        let entries = self.visible(cx);
        let width = f32::from(window.viewport_size().width);
        let available = self
            .parent_width
            .unwrap_or_else(|| width - crate::layout::sidebar_width(width))
            - 52.
            - crate::layout::DRAWER_WIDTH * self.drawer_motion.amount();
        let grid = crate::layout::CardGrid::new(available, entries.len(), self.preferences.list);
        let search_open = self.preferences.search_open || !self.search.read(cx).value().is_empty();
        let collapse_labels = crate::components::management::toolbar_labels_collapsed(
            available + 52.,
            &[("New proxy", true)],
            (if search_open { 2 } else { 3 })
                + usize::from(self.error.is_some() || self.reference_error.is_some()),
            search_open,
            window,
        );
        let sort = self.preferences.sort;
        let owner = cx.weak_entity();
        let drawer = (open || amount > 0.).then(|| {
            let content = self.display_drawer(window, cx);
            crate::components::panel::slide(
                "proxy-drawer-slide",
                crate::layout::DRAWER_WIDTH,
                amount,
                open,
                content,
            )
        });
        div().size_full().relative().flex().min_h_0().track_focus(&self.focus).key_context("Nauterm")
            .on_key_down(cx.listener(Self::navigate_cards))
            .on_action(cx.listener(|view, _: &NewItem, window, cx| { if !crate::components::overlay::is_modal_open(window, cx) { view.create(window, cx); } cx.stop_propagation(); }))
            .on_action(cx.listener(|view, _: &CloseEditor, window, cx| { view.back_editor(window, cx); cx.stop_propagation(); }))
            .on_action(cx.listener(|view, _: &SelectCards, window, cx| { if !crate::components::overlay::is_modal_open(window, cx) && !view.selection.is_empty() { view.selection = view.visible(cx).iter().map(|entry| entry.id).collect(); cx.notify(); } cx.stop_propagation(); }))
            .on_action(cx.listener(|view, _: &DeleteCards, window, cx| { if !crate::components::overlay::is_modal_open(window, cx) { view.request_delete(cx); } cx.stop_propagation(); }))
            .child(div().flex_1().min_w_0().min_h_0().flex().flex_col()
                .child(crate::components::management::toolbar(self.dark)
                    .child(Button::new("new-proxy").toolbar().icon(crate::assets::icon(IconName::Plus)).label("New proxy").label_collapsed(collapse_labels).disabled(self.busy()).on_click(cx.listener(|view, _, window, cx| view.create(window, cx))))
                    .child(div().flex_1())
                    .when(self.error.is_some() || self.reference_error.is_some(), |view| view.child(Button::new("reload-proxies").toolbar().ghost().icon(IconName::RefreshCw).tooltip("Reload proxies").disabled(self.loading || self.saving || self.deletion.is_some()).on_click(cx.listener(|view, _, _, cx| view.refresh(cx)))))
                    .child(Button::new("sort-proxies").toolbar().ghost().icon(sort.icon()).tooltip(sort.label()).dropdown_menu(move |menu, _, _| { let owner = owner.clone(); sort_menu(menu, sort, move |sort, _, cx| { let _ = owner.update(cx, |view, cx| { view.preferences.sort = sort; cx.notify(); }); }) }))
                    .child({ let owner = cx.weak_entity(); crate::components::management::toolbar_search("proxies-search", &self.search, self.preferences.search_open, move |open, window, cx| { let _ = owner.update(cx, |view, cx| { view.preferences.search_open = open; if open { view.search.update(cx, |input, cx| input.focus(window, cx)); } else { view.focus.focus(window, cx); } cx.notify(); }); }, cx) })
                    .child(Button::new("proxy-view").toolbar().ghost().icon(if self.preferences.list { IconName::LayoutGrid } else { IconName::List }).tooltip("Switch list/grid view").on_click(cx.listener(|view, _, _, cx| { view.preferences.list = !view.preferences.list; cx.notify(); }))))
                .child(div().id("proxies-scroll").flex_1().min_h_0().overflow_y_scroll().scrollbars().map(crate::components::management::collection_padding).flex().flex_col()
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| view.close_editor(window, cx)))
                    .when(!entries.is_empty(), |view| view.child(crate::components::management::section_title("Proxies", cx).mb(px(crate::components::management::COLLECTION_TITLE_GAP))))
                    .child(crate::components::card_grid(grid).children(entries.iter().cloned().map(|entry| self.card(entry, cx))))
                    .when(entries.is_empty(), |view| view.child({
                        let empty = crate::components::EmptyState::new(IconName::Network, if self.loading { "Loading proxies…" } else if self.search.read(cx).value().is_empty() { "No proxies yet" } else { "No matching proxies" }, self.dark)
                            .description("Route SSH connections through an HTTP or SOCKS5 proxy.");
                        if !self.loading && self.search.read(cx).value().is_empty() { empty.action(Button::new("empty-add-proxy").primary().icon(crate::assets::icon(IconName::Plus)).label("Add proxy").disabled(self.busy()).on_click(cx.listener(|view, _, window, cx| view.create(window, cx)))) } else { empty }
                    }))))
            .children(drawer)
            .when_some(self.deletion.as_ref(), |view, ids| {
                let title = format!("Delete {} proxy/proxies?", ids.len());
                view.child(crate::components::Dialog::new("delete-proxies-modal", title.clone()).return_focus(&self.focus).dismissible(!self.saving)
                    .on_dismiss(cx.listener(|view, _, _, cx| { view.deletion = None; cx.notify(); }))
                    .content(DialogFrame::new("delete-proxies-dialog", title).width(380.).colors(DialogColors::workspace(self.dark))
                        .content("This removes the saved proxy and its stored authentication. Existing connections are not closed.")
                        .action(Button::new("cancel-delete-proxies").ghost().label("Cancel").disabled(self.saving).on_click(|_, window, cx| Modal::dismiss(window, cx)))
                        .action(Button::new("confirm-delete-proxies").danger().label("Delete").disabled(self.saving).on_click(cx.listener(|view, _, _, cx| view.mutate(true, cx))))))
            })
            .into_any_element()
    }
}
