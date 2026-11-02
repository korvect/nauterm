//! Native OSC 7 state wins; prompt fallback mirrors the reference and is never
//! updated from scrollback or alternate-screen output.
use crate::native::Frame;
#[derive(Default)]
pub struct WorkingDirectory {
    value: Option<String>,
    structured: bool,
    current_prompt_only: bool,
}
impl WorkingDirectory {
    pub fn value(&self) -> Option<String> {
        self.value.clone()
    }
    pub fn reset(&mut self) -> bool {
        let changed = self.value.take().is_some();
        self.structured = false;
        // Reconnect inserts a boundary and leaves the cursor on a fresh row.
        // The preceding prompt belongs to the previous transport, even though
        // those cells are intentionally still visible in the terminal.
        self.current_prompt_only = true;
        changed
    }
    pub fn update(&mut self, frame: &Frame) -> bool {
        let previous = self.value.clone();
        if let Some(directory) = frame
            .working_directory
            .as_ref()
            .filter(|directory| crate::recovery::safe_directory(directory))
        {
            self.structured = true;
            self.value = Some(directory.clone());
        } else if !self.structured
            && !frame.alternate_screen
            && frame.display_offset == 0
            && let Some(directory) = prompt(frame, self.current_prompt_only)
        {
            self.value = Some(directory);
            self.current_prompt_only = false;
        }
        self.value != previous
    }
}
fn prompt(frame: &Frame, current_only: bool) -> Option<String> {
    if frame.columns == 0 || frame.rows == 0 {
        return None;
    }
    let row = frame.cursor.1.min(frame.rows - 1) as usize;
    let first = if current_only {
        row
    } else {
        row.saturating_sub(2)
    };
    for row in (first..=row).rev() {
        let line = frame
            .cells
            .iter()
            .skip(row * frame.columns as usize)
            .take(frame.columns as usize)
            .filter(|cell| cell.flags & (0x0040 | 0x0100) == 0)
            .map(|cell| cell.text.as_str())
            .collect::<String>();
        let line = line.trim_end();
        let Some(marker) = line
            .chars()
            .last()
            .filter(|character| matches!(character, '$' | '%' | '#' | '❯' | '➜' | '>'))
        else {
            continue;
        };
        let prompt = line[..line.len() - marker.len_utf8()].trim_end();
        for part in prompt.split_whitespace().rev() {
            let mut part = part.trim_matches(['[', ']', '(', ')']);
            if !part.starts_with('/')
                && !part.starts_with('~')
                && let Some((_, path)) = part.split_once(':')
            {
                part = path;
            }
            if (part.starts_with('/') || part == "~" || part.starts_with("~/"))
                && crate::recovery::safe_directory(part)
            {
                return Some(part.into());
            }
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(text: &str) -> Frame {
        Frame {
            columns: 80,
            rows: 1,
            cells: text
                .chars()
                .map(|character| crate::native::Cell {
                    text: character.to_string(),
                    hyperlink: None,
                    foreground: 0,
                    background: 0,
                    flags: 0,
                })
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn emits_only_changes_and_keeps_structured_directory_over_prompt_and_bad_reports() {
        let mut cwd = WorkingDirectory::default();
        assert!(cwd.update(&frame("user@host:/tmp $ ")));
        assert!(!cwd.update(&frame("user@host:/tmp $ ")));
        let reported = Frame {
            working_directory: Some("/tmp/with spaces/工作区".into()),
            ..frame("/wrong $")
        };
        assert!(cwd.update(&reported));
        assert!(!cwd.update(&frame("/different $")));
        assert_eq!(cwd.value().as_deref(), Some("/tmp/with spaces/工作区"));
        let invalid = Frame {
            working_directory: Some("/tmp\ncommand".into()),
            ..frame("/wrong $")
        };
        assert!(!cwd.update(&invalid));
    }
    #[test]
    fn alternate_screen_and_scrollback_do_not_guess_a_new_directory() {
        let mut cwd = WorkingDirectory::default();
        assert!(!cwd.update(&Frame {
            alternate_screen: true,
            ..frame("/wrong $")
        }));
        assert!(!cwd.update(&Frame {
            display_offset: 4,
            ..frame("/wrong $")
        }));
        assert!(cwd.update(&frame("[root@host:~/src]# ")));
        assert_eq!(cwd.value().as_deref(), Some("~/src"));
    }
    #[test]
    fn bold_and_inverse_prompt_text_still_reports_the_directory() {
        let mut styled = frame("root@host:/var/工作区 # ");
        for cell in &mut styled.cells {
            cell.flags = 3;
        }
        let mut cwd = WorkingDirectory::default();
        assert!(cwd.update(&styled));
        assert_eq!(cwd.value().as_deref(), Some("/var/工作区"));
        assert!(!cwd.update(&styled));
    }
    #[test]
    fn reconnect_ignores_retained_prompts_and_reenables_the_new_peers_fallback() {
        let mut cwd = WorkingDirectory::default();
        assert!(cwd.update(&Frame {
            working_directory: Some("/old".into()),
            ..frame("user@host:/old $ ")
        }));
        assert!(cwd.reset());
        assert!(!cwd.reset());
        let mut retained = frame("user@host:/old $ ");
        retained.rows = 3;
        retained.cursor = (0, 2);
        let blank = frame(" ").cells.remove(0);
        retained.cells.resize(3 * 80, blank);
        assert!(!cwd.update(&retained));
        assert_eq!(cwd.value(), None);
        assert!(cwd.update(&frame("user@host:/new $ ")));
        assert_eq!(cwd.value().as_deref(), Some("/new"));
        assert!(cwd.update(&Frame {
            working_directory: Some("/reported".into()),
            ..frame("user@host:/new $ ")
        }));
        assert!(!cwd.update(&frame("user@host:/guessed $ ")));
        assert_eq!(cwd.value().as_deref(), Some("/reported"));
    }
}
