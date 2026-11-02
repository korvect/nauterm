use std::collections::HashMap;
use std::ffi::c_char;
use std::path::PathBuf;
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use super::common::{guard, string_from_ptr, string_to_c_ptr};
use crate::capture_crypto::{
    prepare_capture_directory, recover_capture, write_capture_tail, CaptureReader, CaptureWriter,
};
use crate::recording_lease::RecordingLease;

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

fn recording_leases() -> &'static Mutex<HashMap<u64, RecordingLease>> {
    static LEASES: OnceLock<Mutex<HashMap<u64, RecordingLease>>> = OnceLock::new();
    LEASES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[no_mangle]
pub extern "C" fn nauterm_recording_lease_try_acquire(
    directory: *const c_char,
    recording_id: *const c_char,
) -> *mut c_char {
    acquire_recording_lease(directory, recording_id, false)
}

#[no_mangle]
pub extern "C" fn nauterm_recording_lease_try_acquire_existing(
    directory: *const c_char,
    recording_id: *const c_char,
) -> *mut c_char {
    acquire_recording_lease(directory, recording_id, true)
}

fn acquire_recording_lease(
    directory: *const c_char,
    recording_id: *const c_char,
    existing_only: bool,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let result = (|| {
            let directory = string_from_ptr(directory).ok_or("Missing recording directory")?;
            let id = string_from_ptr(recording_id).ok_or("Missing recording ID")?;
            let directory = PathBuf::from(directory);
            let lease = if existing_only {
                RecordingLease::try_acquire_existing(&directory, &id)
            } else {
                RecordingLease::try_acquire(&directory, &id)
            }
            .map_err(|error| error.to_string())?;
            let Some(lease) = lease else {
                return Ok(serde_json::json!({"busy":true}));
            };
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            recording_leases()
                .lock()
                .map_err(|_| "Recording leases are unavailable")?
                .insert(handle, lease);
            Ok::<_, String>(serde_json::json!({"handle":handle}))
        })();
        let value = match result {
            Ok(value) => value,
            Err(error) => serde_json::json!({"error":error}),
        };
        string_to_c_ptr(value.to_string())
    })
}

#[no_mangle]
pub extern "C" fn nauterm_recording_lease_release(handle: u64) -> bool {
    guard(false, || {
        recording_leases()
            .lock()
            .ok()
            .and_then(|mut leases| leases.remove(&handle))
            .is_some()
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_prepare_directory(path: *const c_char) -> bool {
    guard(false, || {
        string_from_ptr(path)
            .map(PathBuf::from)
            .map(|path| prepare_capture_directory(&path).is_ok())
            .unwrap_or(false)
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TailRequest {
    source: PathBuf,
    destination: PathBuf,
    recording_id: String,
    max_bytes: u64,
    expected_sha256: String,
}

#[no_mangle]
pub extern "C" fn nauterm_capture_write_tail(request_json: *const c_char) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let result = (|| {
            let request = string_from_ptr(request_json).ok_or("Missing capture tail request")?;
            let request: TailRequest =
                serde_json::from_str(&request).map_err(|error| error.to_string())?;
            let finalized = write_capture_tail(
                &request.source,
                &request.destination,
                &request.recording_id,
                request.max_bytes,
                &request.expected_sha256,
            )
            .map_err(|error| error.to_string())?;
            serde_json::to_value(finalized).map_err(|error| error.to_string())
        })();
        let value = result.unwrap_or_else(|error: String| serde_json::json!({"error":error}));
        string_to_c_ptr(value.to_string())
    })
}

fn writers() -> &'static Mutex<HashMap<u64, CaptureWriter>> {
    static WRITERS: OnceLock<Mutex<HashMap<u64, CaptureWriter>>> = OnceLock::new();
    WRITERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn readers() -> &'static Mutex<HashMap<u64, CaptureReader>> {
    static READERS: OnceLock<Mutex<HashMap<u64, CaptureReader>>> = OnceLock::new();
    READERS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[no_mangle]
pub extern "C" fn nauterm_capture_writer_open(
    path: *const c_char,
    recording_id: *const c_char,
) -> u64 {
    guard(0, || {
        let Some(path) = string_from_ptr(path) else {
            return 0;
        };
        let Some(recording_id) = string_from_ptr(recording_id) else {
            return 0;
        };
        let Ok(writer) = CaptureWriter::open(&PathBuf::from(path), &recording_id) else {
            return 0;
        };
        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        writers()
            .lock()
            .ok()
            .map(|mut map| map.insert(handle, writer));
        handle
    })
}

#[no_mangle]
// The pointer is owned by the Dart FFI caller and is copied before this call
// returns; null and zero-length inputs are handled explicitly below.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nauterm_capture_writer_append(
    handle: u64,
    bytes: *const u8,
    length: usize,
) -> bool {
    guard(false, || {
        if bytes.is_null() || length == 0 {
            return length == 0;
        }
        let bytes = unsafe { slice::from_raw_parts(bytes, length) };
        writers()
            .lock()
            .ok()
            .and_then(|mut map| {
                map.get_mut(&handle)
                    .map(|writer| writer.append(bytes).is_ok())
            })
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_writer_flush(handle: u64) -> bool {
    guard(false, || {
        writers()
            .lock()
            .ok()
            .and_then(|mut map| map.get_mut(&handle).map(|writer| writer.flush().is_ok()))
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_writer_checkpoint(handle: u64) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let result = writers()
            .lock()
            .ok()
            .and_then(|mut map| map.get_mut(&handle).map(CaptureWriter::checkpoint));
        let value = match result {
            Some(Ok(checkpoint)) => serde_json::to_string(&checkpoint).ok(),
            _ => None,
        };
        value.map(string_to_c_ptr).unwrap_or(ptr::null_mut())
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_writer_finalize(handle: u64) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let result = writers()
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&handle))
            .and_then(|writer| writer.close().ok());
        result
            .and_then(|finalized| serde_json::to_string(&finalized).ok())
            .map(string_to_c_ptr)
            .unwrap_or(ptr::null_mut())
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_writer_close(handle: u64) -> bool {
    guard(false, || {
        writers()
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&handle))
            .map(|writer| writer.close().is_ok())
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_writer_abort(handle: u64) -> bool {
    guard(false, || {
        writers()
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&handle))
            .is_some()
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_reader_open(
    path: *const c_char,
    recording_id: *const c_char,
) -> u64 {
    guard(0, || {
        let Some(path) = string_from_ptr(path) else {
            return 0;
        };
        let Some(recording_id) = string_from_ptr(recording_id) else {
            return 0;
        };
        let Ok(reader) = CaptureReader::open(&PathBuf::from(path), &recording_id) else {
            return 0;
        };
        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        readers()
            .lock()
            .ok()
            .map(|mut map| map.insert(handle, reader));
        handle
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_verify_complete(
    path: *const c_char,
    recording_id: *const c_char,
) -> bool {
    guard(false, || {
        let Some(path) = string_from_ptr(path) else {
            return false;
        };
        let Some(recording_id) = string_from_ptr(recording_id) else {
            return false;
        };
        CaptureReader::open(&PathBuf::from(path), &recording_id)
            .and_then(|mut reader| reader.verify_complete())
            .is_ok()
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_recover(
    path: *const c_char,
    recording_id: *const c_char,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let Some(path) = string_from_ptr(path) else {
            return ptr::null_mut();
        };
        let Some(recording_id) = string_from_ptr(recording_id) else {
            return ptr::null_mut();
        };
        recover_capture(&PathBuf::from(path), &recording_id)
            .ok()
            .and_then(|value| serde_json::to_string(&value).ok())
            .map(string_to_c_ptr)
            .unwrap_or(ptr::null_mut())
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_reader_next(handle: u64) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let result = readers()
            .lock()
            .ok()
            .and_then(|mut map| map.get_mut(&handle).map(CaptureReader::next_chunk));
        let value = match result {
            Some(Ok(Some(bytes))) => {
                serde_json::json!({"done": false, "data": STANDARD.encode(bytes)})
            }
            Some(Ok(None)) => serde_json::json!({"done": true}),
            Some(Err(error)) => serde_json::json!({"done": true, "error": error.to_string()}),
            None => serde_json::json!({"done": true, "error": "capture reader is unavailable"}),
        };
        string_to_c_ptr(value.to_string())
    })
}

#[no_mangle]
pub extern "C" fn nauterm_capture_reader_close(handle: u64) {
    guard((), || {
        if let Ok(mut map) = readers().lock() {
            map.remove(&handle);
        }
    });
}

#[no_mangle]
pub extern "C" fn nauterm_capture_shutdown() {
    guard((), shutdown_captures);
}

pub(crate) fn shutdown_captures() {
    if let Ok(mut map) = writers().lock() {
        for (_, writer) in map.drain() {
            let _ = writer.close();
        }
    }
    if let Ok(mut map) = readers().lock() {
        map.clear();
    }
    if let Ok(mut leases) = recording_leases().lock() {
        leases.clear();
    }
}
