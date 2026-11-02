//! Typed owners for the existing encrypted capture format. No shell is created.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::path::Path;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Default)]
pub(crate) struct Api;
impl Api {
    pub fn prepare(&self, directory: &Path) -> Result<()> {
        Ok(nauterm_core::capture_crypto::prepare_capture_directory(
            directory,
        )?)
    }
    pub fn writer(&self, path: &Path, id: &str) -> Result<Writer> {
        Ok(Writer {
            inner: Some(nauterm_core::capture_crypto::CaptureWriter::open(path, id)?),
        })
    }
    pub fn recover(&self, path: &Path, id: &str) -> Result<Finalized> {
        Ok(finalized(nauterm_core::capture_crypto::recover_capture(
            path, id,
        )?))
    }
    pub fn write_tail(
        &self,
        source: &Path,
        id: &str,
        max_bytes: u64,
        destination: &Path,
        expected_sha256: &str,
    ) -> Result<Finalized> {
        Ok(finalized(nauterm_core::capture_crypto::write_capture_tail(
            source,
            destination,
            id,
            max_bytes,
            expected_sha256,
        )?))
    }
    pub fn read(
        &self,
        path: &Path,
        id: &str,
        mut consume: impl FnMut(&[u8]) -> Result<()>,
    ) -> Result<Finalized> {
        let mut reader = nauterm_core::capture_crypto::CaptureReader::open(path, id)?;
        while let Some(bytes) = reader.next_chunk()? {
            let bytes = Zeroizing::new(bytes);
            consume(&bytes)?;
        }
        Ok(finalized(reader.verify_complete()?))
    }
    pub fn readable(
        &self,
        path: &Path,
        id: &str,
        columns: u32,
        rows: u32,
    ) -> Result<(Zeroizing<String>, Finalized)> {
        let mut terminal = nauterm_core::terminal::TerminalEngine::new(
            columns.clamp(2, 1000) as usize,
            rows.clamp(1, 500) as usize,
        );
        let mut filter = super::sanitizer::Sanitizer::default();
        let info = self.read(path, id, |bytes| {
            terminal.write_bytes(&filter.add(bytes));
            Ok(())
        })?;
        terminal.write_bytes(&filter.finish());
        let text = Zeroizing::new(terminal.plain_text());
        ensure!(
            text.len() <= 4 * 1024 * 1024,
            "The readable snapshot is too large. Export the raw terminal stream instead."
        );
        Ok((text, info))
    }
}

#[derive(Deserialize)]
pub struct Finalized {
    pub ciphertext_bytes: u64,
    pub file_sha256: String,
}
impl Finalized {
    pub fn verify(&self, expected_sha256: &str, expected_bytes: u64) -> Result<()> {
        ensure!(
            self.ciphertext_bytes == expected_bytes
                && self.file_sha256.eq_ignore_ascii_case(expected_sha256),
            "The encrypted capture is incomplete or has been modified."
        );
        Ok(())
    }
}
#[derive(Deserialize)]
pub struct Checkpoint {
    pub committed_ciphertext_bytes: u64,
}
fn finalized(value: nauterm_core::capture_crypto::CaptureFinalized) -> Finalized {
    Finalized {
        ciphertext_bytes: value.ciphertext_bytes,
        file_sha256: value.file_sha256,
    }
}

pub struct Writer {
    inner: Option<nauterm_core::capture_crypto::CaptureWriter>,
}
impl Writer {
    pub fn append(&mut self, bytes: &[u8]) -> Result<()> {
        Ok(self
            .inner
            .as_mut()
            .context("Recording is closed")?
            .append(bytes)?)
    }
    pub fn checkpoint(&mut self) -> Result<Checkpoint> {
        let value = self
            .inner
            .as_mut()
            .context("Recording is closed")?
            .checkpoint()?;
        Ok(Checkpoint {
            committed_ciphertext_bytes: value.committed_ciphertext_bytes,
        })
    }
    pub fn finish(mut self) -> Result<Finalized> {
        Ok(finalized(
            self.inner.take().context("Recording is closed")?.close()?,
        ))
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        if let Some(writer) = self.inner.take() {
            let _ = writer.close();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn exported_capture_requires_the_catalog_hash_and_complete_size() {
        let info = super::Finalized {
            ciphertext_bytes: 64,
            file_sha256: "ab".repeat(32),
        };
        assert!(info.verify(&"AB".repeat(32), 64).is_ok());
        assert!(info.verify(&"cd".repeat(32), 64).is_err());
        assert!(info.verify(&"ab".repeat(32), 63).is_err());
    }
}
