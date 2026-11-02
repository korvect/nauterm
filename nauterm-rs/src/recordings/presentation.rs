//! Reference log-row labels, independent of GPUI and database access.
use super::Entry;
use crate::management::SortOrder;
use chrono::{DateTime, Local, NaiveDateTime};

/// Build the table's public index only when its data/query/order changes.
/// Scroll repainting can retain the same catalog and index without cloning or
/// sorting every record; visible rows still resolve against that exact catalog.
pub(super) fn visible_indices(entries: &[Entry], query: &str, order: SortOrder) -> Vec<usize> {
    let mut visible: Vec<_> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| entry.matches(query).then_some(index))
        .collect();
    visible.sort_by(|left, right| {
        let (a, b) = (&entries[*left], &entries[*right]);
        match order {
            SortOrder::NewestFirst => b
                .started_at
                .cmp(&a.started_at)
                .then_with(|| b.id.cmp(&a.id)),
            SortOrder::OldestFirst => a
                .started_at
                .cmp(&b.started_at)
                .then_with(|| a.id.cmp(&b.id)),
            order => order
                .compare(
                    (&a.title, a.started_at as u64),
                    (&b.title, b.started_at as u64),
                )
                .then_with(|| a.id.cmp(&b.id)),
        }
    });
    visible
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Local,
    Remote,
    Serial,
}
fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
pub(super) fn kind(entry: &Entry) -> Kind {
    let title = entry.title.trim();
    let com = title.to_ascii_uppercase();
    if title.starts_with("/dev/")
        || title.starts_with(r"\\.\COM")
        || com
            .strip_prefix("COM")
            .is_some_and(|port| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
    {
        Kind::Serial
    } else if nonempty(entry.host.as_deref()).is_some() {
        Kind::Remote
    } else {
        Kind::Local
    }
}
pub(super) fn account(entry: &Entry) -> (String, String) {
    (
        nonempty(entry.username.as_deref())
            .unwrap_or("Local")
            .into(),
        nonempty(entry.host.as_deref())
            .map(str::to_owned)
            .unwrap_or_else(super::local_hostname),
    )
}
pub(super) fn subtitle(entry: &Entry) -> String {
    match kind(entry) {
        Kind::Serial => "Serial device".into(),
        // Keep the actual endpoint: a remote log can also be Telnet or Mosh,
        // and the shared recording schema does not identify its transport.
        Kind::Remote => entry.target(),
        Kind::Local => [
            entry.cwd.as_deref(),
            entry.work_dir.as_deref(),
            entry.shell_path.as_deref(),
        ]
        .into_iter()
        .find_map(nonempty)
        .unwrap_or("local")
        .into(),
    }
}
fn time_range(
    start: NaiveDateTime,
    end: NaiveDateTime,
    language: crate::localization::Language,
) -> String {
    let days = end.date().signed_duration_since(start.date()).num_days();
    let suffix = if days > 0 {
        crate::localization::format_for(language, " (+{days}d)", &[("days", &days.to_string())])
            .to_string()
    } else {
        String::new()
    };
    format!(
        "{} - {}{suffix}",
        start.format("%H:%M"),
        end.format("%H:%M")
    )
}
pub(super) fn date_range(
    entry: &Entry,
    language: crate::localization::Language,
) -> (String, String) {
    let Some(start) = DateTime::from_timestamp_millis(entry.started_at) else {
        return (
            crate::localization::text_for(language, "Unknown date").to_string(),
            String::new(),
        );
    };
    let start = start.with_timezone(&Local).naive_local();
    let end = entry
        .ended_at
        .and_then(DateTime::from_timestamp_millis)
        .map(|end| end.with_timezone(&Local).naive_local())
        .unwrap_or_else(|| Local::now().naive_local());
    (
        start
            .format(&crate::localization::text_for(
                language,
                "rs.format.logDate",
            ))
            .to_string(),
        time_range(start, end, language),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn table_indices_preserve_search_sort_ties_and_record_identity() {
        let entries: Vec<_> = [
            ("older-b", "beta", 100, "fixture.invalid", "ADMIN"),
            ("new", "ALPHA", 200, "other.invalid", "root"),
            ("older-a", "alpha", 100, "fixture.invalid", "admin"),
            ("new-b", "alpha", 200, "fixture.invalid", "admin"),
        ]
        .into_iter()
        .map(|(id, title, started_at, host, username)| Entry {
            id: id.into(),
            title: title.into(),
            started_at,
            host: Some(host.into()),
            username: Some(username.into()),
            ..Default::default()
        })
        .collect();
        for (order, expected) in [
            (SortOrder::NewestFirst, [3, 1, 0, 2]),
            (SortOrder::OldestFirst, [2, 0, 1, 3]),
            (SortOrder::NameAscending, [2, 1, 3, 0]),
            (SortOrder::NameDescending, [0, 1, 3, 2]),
        ] {
            assert_eq!(visible_indices(&entries, "", order), expected);
        }
        let selected = visible_indices(&entries, "FIXTURE admin", SortOrder::NewestFirst);
        assert_eq!(selected, [3, 0, 2]);
        assert_eq!(
            selected
                .iter()
                .map(|index| entries[*index].id.as_str())
                .collect::<Vec<_>>(),
            ["new-b", "older-b", "older-a"]
        );
        assert!(visible_indices(&entries, "missing", SortOrder::NewestFirst).is_empty());
    }
    #[test]
    fn date_ranges_show_calendar_day_crossings_without_padding_the_day() {
        let at = |day, hour, minute| {
            chrono::NaiveDate::from_ymd_opt(2026, 10, day)
                .unwrap()
                .and_hms_opt(hour, minute, 0)
                .unwrap()
        };
        assert_eq!(
            time_range(
                at(23, 22, 3),
                at(24, 2, 54),
                crate::localization::Language::English
            ),
            "22:03 - 02:54 (+1d)"
        );
        assert_eq!(
            time_range(
                at(23, 22, 3),
                at(25, 22, 2),
                crate::localization::Language::English
            ),
            "22:03 - 22:02 (+2d)"
        );
        assert_eq!(
            time_range(
                at(23, 2, 3),
                at(23, 5, 8),
                crate::localization::Language::English
            ),
            "02:03 - 05:08"
        );
        assert_eq!(
            time_range(
                at(23, 22, 3),
                at(24, 2, 54),
                crate::localization::Language::Chinese
            ),
            "22:03 - 02:54（+1 天）"
        );
        assert_eq!(at(3, 2, 3).format("%b %-d, %Y").to_string(), "Oct 3, 2026");
    }
    #[test]
    fn local_and_serial_rows_use_real_metadata_instead_of_an_ssh_or_shell_fallback() {
        let mut entry = Entry {
            title: "Local Terminal".into(),
            cwd: Some(" /work/current ".into()),
            work_dir: Some("/work/start".into()),
            shell_path: Some("/bin/zsh".into()),
            ..Default::default()
        };
        assert!(kind(&entry) == Kind::Local);
        assert_eq!(subtitle(&entry), "/work/current");
        entry.cwd = Some(" ".into());
        assert_eq!(subtitle(&entry), "/work/start");
        for title in ["/dev/ttyUSB0", "com12", r"\\.\COM3"] {
            entry.title = title.into();
            assert!(kind(&entry) == Kind::Serial);
            assert_eq!(subtitle(&entry), "Serial device");
        }
        entry.title = "compute".into();
        entry.host = Some("fixture.invalid".into());
        entry.port = Some(2022);
        entry.username = Some(" fixture ".into());
        assert!(kind(&entry) == Kind::Remote);
        assert_eq!(
            account(&entry),
            ("fixture".into(), "fixture.invalid".into())
        );
        entry.username = Some("fixture".into());
        assert_eq!(subtitle(&entry), "fixture@fixture.invalid:2022");
    }
}
