use std::ffi::c_char;
use std::ptr;

use crate::ssh;

use super::common::{guard, optional_string_from_ptr, string_from_ptr, string_to_c_ptr};

fn json_value_to_c_ptr(value: serde_json::Value) -> *mut c_char {
    serde_json::to_string(&value)
        .ok()
        .map(string_to_c_ptr)
        .unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn nauterm_ssh_list_directories(
    host: *const c_char,
    port: u16,
    username: *const c_char,
    password: *const c_char,
    private_key: *const c_char,
    certificate: *const c_char,
    passphrase: *const c_char,
    known_hosts_path: *const c_char,
    directory: *const c_char,
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
        let value = match ssh::list_directory_entries_blocking(
            &host,
            port,
            &username,
            password.as_deref(),
            private_key.as_deref(),
            certificate.as_deref(),
            passphrase.as_deref(),
            known_hosts_path.as_deref(),
            &directory,
        ) {
            Ok(result) => serde_json::json!({
                "directory": result.directory,
                "entries": result
                    .entries
                    .into_iter()
                    .filter(|entry| entry.is_directory)
                    .map(|entry| entry.name)
                    .collect::<Vec<_>>(),
                "error": null,
            }),
            Err(error) => serde_json::json!({
                "entries": [],
                "error": error,
            }),
        };
        json_value_to_c_ptr(value)
    })
}

#[no_mangle]
pub extern "C" fn nauterm_ssh_list_directory_entries(
    host: *const c_char,
    port: u16,
    username: *const c_char,
    password: *const c_char,
    private_key: *const c_char,
    certificate: *const c_char,
    passphrase: *const c_char,
    known_hosts_path: *const c_char,
    directory: *const c_char,
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
        let value = match ssh::list_directory_entries_blocking(
            &host,
            port,
            &username,
            password.as_deref(),
            private_key.as_deref(),
            certificate.as_deref(),
            passphrase.as_deref(),
            known_hosts_path.as_deref(),
            &directory,
        ) {
            Ok(result) => serde_json::json!({
                "directory": result.directory,
                "entries": result.entries,
                "error": null,
            }),
            Err(error) => serde_json::json!({
                "entries": [],
                "error": error,
            }),
        };
        json_value_to_c_ptr(value)
    })
}

#[no_mangle]
pub extern "C" fn nauterm_ssh_detect_host_os(
    host: *const c_char,
    port: u16,
    username: *const c_char,
    password: *const c_char,
    private_key: *const c_char,
    certificate: *const c_char,
    passphrase: *const c_char,
    known_hosts_path: *const c_char,
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
        let proxy = ssh::proxy_config_from_json_ptr(proxy_json);
        let result = ssh::detect_host_os_blocking_with_trust(
            &host,
            port,
            &username,
            password.as_deref(),
            private_key.as_deref(),
            certificate.as_deref(),
            passphrase.as_deref(),
            known_hosts_path.as_deref(),
            proxy,
            ssh::HostKeyTrustMode::from_u32(host_key_trust_mode),
        );
        json_value_to_c_ptr(serde_json::to_value(result).unwrap_or_else(|error| {
            serde_json::json!({
                "os": null,
                "distro": null,
                "events": [],
                "error": format!("failed to encode host OS detection result: {error}"),
            })
        }))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_ssh_collect_system_info(
    host: *const c_char,
    port: u16,
    username: *const c_char,
    password: *const c_char,
    private_key: *const c_char,
    certificate: *const c_char,
    passphrase: *const c_char,
    known_hosts_path: *const c_char,
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
        let proxy = ssh::proxy_config_from_json_ptr(proxy_json);
        let result = ssh::collect_host_system_info_blocking_with_trust(
            &host,
            port,
            &username,
            password.as_deref(),
            private_key.as_deref(),
            certificate.as_deref(),
            passphrase.as_deref(),
            known_hosts_path.as_deref(),
            proxy,
            ssh::HostKeyTrustMode::from_u32(host_key_trust_mode),
        );
        json_value_to_c_ptr(serde_json::to_value(result).unwrap_or_else(|error| {
            serde_json::json!({
                "events": [],
                "error": format!("failed to encode host system information: {error}"),
            })
        }))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_ssh_export_public_key(
    host: *const c_char,
    port: u16,
    username: *const c_char,
    password: *const c_char,
    private_key: *const c_char,
    certificate: *const c_char,
    passphrase: *const c_char,
    known_hosts_path: *const c_char,
    host_key_trust_mode: u32,
    proxy_json: *const c_char,
    public_key: *const c_char,
    location: *const c_char,
    filename: *const c_char,
    script: *const c_char,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let host = string_from_ptr(host).unwrap_or_default();
        let username = string_from_ptr(username).unwrap_or_default();
        let password = optional_string_from_ptr(password);
        let private_key = optional_string_from_ptr(private_key);
        let certificate = optional_string_from_ptr(certificate);
        let passphrase = optional_string_from_ptr(passphrase);
        let known_hosts_path = optional_string_from_ptr(known_hosts_path);
        let proxy = ssh::proxy_config_from_json_ptr(proxy_json);
        let public_key = string_from_ptr(public_key).unwrap_or_default();
        let location = string_from_ptr(location).unwrap_or_default();
        let filename = string_from_ptr(filename).unwrap_or_default();
        let script = string_from_ptr(script).unwrap_or_default();
        let result = ssh::export_public_key_blocking_with_trust(
            &host,
            port,
            &username,
            password.as_deref(),
            private_key.as_deref(),
            certificate.as_deref(),
            passphrase.as_deref(),
            known_hosts_path.as_deref(),
            proxy,
            &public_key,
            &location,
            &filename,
            &script,
            ssh::HostKeyTrustMode::from_u32(host_key_trust_mode),
        );
        json_value_to_c_ptr(serde_json::to_value(result).unwrap_or_else(|error| {
            serde_json::json!({
                "ok": false,
                "events": [],
                "error": format!("failed to encode SSH key export result: {error}"),
            })
        }))
    })
}
