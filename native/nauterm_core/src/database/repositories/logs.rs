use super::super::*;

const TERMINAL_LOG_SELECT: &str = r"
            SELECT
              terminal_logs.id AS local_id,
              terminal_logs.uuid AS id,
              terminal_logs.title,
              terminal_logs.theme_id,
              terminal_logs.host_uuid,
              terminal_logs.host,
              terminal_logs.port,
              terminal_logs.username,
              terminal_logs.shell_path,
              terminal_logs.work_dir,
              terminal_logs.cwd,
              terminal_logs.capture_file,
              terminal_logs.capture_bytes,
              terminal_logs.capture_sha256,
              terminal_logs.columns,
              terminal_logs.rows,
              terminal_logs.started_at,
              terminal_logs.ended_at,
              (SELECT id FROM hosts WHERE hosts.uuid = terminal_logs.host_uuid AND hosts.deleted_at IS NULL) AS host_id
            FROM terminal_logs
";

impl NautermDatabase {
    pub fn save_terminal_log(
        &mut self,
        log: &TerminalLogEntry,
        events: &[TerminalLogEvent],
    ) -> rusqlite::Result<String> {
        let host_uuid = relation_uuid(
            &self.connection,
            "hosts",
            log.host_id,
            log.host_uuid.as_deref(),
        )?;
        let log_uuid = uuid_or_new(&self.connection, Some(log.id.as_str()))?;
        let transaction = self.connection.savepoint()?;
        transaction.execute(
            r#"
            INSERT INTO terminal_logs (
              uuid, title, theme_id, host_uuid, host, port, username, shell_path,
              work_dir, cwd, capture_file, capture_bytes, capture_sha256,
              columns, rows, started_at, ended_at
            )
            VALUES (
              ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
            )
            ON CONFLICT(uuid) DO UPDATE SET
              title = excluded.title,
              theme_id = excluded.theme_id,
              host_uuid = excluded.host_uuid,
              host = excluded.host,
              port = excluded.port,
              username = excluded.username,
              shell_path = excluded.shell_path,
              work_dir = excluded.work_dir,
              cwd = excluded.cwd,
              capture_file = excluded.capture_file,
              capture_bytes = excluded.capture_bytes,
              capture_sha256 = excluded.capture_sha256,
              columns = excluded.columns,
              rows = excluded.rows,
              started_at = excluded.started_at,
              ended_at = excluded.ended_at
            "#,
            params![
                log_uuid,
                log.title,
                log.theme_id,
                host_uuid,
                log.host,
                log.port,
                log.username,
                log.shell_path,
                log.work_dir,
                log.cwd,
                log.capture_file,
                log.capture_bytes,
                log.capture_sha256,
                log.columns,
                log.rows,
                log.started_at,
                log.ended_at
            ],
        )?;
        transaction.execute(
            "DELETE FROM terminal_log_events WHERE log_uuid = ?",
            params![log_uuid],
        )?;
        for event in events {
            transaction.execute(
                r#"
                INSERT INTO terminal_log_events (
                  log_uuid, timestamp, type, message, connection_kind, data
                )
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
                params![
                    event
                        .log_uuid
                        .as_ref()
                        .or(event.log_id.as_ref())
                        .unwrap_or(&log_uuid),
                    event.timestamp,
                    event.event_type,
                    event.message,
                    event.connection_kind,
                    event.data
                ],
            )?;
        }
        transaction.commit()?;
        Ok(log_uuid)
    }

    pub fn list_terminal_logs(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> rusqlite::Result<Vec<TerminalLogEntry>> {
        let limit = limit.unwrap_or(80).clamp(1, 500);
        let offset = offset.unwrap_or(0).max(0);
        let mut statement = self.connection.prepare(&format!(
            "{TERMINAL_LOG_SELECT} ORDER BY started_at DESC, id DESC LIMIT ? OFFSET ?"
        ))?;
        let logs = statement
            .query_map(params![limit, offset], terminal_log_from_row)?
            .collect();
        logs
    }

    pub fn get_terminal_log(&self, log_id: &str) -> rusqlite::Result<Option<TerminalLogEntry>> {
        self.connection
            .query_row(
                &format!("{TERMINAL_LOG_SELECT} WHERE terminal_logs.uuid = ?"),
                [log_id],
                terminal_log_from_row,
            )
            .optional()
    }

    pub fn list_terminal_log_events(
        &self,
        log_id: &str,
    ) -> rusqlite::Result<Vec<TerminalLogEvent>> {
        let mut statement = self.connection.prepare(
            r#"
            SELECT
              terminal_log_events.*,
              log_uuid AS log_id
            FROM terminal_log_events
            WHERE log_uuid = ?
            ORDER BY timestamp ASC, id ASC
            "#,
        )?;
        let events = statement
            .query_map(params![log_id], terminal_log_event_from_row)?
            .collect();
        events
    }

    pub fn delete_terminal_log(&mut self, log_id: &str) -> rusqlite::Result<Option<String>> {
        let capture_file = self
            .connection
            .query_row(
                "SELECT capture_file FROM terminal_logs WHERE uuid = ?",
                params![log_id],
                |row| row.get(0),
            )
            .optional()?;
        let transaction = self.connection.savepoint()?;
        transaction.execute(
            "DELETE FROM terminal_log_events WHERE log_uuid = ?",
            params![log_id],
        )?;
        transaction.execute("DELETE FROM terminal_logs WHERE uuid = ?", params![log_id])?;
        transaction.commit()?;
        Ok(capture_file)
    }

    pub fn clear_terminal_logs(&mut self) -> rusqlite::Result<Vec<String>> {
        let capture_files = self.list_terminal_capture_files()?;
        let transaction = self.connection.savepoint()?;
        transaction.execute("DELETE FROM terminal_log_events", [])?;
        transaction.execute("DELETE FROM terminal_logs", [])?;
        transaction.commit()?;
        Ok(capture_files)
    }

    pub fn delete_terminal_logs(&mut self, log_ids: &[String]) -> rusqlite::Result<Vec<String>> {
        self.atomic(|database| {
            let mut files = vec![];
            for id in log_ids {
                if let Some(file) = database.delete_terminal_log(id)? {
                    files.push(file);
                }
            }
            Ok(files)
        })
    }

    pub fn list_terminal_capture_files(&self) -> rusqlite::Result<Vec<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT capture_file FROM terminal_logs WHERE capture_file <> ''")?;
        let capture_files = statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(capture_files)
    }

    pub fn list_incomplete_terminal_captures(&self) -> rusqlite::Result<Vec<TerminalLogEntry>> {
        let mut statement = self.connection.prepare(
            r#"
            SELECT
              terminal_logs.id AS local_id,
              terminal_logs.uuid AS id,
              terminal_logs.title,
              terminal_logs.theme_id,
              terminal_logs.host_uuid,
              terminal_logs.host,
              terminal_logs.port,
              terminal_logs.username,
              terminal_logs.shell_path,
              terminal_logs.work_dir,
              terminal_logs.cwd,
              terminal_logs.capture_file,
              terminal_logs.capture_bytes,
              terminal_logs.capture_sha256,
              terminal_logs.columns,
              terminal_logs.rows,
              terminal_logs.started_at,
              terminal_logs.ended_at,
              (SELECT id FROM hosts WHERE hosts.uuid = terminal_logs.host_uuid AND hosts.deleted_at IS NULL) AS host_id
            FROM terminal_logs
            WHERE capture_file LIKE '%.ntrcap'
              AND capture_file <> ''
              AND capture_sha256 IS NULL
            ORDER BY started_at ASC
            "#,
        )?;
        let logs = statement
            .query_map([], terminal_log_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(logs)
    }

    pub fn finalize_recovered_terminal_capture(
        &mut self,
        log_id: &str,
        capture_bytes: i64,
        capture_sha256: &str,
        ended_at: i64,
    ) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE terminal_logs SET capture_bytes = ?, capture_sha256 = ?, \
             ended_at = COALESCE(ended_at, ?) WHERE uuid = ? \
             AND capture_sha256 IS NULL",
            params![capture_bytes, capture_sha256, ended_at, log_id],
        )?;
        Ok(())
    }

    pub fn clear_missing_terminal_capture(&mut self, log_id: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE terminal_logs SET capture_file = '', capture_bytes = 0, \
             capture_sha256 = NULL WHERE uuid = ? AND capture_sha256 IS NULL",
            params![log_id],
        )?;
        Ok(())
    }
}
