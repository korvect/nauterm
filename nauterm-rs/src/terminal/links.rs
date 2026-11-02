//! Web targets are passive until an explicit modifier-click is completed.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub url: String,
    pub selection: Selection,
    pub local_path: Option<std::path::PathBuf>,
}
pub(super) struct CachedTarget {
    frame: Arc<Frame>,
    cell: (u32, u32),
    directory: Option<String>,
    target: Option<Target>,
}
fn web_url(value: &str) -> Option<String> {
    if value.chars().any(char::is_control) {
        return None;
    }
    let url = url::Url::parse(value).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.has_host()).then(|| url.to_string())
}
fn spacer(cell: &crate::native::Cell) -> bool {
    cell.flags & (0x40 | 0x400) != 0
}
fn separator(cell: &crate::native::Cell) -> bool {
    !spacer(cell) && (cell.text.trim().is_empty() || "\"'`|".contains(&cell.text))
}
pub fn target_at(frame: &Frame, (column, row): (u32, u32)) -> Option<Target> {
    if column >= frame.columns || row >= frame.rows {
        return None;
    }
    let start = row as usize * frame.columns as usize;
    let cells = frame.cells.get(start..start + frame.columns as usize)?;
    let column = column as usize;
    let selection = |start: usize, end: usize| Selection {
        start: crate::terminal_search::viewport_start(
            frame.columns,
            frame.history_lines,
            frame.display_offset,
        ) + i64::from(row) * i64::from(frame.columns)
            + start as i64,
        end: crate::terminal_search::viewport_start(
            frame.columns,
            frame.history_lines,
            frame.display_offset,
        ) + i64::from(row) * i64::from(frame.columns)
            + end as i64,
    };
    if let Some(link) = &cells[column].hyperlink
        && let Some(url) = web_url(link)
    {
        let mut start = column;
        let mut end = column + 1;
        while start > 0 && cells[start - 1].hyperlink.as_ref() == Some(link) {
            start -= 1;
        }
        while end < cells.len() && cells[end].hyperlink.as_ref() == Some(link) {
            end += 1;
        }
        return Some(Target {
            url,
            selection: selection(start, end),
            local_path: None,
        });
    }
    let mut start = column;
    let mut end = column + 1;
    while start > 0 && !separator(&cells[start - 1]) {
        start -= 1;
    }
    while end < cells.len() && !separator(&cells[end]) {
        end += 1;
    }
    while start < end && "([{<".contains(&cells[start].text) {
        start += 1;
    }
    while end > start && ".,;:)]}>".contains(&cells[end - 1].text) {
        end -= 1;
    }
    if column < start || column >= end || start >= end {
        return None;
    }
    let value = cells[start..end]
        .iter()
        .filter(|cell| !spacer(cell))
        .map(|cell| cell.text.as_str())
        .collect::<String>();
    Some(Target {
        url: web_url(&value)?,
        selection: selection(start, end),
        local_path: None,
    })
}
fn modifier(modifiers: Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.platform
    } else {
        modifiers.control
    }
}
pub(super) fn paint_underline(
    selection: Selection,
    frame: &Frame,
    bounds: Bounds<Pixels>,
    width: Pixels,
    height: Pixels,
    accent: u32,
    window: &mut Window,
) {
    if frame.columns == 0 || frame.rows == 0 {
        return;
    }
    let columns = i64::from(frame.columns);
    let viewport = crate::terminal_search::viewport_start(
        frame.columns,
        frame.history_lines,
        frame.display_offset,
    );
    let length = columns * i64::from(frame.rows);
    let start = (selection.start - viewport).clamp(0, length);
    let end = (selection.end - viewport).clamp(0, length);
    if start >= end {
        return;
    }
    let first = start / columns;
    let last = (end - 1) / columns;
    for row in first..=last {
        let left = if row == first { start % columns } else { 0 };
        let right = if row == last {
            (end - 1) % columns + 1
        } else {
            columns
        };
        window.paint_quad(fill(
            Bounds::new(
                point(
                    bounds.left() + width * left as f32,
                    bounds.top() + height * (row + 1) as f32 - px(1.),
                ),
                size(width * (right - left) as f32, px(1.)),
            ),
            rgb(accent),
        ));
    }
}
impl Terminal {
    fn resolve_link_target(&mut self, cell: (u32, u32)) -> Option<Target> {
        let directory = self.working_directory.value();
        if let Some(cache) = &self.link_cache
            && Arc::ptr_eq(&cache.frame, &self.frame)
            && cache.cell == cell
            && cache.directory == directory
        {
            return cache.target.clone();
        }
        let web = target_at(&self.frame, cell);
        let native_web = self
            .frame
            .cells
            .get((cell.1 * self.frame.columns + cell.0) as usize)
            .and_then(|cell| cell.hyperlink.as_ref())
            .is_some_and(|link| web_url(link).is_some());
        let target = if native_web || !self.session.is_local() {
            web
        } else {
            // Session transport, not a remote-looking title/prompt, determines
            // whether any host filesystem lookup is permitted.
            let home = directories::UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf());
            let process_directory = std::env::current_dir().ok();
            let local = self
                .session
                .local_command_context(self.offset_for_cell(cell), &self.frame)
                .ok()
                .and_then(|command| {
                    let context = super::local_paths::Context {
                        command_directory: command.directory.as_deref(),
                        command_prompt: command.prompt.as_deref(),
                        current_directory: directory.as_deref(),
                        home: home.as_deref(),
                        process_directory: process_directory.as_deref(),
                    };
                    super::local_paths::target_at(&self.frame, cell, &context, |path| path.exists())
                        .and_then(|target| {
                            Some(Target {
                                url: url::Url::from_file_path(&target.path).ok()?.into(),
                                selection: target.selection,
                                local_path: Some(target.path),
                            })
                        })
                });
            local.or(web)
        };
        self.link_cache = Some(CachedTarget {
            frame: self.frame.clone(),
            cell,
            directory,
            target: target.clone(),
        });
        target
    }
    pub(super) fn update_link_hover(
        &mut self,
        position: Point<Pixels>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let target = (self.appearance.command_click_opens_filename_or_url
            && self.bounds.contains(&position)
            && modifier(modifiers))
        .then(|| self.resolve_link_target(self.viewport_cell(position)))
        .flatten();
        if self.hovered_link != target {
            self.hovered_link = target;
            cx.notify();
        }
    }
    pub(super) fn press_link(&mut self, event: &MouseDownEvent) -> bool {
        self.pressed_link = None;
        if self.appearance.command_click_opens_filename_or_url
            && event.button == MouseButton::Left
            && modifier(event.modifiers)
        {
            self.pressed_link = self.resolve_link_target(self.viewport_cell(event.position));
        }
        self.pressed_link.is_some()
    }
    pub(super) fn release_link(&mut self, event: &MouseUpEvent, cx: &App) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        let Some(pressed) = self.pressed_link.take() else {
            return false;
        };
        if self.bounds.contains(&event.position)
            && self
                .resolve_link_target(self.viewport_cell(event.position))
                .is_some_and(|target| target.url == pressed.url)
            && pressed.local_path.as_ref().is_none_or(|path| path.exists())
        {
            cx.open_url(&pressed.url);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn sample_frame(text: &str) -> Frame {
        Frame {
            columns: text.chars().count() as u32,
            rows: 1,
            history_lines: 20,
            display_offset: 3,
            cells: text
                .chars()
                .map(|ch| crate::native::Cell {
                    text: ch.to_string(),
                    hyperlink: None,
                    flags: 0,
                    foreground: 0,
                    background: 0,
                })
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn native_web_links_and_plain_url_spans_preserve_history_coordinates() {
        let mut frame = sample_frame("log (https://example.com/a). done");
        let target = target_at(&frame, (10, 0)).unwrap();
        assert_eq!(target.url, "https://example.com/a");
        let start = crate::terminal_search::viewport_start(frame.columns, 20, 3);
        assert_eq!(target.selection.start, start + 5);
        assert_eq!(target_at(&frame, (0, 0)), None);
        assert_eq!(target_at(&frame, (frame.columns, 0)), None);
        for cell in &mut frame.cells[0..3] {
            cell.hyperlink = Some("https://example.com/hidden".into());
        }
        let link = target_at(&frame, (1, 0)).unwrap();
        assert_eq!(link.url, "https://example.com/hidden");
        assert_eq!(
            link.selection,
            Selection {
                start,
                end: start + 3
            }
        );
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://",
            "https://x\n.example/",
        ] {
            assert_eq!(web_url(url), None);
        }
        let mut wide = sample_frame("https://例子.com/");
        let spacer = crate::native::Cell {
            text: String::new(),
            hyperlink: None,
            flags: 0x40,
            foreground: 0,
            background: 0,
        };
        wide.cells.insert(9, spacer.clone());
        wide.cells.insert(11, spacer);
        wide.columns = wide.cells.len() as u32;
        let target = target_at(&wide, (9, 0)).unwrap();
        assert!(target.url.starts_with("https://xn--"));
        assert!(target.url.ends_with(".com/"));
        assert_eq!(
            target.selection.end - target.selection.start,
            i64::from(wide.columns)
        );
    }
}
