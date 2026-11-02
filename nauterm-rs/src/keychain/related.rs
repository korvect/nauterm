//! The same credential editor can be pushed inside another resource drawer.
//! The parent retains its draft; only successful native saves return an ID.
use super::*;

#[derive(Clone, Copy)]
pub enum RelatedEvent {
    Saved(CredentialRef),
    Cancelled,
}
impl EventEmitter<RelatedEvent> for Keychain {}

pub(super) struct Nested {
    pub view: Entity<Keychain>,
    _events: Subscription,
}

pub(crate) struct RelatedSource {
    known_hosts_path: PathBuf,
    database_path: PathBuf,
    vault: Vault,
    inventory: Inventory,
    dark: bool,
    owner: WeakEntity<Keychain>,
}

impl Keychain {
    pub(crate) fn related_source(&self, cx: &Context<Self>) -> Option<RelatedSource> {
        Some(RelatedSource {
            known_hosts_path: self.known_hosts_path.clone(),
            database_path: self.database_path.clone(),
            vault: self.vault.clone()?,
            inventory: self.inventory.clone(),
            dark: self.dark,
            owner: self
                .inventory_owner
                .clone()
                .unwrap_or_else(|| cx.weak_entity()),
        })
    }
    pub(crate) fn related(
        source: RelatedSource,
        kind: Kind,
        initial_name: &str,
        username: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch_many(window, cx, Self::feedback);
        let search = cx.new(|cx| InputState::new(window, cx));
        let mut this = Self::empty(
            source.known_hosts_path,
            source.database_path,
            search,
            source.dark,
            cx,
        );
        this.related = true;
        this.vault = Some(source.vault);
        this.inventory = source.inventory;
        this.inventory_owner = Some(source.owner);
        this.install_editor(kind, None, None, window, cx);
        this.focus_editor(window, cx);
        let editor = this.editor.as_mut().unwrap();
        if !initial_name.trim().is_empty() {
            editor.automatic_name = false;
            editor.name.update(cx, |field, cx| {
                field.set_value(initial_name.trim(), window, cx)
            });
        }
        if kind == Kind::Identity {
            editor
                .username
                .update(cx, |field, cx| field.set_value(username, window, cx));
        }
        this.refresh_automatic_name(window, cx);
        this
    }

    pub(super) fn finish_save(
        &mut self,
        id: i64,
        identity: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.related {
            self.retire_editor();
            self.revision += 1;
            cx.emit(RelatedEvent::Saved(if identity {
                CredentialRef::Identity(id)
            } else {
                CredentialRef::Key(id)
            }));
        } else {
            self.close_editor(window, cx);
        }
    }

    pub(super) fn open_related_key(
        &mut self,
        kind: Kind,
        initial_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving
            || !self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.kind == Kind::Identity && editor.related.is_none())
        {
            return;
        }
        let Some(source) = self.related_source(cx) else {
            return;
        };
        let view = cx.new(|cx| Self::related(source, kind, initial_name, "", window, cx));
        let events = cx.subscribe_in(
            &view,
            window,
            |this, view, event: &RelatedEvent, window, cx| {
                let Some(editor) = this.editor.as_mut().filter(|editor| {
                    editor
                        .related
                        .as_ref()
                        .is_some_and(|nested| nested.view == *view)
                }) else {
                    return;
                };
                if let RelatedEvent::Saved(CredentialRef::Key(id)) = event {
                    editor.key_id = Some(*id);
                }
                editor.related = None;
                if let Some(select) = &editor.key_select {
                    select
                        .select
                        .state
                        .read(cx)
                        .focus_handle(cx)
                        .focus(window, cx);
                }
                if this.related && matches!(event, RelatedEvent::Saved(_)) {
                    this.refresh(cx);
                }
                cx.notify();
            },
        );
        self.editor.as_mut().unwrap().related = Some(Nested {
            view,
            _events: events,
        });
        cx.notify();
    }
}
