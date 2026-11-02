use super::*;

#[derive(Clone, Copy)]
pub enum RelatedEvent {
    Saved(i64),
    Cancelled,
}
impl EventEmitter<RelatedEvent> for Proxies {}

pub(crate) struct RelatedSource {
    database_path: PathBuf,
    vault: Vault,
    inventory: Inventory,
    entries: Vec<Summary>,
    dark: bool,
    keychain: WeakEntity<crate::keychain::Keychain>,
    owner: WeakEntity<Proxies>,
}
impl Proxies {
    pub(crate) fn related_source(&self, cx: &Context<Self>) -> Option<RelatedSource> {
        if !self.available() {
            return None;
        }
        Some(RelatedSource {
            database_path: self.database_path.clone(),
            vault: self.vault.clone()?,
            inventory: self.inventory.clone(),
            entries: self.entries.clone(),
            dark: self.dark,
            keychain: self.keychain.clone(),
            owner: cx.weak_entity(),
        })
    }
    pub(crate) fn related(
        source: RelatedSource,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::components::message::watch_many(window, cx, Self::feedback);
        let search = cx.new(|cx| InputState::new(window, cx));
        let mut view = Self::empty(
            source.database_path,
            Some(source.vault),
            vec![],
            search,
            source.dark,
            source.keychain,
            cx,
        );
        view.ready = true;
        view.inventory = source.inventory;
        view.entries = source.entries;
        view.owner = Some(source.owner);
        view.create(window, cx);
        view.editor
            .as_mut()
            .unwrap()
            .name
            .update(cx, |field, cx| field.set_value(name, window, cx));
        view
    }
}
