//! A drawer's live form can be set aside until the user opens it again.
//! Retired drawers borrow parked forms for the closing animation.
pub(crate) enum Retired<K, E> {
    Draft(K),
    Closed(Box<E>),
}

impl<K: Eq + std::hash::Hash + Copy, E> Retired<K, E> {
    pub(crate) fn take_form(
        self,
        drafts: &mut std::collections::HashMap<K, E>,
    ) -> Option<(Option<K>, E)> {
        match self {
            Self::Draft(key) => drafts.remove(&key).map(|editor| (Some(key), editor)),
            Self::Closed(editor) => Some((None, *editor)),
        }
    }

    pub(crate) fn put_form(
        key: Option<K>,
        editor: E,
        drafts: &mut std::collections::HashMap<K, E>,
    ) -> Self {
        match key {
            Some(key) => {
                drafts.insert(key, editor);
                Self::Draft(key)
            }
            None => Self::Closed(Box::new(editor)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn closing_animation_preserves_only_parked_forms() {
        let mut drafts = HashMap::new();
        let closed = Retired::put_form(None, "saved", &mut drafts);
        let parked = Retired::put_form(Some(1), "unsaved", &mut drafts);

        let (key, form) = parked.take_form(&mut drafts).unwrap();
        assert!(drafts.is_empty());
        Retired::put_form(key, form, &mut drafts);
        assert_eq!(closed.take_form(&mut drafts), Some((None, "saved")));
        assert_eq!(drafts.remove(&1), Some("unsaved"));
        assert!(drafts.is_empty());
    }
}
