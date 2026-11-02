use super::{
    invalid_data, CaptureFinalized, CaptureReader, CaptureWriter, FOOTER_PAYLOAD_LENGTH,
    HEADER_LENGTH, MAX_CHUNK_LENGTH, RECORD_HEADER_LENGTH, TAG_LENGTH,
};
use std::{collections::VecDeque, io, path::Path};
use zeroize::{Zeroize, Zeroizing};

const FIXED_BYTES: u64 =
    (HEADER_LENGTH + RECORD_HEADER_LENGTH + FOOTER_PAYLOAD_LENGTH + TAG_LENGTH) as u64;
const CHUNK_OVERHEAD: u64 = (RECORD_HEADER_LENGTH + TAG_LENGTH) as u64;

fn tail_budget(max_bytes: u64) -> io::Result<usize> {
    let payload = max_bytes.checked_sub(FIXED_BYTES).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Recording limit cannot hold a capture",
        )
    })?;
    let mut bytes = payload.saturating_sub(CHUNK_OVERHEAD);
    loop {
        let chunks = bytes.div_ceil(MAX_CHUNK_LENGTH as u64);
        let next = payload.saturating_sub(chunks * CHUNK_OVERHEAD);
        if next >= bytes {
            return usize::try_from(bytes)
                .ok()
                .filter(|value| *value <= usize::MAX - MAX_CHUNK_LENGTH)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Recording limit is too large")
                });
        }
        bytes = next;
    }
}

struct Tail {
    chunks: VecDeque<Zeroizing<Vec<u8>>>,
    bytes: usize,
    limit: usize,
}
impl Tail {
    fn push(&mut self, bytes: &[u8]) {
        if bytes.is_empty() || self.limit == 0 {
            return;
        }
        let bytes = &bytes[bytes.len().saturating_sub(self.limit)..];
        self.bytes += bytes.len();
        self.chunks.push_back(Zeroizing::new(bytes.to_vec()));
        while self.bytes > self.limit {
            let first = self.chunks.pop_front().unwrap();
            let remove = self.bytes - self.limit;
            self.bytes -= first.len();
            if remove < first.len() {
                let kept = Zeroizing::new(first[remove..].to_vec());
                self.bytes += kept.len();
                self.chunks.push_front(kept);
            }
        }
    }
}

/// Authenticate the complete source and its committed hash, then write an
/// encrypted suffix to a new, exclusively created destination. The caller holds
/// the recording lease and publishes the new filename in the database before
/// removing the old file. Neither source nor existing destination is replaced.
pub fn write_capture_tail(
    source: &Path,
    destination: &Path,
    recording_id: &str,
    max_bytes: u64,
    expected_sha256: &str,
) -> io::Result<CaptureFinalized> {
    let mut tail = Tail {
        chunks: VecDeque::new(),
        bytes: 0,
        limit: tail_budget(max_bytes)?,
    };
    let mut reader = CaptureReader::open(source, recording_id)?;
    while let Some(bytes) = reader.next_chunk()? {
        let bytes = Zeroizing::new(bytes);
        tail.push(&bytes);
    }
    let original = reader.verify_complete()?;
    if original.file_sha256 != expected_sha256 {
        return Err(invalid_data("Recording changed before compaction"));
    }
    drop(reader);

    // Authenticate first; even a corrupt discarded prefix must not be accepted.
    // Plaintext is never staged on disk, and discarded bytes are zeroized.
    let mut writer = CaptureWriter::open(destination, recording_id)?;
    let mut block = Zeroizing::new(Vec::with_capacity(tail.bytes.min(MAX_CHUNK_LENGTH)));
    for chunk in tail.chunks {
        let mut bytes = chunk.as_slice();
        while !bytes.is_empty() {
            let length = bytes.len().min(MAX_CHUNK_LENGTH - block.len());
            block.extend_from_slice(&bytes[..length]);
            bytes = &bytes[length..];
            if block.len() == MAX_CHUNK_LENGTH {
                writer.append(&block)?;
                block.zeroize();
            }
        }
    }
    if !block.is_empty() {
        writer.append(&block)?;
    }
    let finalized = writer.close()?;
    if finalized.ciphertext_bytes > max_bytes {
        return Err(invalid_data("Encrypted recording exceeds its limit"));
    }
    Ok(finalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_tail_budget_accounts_for_format_overhead() {
        let mut tail = Tail {
            chunks: VecDeque::new(),
            bytes: 0,
            limit: 7,
        };
        for bytes in [
            b"first".as_slice(),
            b"second".as_slice(),
            b"third".as_slice(),
        ] {
            tail.push(bytes);
        }
        assert_eq!(
            tail.chunks
                .iter()
                .flat_map(|chunk| chunk.iter().copied())
                .collect::<Vec<_>>(),
            b"ndthird"
        );
        tail.push(b"a much longer final chunk");
        assert_eq!(
            tail.chunks
                .iter()
                .flat_map(|chunk| chunk.iter().copied())
                .collect::<Vec<_>>(),
            b"l chunk"
        );
        for limit in [114, 140, 1024, 25 * 1024 * 1024, 500 * 1024 * 1024] {
            let bytes = tail_budget(limit).unwrap() as u64;
            assert!(
                FIXED_BYTES + bytes + bytes.div_ceil(MAX_CHUNK_LENGTH as u64) * CHUNK_OVERHEAD
                    <= limit
            );
        }
        assert!(tail_budget(113).is_err());
    }

    #[test]
    fn shared_tail_authenticates_source_and_preserves_files_until_publication() -> io::Result<()> {
        let mut token = [0; 16];
        getrandom::fill(&mut token).unwrap();
        let directory =
            std::env::temp_dir().join(format!("nauterm-tail-{:032x}", u128::from_be_bytes(token)));
        let source = directory.join("source.ntrcap");
        let destination = directory.join("revision.ntrcap");
        let mut writer = CaptureWriter::open(&source, "tail-recording")?;
        for bytes in [
            b"first".as_slice(),
            b"second".as_slice(),
            b"third".as_slice(),
        ] {
            writer.append(bytes)?;
        }
        let original = writer.close()?;
        let original_bytes = std::fs::read(&source)?;
        let result = write_capture_tail(
            &source,
            &destination,
            "tail-recording",
            150,
            &original.file_sha256,
        )?;
        assert!(result.ciphertext_bytes <= 150);
        let mut reader = CaptureReader::open(&destination, "tail-recording")?;
        assert_eq!(reader.next_chunk()?.unwrap(), b"ndthird");
        assert_eq!(reader.verify_complete()?.file_sha256, result.file_sha256);
        drop(reader);
        assert_eq!(std::fs::read(&source)?, original_bytes);
        let destination_bytes = std::fs::read(&destination)?;
        assert!(write_capture_tail(
            &source,
            &destination,
            "tail-recording",
            150,
            &original.file_sha256
        )
        .is_err());
        assert_eq!(std::fs::read(&destination)?, destination_bytes);
        let unpublished = directory.join("unpublished.ntrcap");
        assert!(
            write_capture_tail(&source, &unpublished, "tail-recording", 150, "wrong hash").is_err()
        );
        assert!(!unpublished.exists());
        // Damage a prefix that would otherwise be discarded from the suffix.
        let mut corrupt = original_bytes;
        corrupt[HEADER_LENGTH + RECORD_HEADER_LENGTH] ^= 1;
        std::fs::write(&source, &corrupt)?;
        assert!(write_capture_tail(
            &source,
            &unpublished,
            "tail-recording",
            150,
            &original.file_sha256
        )
        .is_err());
        assert!(!unpublished.exists());
        assert_eq!(std::fs::read(&source)?, corrupt);
        std::fs::remove_dir_all(directory)
    }
}
