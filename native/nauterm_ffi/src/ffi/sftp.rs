use std::ffi::{c_char, c_void};
use std::ptr;

use crate::ssh::{self, HostKeyTrustMode};

use super::common::{guard, optional_string_from_ptr, string_from_ptr, string_to_c_ptr};

type FfiSftpTaskProgressCallback = Option<extern "C" fn(*mut c_void, u64, u64, *const c_char)>;

fn json_value_to_c_ptr(value: serde_json::Value) -> *mut c_char {
    serde_json::to_string(&value)
        .ok()
        .map(string_to_c_ptr)
        .unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn nauterm_sftp_list_directory_entries(
    request_id: u64,
    host: *const c_char,
    port: u16,
    username: *const c_char,
    password: *const c_char,
    private_key: *const c_char,
    certificate: *const c_char,
    passphrase: *const c_char,
    known_hosts_path: *const c_char,
    directory: *const c_char,
    host_key_trust_mode: u32,
    proxy_json: *const c_char,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let host = string_from_ptr(host).unwrap_or_default();
        let username = string_from_ptr(username).unwrap_or_default();
        let password = optional_string_from_ptr(password);
        let private_key = optional_string_from_ptr(private_key);
        let certificate = optional_string_from_ptr(certificate);
        let passphrase = optional_string_from_ptr(passphrase);
        let known_hosts_path = optional_string_from_ptr(known_hosts_path);
        let directory = string_from_ptr(directory).unwrap_or_else(|| "~".to_owned());
        let proxy = ssh::proxy_config_from_json_ptr(proxy_json);
        let result = ssh::list_sftp_directory_entries_blocking_with_trust(
            request_id,
            &host,
            port,
            &username,
            password.as_deref(),
            private_key.as_deref(),
            certificate.as_deref(),
            passphrase.as_deref(),
            known_hosts_path.as_deref(),
            proxy,
            &directory,
            HostKeyTrustMode::from_u32(host_key_trust_mode),
        );
        json_value_to_c_ptr(serde_json::json!({
            "directory": result.directory,
            "entries": result.entries,
            "events": result.events,
            "error": result.error,
        }))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_sftp_execute_task(
    task_id: u64,
    host: *const c_char,
    port: u16,
    username: *const c_char,
    password: *const c_char,
    private_key: *const c_char,
    certificate: *const c_char,
    passphrase: *const c_char,
    known_hosts_path: *const c_char,
    operation_json: *const c_char,
    host_key_trust_mode: u32,
    proxy_json: *const c_char,
    progress_callback: FfiSftpTaskProgressCallback,
    progress_user_data: *mut c_void,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let host = string_from_ptr(host).unwrap_or_default();
        let username = string_from_ptr(username).unwrap_or_default();
        let password = optional_string_from_ptr(password);
        let private_key = optional_string_from_ptr(private_key);
        let certificate = optional_string_from_ptr(certificate);
        let passphrase = optional_string_from_ptr(passphrase);
        let known_hosts_path = optional_string_from_ptr(known_hosts_path);
        let operation_json = string_from_ptr(operation_json).unwrap_or_else(|| "{}".to_owned());
        let proxy = ssh::proxy_config_from_json_ptr(proxy_json);
        let result = ssh::execute_sftp_task_blocking_with_trust(
            task_id,
            &host,
            port,
            &username,
            password.as_deref(),
            private_key.as_deref(),
            certificate.as_deref(),
            passphrase.as_deref(),
            known_hosts_path.as_deref(),
            proxy,
            &operation_json,
            HostKeyTrustMode::from_u32(host_key_trust_mode),
            progress_callback,
            progress_user_data,
        );
        json_value_to_c_ptr(serde_json::json!({
            "ok": result.ok,
            "bytes": result.bytes,
            "item_kind": result.item_kind,
            "events": result.events,
            "error": result.error,
        }))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_sftp_cancel_task(task_id: u64) -> bool {
    guard(false, || ssh::cancel_sftp_task(task_id))
}

#[no_mangle]
pub extern "C" fn nauterm_sftp_close_sudo_session(session_id: *const c_char) -> bool {
    guard(false, || {
        let session_id = string_from_ptr(session_id).unwrap_or_default();
        ssh::close_sudo_sftp_session(&session_id)
    })
}
