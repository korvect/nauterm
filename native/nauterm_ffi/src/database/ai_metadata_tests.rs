use super::*;

fn conversation() -> AiConversationEntry {
    serde_json::from_value(json!({
        "title": "Inspect directory", "scope": "terminal",
        "messages": [{"role": "assistant", "content": "Checking.", "sequence": 0}]
    }))
    .unwrap()
}

#[test]
fn ai_message_provider_metadata_roundtrips_and_updates() {
    let mut database = NautermDatabase::open_in_memory().unwrap();
    let mut conversation = conversation();
    let metadata = json!({
        "google": {"parts": [
            {"text": "Checking."},
            {"functionCall": {"id": "call-1", "name": "run_terminal_command", "args": {"command": "pwd"}}, "thoughtSignature": "c2lnbmF0dXJl"},
            {"text": "", "thoughtSignature": "dGFpbA=="}
        ]},
        "future_protocol": {"opaque": [1, 2]}
    });
    conversation.messages[0].provider_metadata = metadata.as_object().unwrap().clone();
    let mut saved = database.save_ai_conversation(&conversation).unwrap();
    let restored = database
        .get_ai_conversation(saved.uuid.as_deref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        restored.messages[0].provider_metadata,
        metadata.as_object().unwrap().clone()
    );
    saved.messages[0]
        .provider_metadata
        .get_mut("google")
        .unwrap()["parts"] = json!([
        {"text": "Done.", "thoughtSignature": "bmV3"}
    ]);
    let updated = database.save_ai_conversation(&saved).unwrap();
    assert_eq!(updated.messages[0].uuid, saved.messages[0].uuid);
    assert_eq!(
        updated.messages[0].provider_metadata,
        saved.messages[0].provider_metadata
    );
}

#[test]
fn schema_six_messages_receive_empty_provider_metadata() {
    let mut database = NautermDatabase::open_in_memory().unwrap();
    let saved = database.save_ai_conversation(&conversation()).unwrap();
    database
        .connection
        .execute_batch(
            "ALTER TABLE ai_messages DROP COLUMN provider_metadata; PRAGMA user_version = 6;",
        )
        .unwrap();
    NautermDatabase::ensure_schema(&mut database.connection).unwrap();
    let restored = database
        .get_ai_conversation(saved.uuid.as_deref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(database.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(restored.messages, saved.messages);
    assert!(restored.messages[0].provider_metadata.is_empty());
    NautermDatabase::ensure_schema(&mut database.connection).unwrap();
}

#[test]
fn ai_message_provider_metadata_requires_a_json_object() {
    let mut database = NautermDatabase::open_in_memory().unwrap();
    database.save_ai_conversation(&conversation()).unwrap();
    for invalid in ["[]", "null", "1", "not-json"] {
        assert!(database
            .connection
            .execute(
                "UPDATE ai_messages SET provider_metadata = ?",
                params![invalid]
            )
            .is_err());
    }
}
