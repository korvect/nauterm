use super::*;

fn serial_host() -> HostEntry {
    serde_json::from_value(json!({
        "name": "Console", "type": "serial",
        "serial_port": "/dev/cu.test-console", "serial_baud_rate": 9600,
        "config": {
            "serial": {"data_bits": 7, "stop_bits": 2, "parity": "even", "flow_control": "hardware", "future_option": true},
            "ssh": {"compatibility_mode": true}
        }
    })).unwrap()
}

#[test]
fn saved_serial_host_survives_reopen_edit_duplicate_and_delete() {
    let path = std::env::temp_dir().join(format!(
        "nauterm-serial-host-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let original = serial_host();
    let id = {
        let mut database = NautermDatabase::open(&path).unwrap();
        database.save_host(&original).unwrap()
    };
    let mut database = NautermDatabase::open(&path).unwrap();
    let mut saved = database.get_host(id).unwrap().unwrap();
    assert_eq!(saved.host_type, HostType::Serial);
    assert_eq!(saved.serial_port, original.serial_port);
    assert_eq!(saved.serial_baud_rate, original.serial_baud_rate);
    assert_eq!(saved.config, original.config);
    let summary = database.list_hosts(None).unwrap().pop().unwrap();
    assert_eq!(summary.config, original.config);
    assert_eq!(summary.serial_port, original.serial_port);
    saved.serial_baud_rate = Some(115200);
    database.save_host(&saved).unwrap();
    assert_eq!(
        database.get_host(id).unwrap().unwrap().serial_baud_rate,
        Some(115200)
    );
    saved.id = None;
    saved.uuid = None;
    let copy_id = database.save_host(&saved).unwrap();
    assert_ne!(id, copy_id);
    assert_eq!(
        database.get_host(copy_id).unwrap().unwrap().config,
        original.config
    );
    database.delete_host(id).unwrap();
    assert!(database.get_host(id).unwrap().is_none());
    assert_eq!(database.list_hosts(None).unwrap().len(), 1);
    drop(database);
    let _ = std::fs::remove_file(path);
}

#[test]
fn serial_host_rejects_invalid_connection_settings() {
    let mut database = NautermDatabase::open_in_memory().unwrap();
    let mut host = serial_host();
    host.serial_port = Some(" ".into());
    assert!(database.save_host(&host).is_err());
    host = serial_host();
    host.serial_baud_rate = Some(0);
    assert!(database.save_host(&host).is_err());
    for (field, value) in [
        ("data_bits", json!(9)),
        ("stop_bits", json!(3)),
        ("parity", json!("invalid")),
        ("flow_control", json!("invalid")),
    ] {
        host = serial_host();
        host.config.get_mut("serial").unwrap()[field] = value;
        assert!(
            database.save_host(&host).is_err(),
            "accepted invalid {field}"
        );
    }
}

#[test]
fn schema_five_hosts_receive_empty_extensible_config() {
    let mut database = NautermDatabase::open_in_memory().unwrap();
    let host: HostEntry = serde_json::from_value(
        json!({"name": "Existing", "type": "remote", "host": "example.test"}),
    )
    .unwrap();
    let id = database.save_host(&host).unwrap();
    database
        .connection
        .execute_batch(
            "ALTER TABLE hosts DROP COLUMN serial_port;
        ALTER TABLE hosts DROP COLUMN serial_baud_rate; ALTER TABLE hosts DROP COLUMN config;
        PRAGMA user_version = 5;",
        )
        .unwrap();
    migrations::migrate_schema(&mut database.connection, 5).unwrap();
    let saved = database.get_host(id).unwrap().unwrap();
    assert_eq!(saved.name, "Existing");
    assert!(saved.config.is_empty());
    assert_eq!(saved.serial_port, None);
    assert_eq!(database.schema_version().unwrap(), SCHEMA_VERSION);
}

#[test]
fn extensible_remote_host_config_syncs_and_serial_hosts_remain_local() {
    let path = std::env::temp_dir().join(format!(
        "nauterm-host-config-sync-{}-{}.enc",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut source = NautermDatabase::open_in_memory().unwrap();
    let host: HostEntry = serde_json::from_value(json!({"name": "Remote", "type": "remote", "host": "example.test", "config": {"ssh": {"compatibility_mode": true}, "future": [1,2]}})).unwrap();
    source.save_host(&host).unwrap();
    source.save_host(&serial_host()).unwrap();
    source
        .sync_local_file(path.to_str().unwrap(), "Serial host sync key 1!")
        .unwrap();
    let mut target = NautermDatabase::open_in_memory().unwrap();
    target
        .sync_local_file(path.to_str().unwrap(), "Serial host sync key 1!")
        .unwrap();
    let hosts = target.list_hosts(None).unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].name, "Remote");
    assert_eq!(hosts[0].config, host.config);
    let _ = std::fs::remove_file(path);
}
