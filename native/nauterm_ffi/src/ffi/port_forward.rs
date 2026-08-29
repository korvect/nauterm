use std::ffi::{c_char, CString};
use std::ptr;

use crate::port_forward::{port_forward_manager, PortForwardConfig, PortForwardStatus};
use crate::ssh;

use super::common::{guard, optional_string_from_ptr, string_from_ptr};

fn port_forward_status_json<T: serde::Serialize>(status: T) -> *mut c_char {
    serde_json::to_string(&status)
        .ok()
        .and_then(|json| CString::new(json).ok())
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

#[allow(clippy::too_many_arguments)]
#[no_mangle]
pub extern "C" fn nauterm_port_forward_start(
    id: u64,
    forward_type: *const c_char,
    ssh_host: *const c_char,
    ssh_port: u16,
    username: *const c_char,
    password: *const c_char,
    private_key: *const c_char,
    certificate: *const c_char,
    passphrase: *const c_char,
    known_hosts_path: *const c_char,
    bind_address: *const c_char,
    bind_port: u16,
    destination_host: *const c_char,
    destination_port: u16,
    proxy_json: *const c_char,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let config = PortForwardConfig {
            id,
            forward_type: string_from_ptr(forward_type)
                .unwrap_or_else(|| "local".to_owned())
                .to_lowercase(),
            ssh_host: string_from_ptr(ssh_host).unwrap_or_default(),
            ssh_port,
            username: string_from_ptr(username).unwrap_or_else(|| "user".to_owned()),
            password: optional_string_from_ptr(password),
            private_key: optional_string_from_ptr(private_key),
            certificate: optional_string_from_ptr(certificate),
            passphrase: optional_string_from_ptr(passphrase),
            known_hosts_path: optional_string_from_ptr(known_hosts_path),
            proxy: ssh::proxy_config_from_json_ptr(proxy_json),
            bind_address: string_from_ptr(bind_address).unwrap_or_else(|| "127.0.0.1".to_owned()),
            bind_port,
            destination_host: optional_string_from_ptr(destination_host),
            destination_port,
        };
        if config.ssh_host.trim().is_empty() {
            return port_forward_status_json(PortForwardStatus {
                id,
                state: "error".to_owned(),
                error: Some("SSH host is required.".to_owned()),
                bound_port: None,
                active_connections: 0,
            });
        }
        match port_forward_manager().lock() {
            Ok(mut manager) => port_forward_status_json(manager.start(config)),
            Err(_) => port_forward_status_json(PortForwardStatus {
                id,
                state: "error".to_owned(),
                error: Some("Port forward manager is unavailable.".to_owned()),
                bound_port: None,
                active_connections: 0,
            }),
        }
    })
}

#[no_mangle]
pub extern "C" fn nauterm_port_forward_stop(id: u64) -> bool {
    guard(false, || {
        port_forward_manager()
            .lock()
            .map(|mut manager| manager.stop(id))
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn nauterm_port_forward_stop_all() -> u32 {
    guard(0, || {
        port_forward_manager()
            .lock()
            .map(|mut manager| manager.stop_all() as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn nauterm_port_forward_status(id: u64) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let status = port_forward_manager()
            .lock()
            .map(|manager| manager.status(id))
            .unwrap_or(PortForwardStatus {
                id,
                state: "error".to_owned(),
                error: Some("Port forward manager is unavailable.".to_owned()),
                bound_port: None,
                active_connections: 0,
            });
        port_forward_status_json(status)
    })
}
