//! Retained command records for UI frames; output is copied only after mutation.
use nauterm_core::AiCommandBlockEntry;
use std::{collections::HashMap, sync::Arc};

pub(super) type CommandRecords = Arc<[Arc<AiCommandBlockEntry>]>;

#[derive(Default)]
pub(super) struct Commands {
    records: CommandRecords,
    current: bool,
}
impl Commands {
    pub fn invalidate(&mut self) {
        self.current = false;
    }
    pub fn snapshot(&mut self, source: &[AiCommandBlockEntry]) -> CommandRecords {
        if !self.current {
            let previous: HashMap<_, _> = self
                .records
                .iter()
                .map(|record| (record.tool_call_id.as_str(), record))
                .collect();
            let mut records: Vec<_> = source
                .iter()
                .map(|record| {
                    previous
                        .get(record.tool_call_id.as_str())
                        .copied()
                        .filter(|cached| cached.as_ref() == record)
                        .map_or_else(|| Arc::new(record.clone()), Arc::clone)
                })
                .collect();
            records.sort_by_key(|record| record.sequence);
            self.records = records.into();
            self.current = true;
        }
        self.records.clone()
    }
}

pub(super) fn between(
    records: &[Arc<AiCommandBlockEntry>],
    after: i64,
    before: i64,
) -> &[Arc<AiCommandBlockEntry>] {
    if before <= after {
        return &[];
    }
    let start = records.partition_point(|record| record.sequence <= after);
    let end = records.partition_point(|record| record.sequence < before);
    &records[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, sequence: i64, status: &str) -> AiCommandBlockEntry {
        serde_json::from_value(serde_json::json!({
            "tool_call_id": id, "sequence": sequence, "command": "pwd",
            "explanation": "Read the current directory", "status": status
        }))
        .unwrap()
    }

    #[test]
    fn redraws_reuse_output_and_another_command_does_not_replace_it() {
        let mut source = vec![
            record("done", 2, "succeeded"),
            record("pending", 5, "pending"),
        ];
        source[0].output = Some("fixture output\n".repeat(20_000));
        let mut commands = Commands::default();
        let first = commands.snapshot(&source);
        assert!(Arc::ptr_eq(&first, &commands.snapshot(&source)));
        commands.invalidate(); // Database normalization may return equal records.
        let normalized = commands.snapshot(&source);
        assert!(Arc::ptr_eq(&first[0], &normalized[0]));
        source[1].status = "running".into();
        commands.invalidate();
        let running = commands.snapshot(&source);
        assert!(Arc::ptr_eq(&first[0], &running[0]));
        assert_eq!(
            first[0].output.as_ref().unwrap().as_ptr(),
            running[0].output.as_ref().unwrap().as_ptr()
        );
        assert!(!Arc::ptr_eq(&first[1], &running[1]));
        assert_eq!(first[1].status, "pending");
        assert_eq!(running[1].status, "running");
    }

    #[test]
    fn command_ranges_follow_sequence_and_replace_edited_or_removed_records() {
        let mut source = vec![
            record("last", 8, "pending"),
            record("first", 2, "pending"),
            record("middle", 3, "pending"),
        ];
        let mut commands = Commands::default();
        let first = commands.snapshot(&source);
        let ids = |rows: &[Arc<AiCommandBlockEntry>]| {
            rows.iter()
                .map(|row| row.tool_call_id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(between(&first, 1, 4)), ["first", "middle"]);
        assert_eq!(ids(between(&first, 3, 9)), ["last"]);
        assert!(between(&first, 2, 3).is_empty());
        assert!(between(&first, 9, 1).is_empty());
        source[1].command = "printf changed".into();
        source.remove(0);
        commands.invalidate();
        let edited = commands.snapshot(&source);
        assert_eq!(ids(&edited), ["first", "middle"]);
        assert!(!Arc::ptr_eq(&first[0], &edited[0]));
        assert!(Arc::ptr_eq(&first[1], &edited[1]));
        assert_eq!(edited[0].command, "printf changed");
        source.clear();
        commands.invalidate();
        assert!(commands.snapshot(&source).is_empty());
    }
}
