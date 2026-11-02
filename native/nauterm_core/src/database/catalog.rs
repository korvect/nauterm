//! Atomic catalog edits shared by desktop clients. Relationships use UUIDs;
//! numeric IDs remain local to each table and include tombstoned records.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// A new row. Existing rows must be read and updated through their repository
/// so callers retain UUIDs, version metadata and fields they do not edit.
pub enum NewCatalogRecord<'a> {
    Group(&'a HostGroup),
    Host(&'a HostEntry),
    Tag(&'a TagEntry),
    PortForward(&'a PortForwardEntry),
    SnippetPackage(&'a SnippetPackageEntry),
    Snippet(&'a SnippetEntry),
}

impl NewCatalogRecord<'_> {
    fn table_and_id(&self) -> (&'static str, Option<i64>) {
        match self {
            Self::Group(row) => ("groups", row.id),
            Self::Host(row) => ("hosts", row.id),
            Self::Tag(row) => ("tags", row.id),
            Self::PortForward(row) => ("port_forwards", row.id),
            Self::SnippetPackage(row) => ("snippet_packages", row.id),
            Self::Snippet(row) => ("snippets", row.id),
        }
    }
}

impl NautermDatabase {
    /// Run an edit under a write reservation before reading conflict checks.
    /// Nested repository edits use savepoints; errors and unwinds roll back.
    pub fn atomic<T, E: From<rusqlite::Error>>(
        &mut self,
        work: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let nested = !self.connection.is_autocommit();
        let name = format!("nauterm_edit_{}", NEXT.fetch_add(1, Ordering::Relaxed));
        let (begin, commit, rollback) = if nested {
            (
                format!("SAVEPOINT {name}"),
                format!("RELEASE {name}"),
                format!("ROLLBACK TO {name}; RELEASE {name}"),
            )
        } else {
            ("BEGIN IMMEDIATE".into(), "COMMIT".into(), "ROLLBACK".into())
        };
        self.connection.execute_batch(&begin)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(self)));
        match result {
            Ok(Ok(value)) => match self.connection.execute_batch(&commit) {
                Ok(()) => Ok(value),
                Err(error) => {
                    let _ = self.connection.execute_batch(&rollback);
                    Err(error.into())
                }
            },
            Ok(Err(error)) => {
                self.connection.execute_batch(&rollback)?;
                Err(error)
            }
            Err(panic) => {
                let _ = self.connection.execute_batch(&rollback);
                std::panic::resume_unwind(panic)
            }
        }
    }

    /// A UI may allocate its next draft ID before opening an editor. Include
    /// tombstones, so such an ID never overwrites a previously deleted row.
    pub fn next_catalog_id(&self) -> rusqlite::Result<i64> {
        let maximum: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM (
                SELECT id FROM groups UNION ALL SELECT id FROM hosts
                UNION ALL SELECT id FROM keys UNION ALL SELECT id FROM identities
                UNION ALL SELECT id FROM tags UNION ALL SELECT id FROM proxies
                UNION ALL SELECT id FROM port_forwards
                UNION ALL SELECT id FROM snippets
                UNION ALL SELECT id FROM snippet_packages
                UNION ALL SELECT seq FROM sqlite_sequence WHERE name IN
                  ('groups', 'hosts', 'keys', 'identities', 'tags', 'proxies',
                   'port_forwards', 'snippets', 'snippet_packages')
            )",
            [],
            |row| row.get(0),
        )?;
        maximum
            .checked_add(1)
            .ok_or(rusqlite::Error::IntegralValueOutOfRange(0, maximum))
    }

    /// Insert at a preallocated draft ID without changing the caller's card,
    /// drawer or relationship references. A concurrent collision is an error,
    /// including when the other row is a tombstone. No existing row is moved.
    pub fn insert_catalog_at(
        &mut self,
        id: i64,
        record: NewCatalogRecord<'_>,
    ) -> rusqlite::Result<i64> {
        let (table, existing) = record.table_and_id();
        if id <= 0 || existing.is_some() {
            return Err(rusqlite::Error::InvalidParameterName(
                "A new catalog row requires a positive, unassigned draft ID".into(),
            ));
        }
        self.atomic(|database| {
            // `table` is selected by the typed enum, never supplied by input.
            let occupied: bool = database.connection.query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?)"),
                [id],
                |row| row.get(0),
            )?;
            if occupied {
                return Err(rusqlite::Error::InvalidParameterName(
                    "The catalog changed in another window; reload before saving".into(),
                ));
            }
            let assigned = match record {
                NewCatalogRecord::Group(row) => database.save_group(row)?,
                NewCatalogRecord::Host(row) => database.save_host(row)?,
                NewCatalogRecord::Tag(row) => database.save_tag(row)?,
                NewCatalogRecord::PortForward(row) => database.save_port_forward(row)?,
                NewCatalogRecord::SnippetPackage(row) => database.save_snippet_package(row)?,
                NewCatalogRecord::Snippet(row) => database.save_snippet(row)?,
            };
            if assigned != id {
                // Catalog relationships are UUID based, so retaining a draft's
                // numeric ID does not rewrite another resource's relationships.
                database.connection.execute(
                    &format!("UPDATE {table} SET id = ? WHERE id = ?"),
                    params![id, assigned],
                )?;
            }
            Ok(id)
        })
    }
}
