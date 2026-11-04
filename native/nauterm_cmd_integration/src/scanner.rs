//! Lossless streaming recognition of an internal prompt marker. Windows can
//! split console writes anywhere, including in the middle of the marker.

#[derive(Debug, PartialEq, Eq)]
pub enum ScanChunk<T> {
    Visible(Vec<T>),
    Marker,
}

pub struct MarkerScanner<T> {
    marker: Vec<T>,
    candidate: Vec<T>,
}

impl<T: Clone + Eq> MarkerScanner<T> {
    /// Empty markers cannot be distinguished from arbitrary console output.
    pub fn new(marker: Vec<T>) -> Option<Self> {
        (!marker.is_empty()).then(|| Self {
            candidate: Vec::with_capacity(marker.len()),
            marker,
        })
    }

    pub fn feed(&mut self, input: &[T]) -> Vec<ScanChunk<T>> {
        let mut chunks = Vec::new();
        let mut visible = Vec::new();
        for unit in input {
            self.candidate.push(unit.clone());
            while !self.marker.starts_with(&self.candidate) {
                visible.push(self.candidate.remove(0));
            }
            if self.candidate.len() == self.marker.len() {
                if !visible.is_empty() {
                    chunks.push(ScanChunk::Visible(std::mem::take(&mut visible)));
                }
                self.candidate.clear();
                chunks.push(ScanChunk::Marker);
            }
        }
        if !visible.is_empty() {
            chunks.push(ScanChunk::Visible(visible));
        }
        chunks
    }

    /// A partial marker is ordinary output until a full marker is observed.
    /// Flush before an unrelated console read or when disabling interception.
    pub fn flush(&mut self) -> Vec<T> {
        std::mem::take(&mut self.candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flatten<T>(chunks: Vec<ScanChunk<T>>) -> (Vec<T>, usize) {
        let mut output = Vec::new();
        let mut markers = 0;
        for chunk in chunks {
            match chunk {
                ScanChunk::Visible(units) => output.extend(units),
                ScanChunk::Marker => markers += 1,
            }
        }
        (output, markers)
    }

    #[test]
    fn fragmented_wide_console_output_keeps_unicode_and_finds_each_marker() {
        let marker: Vec<u16> = "\x1b]777;nauterm-cmd-prompt=fixture\x07"
            .encode_utf16()
            .collect();
        let mut input: Vec<u16> = "用户 😀\r\n".encode_utf16().collect();
        input.extend_from_slice(&marker);
        input.extend("C:\\用户> ".encode_utf16());
        input.extend_from_slice(&marker);
        input.extend("尾部".encode_utf16());
        let expected: Vec<u16> = "用户 😀\r\nC:\\用户> 尾部".encode_utf16().collect();
        for chunk_size in 1..=input.len() {
            let mut scanner = MarkerScanner::new(marker.clone()).unwrap();
            let mut chunks = Vec::new();
            for chunk in input.chunks(chunk_size) {
                chunks.extend(scanner.feed(chunk));
            }
            let (mut visible, count) = flatten(chunks);
            visible.extend(scanner.flush());
            assert_eq!(visible, expected, "chunk size {chunk_size}");
            assert_eq!(count, 2);
        }
    }

    #[test]
    fn overlapping_prefixes_and_unfinished_markers_do_not_swallow_output() {
        let mut scanner = MarkerScanner::new(b"ababac".to_vec()).unwrap();
        let (mut output, count) = flatten(scanner.feed(b"ababababac tail ababa"));
        output.extend(scanner.flush());
        assert_eq!(output, b"abab tail ababa");
        assert_eq!(count, 1);
        assert!(MarkerScanner::<u8>::new(vec![]).is_none());
    }

    #[test]
    fn bytes_are_preserved_without_assuming_a_console_code_page() {
        let mut scanner = MarkerScanner::new(b"\x1b]marker\x07".to_vec()).unwrap();
        let input = b"\x81\xff\0\x1b]marker\x07\x82\xfe";
        let (mut output, count) = flatten(scanner.feed(input));
        output.extend(scanner.flush());
        assert_eq!(output, b"\x81\xff\0\x82\xfe");
        assert_eq!(count, 1);
    }
}
