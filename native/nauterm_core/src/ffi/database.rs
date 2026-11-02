//! C adapter for the owned Rust database API.
use crate::{default_database_path, NautermDatabase};
use serde_json::{json, Value};
use std::{
    error::Error,
    ffi::{c_char, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use zeroize::Zeroize;

fn guard<T>(fallback: T, work: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or(fallback)
}

fn response_ok(data: Value) -> *mut c_char {
    response_json(json!({"ok": true, "data": data}))
}

fn response_error(error: impl ToString) -> *mut c_char {
    response_json(json!({"ok": false, "error": error.to_string()}))
}

fn response_json(value: Value) -> *mut c_char {
    let text = serde_json::to_string(&value)
        .unwrap_or_else(|_| r#"{"ok":false,"error":"failed to serialize response"}"#.to_string());
    CString::new(text)
        .unwrap_or_else(|_| CString::new(r#"{"ok":false,"error":"invalid response"}"#).unwrap())
        .into_raw()
}

unsafe fn cstr_to_str<'a>(value: *const c_char) -> Result<&'a str, Box<dyn Error>> {
    if value.is_null() {
        return Err("null string pointer".into());
    }
    Ok(unsafe { CStr::from_ptr(value) }.to_str()?)
}

/// # Safety
///
/// The returned handle must be released with `nauterm_database_destroy`.
#[no_mangle]
pub extern "C" fn nauterm_database_open_default() -> *mut NautermDatabase {
    guard(ptr::null_mut(), || match NautermDatabase::open_default() {
        Ok(database) => Box::into_raw(Box::new(database)),
        Err(_) => ptr::null_mut(),
    })
}

/// # Safety
///
/// `path` must point to a valid UTF-8 C string. The returned handle must be
/// released with `nauterm_database_destroy`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_database_open_path(path: *const c_char) -> *mut NautermDatabase {
    guard(ptr::null_mut(), || {
        let Ok(path) = (unsafe { cstr_to_str(path) }) else {
            return ptr::null_mut();
        };
        match NautermDatabase::open(path) {
            Ok(database) => Box::into_raw(Box::new(database)),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// # Safety
///
/// `handle` must either be null or a pointer returned by
/// `nauterm_database_open_default` or `nauterm_database_open_path`. After this
/// call returns, the handle must not be used again.
#[no_mangle]
pub unsafe extern "C" fn nauterm_database_destroy(handle: *mut NautermDatabase) {
    guard((), || {
        if !handle.is_null() {
            drop(unsafe { Box::from_raw(handle) });
        }
    });
}

/// # Safety
///
/// `handle` must be a live database pointer. `request` must point to a valid
/// UTF-8 JSON C string. The returned string must be released with
/// `nauterm_string_free`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_database_call(
    handle: *mut NautermDatabase,
    request: *const c_char,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if handle.is_null() {
            return response_error("null database handle");
        }
        let request = match unsafe { cstr_to_str(request) } {
            Ok(value) => value,
            Err(error) => return response_error(error),
        };

        let database = unsafe { &mut *handle };
        match database.handle_json_request(request) {
            Ok(data) => response_ok(data),
            Err(error) => response_error(error),
        }
    })
}

#[no_mangle]
pub extern "C" fn nauterm_database_default_path() -> *mut c_char {
    guard(ptr::null_mut(), || {
        CString::new(default_database_path().to_string_lossy().as_bytes())
            .unwrap()
            .into_raw()
    })
}

/// # Safety
///
/// `value` must either be null or a string returned by this library.
#[no_mangle]
pub unsafe extern "C" fn nauterm_string_free(value: *mut c_char) {
    guard((), || {
        if !value.is_null() {
            // Reconstruct before erasing: CString::from_raw needs the original
            // NUL position to recover its allocation layout. Responses may hold
            // passwords or private keys, including from the Flutter adapter.
            let string = unsafe { CString::from_raw(value) };
            let mut bytes = string.into_bytes_with_nul();
            bytes.zeroize();
        }
    });
}
