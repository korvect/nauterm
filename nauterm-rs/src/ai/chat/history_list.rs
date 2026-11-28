//! Retained history summaries; redraws only look up the requested visible rows.
use nauterm_core::AiConversationEntry;

pub(super) struct Row {
    pub uuid: String,
    pub title: String,
    pub updated_at: Option<i64>,
    search: String,
}

pub(super) struct Catalog {
    rows: Vec<Row>,
    visible: Vec<usize>,
    query: String,
}
impl Catalog {
    pub fn new(entries: Vec<AiConversationEntry>) -> Self {
        let rows: Vec<_> = entries
            .into_iter()
            .filter_map(|entry| {
                let uuid = entry.uuid?;
                let search = format!(
                    "{} {}",
                    entry.title,
                    entry.preview.as_deref().unwrap_or_default()
                )
                .to_lowercase();
                Some(Row {
                    uuid,
                    title: entry.title,
                    updated_at: entry.updated_at,
                    search,
                })
            })
            .collect();
        let visible = (0..rows.len()).collect();
        Self {
            rows,
            visible,
            query: String::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }
    pub fn row(&self, index: usize) -> Option<&Row> {
        self.visible
            .get(index)
            .and_then(|index| self.rows.get(*index))
    }
    pub fn set_query(&mut self, query: &str) -> bool {
        let query = query.to_lowercase();
        if self.query == query {
            return false;
        }
        self.query = query;
        self.rebuild();
        true
    }
    pub fn remove(&mut self, uuid: &str) {
        self.rows.retain(|row| row.uuid != uuid);
        self.rebuild();
    }
    fn rebuild(&mut self) {
        self.visible.clear();
        self.visible
            .extend(self.rows.iter().enumerate().filter_map(|(index, row)| {
                (self.query.is_empty() || row.search.contains(&self.query)).then_some(index)
            }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(uuid: Option<&str>, title: &str, preview: &str) -> AiConversationEntry {
        serde_json::from_value(serde_json::json!({
            "uuid": uuid,
            "title": title,
            "preview": preview,
            "scope": "terminal"
        }))
        .unwrap()
    }

    #[test]
    fn history_search_retains_order_and_normalizes_title_and_preview() {
        let mut catalog = Catalog::new(vec![
            entry(Some("first"), "Ping HOST", ""),
            entry(None, "Unopenable", "host"),
            entry(Some("second"), "Other", "Host preview"),
            entry(Some("third"), "中文", ""),
        ]);
        assert_eq!(catalog.visible_len(), 3);
        assert!(catalog.set_query("HOST"));
        assert_eq!(catalog.visible_len(), 2);
        assert_eq!(catalog.row(0).unwrap().uuid, "first");
        assert_eq!(catalog.row(1).unwrap().uuid, "second");
        let retained = catalog.row(0).unwrap() as *const Row;
        assert!(!catalog.set_query("host"));
        assert_eq!(catalog.row(0).unwrap() as *const Row, retained);
        assert!(catalog.set_query("中文"));
        assert_eq!(catalog.row(0).unwrap().uuid, "third");
        catalog.remove("first");
        assert_eq!(catalog.row(0).unwrap().uuid, "third");
        catalog.set_query("");
        assert_eq!(catalog.visible_len(), 2);
        assert_eq!(catalog.row(0).unwrap().uuid, "second");
        assert!(catalog.row(2).is_none());
    }

    #[test]
    fn indexed_history_lookup_does_not_recreate_summary_rows() {
        let entries = (0..1000)
            .map(|index| {
                entry(
                    Some(&format!("id-{index}")),
                    &format!("Title {index}"),
                    "Preview",
                )
            })
            .collect();
        let mut catalog = Catalog::new(entries);
        let retained = catalog.row(700).unwrap() as *const Row;
        for _ in 0..20 {
            assert_eq!(catalog.row(700).unwrap() as *const Row, retained);
            assert_eq!(catalog.row(704).unwrap().uuid, "id-704");
        }
        catalog.set_query("not found");
        assert_eq!(catalog.visible_len(), 0);
        assert!(!catalog.is_empty());
        catalog.set_query("");
        assert_eq!(catalog.row(700).unwrap() as *const Row, retained);
    }
}
