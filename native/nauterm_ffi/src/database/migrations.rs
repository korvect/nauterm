use super::*;

/// Applies migrations beginning with the new Korvect schema baseline.
///
/// Schema version 1 is the first supported version. When version 2 is added,
/// its v1-to-v2 migration belongs here; schemas from the previous application
/// identity are intentionally unsupported.
pub(super) fn migrate_schema(connection: &mut Connection, version: i32) -> rusqlite::Result<()> {
    match version {
        1 => {
            migrate_v1_to_v2(connection)?;
            migrate_v2_to_v3(connection)
        }
        2 => migrate_v2_to_v3(connection),
        SCHEMA_VERSION => Ok(()),
        _ => Err(rusqlite::Error::InvalidParameterName(format!(
            "database schema version {version} is unsupported; expected {SCHEMA_VERSION}"
        ))),
    }
}

pub(super) fn migrate_v1_to_v2(connection: &mut Connection) -> rusqlite::Result<()> {
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
    transaction.pragma_update(None, "user_version", 2)?;
    transaction.commit()
}

fn migrate_v2_to_v3(connection: &mut Connection) -> rusqlite::Result<()> {
    connection.pragma_update(None, "legacy_alter_table", true)?;
    let result = migrate_v2_to_v3_inner(connection);
    let reset_result = connection.pragma_update(None, "legacy_alter_table", false);
    result.and(reset_result)
}

fn migrate_v2_to_v3_inner(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        r#"
        ALTER TABLE hosts RENAME TO hosts_v2;
        CREATE TABLE hosts (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          group_uuid TEXT,
          identity_uuid TEXT,
          proxy_uuid TEXT,
          host TEXT,
          port INTEGER CHECK (port IS NULL OR port BETWEEN 1 AND 65535),
          username TEXT,
          password TEXT,
          theme_id TEXT,
          startup_snippet_uuid TEXT,
          ssh_enabled INTEGER CHECK (ssh_enabled IS NULL OR ssh_enabled IN (0, 1)),
          mosh_enabled INTEGER CHECK (mosh_enabled IS NULL OR mosh_enabled IN (0, 1)),
          mosh_server_command TEXT,
          telnet_enabled INTEGER CHECK (telnet_enabled IS NULL OR telnet_enabled IN (0, 1)),
          telnet_identity_uuid TEXT,
          telnet_username TEXT,
          telnet_password TEXT,
          telnet_port INTEGER CHECK (telnet_port IS NULL OR telnet_port BETWEEN 1 AND 65535),
          telnet_theme_id TEXT,
          environment_variables TEXT
            CHECK (
              environment_variables IS NULL OR
              (json_valid(environment_variables) AND json_type(environment_variables) = 'array')
            ),
          encoding TEXT,
          telnet_encoding TEXT,
          type TEXT NOT NULL DEFAULT 'remote',
          key_uuid TEXT,
          shell_path TEXT,
          work_dir TEXT,
          os TEXT,
          distro TEXT,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT,
          FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
          FOREIGN KEY (identity_uuid) REFERENCES identities(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
          FOREIGN KEY (proxy_uuid) REFERENCES proxies(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
          FOREIGN KEY (telnet_identity_uuid) REFERENCES identities(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
          FOREIGN KEY (key_uuid) REFERENCES keys(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
          FOREIGN KEY (startup_snippet_uuid) REFERENCES snippets(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
        );
        INSERT INTO hosts SELECT * FROM hosts_v2;
        DROP TABLE hosts_v2;
        CREATE INDEX idx_hosts_group_uuid ON hosts(group_uuid, name COLLATE NOCASE)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_hosts_proxy_uuid ON hosts(proxy_uuid)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_hosts_type ON hosts(type, name COLLATE NOCASE)
          WHERE deleted_at IS NULL;

        ALTER TABLE port_forwards RENAME TO port_forwards_v2;
        CREATE TABLE port_forwards (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          type TEXT NOT NULL,
          bind_address TEXT NOT NULL,
          bind_port INTEGER NOT NULL CHECK (bind_port BETWEEN 1 AND 65535),
          destination_host TEXT NOT NULL,
          destination_port INTEGER NOT NULL
            CHECK ((type = 'dynamic' AND destination_port = 0) OR destination_port BETWEEN 1 AND 65535),
          host_uuid TEXT,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT,
          FOREIGN KEY (host_uuid) REFERENCES hosts(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
        );
        INSERT INTO port_forwards SELECT * FROM port_forwards_v2;
        DROP TABLE port_forwards_v2;
        CREATE INDEX idx_port_forwards_host_uuid
          ON port_forwards(host_uuid, name COLLATE NOCASE)
          WHERE deleted_at IS NULL;

        ALTER TABLE proxies RENAME TO proxies_v2;
        CREATE TABLE proxies (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          type TEXT NOT NULL,
          host TEXT NOT NULL,
          port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
          identity_uuid TEXT,
          username TEXT,
          password TEXT,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT,
          FOREIGN KEY (identity_uuid) REFERENCES identities(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
        );
        INSERT INTO proxies SELECT * FROM proxies_v2;
        DROP TABLE proxies_v2;
        CREATE UNIQUE INDEX idx_proxies_uuid ON proxies(uuid);
        CREATE INDEX idx_proxies_identity_uuid
          ON proxies(identity_uuid) WHERE deleted_at IS NULL;
        CREATE INDEX idx_proxies_name
          ON proxies(name COLLATE NOCASE) WHERE deleted_at IS NULL;

        ALTER TABLE sftp_favorites RENAME TO sftp_favorites_v2;
        CREATE TABLE sftp_favorites (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          scope TEXT NOT NULL DEFAULT 'remote',
          host_uuid TEXT NOT NULL,
          path TEXT NOT NULL,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT,
          FOREIGN KEY (host_uuid) REFERENCES hosts(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
        );
        INSERT INTO sftp_favorites SELECT * FROM sftp_favorites_v2;
        DROP TABLE sftp_favorites_v2;
        CREATE UNIQUE INDEX idx_sftp_favorites_unique
          ON sftp_favorites(host_uuid, path)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_sftp_favorites_host
          ON sftp_favorites(host_uuid, updated_at DESC)
          WHERE deleted_at IS NULL;

        ALTER TABLE snippets RENAME TO snippets_v2;
        CREATE TABLE snippets (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          package_uuid TEXT,
          scope TEXT NOT NULL DEFAULT 'global',
          description TEXT NOT NULL,
          script TEXT NOT NULL,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT,
          FOREIGN KEY (package_uuid) REFERENCES snippet_packages(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
        );
        INSERT INTO snippets SELECT * FROM snippets_v2;
        DROP TABLE snippets_v2;
        CREATE INDEX idx_snippets_package
          ON snippets(package_uuid, description COLLATE NOCASE)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_snippets_scope
          ON snippets(scope) WHERE deleted_at IS NULL;

        ALTER TABLE sftp_tasks RENAME TO sftp_tasks_v2;
        CREATE TABLE sftp_tasks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          host_uuid TEXT,
          transfer_type TEXT NOT NULL,
          host TEXT NOT NULL DEFAULT '',
          username TEXT NOT NULL DEFAULT '',
          port INTEGER NOT NULL DEFAULT 22 CHECK (port BETWEEN 1 AND 65535),
          display_name TEXT NOT NULL DEFAULT '',
          source_path TEXT NOT NULL DEFAULT '',
          target_path TEXT NOT NULL DEFAULT '',
          item_kind TEXT NOT NULL DEFAULT 'unknown',
          status TEXT NOT NULL,
          bytes INTEGER NOT NULL DEFAULT 0 CHECK (bytes >= 0),
          total_bytes INTEGER NOT NULL DEFAULT 0 CHECK (total_bytes >= 0),
          error_text TEXT,
          created_at INTEGER NOT NULL,
          finished_at INTEGER NOT NULL CHECK (finished_at >= created_at)
        );
        INSERT INTO sftp_tasks SELECT * FROM sftp_tasks_v2;
        DROP TABLE sftp_tasks_v2;
        CREATE INDEX idx_sftp_tasks_finished_at
          ON sftp_tasks(finished_at DESC);
        CREATE INDEX idx_sftp_tasks_host_finished_at
          ON sftp_tasks(host_uuid, finished_at DESC);

        ALTER TABLE ai_conversations RENAME TO ai_conversations_v2;
        CREATE TABLE ai_conversations (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          title TEXT NOT NULL,
          scope TEXT NOT NULL,
          host_uuid TEXT,
          provider_uuid TEXT,
          model TEXT NOT NULL DEFAULT '',
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
        );
        INSERT INTO ai_conversations SELECT * FROM ai_conversations_v2;
        DROP TABLE ai_conversations_v2;
        CREATE INDEX idx_ai_conversations_updated_at
          ON ai_conversations(updated_at DESC);
        CREATE INDEX idx_ai_conversations_host
          ON ai_conversations(host_uuid, updated_at DESC);

        ALTER TABLE ai_messages RENAME TO ai_messages_v2;
        CREATE TABLE ai_messages (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          conversation_uuid TEXT NOT NULL,
          role TEXT NOT NULL,
          content TEXT NOT NULL,
          context TEXT NOT NULL DEFAULT '',
          sequence INTEGER NOT NULL,
          tool_calls TEXT NOT NULL DEFAULT '[]'
            CHECK (json_valid(tool_calls) AND json_type(tool_calls) = 'array'),
          tool_result TEXT CHECK (tool_result IS NULL OR json_valid(tool_result)),
          attachments TEXT NOT NULL DEFAULT '[]'
            CHECK (json_valid(attachments) AND json_type(attachments) = 'array'),
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          FOREIGN KEY (conversation_uuid) REFERENCES ai_conversations(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
        );
        INSERT INTO ai_messages SELECT * FROM ai_messages_v2;
        DROP TABLE ai_messages_v2;
        CREATE INDEX idx_ai_messages_conversation
          ON ai_messages(conversation_uuid, sequence ASC);

        ALTER TABLE ai_command_blocks RENAME TO ai_command_blocks_v2;
        CREATE TABLE ai_command_blocks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          conversation_uuid TEXT NOT NULL,
          tool_call_id TEXT NOT NULL,
          command TEXT NOT NULL,
          explanation TEXT NOT NULL,
          status TEXT NOT NULL,
          sequence INTEGER NOT NULL,
          output TEXT,
          exit_code INTEGER,
          error TEXT,
          started_at INTEGER,
          finished_at INTEGER,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          CHECK (finished_at IS NULL OR started_at IS NULL OR finished_at >= started_at),
          FOREIGN KEY (conversation_uuid) REFERENCES ai_conversations(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
        );
        INSERT INTO ai_command_blocks SELECT * FROM ai_command_blocks_v2;
        DROP TABLE ai_command_blocks_v2;
        CREATE INDEX idx_ai_command_blocks_conversation
          ON ai_command_blocks(conversation_uuid, sequence ASC);
        "#,
    )?;
    schema::create_device_tracking_triggers(
        &transaction,
        &[
            "hosts",
            "port_forwards",
            "proxies",
            "sftp_favorites",
            "snippets",
        ],
    )?;
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit()
}
