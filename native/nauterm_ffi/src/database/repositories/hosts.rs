use super::super::*;

impl NautermDatabase {
    pub fn save_group(&mut self, group: &HostGroup) -> rusqlite::Result<i64> {
        let dek = configured_dek(&self.connection)?;
        let password = encrypt_optional_field(dek.as_ref(), group.password.as_deref())?;
        let telnet_password =
            encrypt_optional_field(dek.as_ref(), group.telnet_password.as_deref())?;
        let parent_uuid = relation_uuid(
            &self.connection,
            "groups",
            group.parent_id,
            group.parent_uuid.as_deref(),
        )?;
        let identity_uuid = relation_uuid(
            &self.connection,
            "identities",
            group.identity_id,
            group.identity_uuid.as_deref(),
        )?;
        let proxy_uuid = relation_uuid(
            &self.connection,
            "proxies",
            group.proxy_id,
            group.proxy_uuid.as_deref(),
        )?;
        let telnet_identity_uuid = relation_uuid(
            &self.connection,
            "identities",
            group.telnet_identity_id,
            group.telnet_identity_uuid.as_deref(),
        )?;
        let key_uuid = relation_uuid(
            &self.connection,
            "keys",
            group.key_id,
            group.key_uuid.as_deref(),
        )?;
        let startup_snippet_uuid = relation_uuid(
            &self.connection,
            "snippets",
            group.startup_snippet_id,
            group.startup_snippet_uuid.as_deref(),
        )?;
        let environment_variables = (!group.environment_variables.is_empty())
            .then(|| host_environment_variables_json(&group.environment_variables));
        let encoding = normalize_optional_host_encoding(group.encoding.as_deref());
        let telnet_encoding = normalize_optional_host_encoding(group.telnet_encoding.as_deref());
        let mosh_server_command =
            normalize_optional_mosh_server_command(group.mosh_server_command.as_deref());
        if let Some(id) = group.id {
            self.connection.execute(
                r#"
                UPDATE groups
                SET name = ?, parent_uuid = ?, identity_uuid = ?,
                    proxy_uuid = ?, port = ?, username = ?, password = ?,
                    theme_id = ?, startup_snippet_uuid = ?, ssh_enabled = ?,
                    mosh_enabled = ?, mosh_server_command = ?,
                    telnet_enabled = ?, telnet_identity_uuid = ?,
                    telnet_username = ?, telnet_password = ?, telnet_port = ?,
                    telnet_theme_id = ?, environment_variables = ?,
                    encoding = ?, telnet_encoding = ?, key_uuid = ?,
                    updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                    deleted_at = NULL,
                    version = COALESCE(version, 0) + 1
                WHERE id = ?
                "#,
                params![
                    group.name,
                    parent_uuid,
                    identity_uuid,
                    proxy_uuid,
                    group.port,
                    group.username,
                    password,
                    group.theme_id,
                    startup_snippet_uuid,
                    group.ssh_enabled,
                    group.mosh_enabled,
                    mosh_server_command,
                    group.telnet_enabled,
                    telnet_identity_uuid,
                    group.telnet_username,
                    telnet_password,
                    group.telnet_port,
                    group.telnet_theme_id,
                    environment_variables,
                    encoding,
                    telnet_encoding,
                    key_uuid,
                    id
                ],
            )?;
            return Ok(id);
        }

        let uuid = uuid_or_new(&self.connection, group.uuid.as_deref())?;
        self.connection.execute(
            r#"
            INSERT INTO groups (
              uuid, name, parent_uuid, identity_uuid, proxy_uuid, port,
              username, password, theme_id, startup_snippet_uuid, ssh_enabled,
              mosh_enabled, mosh_server_command, telnet_enabled,
              telnet_identity_uuid, telnet_username, telnet_password,
              telnet_port, telnet_theme_id, environment_variables, encoding,
              telnet_encoding, key_uuid, created_at, updated_at
            )
            VALUES (
              ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
              ?, ?, CAST(unixepoch('subsec') * 1000 AS INTEGER),
              CAST(unixepoch('subsec') * 1000 AS INTEGER)
            )
            "#,
            params![
                uuid,
                group.name,
                parent_uuid,
                identity_uuid,
                proxy_uuid,
                group.port,
                group.username,
                password,
                group.theme_id,
                startup_snippet_uuid,
                group.ssh_enabled,
                group.mosh_enabled,
                mosh_server_command,
                group.telnet_enabled,
                telnet_identity_uuid,
                group.telnet_username,
                telnet_password,
                group.telnet_port,
                group.telnet_theme_id,
                environment_variables,
                encoding,
                telnet_encoding,
                key_uuid
            ],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_group(&self, id: i64) -> rusqlite::Result<Option<HostGroup>> {
        let dek = configured_dek(&self.connection)?;
        let value = self
            .connection
            .query_row(
                r#"
                SELECT
                  groups.*,
                  (SELECT id FROM groups parent WHERE parent.uuid = groups.parent_uuid AND parent.deleted_at IS NULL) AS parent_id,
                  (SELECT id FROM identities WHERE identities.uuid = groups.identity_uuid AND identities.deleted_at IS NULL) AS identity_id,
                  (SELECT id FROM proxies WHERE proxies.uuid = groups.proxy_uuid AND proxies.deleted_at IS NULL) AS proxy_id,
                  (SELECT id FROM snippets WHERE snippets.uuid = groups.startup_snippet_uuid AND snippets.deleted_at IS NULL) AS startup_snippet_id,
                  (SELECT id FROM identities WHERE identities.uuid = groups.telnet_identity_uuid AND identities.deleted_at IS NULL) AS telnet_identity_id,
                  (SELECT id FROM keys WHERE keys.uuid = groups.key_uuid AND keys.deleted_at IS NULL) AS key_id
                FROM groups
                WHERE id = ? AND deleted_at IS NULL
                "#,
                params![id],
                group_from_row,
            )
            .optional()?;
        value
            .map(|mut group| {
                group.password = decrypt_optional_field(dek.as_ref(), group.password)?;
                group.telnet_password =
                    decrypt_optional_field(dek.as_ref(), group.telnet_password)?;
                Ok(group)
            })
            .transpose()
    }

    pub fn list_groups(&self) -> rusqlite::Result<Vec<HostGroup>> {
        let dek = configured_dek(&self.connection)?;
        let mut statement = self.connection.prepare(
            r#"
            SELECT
              groups.*,
              (SELECT id FROM groups parent WHERE parent.uuid = groups.parent_uuid AND parent.deleted_at IS NULL) AS parent_id,
              (SELECT id FROM identities WHERE identities.uuid = groups.identity_uuid AND identities.deleted_at IS NULL) AS identity_id,
              (SELECT id FROM proxies WHERE proxies.uuid = groups.proxy_uuid AND proxies.deleted_at IS NULL) AS proxy_id,
              (SELECT id FROM snippets WHERE snippets.uuid = groups.startup_snippet_uuid AND snippets.deleted_at IS NULL) AS startup_snippet_id,
              (SELECT id FROM identities WHERE identities.uuid = groups.telnet_identity_uuid AND identities.deleted_at IS NULL) AS telnet_identity_id,
              (SELECT id FROM keys WHERE keys.uuid = groups.key_uuid AND keys.deleted_at IS NULL) AS key_id
            FROM groups
            WHERE deleted_at IS NULL
            ORDER BY name COLLATE NOCASE
            "#,
        )?;
        let mut groups = statement
            .query_map([], group_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for group in &mut groups {
            group.password = decrypt_optional_field(dek.as_ref(), group.password.take())?;
            group.telnet_password =
                decrypt_optional_field(dek.as_ref(), group.telnet_password.take())?;
        }
        Ok(groups)
    }

    pub fn delete_group(&mut self, id: i64) -> rusqlite::Result<usize> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE groups SET parent_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE parent_uuid = (SELECT uuid FROM groups WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE hosts SET group_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE group_uuid = (SELECT uuid FROM groups WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "DELETE FROM snippet_target_groups WHERE group_uuid = (SELECT uuid FROM groups WHERE id = ?)",
            params![id],
        )?;
        let deleted = transaction.execute(
            "UPDATE groups SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE id = ? AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn save_key(&mut self, key: &KeyEntry) -> rusqlite::Result<i64> {
        let dek = configured_dek(&self.connection)?;
        let private_key = encrypt_optional_field(dek.as_ref(), key.private_key.as_deref())?;
        let certificate = encrypt_optional_field(dek.as_ref(), key.certificate.as_deref())?;
        if let Some(id) = key.id {
            self.connection.execute(
                r#"
                UPDATE keys
                SET name = ?, private_key = ?, public_key = ?, certificate = ?,
                    updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                    deleted_at = NULL,
                    version = COALESCE(version, 0) + 1
                WHERE id = ?
                "#,
                params![key.name, private_key, key.public_key, certificate, id],
            )?;
            return Ok(id);
        }

        let uuid = uuid_or_new(&self.connection, key.uuid.as_deref())?;
        self.connection.execute(
            r#"
            INSERT INTO keys (
              uuid, name, private_key, public_key, certificate, created_at, updated_at
            )
            VALUES (
              ?, ?, ?, ?, ?, CAST(unixepoch('subsec') * 1000 AS INTEGER),
              CAST(unixepoch('subsec') * 1000 AS INTEGER)
            )
            "#,
            params![uuid, key.name, private_key, key.public_key, certificate],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_key(&self, id: i64) -> rusqlite::Result<Option<KeyEntry>> {
        let dek = configured_dek(&self.connection)?;
        let value = self
            .connection
            .query_row(
                "SELECT * FROM keys WHERE id = ? AND deleted_at IS NULL",
                params![id],
                key_from_row,
            )
            .optional()?;
        value
            .map(|mut key| {
                key.private_key = decrypt_optional_field(dek.as_ref(), key.private_key)?;
                key.certificate = decrypt_optional_field(dek.as_ref(), key.certificate)?;
                Ok(key)
            })
            .transpose()
    }

    pub fn list_keys(&self) -> rusqlite::Result<Vec<KeyEntry>> {
        let dek = configured_dek(&self.connection)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, uuid, name, NULL AS private_key, public_key, certificate, created_at, updated_at, deleted_at, version, created_device_id, updated_device_id FROM keys WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE",
            )?;
        let mut values = statement
            .query_map([], key_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for key in &mut values {
            let certificate = decrypt_optional_field(dek.as_ref(), key.certificate.take())?;
            key.certificate = certificate.map(|certificate| {
                crate::ssh::openssh_certificate_type(&certificate).unwrap_or_default()
            });
        }
        Ok(values)
    }

    pub fn delete_key(&mut self, id: i64) -> rusqlite::Result<usize> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE identities SET key_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE key_uuid = (SELECT uuid FROM keys WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE hosts SET key_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE key_uuid = (SELECT uuid FROM keys WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE groups SET key_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE key_uuid = (SELECT uuid FROM keys WHERE id = ?)",
            params![id],
        )?;
        let deleted = transaction.execute(
            "UPDATE keys SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE id = ? AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn save_identity(&mut self, identity: &IdentityEntry) -> rusqlite::Result<i64> {
        let dek = configured_dek(&self.connection)?;
        let password = encrypt_optional_field(dek.as_ref(), identity.password.as_deref())?;
        let key_uuid = relation_uuid(
            &self.connection,
            "keys",
            identity.key_id,
            identity.key_uuid.as_deref(),
        )?;
        if let Some(id) = identity.id {
            self.connection.execute(
                "UPDATE identities SET name = ?, username = ?, password = ?, key_uuid = ?, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), deleted_at = NULL, version = COALESCE(version, 0) + 1 WHERE id = ?",
                params![
                    identity.name,
                    identity.username,
                    password,
                    key_uuid,
                    id
                ],
            )?;
            return Ok(id);
        }

        let uuid = uuid_or_new(&self.connection, identity.uuid.as_deref())?;
        self.connection.execute(
            "INSERT INTO identities (uuid, name, username, password, key_uuid, created_at, updated_at) VALUES (?, ?, ?, ?, ?, CAST(unixepoch('subsec') * 1000 AS INTEGER), CAST(unixepoch('subsec') * 1000 AS INTEGER))",
            params![
                uuid,
                identity.name,
                identity.username,
                password,
                key_uuid
            ],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_identity(&self, id: i64) -> rusqlite::Result<Option<IdentityEntry>> {
        let dek = configured_dek(&self.connection)?;
        let value = self
            .connection
            .query_row(
                r#"
                SELECT
                  identities.*,
                  (SELECT id FROM keys WHERE keys.uuid = identities.key_uuid AND keys.deleted_at IS NULL) AS key_id
                FROM identities
                WHERE id = ? AND deleted_at IS NULL
                "#,
                params![id],
                identity_from_row,
            )
            .optional()?;
        value
            .map(|mut identity| {
                identity.password = decrypt_optional_field(dek.as_ref(), identity.password)?;
                Ok(identity)
            })
            .transpose()
    }

    pub fn list_identities(&self) -> rusqlite::Result<Vec<IdentityEntry>> {
        let mut statement = self.connection.prepare(
            r#"
            SELECT
              identities.id, identities.uuid, identities.name,
              identities.username, NULL AS password, identities.key_uuid,
              identities.created_at, identities.updated_at, identities.deleted_at,
              identities.version, identities.created_device_id, identities.updated_device_id,
              (SELECT id FROM keys WHERE keys.uuid = identities.key_uuid AND keys.deleted_at IS NULL) AS key_id
            FROM identities
            WHERE deleted_at IS NULL
            ORDER BY name COLLATE NOCASE
            "#,
        )?;
        let values = statement.query_map([], identity_from_row)?.collect();
        values
    }

    pub fn save_tag(&mut self, tag: &TagEntry) -> rusqlite::Result<i64> {
        let name = tag.name.trim();
        if name.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "tag name cannot be empty".to_string(),
            ));
        }
        if let Some(id) = tag.id {
            self.connection.execute(
                "UPDATE tags SET name = ?, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), deleted_at = NULL, version = COALESCE(version, 0) + 1 WHERE id = ?",
                params![name, id],
            )?;
            return Ok(id);
        }
        let uuid = uuid_or_new(&self.connection, tag.uuid.as_deref())?;
        self.connection.execute(
            "INSERT INTO tags (uuid, name, created_at, updated_at) VALUES (?, ?, CAST(unixepoch('subsec') * 1000 AS INTEGER), CAST(unixepoch('subsec') * 1000 AS INTEGER))",
            params![uuid, name],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_tag(&self, id: i64) -> rusqlite::Result<Option<TagEntry>> {
        self.connection
            .query_row(
                "SELECT * FROM tags WHERE id = ? AND deleted_at IS NULL",
                params![id],
                tag_from_row,
            )
            .optional()
    }

    pub fn list_tags(&self) -> rusqlite::Result<Vec<TagEntry>> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM tags WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE")?;
        let values = statement.query_map([], tag_from_row)?.collect();
        values
    }

    pub fn delete_tag(&mut self, id: i64) -> rusqlite::Result<usize> {
        let tag_uuid: Option<String> = self
            .connection
            .query_row("SELECT uuid FROM tags WHERE id = ?", params![id], |row| {
                row.get(0)
            })
            .optional()?;
        let Some(tag_uuid) = tag_uuid else {
            return Ok(0);
        };
        let transaction = self.connection.transaction()?;
        transaction.execute(
            r#"
            UPDATE hosts
            SET updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                version = COALESCE(version, 0) + 1
            WHERE uuid IN (
              SELECT host_uuid FROM host_tags WHERE tag_uuid = ?
            )
            "#,
            params![tag_uuid],
        )?;
        transaction.execute(
            "DELETE FROM host_tags WHERE tag_uuid = ?",
            params![tag_uuid],
        )?;
        let deleted = transaction.execute(
            "UPDATE tags SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE id = ? AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn delete_identity(&mut self, id: i64) -> rusqlite::Result<usize> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE hosts SET identity_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE identity_uuid = (SELECT uuid FROM identities WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE hosts SET telnet_identity_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE telnet_identity_uuid = (SELECT uuid FROM identities WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE groups SET identity_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE identity_uuid = (SELECT uuid FROM identities WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE groups SET telnet_identity_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE telnet_identity_uuid = (SELECT uuid FROM identities WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE proxies SET identity_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE identity_uuid = (SELECT uuid FROM identities WHERE id = ?)",
            params![id],
        )?;
        let deleted = transaction.execute(
            "UPDATE identities SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE id = ? AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn save_host(&mut self, host: &HostEntry) -> rusqlite::Result<i64> {
        let dek = configured_dek(&self.connection)?;
        let password = encrypt_optional_field(dek.as_ref(), host.password.as_deref())?;
        let telnet_password =
            encrypt_optional_field(dek.as_ref(), host.telnet_password.as_deref())?;
        let environment_variables = (!host.environment_variables.is_empty())
            .then(|| host_environment_variables_json(&host.environment_variables));
        let encoding = normalize_optional_host_encoding(host.encoding.as_deref());
        let telnet_encoding = normalize_optional_host_encoding(host.telnet_encoding.as_deref());
        let mosh_server_command =
            normalize_optional_mosh_server_command(host.mosh_server_command.as_deref());
        let group_uuid = relation_uuid(
            &self.connection,
            "groups",
            host.group_id,
            host.group_uuid.as_deref(),
        )?;
        let identity_uuid = relation_uuid(
            &self.connection,
            "identities",
            host.identity_id,
            host.identity_uuid.as_deref(),
        )?;
        let proxy_uuid = relation_uuid(
            &self.connection,
            "proxies",
            host.proxy_id,
            host.proxy_uuid.as_deref(),
        )?;
        let telnet_identity_uuid = relation_uuid(
            &self.connection,
            "identities",
            host.telnet_identity_id,
            host.telnet_identity_uuid.as_deref(),
        )?;
        let key_uuid = relation_uuid(
            &self.connection,
            "keys",
            host.key_id,
            host.key_uuid.as_deref(),
        )?;
        let startup_snippet_uuid = relation_uuid(
            &self.connection,
            "snippets",
            host.startup_snippet_id,
            host.startup_snippet_uuid.as_deref(),
        )?;
        if let Some(id) = host.id {
            self.connection.execute(
                r#"
                UPDATE hosts
                SET name = ?, group_uuid = ?,
                    identity_uuid = ?,
                    proxy_uuid = ?,
                    host = ?, port = ?, username = ?, password = ?, theme_id = ?,
                    startup_snippet_uuid = ?, ssh_enabled = ?, mosh_enabled = ?, mosh_server_command = ?, telnet_enabled = ?,
                    telnet_identity_uuid = ?, telnet_username = ?,
                    telnet_password = ?, telnet_port = ?, telnet_theme_id = ?,
	                    environment_variables = ?, encoding = ?, telnet_encoding = ?, type = ?,
	                    key_uuid = ?, shell_path = ?, work_dir = ?,
	                    os = ?, distro = ?,
	                    updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                    deleted_at = NULL,
                    version = COALESCE(version, 0) + 1
                WHERE id = ?
                "#,
                params![
                    host.name,
                    group_uuid,
                    identity_uuid,
                    proxy_uuid,
                    host.host,
                    host.port,
                    host.username,
                    password,
                    host.theme_id,
                    startup_snippet_uuid,
                    host.ssh_enabled,
                    host.mosh_enabled,
                    mosh_server_command,
                    host.telnet_enabled,
                    telnet_identity_uuid,
                    host.telnet_username,
                    telnet_password,
                    host.telnet_port,
                    host.telnet_theme_id,
                    environment_variables,
                    encoding,
                    telnet_encoding,
                    host.host_type.as_str(),
                    key_uuid,
                    host.shell_path,
                    host.work_dir,
                    host.os,
                    host.distro,
                    id
                ],
            )?;
            replace_host_tags(&self.connection, id, &host.tag_uuids)?;
            return Ok(id);
        }

        let uuid = uuid_or_new(&self.connection, host.uuid.as_deref())?;
        self.connection.execute(
            r#"
            INSERT INTO hosts (
              uuid, name, group_uuid, identity_uuid, proxy_uuid, host, port, username, password,
              theme_id, startup_snippet_uuid, ssh_enabled, mosh_enabled, mosh_server_command, telnet_enabled,
              telnet_identity_uuid, telnet_username, telnet_password,
              telnet_port, telnet_theme_id, environment_variables, encoding, telnet_encoding,
	              type, key_uuid, shell_path, work_dir, os, distro,
	              created_at, updated_at
	            )
	            VALUES (
	              ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
	              CAST(unixepoch('subsec') * 1000 AS INTEGER),
	              CAST(unixepoch('subsec') * 1000 AS INTEGER)
            )
            "#,
            params![
                uuid,
                host.name,
                group_uuid,
                identity_uuid,
                proxy_uuid,
                host.host,
                host.port,
                host.username,
                password,
                host.theme_id,
                startup_snippet_uuid,
                host.ssh_enabled,
                host.mosh_enabled,
                mosh_server_command,
                host.telnet_enabled,
                telnet_identity_uuid,
                host.telnet_username,
                telnet_password,
                host.telnet_port,
                host.telnet_theme_id,
                environment_variables,
                encoding,
                telnet_encoding,
                host.host_type.as_str(),
                key_uuid,
                host.shell_path,
                host.work_dir,
                host.os,
                host.distro
            ],
        )?;
        let id = self.connection.last_insert_rowid();
        replace_host_tags(&self.connection, id, &host.tag_uuids)?;
        Ok(id)
    }

    pub fn get_host(&self, id: i64) -> rusqlite::Result<Option<HostEntry>> {
        let dek = configured_dek(&self.connection)?;
        let sql = host_select_sql("WHERE hosts.id = ? AND hosts.deleted_at IS NULL");
        let value = self
            .connection
            .query_row(&sql, params![id], host_from_row)
            .optional()?;
        value
            .map(|mut host| {
                host.password = decrypt_optional_field(dek.as_ref(), host.password)?;
                host.telnet_password = decrypt_optional_field(dek.as_ref(), host.telnet_password)?;
                Ok(host)
            })
            .transpose()
    }

    pub fn list_hosts(&self, group_id: Option<i64>) -> rusqlite::Result<Vec<HostEntry>> {
        if let Some(group_id) = group_id {
            let sql = host_summary_select_sql(
                "WHERE hosts.group_uuid = (SELECT uuid FROM groups WHERE id = ?) AND hosts.deleted_at IS NULL ORDER BY hosts.name COLLATE NOCASE",
            );
            let mut statement = self.connection.prepare(&sql)?;
            let values = statement
                .query_map(params![group_id], host_from_row)?
                .collect();
            return values;
        }

        let sql = host_summary_select_sql(
            "WHERE hosts.deleted_at IS NULL ORDER BY hosts.name COLLATE NOCASE",
        );
        let mut statement = self.connection.prepare(&sql)?;
        let values = statement.query_map([], host_from_row)?.collect();
        values
    }

    pub fn delete_host(&mut self, id: i64) -> rusqlite::Result<usize> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE port_forwards SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE host_uuid = (SELECT uuid FROM hosts WHERE id = ?) AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.execute(
            "DELETE FROM snippet_target_hosts WHERE host_uuid = (SELECT uuid FROM hosts WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE sftp_favorites SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE host_uuid = (SELECT uuid FROM hosts WHERE id = ?) AND deleted_at IS NULL",
            params![id],
        )?;
        let deleted = transaction.execute(
            "UPDATE hosts SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE id = ? AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn save_port_forward(&mut self, port_forward: &PortForwardEntry) -> rusqlite::Result<i64> {
        let forward_type = normalize_port_forward_type(&port_forward.r#type)?;
        let host_uuid = relation_uuid(
            &self.connection,
            "hosts",
            Some(port_forward.connection_id),
            port_forward.host_uuid.as_deref(),
        )?
        .ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("port forward host_uuid is required".to_string())
        })?;
        if let Some(id) = port_forward.id {
            self.connection.execute(
                r#"
                UPDATE port_forwards
                SET name = ?, type = ?, bind_address = ?, bind_port = ?,
                    destination_host = ?, destination_port = ?, host_uuid = ?,
                    updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                    deleted_at = NULL,
                    version = COALESCE(version, 0) + 1
                WHERE id = ?
                "#,
                params![
                    port_forward.name,
                    forward_type,
                    port_forward.bind_address,
                    port_forward.bind_port,
                    port_forward.destination_host,
                    port_forward.destination_port,
                    host_uuid,
                    id
                ],
            )?;
            return Ok(id);
        }

        let uuid = uuid_or_new(&self.connection, port_forward.uuid.as_deref())?;
        self.connection.execute(
            r#"
            INSERT INTO port_forwards (
              name, type, bind_address, bind_port, destination_host,
              destination_port, host_uuid, uuid, created_at, updated_at
            )
            VALUES (
              ?, ?, ?, ?, ?, ?, ?, ?,
              CAST(unixepoch('subsec') * 1000 AS INTEGER),
              CAST(unixepoch('subsec') * 1000 AS INTEGER)
            )
            "#,
            params![
                port_forward.name,
                forward_type,
                port_forward.bind_address,
                port_forward.bind_port,
                port_forward.destination_host,
                port_forward.destination_port,
                host_uuid,
                uuid
            ],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_port_forward(&self, id: i64) -> rusqlite::Result<Option<PortForwardEntry>> {
        self.connection
            .query_row(
                r#"
                SELECT
                  port_forwards.*,
                  COALESCE((SELECT id FROM hosts WHERE hosts.uuid = port_forwards.host_uuid AND hosts.deleted_at IS NULL), 0) AS connection_id
                FROM port_forwards
                WHERE id = ? AND deleted_at IS NULL
                "#,
                params![id],
                port_forward_from_row,
            )
            .optional()
    }

    pub fn list_port_forwards(
        &self,
        connection_id: Option<i64>,
    ) -> rusqlite::Result<Vec<PortForwardEntry>> {
        if let Some(connection_id) = connection_id {
            let mut statement = self.connection.prepare(
                r#"
                SELECT
                  port_forwards.*,
                  COALESCE((SELECT id FROM hosts WHERE hosts.uuid = port_forwards.host_uuid AND hosts.deleted_at IS NULL), 0) AS connection_id
                FROM port_forwards
                WHERE host_uuid = (SELECT uuid FROM hosts WHERE id = ?)
                  AND deleted_at IS NULL
                ORDER BY name COLLATE NOCASE
                "#,
            )?;
            let port_forwards = statement
                .query_map(params![connection_id], port_forward_from_row)?
                .collect();
            return port_forwards;
        }

        let mut statement = self.connection.prepare(
            r#"
            SELECT
              port_forwards.*,
              COALESCE((SELECT id FROM hosts WHERE hosts.uuid = port_forwards.host_uuid AND hosts.deleted_at IS NULL), 0) AS connection_id
            FROM port_forwards
            WHERE deleted_at IS NULL
            ORDER BY name COLLATE NOCASE
            "#,
        )?;
        let port_forwards = statement.query_map([], port_forward_from_row)?.collect();
        port_forwards
    }

    pub fn delete_port_forward(&mut self, id: i64) -> rusqlite::Result<usize> {
        self.connection.execute(
            "UPDATE port_forwards SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE id = ? AND deleted_at IS NULL",
            params![id],
        )
    }

    pub fn save_proxy(&mut self, proxy: &ProxyEntry) -> rusqlite::Result<i64> {
        let proxy_type = normalize_proxy_type(&proxy.r#type)?;
        let dek = configured_dek(&self.connection)?;
        let password = encrypt_optional_field(dek.as_ref(), proxy.password.as_deref())?;
        let identity_uuid = relation_uuid(
            &self.connection,
            "identities",
            proxy.identity_id,
            proxy.identity_uuid.as_deref(),
        )?;
        if let Some(id) = proxy.id {
            self.connection.execute(
                r#"
                UPDATE proxies
                SET name = ?, type = ?, host = ?, port = ?, identity_uuid = ?,
                    username = ?, password = ?,
                    updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                    deleted_at = NULL,
                    version = COALESCE(version, 0) + 1
                WHERE id = ?
                "#,
                params![
                    proxy.name,
                    proxy_type,
                    proxy.host,
                    proxy.port,
                    identity_uuid,
                    proxy.username,
                    password,
                    id
                ],
            )?;
            return Ok(id);
        }

        let uuid = uuid_or_new(&self.connection, proxy.uuid.as_deref())?;
        self.connection.execute(
            r#"
            INSERT INTO proxies (
              name, type, host, port, identity_uuid, username, password,
              uuid, created_at, updated_at
            )
            VALUES (
              ?, ?, ?, ?, ?, ?, ?, ?,
              CAST(unixepoch('subsec') * 1000 AS INTEGER),
              CAST(unixepoch('subsec') * 1000 AS INTEGER)
            )
            "#,
            params![
                proxy.name,
                proxy_type,
                proxy.host,
                proxy.port,
                identity_uuid,
                proxy.username,
                password,
                uuid
            ],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_proxy(&self, id: i64) -> rusqlite::Result<Option<ProxyEntry>> {
        let dek = configured_dek(&self.connection)?;
        let value = self.connection
            .query_row(
                r#"
                SELECT
                  proxies.*,
                  (SELECT id FROM identities WHERE identities.uuid = proxies.identity_uuid AND identities.deleted_at IS NULL) AS identity_id
                FROM proxies
                WHERE id = ? AND deleted_at IS NULL
                "#,
                params![id],
                proxy_from_row,
            )
            .optional()?;
        value
            .map(|mut proxy| {
                proxy.password = decrypt_optional_field(dek.as_ref(), proxy.password)?;
                Ok(proxy)
            })
            .transpose()
    }

    pub fn list_proxies(&self) -> rusqlite::Result<Vec<ProxyEntry>> {
        let mut statement = self.connection.prepare(
            r#"
            SELECT
              proxies.id,
              proxies.uuid,
              proxies.name,
              proxies.type,
              proxies.host,
              proxies.port,
              proxies.identity_uuid,
              proxies.username,
              NULL AS password,
              proxies.created_at,
              proxies.updated_at,
              proxies.deleted_at,
              proxies.version,
              proxies.created_device_id,
              proxies.updated_device_id,
              (SELECT id FROM identities WHERE identities.uuid = proxies.identity_uuid AND identities.deleted_at IS NULL) AS identity_id
            FROM proxies
            WHERE deleted_at IS NULL
            ORDER BY name COLLATE NOCASE
            "#,
        )?;
        let proxies = statement.query_map([], proxy_from_row)?.collect();
        proxies
    }

    pub fn delete_proxy(&mut self, id: i64) -> rusqlite::Result<usize> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE hosts SET proxy_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE proxy_uuid = (SELECT uuid FROM proxies WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE groups SET proxy_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE proxy_uuid = (SELECT uuid FROM proxies WHERE id = ?)",
            params![id],
        )?;
        let deleted = transaction.execute(
            "UPDATE proxies SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE id = ? AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }
}
