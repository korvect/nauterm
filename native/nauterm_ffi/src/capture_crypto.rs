use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

#[cfg(not(test))]
use crate::crypto::load_or_create_database_key;
use crate::crypto::AES_KEY_LENGTH;

const MAGIC: &[u8; 7] = b"NTRCAP1";
const VERSION: u8 = 1;
const ALGORITHM_AES_256_GCM: u8 = 1;
const HEADER_LENGTH: usize = 7 + 1 + 1 + 16 + 4;
const RECORD_HEADER_LENGTH: usize = 1 + 8 + 4;
const RECORD_CHUNK: u8 = 1;
const RECORD_FOOTER: u8 = 2;
const TAG_LENGTH: usize = 16;
const MAX_CHUNK_LENGTH: usize = 16 * 1024 * 1024;
const FOOTER_PAYLOAD_LENGTH: usize = 8 + 8 + 8 + 32;
const STATE_MAGIC: &[u8; 7] = b"NTRSTA1";
const STATE_VERSION: u8 = 1;
const STATE_HEADER_LENGTH: usize = 7 + 1 + 12;

fn aes_nonce(bytes: &[u8]) -> Nonce<aes_gcm::aead::consts::U12> {
    Nonce::try_from(bytes).expect("AES-GCM nonce must be 12 bytes")
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CaptureCheckpoint {
    pub committed_chunk_count: u64,
    pub committed_plaintext_bytes: u64,
    pub committed_ciphertext_bytes: u64,
    pub chain_hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaptureFinalized {
    pub chunk_count: u64,
    pub plaintext_bytes: u64,
    pub ciphertext_bytes: u64,
    pub chain_hash: String,
    pub file_sha256: String,
}

pub struct CaptureWriter {
    file: File,
    path: PathBuf,
    key: Zeroizing<[u8; AES_KEY_LENGTH]>,
    state_key: Zeroizing<[u8; AES_KEY_LENGTH]>,
    nonce_prefix: [u8; 4],
    recording_id: String,
    chunk_index: u64,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
    file_hasher: Sha256,
}

pub struct CaptureReader {
    file: File,
    key: Zeroizing<[u8; AES_KEY_LENGTH]>,
    nonce_prefix: [u8; 4],
    recording_id: String,
    expected_index: u64,
    plaintext_bytes: u64,
    complete: bool,
    ciphertext_bytes: u64,
    file_hasher: Sha256,
    finalized: Option<CaptureFinalized>,
}

impl CaptureWriter {
    pub fn open(path: &Path, recording_id: &str) -> io::Result<Self> {
        ensure_private_parent(path)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(path)?;
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;

        let mut salt = [0u8; 16];
        let mut nonce_prefix = [0u8; 4];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce_prefix);
        let key = derive_file_key(&salt)?;
        let state_key = derive_state_key(key.as_slice())?;
        let mut header = Vec::with_capacity(HEADER_LENGTH);
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&[VERSION, ALGORITHM_AES_256_GCM]);
        header.extend_from_slice(&salt);
        header.extend_from_slice(&nonce_prefix);
        file.write_all(&header)?;
        let mut file_hasher = Sha256::new();
        file_hasher.update(&header);
        Ok(Self {
            file,
            path: path.to_owned(),
            key,
            state_key,
            nonce_prefix,
            recording_id: recording_id.to_owned(),
            chunk_index: 0,
            plaintext_bytes: 0,
            ciphertext_bytes: HEADER_LENGTH as u64,
            file_hasher,
        })
    }

    pub fn append(&mut self, plaintext: &[u8]) -> io::Result<()> {
        if plaintext.is_empty() {
            return Ok(());
        }
        if plaintext.len() > MAX_CHUNK_LENGTH {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "capture chunk exceeds the format limit",
            ));
        }
        let length = u32::try_from(plaintext.len())
            .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "capture chunk is too large"))?;
        let nonce = nonce(self.nonce_prefix, self.chunk_index);
        let aad = aad(&self.recording_id, self.chunk_index, length, RECORD_CHUNK);
        let cipher = Aes256Gcm::new_from_slice(self.key.as_slice())
            .map_err(|_| invalid_data("invalid capture key"))?;
        let ciphertext = cipher
            .encrypt(
                &aes_nonce(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| invalid_data("capture encryption failed"))?;
        let record = encoded_record(RECORD_CHUNK, self.chunk_index, length, &ciphertext);
        self.file.write_all(&record)?;
        self.file_hasher.update(&record);
        self.ciphertext_bytes = self
            .ciphertext_bytes
            .checked_add(record.len() as u64)
            .ok_or_else(|| invalid_data("capture ciphertext length overflow"))?;
        self.chunk_index = self
            .chunk_index
            .checked_add(1)
            .ok_or_else(|| invalid_data("capture chunk index overflow"))?;
        self.plaintext_bytes = self
            .plaintext_bytes
            .checked_add(u64::from(length))
            .ok_or_else(|| invalid_data("capture plaintext length overflow"))?;
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }

    pub fn checkpoint(&mut self) -> io::Result<CaptureCheckpoint> {
        self.file.flush()?;
        self.file.sync_data()?;
        let checkpoint = CaptureCheckpoint {
            committed_chunk_count: self.chunk_index,
            committed_plaintext_bytes: self.plaintext_bytes,
            committed_ciphertext_bytes: self.ciphertext_bytes,
            chain_hash: digest_hex(self.file_hasher.clone().finalize().as_slice()),
        };
        write_checkpoint_sidecar(
            &self.path,
            &self.recording_id,
            self.state_key.as_slice(),
            &checkpoint,
        )?;
        Ok(checkpoint)
    }

    pub fn close(mut self) -> io::Result<CaptureFinalized> {
        let prefix_hash = self.file_hasher.clone().finalize();
        let chain_hash = digest_hex(prefix_hash.as_slice());
        self.write_footer(prefix_hash.as_slice())?;
        self.file.flush()?;
        self.file.sync_all()?;
        let finalized = CaptureFinalized {
            chunk_count: self.chunk_index,
            plaintext_bytes: self.plaintext_bytes,
            ciphertext_bytes: self.ciphertext_bytes,
            chain_hash,
            file_sha256: digest_hex(self.file_hasher.finalize().as_slice()),
        };
        remove_checkpoint_sidecar(&self.path)?;
        Ok(finalized)
    }

    fn write_footer(&mut self, prefix_hash: &[u8]) -> io::Result<()> {
        let footer_size = RECORD_HEADER_LENGTH + FOOTER_PAYLOAD_LENGTH + TAG_LENGTH;
        let final_ciphertext_bytes = self
            .ciphertext_bytes
            .checked_add(footer_size as u64)
            .ok_or_else(|| invalid_data("capture ciphertext length overflow"))?;
        let mut payload = Vec::with_capacity(FOOTER_PAYLOAD_LENGTH);
        payload.extend_from_slice(&self.chunk_index.to_be_bytes());
        payload.extend_from_slice(&self.plaintext_bytes.to_be_bytes());
        payload.extend_from_slice(&final_ciphertext_bytes.to_be_bytes());
        payload.extend_from_slice(prefix_hash);
        let length = payload.len() as u32;
        let nonce = nonce(self.nonce_prefix, self.chunk_index);
        let aad = aad(&self.recording_id, self.chunk_index, length, RECORD_FOOTER);
        let cipher = Aes256Gcm::new_from_slice(self.key.as_slice())
            .map_err(|_| invalid_data("invalid capture key"))?;
        let ciphertext = cipher
            .encrypt(
                &aes_nonce(&nonce),
                Payload {
                    msg: payload.as_slice(),
                    aad: &aad,
                },
            )
            .map_err(|_| invalid_data("capture footer encryption failed"))?;
        let record = encoded_record(RECORD_FOOTER, self.chunk_index, length, &ciphertext);
        self.file.write_all(&record)?;
        self.file_hasher.update(&record);
        self.ciphertext_bytes = self
            .ciphertext_bytes
            .checked_add(record.len() as u64)
            .ok_or_else(|| invalid_data("capture ciphertext length overflow"))?;
        Ok(())
    }
}

impl CaptureReader {
    pub fn open(path: &Path, recording_id: &str) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let mut header = [0u8; HEADER_LENGTH];
        file.read_exact(&mut header)?;
        if &header[..7] != MAGIC || header[7] != VERSION || header[8] != ALGORITHM_AES_256_GCM {
            return Err(invalid_data("unsupported or unencrypted capture file"));
        }
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&header[9..25]);
        let mut nonce_prefix = [0u8; 4];
        nonce_prefix.copy_from_slice(&header[25..29]);
        let mut file_hasher = Sha256::new();
        file_hasher.update(header);
        Ok(Self {
            file,
            key: derive_file_key(&salt)?,
            nonce_prefix,
            recording_id: recording_id.to_owned(),
            expected_index: 0,
            plaintext_bytes: 0,
            complete: false,
            ciphertext_bytes: HEADER_LENGTH as u64,
            file_hasher,
            finalized: None,
        })
    }

    pub fn next_chunk(&mut self) -> io::Result<Option<Vec<u8>>> {
        if self.complete {
            return Ok(None);
        }
        let mut record_header = [0u8; RECORD_HEADER_LENGTH];
        match self.file.read_exact(&mut record_header[..1]) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
                return Err(invalid_data("capture is incomplete"));
            }
            Err(error) => return Err(error),
        }
        self.file.read_exact(&mut record_header[1..])?;
        if record_header[0] != RECORD_CHUNK && record_header[0] != RECORD_FOOTER {
            return Err(invalid_data("invalid capture record type"));
        }
        let index = u64::from_be_bytes(record_header[1..9].try_into().unwrap());
        let length = u32::from_be_bytes(record_header[9..13].try_into().unwrap());
        if length as usize > MAX_CHUNK_LENGTH {
            return Err(invalid_data("capture chunk exceeds the format limit"));
        }
        if index != self.expected_index {
            return Err(invalid_data("capture chunks are missing or reordered"));
        }
        if record_header[0] == RECORD_FOOTER && length as usize != FOOTER_PAYLOAD_LENGTH {
            return Err(invalid_data("invalid capture footer length"));
        }
        let mut ciphertext = vec![0u8; length as usize + TAG_LENGTH];
        self.file.read_exact(&mut ciphertext)?;
        let nonce = nonce(self.nonce_prefix, index);
        let aad = aad(&self.recording_id, index, length, record_header[0]);
        let cipher = Aes256Gcm::new_from_slice(self.key.as_slice())
            .map_err(|_| invalid_data("invalid capture key"))?;
        let plaintext = cipher
            .decrypt(
                &aes_nonce(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| invalid_data("capture authentication failed"))?;
        if record_header[0] == RECORD_FOOTER {
            let footer_chunk_count = read_u64(&plaintext[0..8]);
            let footer_plaintext_bytes = read_u64(&plaintext[8..16]);
            let footer_ciphertext_bytes = read_u64(&plaintext[16..24]);
            let expected_chain_hash = &plaintext[24..56];
            let actual_chain_hash = self.file_hasher.clone().finalize();
            if footer_chunk_count != self.expected_index
                || footer_plaintext_bytes != self.plaintext_bytes
                || footer_ciphertext_bytes
                    != self.ciphertext_bytes
                        + (RECORD_HEADER_LENGTH + FOOTER_PAYLOAD_LENGTH + TAG_LENGTH) as u64
                || expected_chain_hash != actual_chain_hash.as_slice()
            {
                return Err(invalid_data("capture footer length does not match"));
            }
            let mut trailing = [0u8; 1];
            if self.file.read(&mut trailing)? != 0 {
                return Err(invalid_data("capture has trailing data"));
            }
            let record = encoded_record(record_header[0], index, length, &ciphertext);
            self.file_hasher.update(&record);
            self.ciphertext_bytes += record.len() as u64;
            self.finalized = Some(CaptureFinalized {
                chunk_count: self.expected_index,
                plaintext_bytes: self.plaintext_bytes,
                ciphertext_bytes: self.ciphertext_bytes,
                chain_hash: digest_hex(actual_chain_hash.as_slice()),
                file_sha256: digest_hex(self.file_hasher.clone().finalize().as_slice()),
            });
            self.complete = true;
            return Ok(None);
        }
        let record = encoded_record(record_header[0], index, length, &ciphertext);
        self.file_hasher.update(&record);
        self.ciphertext_bytes += record.len() as u64;
        self.expected_index += 1;
        self.plaintext_bytes = self
            .plaintext_bytes
            .checked_add(u64::from(length))
            .ok_or_else(|| invalid_data("capture plaintext length overflow"))?;
        Ok(Some(plaintext))
    }

    pub fn verify_complete(&mut self) -> io::Result<CaptureFinalized> {
        while self.next_chunk()?.is_some() {}
        self.finalized
            .clone()
            .ok_or_else(|| invalid_data("capture is incomplete"))
    }
}

pub fn recover_capture(path: &Path, recording_id: &str) -> io::Result<CaptureFinalized> {
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let mut header = [0u8; HEADER_LENGTH];
    file.read_exact(&mut header)?;
    if &header[..7] != MAGIC || header[7] != VERSION || header[8] != ALGORITHM_AES_256_GCM {
        return Err(invalid_data("unsupported or unencrypted capture file"));
    }
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&header[9..25]);
    let key = derive_file_key(&salt)?;
    let state_key = derive_state_key(key.as_slice())?;
    let checkpoint = read_checkpoint_sidecar(path, recording_id, state_key.as_slice())?;
    let checkpoint = checkpoint.as_ref();
    let mut nonce_prefix = [0u8; 4];
    nonce_prefix.copy_from_slice(&header[25..29]);
    let mut hasher = Sha256::new();
    hasher.update(header);
    let mut chunk_count = 0u64;
    let mut plaintext_bytes = 0u64;
    let mut ciphertext_bytes = HEADER_LENGTH as u64;
    let mut checkpoint_matched = checkpoint.is_none();
    if let Some(value) = checkpoint {
        if value.committed_ciphertext_bytes == 0 {
            checkpoint_matched = true;
        } else if value.committed_ciphertext_bytes == HEADER_LENGTH as u64 {
            validate_checkpoint(
                checkpoint,
                true,
                chunk_count,
                plaintext_bytes,
                ciphertext_bytes,
                &hasher,
            )?;
            checkpoint_matched = true;
        }
    }

    loop {
        let record_start = ciphertext_bytes;
        let mut record_header = [0u8; RECORD_HEADER_LENGTH];
        match file.read_exact(&mut record_header[..1]) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
                return finalize_recovered_capture(
                    path,
                    file,
                    recording_id,
                    &key,
                    nonce_prefix,
                    chunk_count,
                    plaintext_bytes,
                    ciphertext_bytes,
                    hasher,
                    checkpoint,
                    checkpoint_matched,
                );
            }
            Err(error) => return Err(error),
        }
        if let Err(error) = file.read_exact(&mut record_header[1..]) {
            if error.kind() == ErrorKind::UnexpectedEof {
                file.set_len(record_start)?;
                file.seek(SeekFrom::Start(record_start))?;
                return finalize_recovered_capture(
                    path,
                    file,
                    recording_id,
                    &key,
                    nonce_prefix,
                    chunk_count,
                    plaintext_bytes,
                    ciphertext_bytes,
                    hasher,
                    checkpoint,
                    checkpoint_matched,
                );
            }
            return Err(error);
        }
        let record_type = record_header[0];
        if record_type != RECORD_CHUNK && record_type != RECORD_FOOTER {
            return Err(invalid_data("invalid capture record type"));
        }
        let index = read_u64(&record_header[1..9]);
        let length = u32::from_be_bytes(record_header[9..13].try_into().unwrap());
        if index != chunk_count || length as usize > MAX_CHUNK_LENGTH {
            return Err(invalid_data("invalid capture chunk sequence"));
        }
        if record_type == RECORD_FOOTER && length as usize != FOOTER_PAYLOAD_LENGTH {
            return Err(invalid_data("invalid capture footer length"));
        }
        let mut ciphertext = vec![0u8; length as usize + TAG_LENGTH];
        if let Err(error) = file.read_exact(&mut ciphertext) {
            if error.kind() == ErrorKind::UnexpectedEof {
                file.set_len(record_start)?;
                file.seek(SeekFrom::Start(record_start))?;
                return finalize_recovered_capture(
                    path,
                    file,
                    recording_id,
                    &key,
                    nonce_prefix,
                    chunk_count,
                    plaintext_bytes,
                    ciphertext_bytes,
                    hasher,
                    checkpoint,
                    checkpoint_matched,
                );
            }
            return Err(error);
        }
        let cipher = Aes256Gcm::new_from_slice(key.as_slice())
            .map_err(|_| invalid_data("invalid capture key"))?;
        let plaintext = cipher
            .decrypt(
                &aes_nonce(&nonce(nonce_prefix, index)),
                Payload {
                    msg: &ciphertext,
                    aad: &aad(recording_id, index, length, record_type),
                },
            )
            .map_err(|_| invalid_data("capture authentication failed"))?;

        if record_type == RECORD_FOOTER {
            let prefix_hash = hasher.clone().finalize();
            if read_u64(&plaintext[0..8]) != chunk_count
                || read_u64(&plaintext[8..16]) != plaintext_bytes
                || read_u64(&plaintext[16..24])
                    != ciphertext_bytes
                        + (RECORD_HEADER_LENGTH + FOOTER_PAYLOAD_LENGTH + TAG_LENGTH) as u64
                || &plaintext[24..56] != prefix_hash.as_slice()
            {
                return Err(invalid_data("capture footer does not match"));
            }
            let mut trailing = [0u8; 1];
            if file.read(&mut trailing)? != 0 {
                return Err(invalid_data("capture has trailing data"));
            }
            validate_checkpoint(
                checkpoint,
                checkpoint_matched,
                chunk_count,
                plaintext_bytes,
                ciphertext_bytes,
                &hasher,
            )?;
            let record = encoded_record(record_type, index, length, &ciphertext);
            hasher.update(&record);
            ciphertext_bytes += record.len() as u64;
            let finalized = CaptureFinalized {
                chunk_count,
                plaintext_bytes,
                ciphertext_bytes,
                chain_hash: digest_hex(prefix_hash.as_slice()),
                file_sha256: digest_hex(hasher.finalize().as_slice()),
            };
            remove_checkpoint_sidecar(path)?;
            return Ok(finalized);
        }

        let record = encoded_record(record_type, index, length, &ciphertext);
        hasher.update(&record);
        ciphertext_bytes += record.len() as u64;
        chunk_count += 1;
        plaintext_bytes = plaintext_bytes
            .checked_add(u64::from(length))
            .ok_or_else(|| invalid_data("capture plaintext length overflow"))?;
        if let Some(value) = checkpoint {
            if ciphertext_bytes == value.committed_ciphertext_bytes {
                validate_checkpoint(
                    checkpoint,
                    true,
                    chunk_count,
                    plaintext_bytes,
                    ciphertext_bytes,
                    &hasher,
                )?;
                checkpoint_matched = true;
            } else if ciphertext_bytes > value.committed_ciphertext_bytes && !checkpoint_matched {
                return Err(invalid_data("capture checkpoint is not a record boundary"));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_recovered_capture(
    capture_path: &Path,
    mut file: File,
    recording_id: &str,
    key: &[u8; AES_KEY_LENGTH],
    nonce_prefix: [u8; 4],
    chunk_count: u64,
    plaintext_bytes: u64,
    mut ciphertext_bytes: u64,
    mut hasher: Sha256,
    checkpoint: Option<&CaptureCheckpoint>,
    checkpoint_matched: bool,
) -> io::Result<CaptureFinalized> {
    validate_checkpoint(
        checkpoint,
        checkpoint_matched,
        chunk_count,
        plaintext_bytes,
        ciphertext_bytes,
        &hasher,
    )?;
    file.set_len(ciphertext_bytes)?;
    file.seek(SeekFrom::Start(ciphertext_bytes))?;
    let prefix_hash = hasher.clone().finalize();
    let mut payload = Vec::with_capacity(FOOTER_PAYLOAD_LENGTH);
    payload.extend_from_slice(&chunk_count.to_be_bytes());
    payload.extend_from_slice(&plaintext_bytes.to_be_bytes());
    let final_ciphertext_bytes =
        ciphertext_bytes + (RECORD_HEADER_LENGTH + FOOTER_PAYLOAD_LENGTH + TAG_LENGTH) as u64;
    payload.extend_from_slice(&final_ciphertext_bytes.to_be_bytes());
    payload.extend_from_slice(prefix_hash.as_slice());
    let length = payload.len() as u32;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| invalid_data("invalid capture key"))?;
    let encrypted = cipher
        .encrypt(
            &aes_nonce(&nonce(nonce_prefix, chunk_count)),
            Payload {
                msg: &payload,
                aad: &aad(recording_id, chunk_count, length, RECORD_FOOTER),
            },
        )
        .map_err(|_| invalid_data("capture footer encryption failed"))?;
    let footer = encoded_record(RECORD_FOOTER, chunk_count, length, &encrypted);
    file.write_all(&footer)?;
    hasher.update(&footer);
    ciphertext_bytes += footer.len() as u64;
    file.flush()?;
    file.sync_all()?;
    let finalized = CaptureFinalized {
        chunk_count,
        plaintext_bytes,
        ciphertext_bytes,
        chain_hash: digest_hex(prefix_hash.as_slice()),
        file_sha256: digest_hex(hasher.finalize().as_slice()),
    };
    remove_checkpoint_sidecar(capture_path)?;
    Ok(finalized)
}

fn validate_checkpoint(
    checkpoint: Option<&CaptureCheckpoint>,
    checkpoint_matched: bool,
    chunk_count: u64,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
    hasher: &Sha256,
) -> io::Result<()> {
    let Some(value) = checkpoint else {
        return Ok(());
    };
    if value.committed_ciphertext_bytes == 0 {
        return Ok(());
    }
    if !checkpoint_matched || value.committed_ciphertext_bytes > ciphertext_bytes {
        return Err(invalid_data("capture is shorter than its checkpoint"));
    }
    if value.committed_ciphertext_bytes == ciphertext_bytes
        && (value.committed_chunk_count != chunk_count
            || value.committed_plaintext_bytes != plaintext_bytes
            || value.chain_hash != digest_hex(hasher.clone().finalize().as_slice()))
    {
        return Err(invalid_data("capture checkpoint does not match"));
    }
    Ok(())
}

fn encoded_record(record_type: u8, index: u64, length: u32, ciphertext: &[u8]) -> Vec<u8> {
    let mut record = Vec::with_capacity(RECORD_HEADER_LENGTH + ciphertext.len());
    record.push(record_type);
    record.extend_from_slice(&index.to_be_bytes());
    record.extend_from_slice(&length.to_be_bytes());
    record.extend_from_slice(ciphertext);
    record
}

fn read_u64(bytes: &[u8]) -> u64 {
    u64::from_be_bytes(bytes.try_into().expect("u64 slice length is fixed"))
}

fn digest_hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(value, "{byte:02x}");
    }
    value
}

fn derive_file_key(salt: &[u8; 16]) -> io::Result<Zeroizing<[u8; AES_KEY_LENGTH]>> {
    #[cfg(not(test))]
    let database_key = load_or_create_database_key()
        .map_err(|error| io::Error::new(ErrorKind::PermissionDenied, error.to_string()))?;
    #[cfg(test)]
    let database_key = Zeroizing::new([0x5au8; AES_KEY_LENGTH]);
    let root_hkdf = Hkdf::<Sha256>::new(None, database_key.as_slice());
    let mut root = Zeroizing::new([0u8; AES_KEY_LENGTH]);
    root_hkdf
        .expand(b"nauterm/terminal-capture/v1", root.as_mut_slice())
        .map_err(|_| invalid_data("capture root key derivation failed"))?;
    let file_hkdf = Hkdf::<Sha256>::new(Some(salt), root.as_slice());
    let mut file_key = Zeroizing::new([0u8; AES_KEY_LENGTH]);
    file_hkdf
        .expand(b"nauterm/terminal-capture/file/v1", file_key.as_mut_slice())
        .map_err(|_| invalid_data("capture file key derivation failed"))?;
    Ok(file_key)
}

fn derive_state_key(file_key: &[u8]) -> io::Result<Zeroizing<[u8; AES_KEY_LENGTH]>> {
    let hkdf = Hkdf::<Sha256>::new(None, file_key);
    let mut state_key = Zeroizing::new([0u8; AES_KEY_LENGTH]);
    hkdf.expand(
        b"nauterm/terminal-capture/state/v1",
        state_key.as_mut_slice(),
    )
    .map_err(|_| invalid_data("capture state key derivation failed"))?;
    Ok(state_key)
}

fn write_checkpoint_sidecar(
    capture_path: &Path,
    recording_id: &str,
    state_key: &[u8],
    checkpoint: &CaptureCheckpoint,
) -> io::Result<()> {
    let payload = serde_json::to_vec(checkpoint).map_err(|_| invalid_data("invalid checkpoint"))?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(state_key)
        .map_err(|_| invalid_data("invalid capture state key"))?;
    let ciphertext = cipher
        .encrypt(
            &aes_nonce(&nonce_bytes),
            Payload {
                msg: &payload,
                aad: &state_aad(recording_id),
            },
        )
        .map_err(|_| invalid_data("capture checkpoint encryption failed"))?;
    let state_path = checkpoint_path(capture_path);
    let temp_path = checkpoint_temp_path(&state_path);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut state_file = options.open(&temp_path)?;
    state_file.write_all(STATE_MAGIC)?;
    state_file.write_all(&[STATE_VERSION])?;
    state_file.write_all(&nonce_bytes)?;
    state_file.write_all(&ciphertext)?;
    state_file.flush()?;
    state_file.sync_all()?;
    replace_checkpoint_file(&temp_path, &state_path)?;
    #[cfg(unix)]
    fs::set_permissions(&state_path, fs::Permissions::from_mode(0o600))?;
    sync_parent_directory(&state_path)
}

fn read_checkpoint_sidecar(
    capture_path: &Path,
    recording_id: &str,
    state_key: &[u8],
) -> io::Result<Option<CaptureCheckpoint>> {
    let state_path = checkpoint_path(capture_path);
    let mut file = match File::open(&state_path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut header = [0u8; STATE_HEADER_LENGTH];
    file.read_exact(&mut header)?;
    if &header[..7] != STATE_MAGIC || header[7] != STATE_VERSION {
        return Err(invalid_data("invalid capture checkpoint format"));
    }
    let mut ciphertext = Vec::new();
    file.read_to_end(&mut ciphertext)?;
    let cipher = Aes256Gcm::new_from_slice(state_key)
        .map_err(|_| invalid_data("invalid capture state key"))?;
    let plaintext = cipher
        .decrypt(
            &aes_nonce(&header[8..20]),
            Payload {
                msg: &ciphertext,
                aad: &state_aad(recording_id),
            },
        )
        .map_err(|_| invalid_data("capture checkpoint authentication failed"))?;
    let checkpoint = serde_json::from_slice(&plaintext)
        .map_err(|_| invalid_data("invalid capture checkpoint payload"))?;
    Ok(Some(checkpoint))
}

fn remove_checkpoint_sidecar(capture_path: &Path) -> io::Result<()> {
    let state_path = checkpoint_path(capture_path);
    match fs::remove_file(&state_path) {
        Ok(()) => sync_parent_directory(&state_path),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn checkpoint_path(capture_path: &Path) -> PathBuf {
    let mut value: OsString = capture_path.as_os_str().to_owned();
    value.push(".state");
    PathBuf::from(value)
}

fn checkpoint_temp_path(state_path: &Path) -> PathBuf {
    let mut random = [0u8; 8];
    OsRng.fill_bytes(&mut random);
    let mut value: OsString = state_path.as_os_str().to_owned();
    value.push(format!(".{}.tmp", digest_hex(&random)));
    PathBuf::from(value)
}

fn state_aad(recording_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(2 + recording_id.len());
    aad.push(STATE_VERSION);
    aad.extend_from_slice(recording_id.as_bytes());
    aad
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        if let Some(parent) = path.parent() {
            File::open(parent)?.sync_all()?;
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_checkpoint_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_checkpoint_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn nonce(prefix: [u8; 4], index: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..4].copy_from_slice(&prefix);
    nonce[4..].copy_from_slice(&index.to_be_bytes());
    nonce
}

fn aad(recording_id: &str, index: u64, length: u32, record_type: u8) -> Vec<u8> {
    let mut aad = Vec::with_capacity(14 + recording_id.len());
    aad.push(VERSION);
    aad.push(record_type);
    aad.extend_from_slice(&(recording_id.len() as u32).to_be_bytes());
    aad.extend_from_slice(recording_id.as_bytes());
    aad.extend_from_slice(&index.to_be_bytes());
    aad.extend_from_slice(&length.to_be_bytes());
    aad
}

fn ensure_private_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "capture path has no parent"))?;
    #[cfg(unix)]
    {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(parent)?;
    Ok(())
}

pub fn prepare_capture_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(path)?;
    Ok(())
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = TEST_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "nauterm-capture-{}-{unique}-{sequence}",
                std::process::id()
            ))
            .join(name)
    }

    #[test]
    fn encrypted_capture_round_trips_in_chunks() {
        let path = test_path("roundtrip.ntrcap");
        let mut writer = CaptureWriter::open(&path, "recording-1").unwrap();
        writer.append(b"hello ").unwrap();
        writer.append(b"world").unwrap();
        writer.close().unwrap();

        let raw = fs::read(&path).unwrap();
        assert_eq!(&raw[..MAGIC.len()], MAGIC);
        assert!(!raw.windows(11).any(|value| value == b"hello world"));

        let mut reader = CaptureReader::open(&path, "recording-1").unwrap();
        assert_eq!(reader.next_chunk().unwrap().unwrap(), b"hello ");
        assert_eq!(reader.next_chunk().unwrap().unwrap(), b"world");
        assert!(reader.next_chunk().unwrap().is_none());

        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn encrypted_capture_binds_recording_id_and_authenticates_payload() {
        let path = test_path("tamper.ntrcap");
        let mut writer = CaptureWriter::open(&path, "recording-1").unwrap();
        writer.append(b"secret").unwrap();
        writer.close().unwrap();

        let mut wrong_id = CaptureReader::open(&path, "recording-2").unwrap();
        assert!(wrong_id.next_chunk().is_err());

        let mut raw = fs::read(&path).unwrap();
        raw[HEADER_LENGTH + RECORD_HEADER_LENGTH] ^= 0x01;
        fs::write(&path, raw).unwrap();
        let mut tampered = CaptureReader::open(&path, "recording-1").unwrap();
        assert!(tampered.next_chunk().is_err());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn checkpoint_recovery_truncates_only_the_partial_tail_record() {
        let path = test_path("partial-tail.ntrcap");
        let mut writer = CaptureWriter::open(&path, "recording-1").unwrap();
        writer.append(b"committed").unwrap();
        let checkpoint = writer.checkpoint().unwrap();
        let state_path = checkpoint_path(&path);
        assert!(state_path.exists());
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&state_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        writer.append(b"partial-tail").unwrap();
        writer.flush().unwrap();
        drop(writer);

        let partial_length =
            checkpoint.committed_ciphertext_bytes + RECORD_HEADER_LENGTH as u64 + 3;
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(partial_length)
            .unwrap();

        let recovered = recover_capture(&path, "recording-1").unwrap();
        assert_eq!(recovered.chunk_count, 1);
        assert!(!state_path.exists());
        let mut reader = CaptureReader::open(&path, "recording-1").unwrap();
        assert_eq!(reader.next_chunk().unwrap().unwrap(), b"committed");
        assert!(reader.next_chunk().unwrap().is_none());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn recovery_accepts_valid_chunks_written_after_the_last_checkpoint() {
        let path = test_path("extra-valid.ntrcap");
        let mut writer = CaptureWriter::open(&path, "recording-1").unwrap();
        writer.append(b"first").unwrap();
        writer.checkpoint().unwrap();
        writer.append(b"second").unwrap();
        writer.flush().unwrap();
        drop(writer);

        let recovered = recover_capture(&path, "recording-1").unwrap();
        assert_eq!(recovered.chunk_count, 2);
        let expected_hash = digest_hex(Sha256::digest(fs::read(&path).unwrap()).as_slice());
        assert_eq!(recovered.file_sha256, expected_hash);
        let mut reader = CaptureReader::open(&path, "recording-1").unwrap();
        assert_eq!(reader.next_chunk().unwrap().unwrap(), b"first");
        assert_eq!(reader.next_chunk().unwrap().unwrap(), b"second");
        assert!(reader.next_chunk().unwrap().is_none());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn recovery_rejects_a_tampered_checkpoint_sidecar() {
        let path = test_path("state-tamper.ntrcap");
        let mut writer = CaptureWriter::open(&path, "recording-1").unwrap();
        writer.append(b"first").unwrap();
        writer.checkpoint().unwrap();
        drop(writer);
        let state_path = checkpoint_path(&path);
        let mut state = fs::read(&state_path).unwrap();
        let last = state.len() - 1;
        state[last] ^= 1;
        fs::write(&state_path, state).unwrap();

        assert!(recover_capture(&path, "recording-1").is_err());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn reordered_deleted_and_repeated_chunks_are_rejected() {
        let source = test_path("sequence-source.ntrcap");
        let mut writer = CaptureWriter::open(&source, "recording-1").unwrap();
        writer.append(b"first").unwrap();
        writer.append(b"second").unwrap();
        writer.close().unwrap();
        let original = fs::read(&source).unwrap();
        let first_start = HEADER_LENGTH;
        let first_length = u32::from_be_bytes(
            original[first_start + 9..first_start + 13]
                .try_into()
                .unwrap(),
        ) as usize;
        let first_end = first_start + RECORD_HEADER_LENGTH + first_length + TAG_LENGTH;
        let second_start = first_end;
        let second_length = u32::from_be_bytes(
            original[second_start + 9..second_start + 13]
                .try_into()
                .unwrap(),
        ) as usize;
        let second_end = second_start + RECORD_HEADER_LENGTH + second_length + TAG_LENGTH;

        let variants = [
            [
                &original[..first_start],
                &original[second_start..second_end],
                &original[first_start..first_end],
                &original[second_end..],
            ]
            .concat(),
            [&original[..first_start], &original[first_end..]].concat(),
            [
                &original[..first_end],
                &original[first_start..first_end],
                &original[first_end..],
            ]
            .concat(),
        ];
        for (index, bytes) in variants.into_iter().enumerate() {
            let path = source
                .parent()
                .unwrap()
                .join(format!("variant-{index}.ntrcap"));
            fs::write(&path, bytes).unwrap();
            let mut reader = CaptureReader::open(&path, "recording-1").unwrap();
            assert!(reader.verify_complete().is_err());
        }
        fs::remove_dir_all(source.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_different_file_key_cannot_decrypt_chunks() {
        let path = test_path("wrong-key.ntrcap");
        let mut writer = CaptureWriter::open(&path, "recording-1").unwrap();
        writer.append(b"secret").unwrap();
        writer.close().unwrap();
        let mut raw = fs::read(&path).unwrap();
        raw[9] ^= 1;
        fs::write(&path, raw).unwrap();

        let mut reader = CaptureReader::open(&path, "recording-1").unwrap();
        assert!(reader.next_chunk().is_err());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
