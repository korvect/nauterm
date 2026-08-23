use super::super::*;

impl NautermDatabase {
    pub fn save_ai_provider(
        &mut self,
        provider: &AiProviderEntry,
    ) -> rusqlite::Result<AiProviderEntry> {
        let protocol = normalize_ai_provider_protocol(&provider.protocol)?;
        let name = provider.name.trim();
        if name.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "AI provider name is required".to_string(),
            ));
        }
        let base_url = provider.base_url.trim();
        if base_url.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "AI provider base URL is required".to_string(),
            ));
        }
        let mut config = provider.config.clone();
        if let Some(max_tokens) = config.get("max_tokens").and_then(Value::as_i64) {
            if max_tokens <= 0 {
                return Err(rusqlite::Error::InvalidParameterName(
                    "AI provider max_tokens must be greater than zero".to_string(),
                ));
            }
            config.insert("max_tokens".to_string(), json!(max_tokens));
        } else {
            config.remove("max_tokens");
        }
        let config = serde_json::to_string(&config)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let transaction = self.connection.transaction()?;
        if provider.active {
            transaction.execute("UPDATE ai_providers SET active = 0 WHERE active != 0", [])?;
        }

        let id = if let Some(id) = provider.id {
            transaction.execute(
                r#"
                UPDATE ai_providers
                SET name = ?, protocol = ?, base_url = ?, model = ?,
                    api_key = ?, config = ?, active = ?,
                    updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
                WHERE id = ?
                "#,
                params![
                    name,
                    protocol,
                    base_url,
                    provider.model.trim(),
                    provider.api_key,
                    config,
                    provider.active,
                    id
                ],
            )?;
            id
        } else {
            let uuid = uuid_or_new(&transaction, provider.uuid.as_deref())?;
            transaction.execute(
                r#"
                INSERT INTO ai_providers (
                  uuid, name, protocol, base_url, model, api_key,
                  config, active, created_at, updated_at
                )
                VALUES (
                  ?, ?, ?, ?, ?, ?, ?, ?,
                  CAST(unixepoch('subsec') * 1000 AS INTEGER),
                  CAST(unixepoch('subsec') * 1000 AS INTEGER)
                )
                "#,
                params![
                    uuid,
                    name,
                    protocol,
                    base_url,
                    provider.model.trim(),
                    provider.api_key,
                    config,
                    provider.active
                ],
            )?;
            transaction.last_insert_rowid()
        };
        transaction.commit()?;
        self.get_ai_provider(id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn get_ai_provider(&self, id: i64) -> rusqlite::Result<Option<AiProviderEntry>> {
        self.connection
            .query_row(
                "SELECT * FROM ai_providers WHERE id = ?",
                params![id],
                ai_provider_from_row,
            )
            .optional()
    }

    pub fn get_active_ai_provider(&self) -> rusqlite::Result<Option<AiProviderEntry>> {
        self.connection
            .query_row(
                "SELECT * FROM ai_providers WHERE active != 0 ORDER BY updated_at DESC, id DESC LIMIT 1",
                [],
                ai_provider_from_row,
            )
            .optional()
    }

    pub fn list_ai_providers(&self) -> rusqlite::Result<Vec<AiProviderEntry>> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM ai_providers ORDER BY active DESC, name COLLATE NOCASE, id")?;
        let result = statement.query_map([], ai_provider_from_row)?.collect();
        result
    }

    pub fn delete_ai_provider(&mut self, id: i64) -> rusqlite::Result<usize> {
        self.connection
            .execute("DELETE FROM ai_providers WHERE id = ?", params![id])
    }

    pub fn save_ai_conversation(
        &mut self,
        conversation: &AiConversationEntry,
    ) -> rusqlite::Result<AiConversationEntry> {
        let scope = normalize_ai_conversation_scope(&conversation.scope)?;
        let transaction = self.connection.transaction()?;
        let conversation_uuid = uuid_or_new(&transaction, conversation.uuid.as_deref())?;
        transaction.execute(
            r#"
            INSERT INTO ai_conversations (
              uuid, title, scope, host_uuid, provider_uuid, model
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(uuid) DO UPDATE SET
              title = excluded.title,
              scope = excluded.scope,
              host_uuid = excluded.host_uuid,
              provider_uuid = excluded.provider_uuid,
              model = excluded.model,
              updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
            "#,
            params![
                conversation_uuid,
                conversation.title,
                scope,
                conversation.host_uuid,
                conversation.provider_uuid,
                conversation.model,
            ],
        )?;

        let mut message_uuids = BTreeSet::new();
        for message in &conversation.messages {
            let role = normalize_ai_message_role(&message.role)?;
            let message_uuid = uuid_or_new(&transaction, message.uuid.as_deref())?;
            let tool_calls = serde_json::to_string(&message.tool_calls)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            let tool_result = message
                .tool_result
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            let attachments = serde_json::to_string(&message.attachments)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            transaction.execute(
                r#"
                INSERT INTO ai_messages (
                  uuid, conversation_uuid, role, content, context, sequence,
                  tool_calls, tool_result, attachments
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(uuid) DO UPDATE SET
                  conversation_uuid = excluded.conversation_uuid,
                  role = excluded.role,
                  content = excluded.content,
                  context = excluded.context,
                  sequence = excluded.sequence,
                  tool_calls = excluded.tool_calls,
                  tool_result = excluded.tool_result,
                  attachments = excluded.attachments,
                  updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
                "#,
                params![
                    message_uuid,
                    conversation_uuid,
                    role,
                    message.content,
                    message.context,
                    message.sequence,
                    tool_calls,
                    tool_result,
                    attachments,
                ],
            )?;
            message_uuids.insert(message_uuid);
        }
        delete_missing_ai_children(
            &transaction,
            "ai_messages",
            &conversation_uuid,
            &message_uuids,
        )?;

        let mut command_uuids = BTreeSet::new();
        for block in &conversation.command_blocks {
            let status = normalize_ai_command_status(&block.status)?;
            let block_uuid = uuid_or_new(&transaction, block.uuid.as_deref())?;
            transaction.execute(
                r#"
                INSERT INTO ai_command_blocks (
                  uuid, conversation_uuid, tool_call_id, command, explanation,
                  status, sequence, output, exit_code, error, started_at, finished_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(uuid) DO UPDATE SET
                  conversation_uuid = excluded.conversation_uuid,
                  tool_call_id = excluded.tool_call_id,
                  command = excluded.command,
                  explanation = excluded.explanation,
                  status = excluded.status,
                  sequence = excluded.sequence,
                  output = excluded.output,
                  exit_code = excluded.exit_code,
                  error = excluded.error,
                  started_at = excluded.started_at,
                  finished_at = excluded.finished_at,
                  updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
                "#,
                params![
                    block_uuid,
                    conversation_uuid,
                    block.tool_call_id,
                    block.command,
                    block.explanation,
                    status,
                    block.sequence,
                    block.output,
                    block.exit_code,
                    block.error,
                    block.started_at,
                    block.finished_at,
                ],
            )?;
            command_uuids.insert(block_uuid);
        }
        delete_missing_ai_children(
            &transaction,
            "ai_command_blocks",
            &conversation_uuid,
            &command_uuids,
        )?;
        transaction.commit()?;
        self.get_ai_conversation(&conversation_uuid)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn get_ai_conversation(&self, uuid: &str) -> rusqlite::Result<Option<AiConversationEntry>> {
        let conversation = self
            .connection
            .query_row(
                "SELECT * FROM ai_conversations WHERE uuid = ?",
                params![uuid],
                ai_conversation_from_row,
            )
            .optional()?;
        let Some(mut conversation) = conversation else {
            return Ok(None);
        };
        conversation.messages = self.list_ai_messages(uuid)?;
        conversation.command_blocks = self.list_ai_command_blocks(uuid)?;
        Ok(Some(conversation))
    }

    pub fn list_ai_conversations(
        &self,
        scope: Option<&str>,
        host_uuid: Option<&str>,
        limit: Option<i64>,
    ) -> rusqlite::Result<Vec<AiConversationEntry>> {
        let mut statement = self.connection.prepare(
            r#"
            SELECT
              ai_conversations.*,
              (
                SELECT content FROM ai_messages
                WHERE conversation_uuid = ai_conversations.uuid
                  AND role = 'user'
                  AND trim(content) != ''
                ORDER BY sequence ASC, id ASC
                LIMIT 1
              ) AS preview
            FROM ai_conversations
            WHERE (?1 IS NULL OR ai_conversations.scope = ?1)
              AND (?2 IS NULL OR ai_conversations.host_uuid = ?2)
            ORDER BY ai_conversations.updated_at DESC, ai_conversations.id DESC
            LIMIT ?3
            "#,
        )?;
        let conversations = statement
            .query_map(
                params![scope, host_uuid, limit.unwrap_or(100).clamp(1, 1000)],
                ai_conversation_summary_from_row,
            )?
            .collect();
        conversations
    }

    pub fn delete_ai_conversation(&mut self, uuid: &str) -> rusqlite::Result<usize> {
        self.connection
            .execute("DELETE FROM ai_conversations WHERE uuid = ?", params![uuid])
    }

    fn list_ai_messages(&self, conversation_uuid: &str) -> rusqlite::Result<Vec<AiMessageEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT * FROM ai_messages WHERE conversation_uuid = ? ORDER BY sequence ASC, id ASC",
        )?;
        let messages = statement
            .query_map(params![conversation_uuid], ai_message_from_row)?
            .collect();
        messages
    }

    fn list_ai_command_blocks(
        &self,
        conversation_uuid: &str,
    ) -> rusqlite::Result<Vec<AiCommandBlockEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT * FROM ai_command_blocks WHERE conversation_uuid = ? ORDER BY sequence ASC, id ASC",
        )?;
        let command_blocks = statement
            .query_map(params![conversation_uuid], ai_command_block_from_row)?
            .collect();
        command_blocks
    }
}
