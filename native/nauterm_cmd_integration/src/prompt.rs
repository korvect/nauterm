use crate::{MarkerScanner, ScanChunk};

/// Result at a console input boundary. Only `MainPrompt` allows the hook to
/// treat the following read as command input. `Other` includes program input
/// and capture overflow; callers must pass those reads through untouched.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadBoundary<T> {
    MainPrompt(Vec<T>),
    Other(Vec<T>),
}

/// Retains the exact prompt after an internal marker until CMD reads input.
/// Instantiate for bytes or UTF-16 units without decoding console output.
///
/// The host must route only writes to its console output through this gate,
/// and call `before_read` before forwarding a console read. File writes and
/// child-process output must not be interpreted as CMD prompts.
pub struct PromptCapture<T> {
    scanner: MarkerScanner<T>,
    prompt: Option<Vec<T>>,
    limit: usize,
}

impl<T: Clone + Eq> PromptCapture<T> {
    pub fn new(marker: Vec<T>, max_prompt_units: usize) -> Option<Self> {
        if max_prompt_units == 0 {
            return None;
        }
        Some(Self {
            scanner: MarkerScanner::new(marker)?,
            prompt: None,
            limit: max_prompt_units,
        })
    }

    /// Return output that can be forwarded immediately. Prompt content is
    /// bounded: overflow flushes it unchanged and disarms command capture.
    pub fn feed(&mut self, input: &[T]) -> Vec<T> {
        let mut visible = Vec::new();
        for chunk in self.scanner.feed(input) {
            match chunk {
                ScanChunk::Visible(units) => self.append(units, &mut visible),
                ScanChunk::Marker => {
                    // A repeated prompt must not silently discard previously
                    // captured output (for example, a prompt redraw).
                    if let Some(previous) = self.prompt.replace(Vec::new()) {
                        visible.extend(previous);
                    }
                }
            }
        }
        visible
    }

    fn append(&mut self, units: Vec<T>, visible: &mut Vec<T>) {
        if let Some(prompt) = &mut self.prompt
            && units.len() <= self.limit.saturating_sub(prompt.len())
        {
            prompt.extend(units);
            return;
        }
        if let Some(prompt) = self.prompt.take() {
            visible.extend(prompt);
        }
        visible.extend(units);
    }

    pub fn before_read(&mut self) -> ReadBoundary<T> {
        // A partial marker at a read boundary is visible prompt/output, never
        // evidence that the next read accepts a shell command.
        let tail = self.scanner.flush();
        let mut visible = Vec::new();
        self.append(tail, &mut visible);
        match self.prompt.take() {
            Some(prompt) => ReadBoundary::MainPrompt(prompt),
            None => ReadBoundary::Other(visible),
        }
    }

    /// Flush before uninstalling interception or switching console streams.
    pub fn drain(&mut self) -> Vec<T> {
        match self.before_read() {
            ReadBoundary::MainPrompt(units) | ReadBoundary::Other(units) => units,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKER: &[u8] = b"\x1b]prompt-token\x07";

    #[test]
    fn exact_prompt_survives_every_write_split_and_is_consumed_once() {
        let prompt = b"\x1b[32mC:\\space & % !>\x1b[0m ";
        let input = [b"command output\r\n".as_slice(), MARKER, prompt].concat();
        for size in 1..=input.len() {
            let mut capture = PromptCapture::new(MARKER.to_vec(), 1024).unwrap();
            let mut visible = Vec::new();
            for chunk in input.chunks(size) {
                visible.extend(capture.feed(chunk));
            }
            assert_eq!(visible, b"command output\r\n");
            assert_eq!(
                capture.before_read(),
                ReadBoundary::MainPrompt(prompt.to_vec())
            );
            assert_eq!(capture.before_read(), ReadBoundary::Other(vec![]));
            assert_eq!(capture.feed(b"Password: "), b"Password: ");
            assert_eq!(capture.before_read(), ReadBoundary::Other(vec![]));
        }
    }

    #[test]
    fn incomplete_marker_and_overflow_do_not_authorize_command_capture() {
        let mut capture = PromptCapture::new(MARKER.to_vec(), 4).unwrap();
        assert!(capture.feed(&MARKER[..8]).is_empty());
        assert_eq!(
            capture.before_read(),
            ReadBoundary::Other(MARKER[..8].to_vec())
        );
        assert_eq!(capture.feed(&[MARKER, b"1234"].concat()), b"");
        assert_eq!(capture.feed(b"5Password: "), b"12345Password: ");
        assert_eq!(capture.before_read(), ReadBoundary::Other(vec![]));
        assert!(capture.feed(&[MARKER, b"ok> "].concat()).is_empty());
        assert_eq!(
            capture.before_read(),
            ReadBoundary::MainPrompt(b"ok> ".to_vec())
        );
        assert!(PromptCapture::<u8>::new(vec![], 1).is_none());
        assert!(PromptCapture::<u8>::new(vec![1], 0).is_none());
    }

    #[test]
    fn partial_marker_in_prompt_is_replayed_and_checked_against_limit() {
        let mut capture = PromptCapture::new(MARKER.to_vec(), 4).unwrap();
        capture.feed(&[MARKER, b">\x1b]p"].concat());
        assert_eq!(
            capture.before_read(),
            ReadBoundary::MainPrompt(b">\x1b]p".to_vec())
        );
        capture.feed(&[MARKER, b"> \x1b]p"].concat());
        assert_eq!(
            capture.before_read(),
            ReadBoundary::Other(b"> \x1b]p".to_vec())
        );
    }

    #[test]
    fn redraw_and_disable_preserve_wide_units_including_partial_surrogates() {
        let marker: Vec<u16> = MARKER.iter().copied().map(u16::from).collect();
        let mut capture = PromptCapture::new(marker.clone(), 100).unwrap();
        let original = vec![0xd83d, 0xde00, 0xd800, 0x3e];
        assert!(capture.feed(&marker).is_empty());
        assert!(capture.feed(&original).is_empty());
        assert_eq!(capture.feed(&marker), original);
        assert!(capture.feed(&[0x7528, 0x6237, 0x3e]).is_empty());
        assert_eq!(capture.drain(), vec![0x7528, 0x6237, 0x3e]);
        assert_eq!(capture.before_read(), ReadBoundary::Other(vec![]));
    }
}
