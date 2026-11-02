//! Completed-session retention, matching the reference policy. Active output
//! counts toward capacity but its log/file is never a deletion candidate.
use super::*;

struct Plan {
    deleted: BTreeSet<String>,
    oversized: BTreeSet<String>,
}
fn plan(entries: &[Entry], active: &BTreeSet<String>, config: Preferences, now: i64) -> Plan {
    let cutoff = now.saturating_sub((config.retention_days * 86_400_000) as i64);
    let completed: Vec<_> = entries
        .iter()
        .filter(|entry| entry.ended_at.is_some() && !active.contains(&entry.id))
        .collect();
    let mut deleted: BTreeSet<_> = completed
        .iter()
        .filter(|entry| entry.ended_at.unwrap() < cutoff)
        .map(|entry| entry.id.clone())
        .collect();
    let mut retained: Vec<_> = completed
        .into_iter()
        .filter(|entry| !deleted.contains(&entry.id))
        .collect();
    let oversized: BTreeSet<_> = retained
        .iter()
        .filter(|entry| {
            !entry.capture_file.is_empty() && entry.capture_bytes > config.max_session_bytes
        })
        .map(|entry| entry.id.clone())
        .collect();
    let effective_bytes = |entry: &Entry| {
        if oversized.contains(&entry.id) {
            config.max_session_bytes
        } else {
            entry.capture_bytes
        }
    };
    let mut total = entries
        .iter()
        .filter(|entry| !deleted.contains(&entry.id))
        .fold(0u64, |total, entry| {
            total.saturating_add(effective_bytes(entry))
        });
    retained.sort_by(|left, right| {
        left.ended_at
            .cmp(&right.ended_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    for entry in retained {
        if total <= config.max_total_bytes {
            break;
        }
        let bytes = effective_bytes(entry);
        if bytes != 0 {
            deleted.insert(entry.id.clone());
            total = total.saturating_sub(bytes);
        }
    }
    Plan { deleted, oversized }
}
impl Runtime {
    pub(super) fn retain(&mut self) -> Result<()> {
        let Some(config) = self.preferences else {
            return Ok(());
        };
        config.validate()?;
        let mut entries = self.entries()?;
        for entry in &mut entries {
            if let Some(active) = self.active.get(&entry.id) {
                *entry = active.entry.clone();
            }
        }
        let plan = plan(
            &entries,
            &self.active.keys().cloned().collect(),
            config,
            now(),
        );
        for entry in &entries {
            if plan.oversized.contains(&entry.id) && !plan.deleted.contains(&entry.id) {
                self.retain_tail(entry, config.max_session_bytes)?;
            }
        }
        let removed: Vec<_> = entries
            .into_iter()
            .filter(|entry| plan.deleted.contains(&entry.id))
            .collect();
        if !removed.is_empty() {
            self.delete(removed)?;
            let _ = self.notify.try_send(());
        }
        self.cleanup_orphans(false)?;
        Ok(())
    }
    fn retain_tail(&mut self, expected: &Entry, limit: u64) -> Result<()> {
        let Some(_lease) = RecordingLease::try_acquire(&self.directory, &expected.id)? else {
            return Ok(());
        };
        let Some(entry) = self.entry(&expected.id)? else {
            return Ok(());
        };
        if entry.ended_at.is_none()
            || entry.capture_file != expected.capture_file
            || entry.capture_sha256 != expected.capture_sha256
        {
            return Ok(());
        }
        let original = self.capture_path(&entry)?;
        let name = format!("{}.{:032x}.ntrcap", entry.id, rand::random::<u128>());
        let destination = managed_path(&self.directory, &entry.id, &name)?;
        let staged = crate::file_commit::DownloadTarget::new_with_prefix(
            &destination,
            false,
            &format!("{}{}-", super::orphans::COMPACTION_PREFIX, entry.id),
        )?;
        let info = self.core.capture.write_tail(
            &original,
            &entry.id,
            limit,
            staged.staged(),
            entry
                .capture_sha256
                .as_deref()
                .context("Recording is not finalized")?,
        )?;
        let events: Vec<Event> =
            self.db(json!({"op":"list_terminal_log_events", "log_id":entry.id}))?;
        staged.publish()?;
        let mut updated = entry.clone();
        updated.capture_file = name;
        updated.capture_bytes = info.ciphertext_bytes;
        updated.capture_sha256 = Some(info.file_sha256);
        // Immutable replacement: metadata publication cannot make the old file
        // unreadable. On database failure both encrypted files are preserved.
        let _: String =
            self.db(json!({"op":"save_terminal_log", "log":updated,"events":events}))?;
        self.cleanup_capture(&entry.id, &entry.capture_file)?;
        let _ = self.notify.try_send(());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, end: Option<i64>, bytes: u64) -> Entry {
        Entry {
            id: id.into(),
            ended_at: end,
            capture_bytes: bytes,
            capture_file: if bytes > 0 {
                "capture".into()
            } else {
                String::new()
            },
            ..Default::default()
        }
    }
    #[test]
    fn retention_expires_by_end_time_trims_before_capacity_and_protects_active() {
        let day = 86_400_000;
        let entries = vec![
            entry("expired", Some(day - 1), 10),
            entry("boundary", Some(day), 0),
            entry("oversized", Some(day + 1), 80),
            entry("new", Some(day + 2), 20),
            entry("active", Some(0), 90),
            entry("unfinished", None, 0),
        ];
        let config = Preferences {
            retention_days: 7,
            max_session_bytes: 30,
            max_total_bytes: 120,
            ..Default::default()
        };
        let plan = plan(
            &entries,
            &BTreeSet::from(["active".into()]),
            config,
            8 * day,
        );
        assert_eq!(plan.oversized, BTreeSet::from(["oversized".into()]));
        assert_eq!(
            plan.deleted,
            BTreeSet::from(["expired".into(), "oversized".into()])
        );
        assert!(
            !plan.deleted.contains("boundary"),
            "Retention cutoff is strict and zero-byte logs are not evicted for capacity"
        );
        let plan = super::plan(
            &entries,
            &BTreeSet::from(["active".into()]),
            Preferences {
                max_total_bytes: 1,
                ..config
            },
            8 * day,
        );
        assert!(!plan.deleted.contains("active") && !plan.deleted.contains("unfinished"));
    }
}
