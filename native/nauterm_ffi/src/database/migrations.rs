use super::*;

/// Applies migrations beginning with the new Korvect schema baseline.
///
/// Schema version 1 is the first supported version. When version 2 is added,
/// its v1-to-v2 migration belongs here; schemas from the previous application
/// identity are intentionally unsupported.
pub(super) fn migrate_schema(connection: &mut Connection, version: i32) -> rusqlite::Result<()> {
    match version {
        1 => migrate_v1_to_v2(connection),
        SCHEMA_VERSION => Ok(()),
        _ => Err(rusqlite::Error::InvalidParameterName(format!(
            "database schema version {version} is unsupported; expected {SCHEMA_VERSION}"
        ))),
    }
}

fn migrate_v1_to_v2(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        r#"
        ALTER TABLE ai_providers RENAME TO ai_providers_v1;
        CREATE TABLE ai_providers (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          protocol TEXT NOT NULL,
          base_url TEXT NOT NULL,
          model TEXT NOT NULL DEFAULT '',
          api_key TEXT NOT NULL DEFAULT '',
          config TEXT NOT NULL DEFAULT '{}'
            CHECK (json_valid(config) AND json_type(config) = 'object'),
          active INTEGER NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
        );
        INSERT INTO ai_providers
          (id, uuid, name, protocol, base_url, model, api_key, config, active, created_at, updated_at)
        SELECT id, uuid, name, protocol, base_url, model, api_key, config, active, created_at, updated_at
        FROM ai_providers_v1;
        DROP TABLE ai_providers_v1;
        CREATE INDEX idx_ai_providers_active ON ai_providers(active);
        CREATE INDEX idx_ai_providers_updated_at ON ai_providers(updated_at DESC);
        "#,
    )?;
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit()
}
