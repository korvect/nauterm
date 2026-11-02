//! Local-only path targets. Parsing never evaluates shell syntax or runs commands.
use crate::{
    native::Frame,
    terminal_search::{Selection, viewport_start},
};
use regex::Regex;
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

static PROMPTS: LazyLock<[Regex; 3]> = LazyLock::new(|| {
    [
        Regex::new(r"^\[.*\s([~/][^\]]*)\][#$%>](?:\s|$)").unwrap(),
        Regex::new(r":(~|/[^$%#>]*?)\s*[#$%>](?:\s|$)").unwrap(),
        Regex::new(r"^PS\s+(.+?)>(?:\s|$)").unwrap(),
    ]
});
static LISTING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[bcdlps-][rwxStTs-]{9}[+@.]?\s+\d+\s+(?:\S+\s+){6}(.+?)\s*$").unwrap()
});
static LINE_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?):\d+(?::\d+)?$").unwrap());
static WINDOWS_DRIVE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z]:[\\/]").unwrap());

pub struct Context<'a> {
    pub command_directory: Option<&'a str>,
    pub command_prompt: Option<&'a str>,
    pub current_directory: Option<&'a str>,
    pub home: Option<&'a Path>,
    pub process_directory: Option<&'a Path>,
}
pub struct LocalTarget {
    pub path: PathBuf,
    pub selection: Selection,
}
struct Row {
    text: String,
    /// Each cell's byte boundary, including wide-character spacer cells.
    boundaries: Vec<usize>,
    spacers: Vec<bool>,
}
impl Row {
    fn read(frame: &Frame, row: u32) -> Option<Self> {
        let start = row as usize * frame.columns as usize;
        let cells = frame.cells.get(start..start + frame.columns as usize)?;
        let mut text = String::new();
        let mut boundaries = Vec::with_capacity(cells.len() + 1);
        let spacers = cells
            .iter()
            .map(|cell| cell.flags & (0x40 | 0x400) != 0)
            .collect();
        for cell in cells {
            boundaries.push(text.len());
            if cell.flags & (0x40 | 0x400) == 0 {
                text.push_str(&cell.text);
            }
        }
        boundaries.push(text.len());
        Some(Self {
            text,
            boundaries,
            spacers,
        })
    }
    fn cells(&self, start: usize, end: usize) -> (usize, usize) {
        let column = |at| {
            self.boundaries
                .iter()
                .position(|byte| *byte >= at)
                .unwrap_or(self.boundaries.len() - 1)
        };
        (column(start), column(end))
    }
}
fn expand(value: &str, home: Option<&Path>) -> Option<PathBuf> {
    if value.chars().any(char::is_control) {
        return None;
    }
    if value == "~" {
        return home.map(Path::to_path_buf);
    }
    if let Some(relative) = value.strip_prefix("~/") {
        return home.map(|home| home.join(relative));
    }
    Some(PathBuf::from(value))
}
fn prompt_directory(line: &str, home: Option<&Path>) -> Option<PathBuf> {
    let value = PROMPTS
        .iter()
        .find_map(|regex| regex.captures(line).and_then(|c| c.get(1)))?;
    expand(value.as_str().trim_end(), home)
}
fn resolve(frame: &Frame, row: u32, value: &str, context: &Context<'_>) -> Option<PathBuf> {
    let value = if WINDOWS_DRIVE.is_match(value) {
        value
    } else {
        LINE_SUFFIX
            .captures(value)
            .and_then(|c| c.get(1))
            .map_or(value, |m| m.as_str())
    };
    let path = expand(value, context.home)?;
    if path.is_absolute() {
        return Some(path);
    }
    let base = context
        .command_directory
        .and_then(|dir| expand(dir, context.home))
        .or_else(|| {
            context
                .command_prompt
                .and_then(|line| prompt_directory(line, context.home))
        })
        .or_else(|| {
            (0..=row).rev().find_map(|row| {
                Row::read(frame, row).and_then(|line| prompt_directory(&line.text, context.home))
            })
        })
        .or_else(|| {
            context
                .current_directory
                .and_then(|dir| expand(dir, context.home))
        })
        .or_else(|| context.process_directory.map(Path::to_path_buf))?;
    base.is_absolute().then(|| base.join(path))
}
fn decode(value: &str) -> String {
    let mut value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('\'') && value.ends_with('\''))
            || (value.starts_with('"') && value.ends_with('"')))
    {
        value = &value[1..value.len() - 1];
    }
    let mut chars = value.chars();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            result.push(chars.next().unwrap_or(ch));
        } else {
            result.push(ch);
        }
    }
    result
}

pub fn target_at(
    frame: &Frame,
    (column, row): (u32, u32),
    context: &Context<'_>,
    exists: impl Fn(&Path) -> bool,
) -> Option<LocalTarget> {
    if column >= frame.columns || row >= frame.rows {
        return None;
    }
    let line = Row::read(frame, row)?;
    let column = column as usize;
    let selection = |start, end| {
        let row_start = viewport_start(frame.columns, frame.history_lines, frame.display_offset)
            + i64::from(row) * i64::from(frame.columns);
        Selection {
            start: row_start + start as i64,
            end: row_start + end as i64,
        }
    };
    // Prompt paths and long listings take precedence over shorter space runs.
    if let Some(value) = PROMPTS
        .iter()
        .find_map(|regex| regex.captures(&line.text).and_then(|c| c.get(1)))
    {
        let (start, end) = line.cells(value.start(), value.end());
        if (start..end).contains(&column)
            && let Some(path) = expand(value.as_str().trim_end(), context.home)
            && path.is_absolute()
            && exists(&path)
        {
            return Some(LocalTarget {
                path,
                selection: selection(start, end),
            });
        }
    }
    if let Some(value) = LISTING.captures(&line.text).and_then(|c| c.get(1)) {
        let name = value.as_str().split(" -> ").next()?.trim_end();
        let (start, end) = line.cells(value.start(), value.start() + name.len());
        if (start..end).contains(&column)
            && let Some(path) = resolve(frame, row, &decode(name), context)
            && exists(&path)
        {
            return Some(LocalTarget {
                path,
                selection: selection(start, end),
            });
        }
    }
    let mut runs = Vec::new();
    let mut start = None;
    for at in 0..=frame.columns as usize {
        let blank = at == frame.columns as usize || {
            let text = &line.text[line.boundaries[at]..line.boundaries[at + 1]];
            !line.spacers[at] && text.trim().is_empty()
        };
        if blank {
            if let Some(start) = start.take() {
                runs.push((start, at));
            }
        } else if start.is_none() {
            start = Some(at);
        }
    }
    let clicked = runs
        .iter()
        .position(|&(start, end)| (start..end).contains(&column))?;
    let mut candidates = Vec::new();
    for first in clicked.saturating_sub(7)..=clicked {
        for last in clicked..runs.len().min(first + 8) {
            candidates.push((last - first + 1, runs[first].0, runs[last].1));
        }
    }
    candidates.sort_by_key(|(count, _, _)| *count);
    for (_, start, end) in candidates {
        let raw = &line.text[line.boundaries[start]..line.boundaries[end]];
        if raw.chars().count() > 256 {
            continue;
        }
        let value = decode(raw);
        if value.is_empty() || value.starts_with('-') {
            continue;
        }
        if let Some(path) = resolve(frame, row, &value, context)
            && exists(&path)
        {
            return Some(LocalTarget {
                path,
                selection: selection(start, end),
            });
        }
    }
    // Match the reference's punctuation-trimmed final token fallback.
    let mut start = column;
    let mut end = column + 1;
    let separator = |at: usize| {
        let value = &line.text[line.boundaries[at]..line.boundaries[at + 1]];
        !line.spacers[at] && (value.trim().is_empty() || "\"'`|".contains(value))
    };
    while start > 0 && !separator(start - 1) {
        start -= 1;
    }
    while end < frame.columns as usize && !separator(end) {
        end += 1;
    }
    while start < end
        && "([{<".contains(&line.text[line.boundaries[start]..line.boundaries[start + 1]])
    {
        start += 1;
    }
    while end > start
        && ".,;:)]}>".contains(&line.text[line.boundaries[end - 1]..line.boundaries[end]])
    {
        end -= 1;
    }
    if !(start..end).contains(&column) {
        return None;
    }
    let value = &line.text[line.boundaries[start]..line.boundaries[end]];
    let path = resolve(frame, row, value, context)?;
    exists(&path).then(|| LocalTarget {
        path,
        selection: selection(start, end),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(lines: &[&str]) -> Frame {
        let columns = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(1)
            + 2;
        let mut cells = Vec::new();
        for line in lines {
            let mut chars = line.chars();
            for _ in 0..columns {
                cells.push(crate::native::Cell {
                    text: chars.next().map(|c| c.to_string()).unwrap_or_default(),
                    hyperlink: None,
                    flags: 0,
                    foreground: 0,
                    background: 0,
                });
            }
        }
        Frame {
            columns: columns as u32,
            rows: lines.len() as u32,
            cells,
            history_lines: 50,
            display_offset: 10,
            ..Default::default()
        }
    }
    fn context<'a>(command: Option<&'a str>, cwd: Option<&'a str>, home: &'a Path) -> Context<'a> {
        Context {
            command_directory: command,
            command_prompt: None,
            current_directory: cwd,
            home: Some(home),
            process_directory: Some(home),
        }
    }
    #[test]
    fn relative_line_suffix_quotes_and_escapes_use_historical_command_directory()
    -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let old = root.path().join("old");
        let current = root.path().join("current");
        let context = context(old.to_str(), current.to_str(), root.path());
        let expected = old.join("src/main.rs");
        let sample = frame(&["src/main.rs:42:8"]);
        let target = target_at(&sample, (3, 0), &context, |path| path == expected).unwrap();
        assert_eq!(target.path, expected);
        assert_eq!(target.selection.start, 40 * i64::from(sample.columns));
        for text in [
            "'report final.txt'",
            "report\\ final.txt",
            "report final.txt",
        ] {
            let sample = frame(&[text]);
            let expected = old.join("report final.txt");
            assert_eq!(
                target_at(&sample, (10, 0), &context, |path| path == expected)
                    .unwrap()
                    .path,
                expected
            );
        }
        Ok(())
    }
    #[test]
    fn visible_prompts_long_listings_and_tilde_keep_full_names() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let old = root.path().join("old");
        let prompt = format!("PS {}> ls -l", old.display());
        let listing = "-rw-r--r-- 1 user staff 10 Oct 23 10:00 报告 final.txt";
        let sample = frame(&[&prompt, listing]);
        let column = listing[..listing.find("报告").unwrap()].chars().count() as u32 + 1;
        let expected = old.join("报告 final.txt");
        let context = context(None, None, root.path());
        let target = target_at(&sample, (column, 1), &context, |path| path == expected).unwrap();
        assert_eq!(target.path, expected);
        assert_eq!(
            target.selection.end - target.selection.start,
            "报告 final.txt".chars().count() as i64
        );
        let sample = frame(&["[user@host ~/with spaces]$ "]);
        let expected = root.path().join("with spaces");
        assert_eq!(
            target_at(&sample, (15, 0), &context, |path| path == expected)
                .unwrap()
                .path,
            expected
        );
        Ok(())
    }
    #[test]
    fn existing_files_and_directories_are_detected_without_running_shell_text() -> anyhow::Result<()>
    {
        let root = tempfile::tempdir()?;
        let file = root.path().join("a space.txt");
        std::fs::write(&file, b"synthetic")?;
        std::fs::create_dir(root.path().join("folder"))?;
        let context = context(None, root.path().to_str(), root.path());
        for (text, expected) in [
            ("a space.txt", file),
            ("folder", root.path().join("folder")),
        ] {
            assert_eq!(
                target_at(&frame(&[text]), (0, 0), &context, Path::exists)
                    .unwrap()
                    .path,
                expected
            );
        }
        assert!(target_at(&frame(&["missing.txt"]), (0, 0), &context, Path::exists).is_none());
        assert!(target_at(&frame(&["$(touch marker)"]), (3, 0), &context, Path::exists).is_none());
        assert!(!root.path().join("marker").exists());
        Ok(())
    }
}
