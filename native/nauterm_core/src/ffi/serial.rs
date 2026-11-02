use std::ffi::c_char;
use std::ptr;

use crate::serial;

use super::common::{guard, string_to_c_ptr};

#[no_mangle]
pub extern "C" fn nauterm_serial_list_ports() -> *mut c_char {
    guard(ptr::null_mut(), || {
        serde_json::to_string(&serial::list_serial_ports())
            .ok()
            .map(string_to_c_ptr)
            .unwrap_or(ptr::null_mut())
    })
}
