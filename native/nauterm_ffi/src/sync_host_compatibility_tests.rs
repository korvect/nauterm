use super::*;

const EXISTING_HOST: &str = "01979f62-8548-7000-8000-000000000301";
const NEW_HOST: &str = "01979f62-8548-7000-8000-000000000302";
const SERIAL_HOST: &str = "01979f62-8548-7000-8000-000000000303";

fn local_config() -> Value {
    json!({"icon": "cisco", "ssh": {"compatibility_mode": true}, "future": [1, 2]})
}

fn remote_config() -> Value {
    json!({"icon": "huawei", "ssh": {"compatibility_mode": false}, "future": {"enabled": true}})
}

fn host_payload(schema: i32) -> SyncPayload {
    let records = [EXISTING_HOST, NEW_HOST]
        .map(|uuid| {
            // Keep the historical shape independent of today's export columns.
            let mut record: SyncRecord = serde_json::from_value(json!({
                "uuid": uuid, "name": "Remote host", "group_uuid": null,
                "identity_uuid": null, "proxy_uuid": null, "host": "example.test",
                "port": 22, "username": "admin", "password": null, "theme_id": null,
                "startup_snippet_uuid": null, "ssh_enabled": 1, "mosh_enabled": 0,
                "mosh_server_command": null, "telnet_enabled": 0,
                "telnet_identity_uuid": null, "telnet_username": null,
                "telnet_password": null, "telnet_port": null, "telnet_theme_id": null,
                "environment_variables": null, "encoding": null, "telnet_encoding": null,
                "type": "remote", "key_uuid": null, "shell_path": null, "work_dir": null,
                "created_at": 1, "updated_at": unix_millis() + 60_000,
                "deleted_at": null, "version": 2,
                "created_device_id": "01979f62-8548-7000-8000-000000000102",
                "updated_device_id": "01979f62-8548-7000-8000-000000000102",
                "tag_uuids": "[]"
            }))
            .unwrap();
            if schema >= 5 {
                record.insert("dark_theme_id".into(), Value::Null);
                record.insert("telnet_dark_theme_id".into(), Value::Null);
            }
            if schema >= 6 {
                record.insert("serial_port".into(), Value::Null);
                record.insert("serial_baud_rate".into(), Value::Null);
                record.insert("config".into(), json!(remote_config().to_string()));
            }
            record
        })
        .to_vec();
    SyncPayload {
        format_version: PAYLOAD_FORMAT_VERSION,
        exported_at: 42,
        source_device_id: "01979f62-8548-7000-8000-000000000102".into(),
        tables: BTreeMap::from([("hosts".into(), records)]),
    }
}

fn temporary_host_snapshot() -> TemporarySyncFile {
    TemporarySyncFile(std::env::temp_dir().join(format!(
        "nauterm-host-compat-{}-{}.enc",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    )))
}

#[test]
fn all_schemas_and_strategies_sync_host_config_and_keep_serial_local() {
    let sync_key = [11u8; 32];
    let seed = encrypt_payload(host_payload(7), Some(MASTER_KEY), 7, None, &sync_key).unwrap();
    for schema in 1..=7 {
        for strategy in [
            SyncStrategy::SmartMerge,
            SyncStrategy::LocalWins,
            SyncStrategy::RemoteWins,
        ] {
            let mut connection = complete_connection_with_key();
            connection
                .execute_batch(
                    "PRAGMA user_version = 7;
                     CREATE UNIQUE INDEX hosts_uuid ON hosts(uuid);
                     ALTER TABLE host_tags ADD COLUMN tag_uuid TEXT;",
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO hosts(uuid, name, type, updated_at, version, config)
                 VALUES (?, 'Local host', 'remote', 10, 1, ?)",
                    params![EXISTING_HOST, local_config().to_string()],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO hosts(uuid, name, type, serial_port, serial_baud_rate, config)
                 VALUES (?, 'Serial host', 'serial', '/dev/cu.test-console', 115200, ?)",
                    params![SERIAL_HOST, json!({"serial": {"data_bits": 7}}).to_string()],
                )
                .unwrap();
            let envelope = encrypt_payload(
                host_payload(schema),
                None,
                schema,
                Some(seed.header.clone()),
                &sync_key,
            )
            .unwrap();
            save_local_sync_state(&connection, &envelope.header, &sync_key).unwrap();
            let original_header = envelope.header.clone();
            let file = temporary_host_snapshot();
            fs::write(&file.0, serde_json::to_vec(&envelope).unwrap()).unwrap();
            sync_file(
                &mut connection,
                file.0.to_str().unwrap(),
                None,
                None,
                SyncFileKind::LinkedLocal,
                strategy,
            )
            .unwrap();

            let (name, config): (String, String) = connection
                .query_row(
                    "SELECT name, config FROM hosts WHERE uuid = ?",
                    [EXISTING_HOST],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            let expected_config = match strategy {
                SyncStrategy::LocalWins => local_config(),
                SyncStrategy::SmartMerge if schema < 6 => local_config(),
                SyncStrategy::RemoteWins if schema < 6 => json!({}),
                _ => remote_config(),
            };
            assert_eq!(
                name,
                if strategy == SyncStrategy::LocalWins {
                    "Local host"
                } else {
                    "Remote host"
                }
            );
            assert_eq!(
                serde_json::from_str::<Value>(&config).unwrap(),
                expected_config
            );
            let new_config: Option<String> = connection
                .query_row(
                    "SELECT config FROM hosts WHERE uuid = ?",
                    [NEW_HOST],
                    |row| row.get(0),
                )
                .optional()
                .unwrap();
            assert_eq!(
                new_config.map(|config| serde_json::from_str::<Value>(&config).unwrap()),
                (strategy != SyncStrategy::LocalWins).then(|| if schema < 6 {
                    json!({})
                } else {
                    remote_config()
                }),
            );
            let serial: (String, i64, String) = connection
                .query_row(
                    "SELECT serial_port, serial_baud_rate, config FROM hosts WHERE uuid = ?",
                    [SERIAL_HOST],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(
                serial,
                (
                    "/dev/cu.test-console".into(),
                    115200,
                    json!({"serial": {"data_bits": 7}}).to_string()
                )
            );

            let written: SyncEnvelope =
                serde_json::from_slice(&fs::read(&file.0).unwrap()).unwrap();
            assert_eq!(written.header.schema_version, 7);
            assert_eq!(written.header.vault_id, original_header.vault_id);
            assert_eq!(written.header.revision, original_header.revision + 1);
            let (_, payload, _) = decrypt_envelope(written, None, Some(&sync_key), 7).unwrap();
            let hosts = &payload.tables["hosts"];
            assert!(hosts.iter().all(|host| host["type"] == "remote"));
            assert_eq!(
                hosts.len(),
                if strategy == SyncStrategy::LocalWins {
                    1
                } else {
                    2
                }
            );
            let host = hosts
                .iter()
                .find(|host| host["uuid"] == EXISTING_HOST)
                .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(host["config"].as_str().unwrap()).unwrap(),
                expected_config
            );
        }
    }
}

#[test]
fn legacy_host_snapshots_import_into_current_database() {
    use crate::database::{HostEntry, NautermDatabase};

    let sync_key = [12u8; 32];
    let seed = encrypt_payload(host_payload(7), Some(MASTER_KEY), 7, None, &sync_key).unwrap();
    for schema in 1..=7 {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let existing: HostEntry = serde_json::from_value(json!({
            "uuid": EXISTING_HOST, "name": "Local host", "type": "remote",
            "host": "local.test", "config": local_config()
        }))
        .unwrap();
        let id = database.save_host(&existing).unwrap();
        let serial: HostEntry = serde_json::from_value(json!({
            "uuid": SERIAL_HOST, "name": "Serial host", "type": "serial",
            "serial_port": "/dev/cu.test-console", "serial_baud_rate": 115200,
            "config": {"serial": {"data_bits": 7}}
        }))
        .unwrap();
        let serial_id = database.save_host(&serial).unwrap();
        let envelope = encrypt_payload(
            host_payload(schema),
            None,
            schema,
            Some(seed.header.clone()),
            &sync_key,
        )
        .unwrap();
        let file = temporary_host_snapshot();
        fs::write(&file.0, serde_json::to_vec(&envelope).unwrap()).unwrap();
        database
            .sync_local_file(file.0.to_str().unwrap(), MASTER_KEY)
            .unwrap();
        let restored = database.get_host(id).unwrap().unwrap();
        assert_eq!(restored.name, "Remote host");
        assert_eq!(
            Value::Object(restored.config),
            if schema < 6 {
                local_config()
            } else {
                remote_config()
            }
        );
        let hosts = database.list_hosts(None).unwrap();
        let new_host = hosts
            .iter()
            .find(|host| host.uuid.as_deref() == Some(NEW_HOST))
            .unwrap();
        assert_eq!(
            Value::Object(new_host.config.clone()),
            if schema < 6 {
                json!({})
            } else {
                remote_config()
            }
        );
        let restored_serial = database.get_host(serial_id).unwrap().unwrap();
        assert_eq!(restored_serial.config, serial.config);
        assert_eq!(restored_serial.serial_port, serial.serial_port);
        assert_eq!(restored_serial.serial_baud_rate, serial.serial_baud_rate);

        if schema == 7 {
            let written: SyncEnvelope =
                serde_json::from_slice(&fs::read(&file.0).unwrap()).unwrap();
            let mut cleared = host_payload(schema);
            let record = &mut cleared.tables.get_mut("hosts").unwrap()[0];
            record.insert("config".into(), json!("{}"));
            record.insert("version".into(), json!(3));
            let envelope =
                encrypt_payload(cleared, None, schema, Some(written.header), &sync_key).unwrap();
            fs::write(&file.0, serde_json::to_vec(&envelope).unwrap()).unwrap();
            database
                .sync_local_file(file.0.to_str().unwrap(), MASTER_KEY)
                .unwrap();
            assert!(database.get_host(id).unwrap().unwrap().config.is_empty());
        }
    }
}
