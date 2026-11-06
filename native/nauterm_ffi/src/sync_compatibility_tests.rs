use super::*;

const MASTER_KEY: &str = "Legacy sync master key 1!";
const KEY_UUID: &str = "01979f62-8548-7000-8000-000000000101";

fn key_payload(passphrase: Option<Value>) -> SyncPayload {
    let mut record: SyncRecord = serde_json::from_value(serde_json::json!({
        "uuid": KEY_UUID,
        "name": "Remote key",
        "private_key": "encrypted-openssh-key",
        "public_key": "ssh-ed25519 test",
        "certificate": null,
        "created_at": 1,
        "updated_at": 20,
        "deleted_at": null,
        "version": 2,
        "created_device_id": "01979f62-8548-7000-8000-000000000102",
        "updated_device_id": "01979f62-8548-7000-8000-000000000102"
    }))
    .unwrap();
    if let Some(passphrase) = passphrase {
        record.insert("passphrase".to_string(), passphrase);
    }
    SyncPayload {
        format_version: PAYLOAD_FORMAT_VERSION,
        exported_at: 42,
        source_device_id: "01979f62-8548-7000-8000-000000000102".to_string(),
        tables: BTreeMap::from([("keys".to_string(), vec![record])]),
    }
}

fn connection_with_key() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE app_metadata (key TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE keys (
                uuid TEXT PRIMARY KEY, name TEXT, private_key TEXT, public_key TEXT,
                certificate TEXT, passphrase TEXT, created_at INTEGER, updated_at INTEGER,
                deleted_at INTEGER, version INTEGER, created_device_id TEXT, updated_device_id TEXT
             );
             CREATE TABLE groups (uuid TEXT);
             CREATE TABLE identities (uuid TEXT);
             CREATE TABLE proxies (uuid TEXT);
             CREATE TABLE hosts (uuid TEXT, type TEXT);
             CREATE TABLE tags (uuid TEXT);
             CREATE TABLE port_forwards (uuid TEXT, host_uuid TEXT);
             CREATE TABLE snippet_packages (uuid TEXT);
             CREATE TABLE snippets (uuid TEXT);
             CREATE TABLE sftp_favorites (uuid TEXT, scope TEXT);
             CREATE TABLE snippet_target_groups (snippet_uuid TEXT);
             CREATE TABLE snippet_target_hosts (snippet_uuid TEXT);
             CREATE TABLE host_tags (host_uuid TEXT);",
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO keys (uuid, name, passphrase, updated_at, version, updated_device_id)
             VALUES (?, 'Local key', 'saved local passphrase', 10, 1, 'local-device')",
            [KEY_UUID],
        )
        .unwrap();
    connection
}

fn saved_key(connection: &Connection) -> (String, Option<String>) {
    connection
        .query_row(
            "SELECT name, passphrase FROM keys WHERE uuid = ?",
            [KEY_UUID],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

#[test]
fn legacy_encrypted_snapshots_authenticate_and_upgrade_to_schema_four() {
    for schema in 1..=3 {
        let sync_key = [7u8; 32];
        // This is the historical payload shape, encrypted with its original schema in AAD.
        let envelope =
            encrypt_payload(key_payload(None), Some(MASTER_KEY), schema, None, &sync_key).unwrap();
        let original_header = envelope.header.clone();
        let bytes = serde_json::to_vec(&envelope).unwrap();
        assert_eq!(inspect_envelope(&bytes, 4).unwrap().revision, 1);
        assert!(decrypt_envelope(envelope.clone(), Some("Wrong master key 2!"), None, 4).is_err());
        let (header, payload, key) =
            decrypt_envelope(envelope.clone(), Some(MASTER_KEY), None, 4).unwrap();
        assert_eq!(header.schema_version, schema);
        let record = &payload.tables["keys"][0];
        assert_eq!(record["passphrase"], Value::Null);
        validate_record(
            SYNC_TABLES.iter().find(|t| t.name == "keys").unwrap(),
            record,
        )
        .unwrap();

        // Rewriting the authenticated header before decryption must still fail.
        let mut tampered = envelope;
        tampered.header.schema_version = 4;
        assert!(decrypt_envelope(tampered, None, Some(&sync_key), 4).is_err());

        let upgraded = encrypt_payload(payload, None, 4, Some(header), &key).unwrap();
        assert_eq!(upgraded.header.schema_version, 4);
        assert_eq!(upgraded.header.vault_id, original_header.vault_id);
        assert_eq!(upgraded.header.revision, original_header.revision + 1);
        assert_ne!(upgraded.header.snapshot_id, original_header.snapshot_id);
        decrypt_envelope(upgraded, Some(MASTER_KEY), None, 4).unwrap();
    }
}

#[test]
fn smart_merge_preserves_local_passphrase_for_each_legacy_schema() {
    for schema in 1..=3 {
        let mut connection = connection_with_key();
        let mut payload = key_payload(None);
        migrate_sync_payload(&mut payload, schema);
        assert_eq!(merge_payload(&mut connection, &payload, 3).unwrap(), 1);
        assert_eq!(
            saved_key(&connection),
            ("Remote key".into(), Some("saved local passphrase".into()))
        );
    }
}

#[test]
fn legacy_new_keys_default_to_no_saved_passphrase() {
    let mut connection = connection_with_key();
    connection.execute("DELETE FROM keys", []).unwrap();
    let mut payload = key_payload(None);
    migrate_sync_payload(&mut payload, 3);
    merge_payload(&mut connection, &payload, 3).unwrap();
    assert_eq!(saved_key(&connection), ("Remote key".into(), None));
}

#[test]
fn current_schema_explicit_null_clears_saved_passphrase() {
    let mut connection = connection_with_key();
    let payload = key_payload(Some(Value::Null));
    merge_payload(&mut connection, &payload, 5).unwrap();
    assert_eq!(saved_key(&connection), ("Remote key".into(), None));
}

#[test]
fn remote_wins_restores_legacy_snapshot_without_saved_passphrase() {
    let mut connection = connection_with_key();
    let mut payload = key_payload(None);
    migrate_sync_payload(&mut payload, 1);
    replace_payload(&mut connection, &payload).unwrap();
    assert_eq!(saved_key(&connection), ("Remote key".into(), None));
}

#[test]
fn migration_does_not_relax_validation_for_corrupt_records() {
    let mut connection = connection_with_key();
    let mut current_payload = key_payload(None);
    migrate_sync_payload(&mut current_payload, 4);
    assert!(merge_payload(&mut connection, &current_payload, 5).is_err());
    let mut old_payload = key_payload(None);
    old_payload.tables.get_mut("keys").unwrap()[0].remove("public_key");
    migrate_sync_payload(&mut old_payload, 3);
    assert!(merge_payload(&mut connection, &old_payload, 3).is_err());
    let mut unknown_field = key_payload(None);
    unknown_field.tables.get_mut("keys").unwrap()[0].insert("unknown".into(), Value::Null);
    migrate_sync_payload(&mut unknown_field, 3);
    assert!(merge_payload(&mut connection, &unknown_field, 3).is_err());
    assert_eq!(
        saved_key(&connection).1.as_deref(),
        Some("saved local passphrase")
    );
}

#[test]
fn unknown_and_future_schema_versions_remain_unsupported() {
    let mut envelope =
        encrypt_payload(key_payload(None), Some(MASTER_KEY), 3, None, &[7u8; 32]).unwrap();
    for schema in [-1, 0, 5, 40] {
        envelope.header.schema_version = schema;
        let bytes = serde_json::to_vec(&envelope).unwrap();
        assert!(inspect_envelope(&bytes, 4).is_err());
        assert!(decrypt_envelope(envelope.clone(), Some(MASTER_KEY), None, 4).is_err());
    }
    envelope.header.schema_version = 3;
    assert!(inspect_envelope(&serde_json::to_vec(&envelope).unwrap(), 2).is_err());
}

struct TemporarySyncFile(PathBuf);

impl Drop for TemporarySyncFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn all_strategies_read_legacy_files_and_write_current_schema() {
    for schema in 1..=4 {
        for strategy in [
            SyncStrategy::SmartMerge,
            SyncStrategy::LocalWins,
            SyncStrategy::RemoteWins,
        ] {
            let mut connection = complete_connection_with_key();
            let envelope = encrypt_payload(
                key_payload(if schema == 4 { Some(Value::Null) } else { None }),
                Some(MASTER_KEY),
                schema,
                None,
                &[7u8; 32],
            )
            .unwrap();
            let vault_id = envelope.header.vault_id.clone();
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let file = TemporarySyncFile(
                std::env::temp_dir().join(format!("nauterm-schema-{schema}-{suffix}.enc")),
            );
            fs::write(&file.0, serde_json::to_vec(&envelope).unwrap()).unwrap();
            let result = sync_file(
                &mut connection,
                file.0.to_str().unwrap(),
                Some(MASTER_KEY),
                None,
                SyncFileKind::LinkedLocal,
                strategy,
            )
            .unwrap();
            assert_eq!(result.revision, 2);
            assert_eq!(
                result.imported_records,
                usize::from(strategy != SyncStrategy::LocalWins)
            );
            let (name, passphrase) = saved_key(&connection);
            assert_eq!(
                name,
                if strategy == SyncStrategy::LocalWins {
                    "Local key"
                } else {
                    "Remote key"
                }
            );
            assert_eq!(
                passphrase.as_deref(),
                if strategy == SyncStrategy::RemoteWins
                    || (schema == 4 && strategy == SyncStrategy::SmartMerge)
                {
                    None
                } else {
                    Some("saved local passphrase")
                }
            );
            let upgraded: SyncEnvelope =
                serde_json::from_slice(&fs::read(&file.0).unwrap()).unwrap();
            assert_eq!(upgraded.header.schema_version, 5);
            assert_eq!(upgraded.header.vault_id, vault_id);
            let (_, payload, _) = decrypt_envelope(upgraded, Some(MASTER_KEY), None, 5).unwrap();
            assert_eq!(
                payload.tables["keys"][0]["passphrase"].as_str(),
                passphrase.as_deref()
            );
        }
    }
}

fn complete_connection_with_key() -> Connection {
    let connection = connection_with_key();
    // Other sync tables are empty: add their export columns to the minimal merge fixture.
    for table in SYNC_TABLES {
        let mut statement = connection
            .prepare(&format!("PRAGMA table_info({})", table.name))
            .unwrap();
        let columns: BTreeSet<String> = statement
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        drop(statement);
        for column in table.columns {
            if !columns.contains(*column) {
                connection
                    .execute_batch(&format!("ALTER TABLE {} ADD COLUMN {}", table.name, column))
                    .unwrap();
            }
        }
    }
    connection.execute_batch(
        "ALTER TABLE app_metadata ADD COLUMN created_at INTEGER DEFAULT 0;
         ALTER TABLE app_metadata ADD COLUMN updated_at INTEGER DEFAULT 0;
         INSERT INTO app_metadata (key, value) VALUES ('device_id', '01979f62-8548-7000-8000-000000000103');
         PRAGMA user_version = 5;"
    ).unwrap();
    connection
}

#[test]
fn schema_four_dark_theme_defaults_preserve_local_overrides_during_merge() {
    let mut connection = complete_connection_with_key();
    let mut payload = key_payload(Some(Value::Null));
    payload.tables.clear();
    for (name, uuid) in [
        ("groups", "01979f62-8548-7000-8000-000000000201"),
        ("hosts", "01979f62-8548-7000-8000-000000000202"),
    ] {
        connection
            .execute_batch(&format!("CREATE UNIQUE INDEX {name}_uuid ON {name}(uuid)"))
            .unwrap();
        connection.execute(&format!("INSERT INTO {name}(uuid, name, updated_at, version, dark_theme_id, telnet_dark_theme_id) VALUES (?, 'Local', 10, 1, 'saved-ssh-dark', 'saved-telnet-dark')"), [uuid]).unwrap();
        let table = SYNC_TABLES.iter().find(|table| table.name == name).unwrap();
        let mut record: SyncRecord = table
            .columns
            .iter()
            .map(|column| ((*column).to_string(), Value::Null))
            .collect();
        record.remove("dark_theme_id");
        record.remove("telnet_dark_theme_id");
        record.insert("uuid".into(), Value::String(uuid.into()));
        record.insert("name".into(), Value::String("Remote".into()));
        record.insert("type".into(), Value::String("remote".into()));
        if name == "groups" {
            record.remove("type");
        }
        record.insert("created_at".into(), Value::from(1));
        record.insert("updated_at".into(), Value::from(20));
        record.insert("version".into(), Value::from(2));
        payload.tables.insert(name.into(), vec![record]);
    }
    migrate_sync_payload(&mut payload, 4);
    merge_payload(&mut connection, &payload, 4).unwrap();
    for name in ["groups", "hosts"] {
        let values: (String, String, String) = connection
            .query_row(
                &format!("SELECT name, dark_theme_id, telnet_dark_theme_id FROM {name}"),
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            values,
            (
                "Remote".into(),
                "saved-ssh-dark".into(),
                "saved-telnet-dark".into()
            )
        );
    }
    replace_payload(&mut connection, &payload).unwrap();
    for name in ["groups", "hosts"] {
        let dark: Option<String> = connection
            .query_row(&format!("SELECT dark_theme_id FROM {name}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(dark, None);
    }
}
