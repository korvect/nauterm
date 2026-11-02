//! Streaming counterpart of the reference TerminalReplaySanitizer. Control
//! sequences may span encrypted chunks; ordinary SGR/OSC 8 output is retained.
use zeroize::{Zeroize, Zeroizing};
const ENTER: [&[u8]; 3] = [b"\x1b[?47h", b"\x1b[?1047h", b"\x1b[?1049h"];
const EXIT: [&[u8]; 3] = [b"\x1b[?47l", b"\x1b[?1047l", b"\x1b[?1049l"];
const MARKER: &[u8] = b"\x1b[7m%\x1b[27m";

#[derive(Default)]
pub struct Sanitizer {
    alternate: bool,
    alternate_pending: Zeroizing<Vec<u8>>,
    osc: Osc,
    marker_pending: Zeroizing<Vec<u8>>,
}
impl Sanitizer {
    pub fn add(&mut self, input: &[u8]) -> Zeroizing<Vec<u8>> {
        let mut primary = Zeroizing::new(Vec::with_capacity(input.len()));
        for &byte in input {
            if self.alternate_pending.is_empty() && byte != 0x1b {
                if !self.alternate {
                    primary.push(byte);
                }
                continue;
            }
            self.alternate_pending.push(byte);
            loop {
                let sequences = if self.alternate { EXIT } else { ENTER };
                if sequences
                    .iter()
                    .any(|sequence| *sequence == self.alternate_pending.as_slice())
                {
                    self.alternate = !self.alternate;
                    self.alternate_pending.clear();
                    break;
                }
                if sequences
                    .iter()
                    .any(|sequence| sequence.starts_with(&self.alternate_pending))
                {
                    break;
                }
                let first = self.alternate_pending.remove(0);
                if !self.alternate {
                    primary.push(first);
                }
                if self.alternate_pending.is_empty() {
                    break;
                }
            }
        }
        let cleaned = self.osc.add(&primary);
        self.markers(&cleaned)
    }
    fn markers(&mut self, input: &[u8]) -> Zeroizing<Vec<u8>> {
        let mut output = Zeroizing::new(Vec::with_capacity(input.len()));
        for &byte in input {
            if self.marker_pending.is_empty() && byte != MARKER[0] {
                output.push(byte);
                continue;
            }
            self.marker_pending.push(byte);
            while !MARKER.starts_with(&self.marker_pending) {
                output.push(self.marker_pending.remove(0));
            }
            if self.marker_pending.as_slice() == MARKER {
                self.marker_pending.clear();
            }
        }
        output
    }
    pub fn finish(&mut self) -> Zeroizing<Vec<u8>> {
        let tail = if self.alternate {
            Zeroizing::new(vec![])
        } else {
            Zeroizing::new(self.alternate_pending.to_vec())
        };
        self.alternate_pending.zeroize();
        self.alternate_pending.clear();
        self.alternate = false;
        let mut cleaned = self.osc.add(&tail);
        cleaned.extend_from_slice(&self.osc.finish());
        let mut output = self.markers(&cleaned);
        output.extend_from_slice(&self.marker_pending);
        self.marker_pending.zeroize();
        self.marker_pending.clear();
        output.extend_from_slice(b"\x1b[?1049l");
        output
    }
}

#[derive(Default)]
enum Mode {
    #[default]
    Text,
    Escape,
    Code,
    Pass,
    Drop,
}
#[derive(Default)]
struct Osc {
    mode: Mode,
    pending: Zeroizing<Vec<u8>>,
    escaped: bool,
}
impl Osc {
    fn internal(code: &[u8]) -> bool {
        matches!(code, b"7" | b"133" | b"4545" | b"777" | b"52")
    }
    fn clear(&mut self) {
        self.pending.zeroize();
        self.pending.clear();
        self.mode = Mode::Text;
        self.escaped = false;
    }
    fn add(&mut self, bytes: &[u8]) -> Zeroizing<Vec<u8>> {
        let mut output = Zeroizing::new(Vec::with_capacity(bytes.len()));
        for &byte in bytes {
            match self.mode {
                Mode::Text => {
                    if byte == 0x1b {
                        self.mode = Mode::Escape;
                    } else {
                        output.push(byte);
                    }
                }
                Mode::Escape => {
                    if byte == b']' {
                        self.pending.extend_from_slice(b"\x1b]");
                        self.mode = Mode::Code;
                    } else {
                        output.push(0x1b);
                        if byte == 0x1b {
                            self.mode = Mode::Escape;
                        } else {
                            output.push(byte);
                            self.mode = Mode::Text;
                        }
                    }
                }
                Mode::Code => {
                    self.pending.push(byte);
                    let terminated = byte == 7 || (self.escaped && byte == b'\\');
                    if terminated {
                        let end = self.pending.len() - if byte == 7 { 1 } else { 2 };
                        if !Self::internal(&self.pending[2..end]) {
                            output.extend_from_slice(&self.pending);
                        }
                        self.clear();
                    } else if byte == b';' || self.pending.len() > 64 {
                        let end = self.pending.len() - usize::from(byte == b';');
                        let internal = Self::internal(&self.pending[2..end]);
                        if !internal {
                            output.extend_from_slice(&self.pending);
                        }
                        self.pending.zeroize();
                        self.pending.clear();
                        self.mode = if internal { Mode::Drop } else { Mode::Pass };
                    }
                }
                Mode::Pass | Mode::Drop => {
                    if matches!(self.mode, Mode::Pass) {
                        output.push(byte);
                    }
                    if byte == 7 || (self.escaped && byte == b'\\') {
                        self.clear();
                    }
                }
            }
            self.escaped = byte == 0x1b;
        }
        output
    }
    fn finish(&mut self) -> Zeroizing<Vec<u8>> {
        let output = match self.mode {
            Mode::Escape => vec![0x1b],
            Mode::Pass => vec![7],
            _ => vec![],
        };
        self.clear();
        Zeroizing::new(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_filter_is_chunk_independent_and_keeps_colors_and_links() {
        let source = b"before\x1b]7;file:///private\x07\x1b]4545;CommandStarted;secret\x1b\\\x1b[7m%\x1b[27m\r\n\x1b[31mred\x1b[0m\x1b[?1049hTUI\x1b]2;hidden title\x07\x1b[?1049lafter\x1b]8;;https://example.invalid\x07link\x1b]8;;\x07\x1b]52;c;secret\x07";
        let expected = b"before\r\n\x1b[31mred\x1b[0mafter\x1b]8;;https://example.invalid\x07link\x1b]8;;\x07\x1b[?1049l";
        for size in 1..=source.len() {
            let mut filter = Sanitizer::default();
            let mut result = vec![];
            for chunk in source.chunks(size) {
                result.extend_from_slice(&filter.add(chunk));
            }
            result.extend_from_slice(&filter.finish());
            assert_eq!(result, expected, "chunk size {size}");
        }
    }
}
