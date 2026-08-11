use super::*;

pub(super) fn create_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE app_metadata (
          key TEXT PRIMARY KEY NOT NULL,
          value TEXT NOT NULL,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
        );

        CREATE TABLE groups (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          parent_uuid TEXT,
          identity_uuid TEXT,
          proxy_uuid TEXT,
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
          key_uuid TEXT,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT,
          FOREIGN KEY (parent_uuid) REFERENCES groups(uuid)
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

        CREATE TABLE keys (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          private_key TEXT,
          public_key TEXT,
          certificate TEXT,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT
        );

        CREATE TABLE identities (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          username TEXT,
          password TEXT,
          key_uuid TEXT,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT,
          FOREIGN KEY (key_uuid) REFERENCES keys(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
        );

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
          type TEXT NOT NULL DEFAULT 'remote'
            CHECK (type IN ('local', 'remote')),
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

        CREATE TABLE port_forwards (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          type TEXT NOT NULL CHECK (type IN ('local', 'remote', 'dynamic')),
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

        CREATE INDEX idx_groups_parent_uuid ON groups(parent_uuid)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_keys_name ON keys(name COLLATE NOCASE)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_identities_name ON identities(name COLLATE NOCASE)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_identities_key_uuid ON identities(key_uuid)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_hosts_group_uuid ON hosts(group_uuid, name COLLATE NOCASE)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_hosts_proxy_uuid ON hosts(proxy_uuid)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_hosts_type ON hosts(type, name COLLATE NOCASE)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_port_forwards_host_uuid
          ON port_forwards(host_uuid, name COLLATE NOCASE)
          WHERE deleted_at IS NULL;
        "#,
    )?;
    create_proxy_schema(connection)?;
    create_tag_schema(connection)?;
    create_host_tag_schema(connection)?;
    create_terminal_log_schema(connection)?;
    create_snippet_schema(connection)?;
    create_sftp_favorites_schema(connection)?;
    create_sftp_task_schema(connection)?;
    create_sync_provider_schema(connection)?;
    create_ai_conversation_schema(connection)?;
    create_ai_provider_schema(connection)?;
    create_device_tracking_schema(connection)
}

fn create_device_tracking_schema(connection: &Connection) -> rusqlite::Result<()> {
    let device_id = generate_uuid_v7(connection)?;
    connection.execute(
        "INSERT INTO app_metadata (key, value) VALUES (?, ?)",
        params![DEVICE_ID_METADATA_KEY, device_id],
    )?;

    for table in DEVICE_TRACKED_TABLES {
        connection.execute_batch(&format!(
            r#"
            CREATE TRIGGER {table}_device_id_after_insert
            AFTER INSERT ON {table}
            WHEN NEW.created_device_id IS NULL
              OR trim(NEW.created_device_id) = ''
              OR NEW.updated_device_id IS NULL
              OR trim(NEW.updated_device_id) = ''
            BEGIN
              UPDATE {table}
              SET created_device_id = COALESCE(
                    NULLIF(trim(NEW.created_device_id), ''),
                    (SELECT value FROM app_metadata WHERE key = 'device_id')
                  ),
                  updated_device_id = COALESCE(
                    NULLIF(trim(NEW.updated_device_id), ''),
                    (SELECT value FROM app_metadata WHERE key = 'device_id')
                  )
              WHERE id = NEW.id;
            END;

            CREATE TRIGGER {table}_device_id_after_update
            AFTER UPDATE ON {table}
            WHEN NEW.updated_device_id IS OLD.updated_device_id
              AND COALESCE(trim(NEW.updated_device_id), '') !=
                  (SELECT value FROM app_metadata WHERE key = 'device_id')
            BEGIN
              UPDATE {table}
              SET updated_device_id = (
                SELECT value FROM app_metadata WHERE key = 'device_id'
              )
              WHERE id = NEW.id;
            END;
            "#
        ))?;
    }
    Ok(())
}

fn create_proxy_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE proxies (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          type TEXT NOT NULL CHECK (type IN ('http', 'socks5')),
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
        CREATE UNIQUE INDEX idx_proxies_uuid ON proxies(uuid);
        CREATE INDEX idx_proxies_identity_uuid
          ON proxies(identity_uuid) WHERE deleted_at IS NULL;
        CREATE INDEX idx_proxies_name
          ON proxies(name COLLATE NOCASE) WHERE deleted_at IS NULL;
        "#,
    )
}

fn create_sftp_favorites_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE sftp_favorites (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          scope TEXT NOT NULL DEFAULT 'remote' CHECK (scope = 'remote'),
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

        CREATE UNIQUE INDEX idx_sftp_favorites_unique
          ON sftp_favorites(host_uuid, path)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_sftp_favorites_host
          ON sftp_favorites(host_uuid, updated_at DESC)
          WHERE deleted_at IS NULL;
        "#,
    )
}

fn create_terminal_log_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE terminal_logs (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          title TEXT NOT NULL,
          theme_id TEXT,
          host_uuid TEXT,
          host TEXT,
          port INTEGER,
          username TEXT,
          shell_path TEXT,
          work_dir TEXT,
          cwd TEXT,
          capture_file TEXT NOT NULL,
          capture_bytes INTEGER NOT NULL DEFAULT 0,
          capture_sha256 TEXT,
          columns INTEGER,
          rows INTEGER,
          started_at INTEGER NOT NULL,
          ended_at INTEGER CHECK (ended_at IS NULL OR ended_at >= started_at),
          FOREIGN KEY (host_uuid) REFERENCES hosts(uuid)
            ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
        );

        CREATE TABLE terminal_log_events (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          log_uuid TEXT NOT NULL,
          timestamp INTEGER NOT NULL,
          type TEXT NOT NULL,
          message TEXT NOT NULL,
          connection_kind TEXT,
          data TEXT,
          FOREIGN KEY (log_uuid) REFERENCES terminal_logs(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
        );

        CREATE INDEX idx_terminal_logs_started_at
          ON terminal_logs(started_at DESC);
        CREATE INDEX idx_terminal_logs_host_uuid
          ON terminal_logs(host_uuid, started_at DESC);
        CREATE INDEX idx_terminal_logs_host_user
          ON terminal_logs(host, port, username, started_at DESC);
        CREATE INDEX idx_terminal_log_events_log_uuid
          ON terminal_log_events(log_uuid, timestamp ASC);
        "#,
    )
}

fn create_snippet_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE snippet_packages (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT
        );

        CREATE TABLE snippets (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          package_uuid TEXT,
          scope TEXT NOT NULL DEFAULT 'global'
            CHECK (scope IN ('global', 'targeted')),
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

        CREATE TABLE snippet_target_groups (
          snippet_uuid TEXT NOT NULL,
          group_uuid TEXT NOT NULL,
          PRIMARY KEY (snippet_uuid, group_uuid),
          FOREIGN KEY (snippet_uuid) REFERENCES snippets(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
          FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
        );

        CREATE TABLE snippet_target_hosts (
          snippet_uuid TEXT NOT NULL,
          host_uuid TEXT NOT NULL,
          PRIMARY KEY (snippet_uuid, host_uuid),
          FOREIGN KEY (snippet_uuid) REFERENCES snippets(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
          FOREIGN KEY (host_uuid) REFERENCES hosts(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
        );

        CREATE INDEX idx_snippet_packages_name
          ON snippet_packages(name COLLATE NOCASE) WHERE deleted_at IS NULL;
        CREATE INDEX idx_snippets_package
          ON snippets(package_uuid, description COLLATE NOCASE)
          WHERE deleted_at IS NULL;
        CREATE INDEX idx_snippets_scope
          ON snippets(scope) WHERE deleted_at IS NULL;
        CREATE INDEX idx_snippet_target_groups_group
          ON snippet_target_groups(group_uuid, snippet_uuid);
        CREATE INDEX idx_snippet_target_hosts_host
          ON snippet_target_hosts(host_uuid, snippet_uuid);
        "#,
    )
}

pub(super) fn create_host_tag_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE host_tags (
          host_uuid TEXT NOT NULL,
          tag_uuid TEXT NOT NULL,
          PRIMARY KEY (host_uuid, tag_uuid),
          FOREIGN KEY (host_uuid) REFERENCES hosts(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
          FOREIGN KEY (tag_uuid) REFERENCES tags(uuid)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
        );
        CREATE INDEX idx_host_tags_tag_uuid
          ON host_tags(tag_uuid, host_uuid);
        "#,
    )
}

pub(super) fn create_sync_provider_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE sync_providers (
          uuid TEXT PRIMARY KEY NOT NULL CHECK (length(trim(uuid)) BETWEEN 1 AND 128),
          provider TEXT NOT NULL CHECK (length(trim(provider)) BETWEEN 1 AND 64),
          name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
          config_json TEXT NOT NULL DEFAULT '{}'
            CHECK (json_valid(config_json) AND json_type(config_json) = 'object'),
          credentials_json TEXT
            CHECK (
              credentials_json IS NULL OR
              (json_valid(credentials_json) AND json_type(credentials_json) = 'object')
            ),
          active INTEGER NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
          last_seen_revision INTEGER CHECK (last_seen_revision IS NULL OR last_seen_revision >= 0),
          last_seen_snapshot_id TEXT,
          last_sync_at INTEGER,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
        );
        CREATE INDEX idx_sync_providers_provider
          ON sync_providers(provider, updated_at DESC);
        "#,
    )
}

pub(super) fn create_sftp_task_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE sftp_tasks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          host_uuid TEXT,
          transfer_type TEXT NOT NULL
            CHECK (transfer_type IN ('download', 'upload', 'edit', 'move', 'copy', 'delete')),
          host TEXT NOT NULL DEFAULT '',
          username TEXT NOT NULL DEFAULT '',
          port INTEGER NOT NULL DEFAULT 22 CHECK (port BETWEEN 1 AND 65535),
          display_name TEXT NOT NULL DEFAULT '',
          source_path TEXT NOT NULL DEFAULT '',
          target_path TEXT NOT NULL DEFAULT '',
          item_kind TEXT NOT NULL DEFAULT 'unknown'
            CHECK (item_kind IN ('file', 'folder', 'unknown')),
          status TEXT NOT NULL
            CHECK (status IN ('completed', 'failed', 'cancelled')),
          bytes INTEGER NOT NULL DEFAULT 0 CHECK (bytes >= 0),
          total_bytes INTEGER NOT NULL DEFAULT 0 CHECK (total_bytes >= 0),
          error_text TEXT,
          created_at INTEGER NOT NULL,
          finished_at INTEGER NOT NULL CHECK (finished_at >= created_at)
        );
        CREATE INDEX idx_sftp_tasks_finished_at
          ON sftp_tasks(finished_at DESC);
        CREATE INDEX idx_sftp_tasks_host_finished_at
          ON sftp_tasks(host_uuid, finished_at DESC);
        "#,
    )
}

pub(super) fn create_tag_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE tags (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL COLLATE NOCASE,
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          deleted_at INTEGER,
          version INTEGER NOT NULL DEFAULT 1,
          created_device_id TEXT,
          updated_device_id TEXT
        );
        CREATE UNIQUE INDEX idx_tags_active_name
          ON tags(name COLLATE NOCASE) WHERE deleted_at IS NULL;
        CREATE INDEX idx_tags_name
          ON tags(name COLLATE NOCASE) WHERE deleted_at IS NULL;
        "#,
    )?;
    Ok(())
}

pub(super) fn create_ai_provider_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
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
        CREATE INDEX idx_ai_providers_active
          ON ai_providers(active);
        CREATE INDEX idx_ai_providers_updated_at
          ON ai_providers(updated_at DESC);
        "#,
    )
}

pub(super) fn create_ai_conversation_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE ai_conversations (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          title TEXT NOT NULL,
          scope TEXT NOT NULL CHECK (scope IN ('terminal', 'workspace')),
          host_uuid TEXT,
          provider_uuid TEXT,
          model TEXT NOT NULL DEFAULT '',
          created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
          updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
        );

        CREATE TABLE ai_messages (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          conversation_uuid TEXT NOT NULL,
          role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
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

        CREATE TABLE ai_command_blocks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          uuid TEXT NOT NULL UNIQUE,
          conversation_uuid TEXT NOT NULL,
          tool_call_id TEXT NOT NULL,
          command TEXT NOT NULL,
          explanation TEXT NOT NULL,
          status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled', 'skipped')),
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

        CREATE INDEX idx_ai_conversations_updated_at
          ON ai_conversations(updated_at DESC);
        CREATE INDEX idx_ai_conversations_host
          ON ai_conversations(host_uuid, updated_at DESC);
        CREATE INDEX idx_ai_messages_conversation
          ON ai_messages(conversation_uuid, sequence ASC);
        CREATE INDEX idx_ai_command_blocks_conversation
          ON ai_command_blocks(conversation_uuid, sequence ASC);
        "#,
    )
}
