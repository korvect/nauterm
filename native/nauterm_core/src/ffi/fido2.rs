use std::ffi::c_char;

use serde::Deserialize;
use serde::Serialize;

use super::common::{guard, string_from_ptr, string_to_c_ptr};
use crate::fido2::{self, Fido2GenerateRequest};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Fido2Response<T: Serialize> {
    ok: bool,
    value: Option<T>,
    error: Option<String>,
}

impl<T: Serialize> Fido2Response<T> {
    fn success(value: T) -> Self {
        Self {
            ok: true,
            value: Some(value),
            error: None,
        }
    }

    fn failure(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            value: None,
            error: Some(error.into()),
        }
    }
}

fn response_json<T: Serialize>(response: Fido2Response<T>) -> *mut c_char {
    string_to_c_ptr(serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"ok":false,"value":null,"error":"Unable to encode FIDO2 response."}"#.to_owned()
    }))
}

#[no_mangle]
pub extern "C" fn nauterm_fido2_list_devices() -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        response_json(Fido2Response::success(fido2::list_devices()))
    })
}

#[no_mangle]
/// Generates an SSH FIDO2 security key from a JSON request.
///
/// # Safety
///
/// `request_json` must either be null or point to a valid, NUL-terminated C
/// string for the duration of this call. The returned string must be released
/// with `nauterm_string_free`.
pub unsafe extern "C" fn nauterm_fido2_generate(request_json: *const c_char) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        let Some(request_json) = string_from_ptr(request_json) else {
            return response_json(Fido2Response::<()>::failure("Missing FIDO2 request."));
        };
        let request = match serde_json::from_str::<Fido2GenerateRequest>(&request_json) {
            Ok(request) => request,
            Err(error) => return response_json(Fido2Response::<()>::failure(error.to_string())),
        };
        match fido2::generate_key(request) {
            Ok(key) => response_json(Fido2Response::success(key)),
            Err(error) => response_json(Fido2Response::<()>::failure(error)),
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fido2VerifyPinRequest {
    device_id: String,
    pin: String,
}

#[no_mangle]
/// Verifies a FIDO2 device PIN from a JSON request.
///
/// # Safety
///
/// `request_json` must either be null or point to a valid, NUL-terminated C
/// string for the duration of this call. The returned string must be released
/// with `nauterm_string_free`.
pub unsafe extern "C" fn nauterm_fido2_verify_pin(request_json: *const c_char) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        let Some(request_json) = string_from_ptr(request_json) else {
            return response_json(Fido2Response::<()>::failure("Missing FIDO2 PIN request."));
        };
        let request = match serde_json::from_str::<Fido2VerifyPinRequest>(&request_json) {
            Ok(request) => request,
            Err(error) => return response_json(Fido2Response::<()>::failure(error.to_string())),
        };
        match fido2::verify_pin(request.device_id, request.pin) {
            Ok(()) => response_json(Fido2Response::success(())),
            Err(error) => response_json(Fido2Response::<()>::failure(error)),
        }
    })
}
