//! Both task panels use the reference's ordering and task metadata.
use crate::transfer_queue::{Kind, Snapshot, Status};
use std::collections::HashMap;

pub(super) fn queue_positions(jobs: &[Snapshot]) -> HashMap<u64, usize> {
    let mut queued: Vec<_> = jobs
        .iter()
        .filter(|job| job.status == Status::Queued)
        .collect();
    queued.sort_by(|a, b| a.oldest_cmp(b));
    queued
        .into_iter()
        .enumerate()
        .map(|(index, job)| (job.id, index + 1))
        .collect()
}
pub(super) fn primary_path(job: &Snapshot) -> &str {
    let path = if matches!(job.kind, Kind::Upload | Kind::Delete) {
        &job.source
    } else {
        &job.target
    };
    if path.trim().is_empty() {
        &job.progress.path
    } else {
        path
    }
}
pub(super) fn subtitle(
    job: &Snapshot,
    position: Option<usize>,
    language: crate::localization::Language,
) -> String {
    let timestamp = chrono::DateTime::from_timestamp_millis(job.created_at)
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_default();
    let status = match job.status {
        Status::Queued => {
            position.map_or_else(|| "queued".into(), |index| format!("queued #{index}"))
        }
        Status::Running => "running".into(),
        Status::Pausing => "pausing".into(),
        Status::Cancelling => "cancelling".into(),
        Status::Paused => "paused".into(),
        Status::Completed => "completed".into(),
        Status::Failed => "failed".into(),
        Status::Cancelled => "cancelled".into(),
    };
    let status = crate::localization::text_for(language, &status);
    let details = if job.status == Status::Failed {
        job.error
            .as_deref()
            .filter(|error| !error.trim().is_empty())
            .map(|error| format!(": {}", crate::localization::message_for(language, error)))
            .unwrap_or_default()
    } else if job.progress.total > 0 {
        format!(
            "  {} / {}",
            size(job.progress.bytes),
            size(job.progress.total)
        )
    } else if job.progress.bytes > 0 {
        format!("  {}", size(job.progress.bytes))
    } else {
        String::new()
    };
    format!("{timestamp}  {status}{details}")
        .trim_start()
        .into()
}
fn size(bytes: u64) -> String {
    // Match Flutter task subtitles, independently of file-table compact sizes.
    let units = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024. && unit < units.len() - 1 {
        size /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", units[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn job(id: u64, status: Status, created_at: i64, finished_at: Option<i64>) -> Snapshot {
        Snapshot {
            id,
            uuid: format!("task-{id}"),
            history_id: None,
            owner: 0,
            kind: Kind::Upload,
            created_at,
            finished_at,
            label: "Upload".into(),
            source: "/local/file".into(),
            target: "/remote/file".into(),
            local_target: None,
            status,
            pausable: true,
            resumable: true,
            permission_retry: false,
            attempt: 0,
            progress: Default::default(),
            error: None,
            refresh_owner: false,
        }
    }
    #[test]
    fn localization_formats_task_status_without_translating_file_paths() {
        let task = job(1, Status::Queued, 0, None);
        assert!(
            subtitle(&task, Some(2), crate::localization::Language::Chinese).contains("已排队 #2")
        );
        assert_eq!(primary_path(&task), "/local/file");
        let mut failed = task;
        failed.status = Status::Failed;
        failed.error = Some("Failed to export key: /tmp/{2}".into());
        assert!(
            subtitle(&failed, None, crate::localization::Language::Chinese)
                .ends_with("失败: 导出密钥失败：/tmp/{2}")
        );
    }
    #[test]
    fn running_then_fifo_queue_then_recent_outcomes_with_stable_positions() {
        let mut jobs = vec![
            job(1, Status::Completed, 1, Some(20)),
            job(2, Status::Queued, 2, None),
            job(3, Status::Paused, 3, None),
            job(4, Status::Failed, 4, Some(40)),
            job(5, Status::Running, 5, None),
            job(6, Status::Queued, 6, None),
            job(7, Status::Pausing, 7, None),
            job(8, Status::Cancelled, 8, Some(30)),
        ];
        jobs.sort_by(Snapshot::compare);
        assert_eq!(
            jobs.iter().map(|job| job.id).collect::<Vec<_>>(),
            [5, 7, 2, 3, 6, 4, 8, 1]
        );
        assert_eq!(queue_positions(&jobs), HashMap::from([(2, 1), (6, 2)]));
        jobs.reverse();
        jobs.sort_by(Snapshot::compare);
        assert_eq!(
            jobs.iter().map(|job| job.id).collect::<Vec<_>>(),
            [5, 7, 2, 3, 6, 4, 8, 1]
        );
        let mut old = job((1 << 63) | 2, Status::Completed, 10, Some(20));
        old.history_id = Some(2);
        let mut tied = [old, job(3, Status::Completed, 10, Some(20))];
        tied.sort_by(Snapshot::compare);
        assert_eq!(tied[0].id, 3);
    }
    #[test]
    fn restored_tasks_keep_size_timestamp_and_source_target_semantics() {
        let mut task = job(
            1,
            Status::Completed,
            1_750_000_000_000,
            Some(1_750_000_010_000),
        );
        task.history_id = Some(45);
        task.pausable = false;
        task.resumable = false;
        task.progress.bytes = 1024;
        task.progress.total = 2048;
        assert_eq!(primary_path(&task), "/local/file");
        assert!(
            subtitle(&task, None, crate::localization::Language::English)
                .ends_with("completed  1.0 KB / 2.0 KB")
        );
        for kind in [Kind::Download, Kind::Edit, Kind::Move, Kind::Copy] {
            task.kind = kind;
            assert_eq!(primary_path(&task), "/remote/file");
        }
        task.status = Status::Queued;
        assert!(
            subtitle(&task, Some(2), crate::localization::Language::English).contains("queued #2")
        );
        task.status = Status::Failed;
        task.error = Some("permission denied".into());
        assert!(
            subtitle(&task, None, crate::localization::Language::English)
                .ends_with("failed: permission denied")
        );
        assert!(
            subtitle(&task, None, crate::localization::Language::English).starts_with(
                &chrono::DateTime::from_timestamp_millis(task.created_at)
                    .unwrap()
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            )
        );
    }
}
