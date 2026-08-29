use std::ffi::{c_char, c_void};
use std::ptr;
use std::slice;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::mosh::MoshServerCommand;
use crate::pty::WakeupCallback;
use crate::serial::{SerialConfig, SerialFlowControl, SerialParity};
use crate::session::{SessionId, SessionManager};
use crate::ssh::{self, HostKeyTrustMode};
use crate::terminal::{TerminalCommand, TerminalSearchDirection};

use super::common::{
    guard, hex_encode, string_from_ptr, string_list_from_ptr, string_to_c_ptr,
    terminal_options_from_args,
};
use super::snapshot::{snapshot_into_ffi, FfiTerminalSnapshot};

fn session_manager() -> &'static Mutex<SessionManager> {
    static MANAGER: OnceLock<Mutex<SessionManager>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(SessionManager::default()))
}

fn with_session_manager<T>(fallback: T, work: impl FnOnce(&mut SessionManager) -> T) -> T {
    match session_manager().lock() {
        Ok(mut manager) => work(&mut manager),
        Err(_) => fallback,
    }
}

pub(crate) fn prepare_sessions_for_runtime_shutdown() {
    with_session_manager((), |manager| manager.prepare_runtime_shutdown());
}

#[no_mangle]
pub extern "C" fn nauterm_session_create_local_configured(
    columns: u32,
    rows: u32,
    emulator_backend: u32,
    scrollback_lines: u32,
    terminal_type: *const c_char,
    color_term: u32,
    osc52_mode: u32,
    cursor_shape: u32,
    cursor_blinking: bool,
    default_foreground: u32,
    default_background: u32,
    default_cursor: u32,
    shell_path: *const c_char,
    working_directory: *const c_char,
    environment: *const c_char,
) -> SessionId {
    guard(0, || {
        let options = terminal_options_from_args(
            emulator_backend,
            scrollback_lines,
            terminal_type,
            color_term,
            osc52_mode,
            cursor_shape,
            cursor_blinking,
            default_foreground,
            default_background,
            default_cursor,
            shell_path,
            working_directory,
            environment,
        );
        with_session_manager(0, |manager| {
            manager
                .create_local(columns as usize, rows as usize, options)
                .unwrap_or(0)
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_create_command_configured(
    columns: u32,
    rows: u32,
    emulator_backend: u32,
    scrollback_lines: u32,
    terminal_type: *const c_char,
    color_term: u32,
    osc52_mode: u32,
    cursor_shape: u32,
    cursor_blinking: bool,
    default_foreground: u32,
    default_background: u32,
    default_cursor: u32,
    program: *const c_char,
    args: *const c_char,
    working_directory: *const c_char,
    environment: *const c_char,
) -> SessionId {
    guard(0, || {
        let Some(program) = string_from_ptr(program).filter(|program| !program.trim().is_empty())
        else {
            return 0;
        };
        let mut options = terminal_options_from_args(
            emulator_backend,
            scrollback_lines,
            terminal_type,
            color_term,
            osc52_mode,
            cursor_shape,
            cursor_blinking,
            default_foreground,
            default_background,
            default_cursor,
            ptr::null(),
            working_directory,
            environment,
        );
        options.command = Some(TerminalCommand {
            program,
            args: string_list_from_ptr(args),
        });
        with_session_manager(0, |manager| {
            manager
                .create_local(columns as usize, rows as usize, options)
                .unwrap_or(0)
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_create_ssh_configured(
    columns: u32,
    rows: u32,
    emulator_backend: u32,
    scrollback_lines: u32,
    terminal_type: *const c_char,
    color_term: u32,
    osc52_mode: u32,
    cursor_shape: u32,
    cursor_blinking: bool,
    default_foreground: u32,
    default_background: u32,
    default_cursor: u32,
    ssh_keepalive_interval_seconds: u32,
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
    environment: *const c_char,
    encoding: *const c_char,
) -> SessionId {
    guard(0, || {
        let options = terminal_options_from_args(
            emulator_backend,
            scrollback_lines,
            terminal_type,
            color_term,
            osc52_mode,
            cursor_shape,
            cursor_blinking,
            default_foreground,
            default_background,
            default_cursor,
            ptr::null(),
            ptr::null(),
            environment,
        );
        let host = string_from_ptr(host).unwrap_or_else(|| "unknown-host".to_owned());
        let username = string_from_ptr(username).unwrap_or_else(|| "user".to_owned());
        let password = string_from_ptr(password).filter(|password| !password.is_empty());
        let private_key =
            string_from_ptr(private_key).filter(|private_key| !private_key.is_empty());
        let certificate =
            string_from_ptr(certificate).filter(|certificate| !certificate.is_empty());
        let passphrase = string_from_ptr(passphrase).filter(|passphrase| !passphrase.is_empty());
        let known_hosts_path = string_from_ptr(known_hosts_path)
            .filter(|known_hosts_path| !known_hosts_path.is_empty());
        let proxy = ssh::proxy_config_from_json_ptr(proxy_json);
        let encoding = string_from_ptr(encoding).unwrap_or_else(|| "UTF-8".to_owned());
        with_session_manager(0, |manager| {
            manager.create_ssh(
                columns as usize,
                rows as usize,
                options,
                &host,
                port,
                &username,
                password.as_deref(),
                private_key.as_deref(),
                certificate.as_deref(),
                passphrase.as_deref(),
                known_hosts_path.as_deref(),
                HostKeyTrustMode::from_u32(host_key_trust_mode),
                proxy,
                ssh_keepalive_interval_seconds,
                &encoding,
            )
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_create_mosh_configured(
    columns: u32,
    rows: u32,
    emulator_backend: u32,
    scrollback_lines: u32,
    terminal_type: *const c_char,
    color_term: u32,
    osc52_mode: u32,
    cursor_shape: u32,
    cursor_blinking: bool,
    default_foreground: u32,
    default_background: u32,
    default_cursor: u32,
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
    environment: *const c_char,
    server_command: *const c_char,
) -> SessionId {
    guard(0, || {
        let options = terminal_options_from_args(
            emulator_backend,
            scrollback_lines,
            terminal_type,
            color_term,
            osc52_mode,
            cursor_shape,
            cursor_blinking,
            default_foreground,
            default_background,
            default_cursor,
            ptr::null(),
            ptr::null(),
            environment,
        );
        let host = string_from_ptr(host).unwrap_or_else(|| "unknown-host".to_owned());
        let username = string_from_ptr(username).unwrap_or_else(|| "user".to_owned());
        let password = string_from_ptr(password).filter(|value| !value.is_empty());
        let private_key = string_from_ptr(private_key).filter(|value| !value.is_empty());
        let certificate = string_from_ptr(certificate).filter(|value| !value.is_empty());
        let passphrase = string_from_ptr(passphrase).filter(|value| !value.is_empty());
        let known_hosts_path = string_from_ptr(known_hosts_path).filter(|value| !value.is_empty());
        let proxy = ssh::proxy_config_from_json_ptr(proxy_json);
        let server_command = MoshServerCommand {
            command: string_from_ptr(server_command)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "mosh-server new -s -l LANG=en_US.UTF-8".to_owned()),
        };
        with_session_manager(0, |manager| {
            manager.create_mosh(
                columns as usize,
                rows as usize,
                options,
                &host,
                port,
                &username,
                password.as_deref(),
                private_key.as_deref(),
                certificate.as_deref(),
                passphrase.as_deref(),
                known_hosts_path.as_deref(),
                HostKeyTrustMode::from_u32(host_key_trust_mode),
                proxy,
                server_command,
            )
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_reconnect_ssh(
    session_id: SessionId,
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
    ssh_keepalive_interval_seconds: u32,
    encoding: *const c_char,
) -> bool {
    guard(false, || {
        let host = string_from_ptr(host).unwrap_or_else(|| "unknown-host".to_owned());
        let username = string_from_ptr(username).unwrap_or_else(|| "user".to_owned());
        let password = string_from_ptr(password).filter(|value| !value.is_empty());
        let private_key = string_from_ptr(private_key).filter(|value| !value.is_empty());
        let certificate = string_from_ptr(certificate).filter(|value| !value.is_empty());
        let passphrase = string_from_ptr(passphrase).filter(|value| !value.is_empty());
        let known_hosts_path = string_from_ptr(known_hosts_path).filter(|value| !value.is_empty());
        let proxy = ssh::proxy_config_from_json_ptr(proxy_json);
        let encoding = string_from_ptr(encoding).unwrap_or_else(|| "UTF-8".to_owned());
        with_session_manager(false, |manager| {
            manager.reconnect_ssh(
                session_id,
                &host,
                port,
                &username,
                password.as_deref(),
                private_key.as_deref(),
                certificate.as_deref(),
                passphrase.as_deref(),
                known_hosts_path.as_deref(),
                HostKeyTrustMode::from_u32(host_key_trust_mode),
                proxy,
                ssh_keepalive_interval_seconds,
                &encoding,
            )
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_reconnect_mosh(
    session_id: SessionId,
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
    server_command: *const c_char,
) -> bool {
    guard(false, || {
        let host = string_from_ptr(host).unwrap_or_else(|| "unknown-host".to_owned());
        let username = string_from_ptr(username).unwrap_or_else(|| "user".to_owned());
        let password = string_from_ptr(password).filter(|value| !value.is_empty());
        let private_key = string_from_ptr(private_key).filter(|value| !value.is_empty());
        let certificate = string_from_ptr(certificate).filter(|value| !value.is_empty());
        let passphrase = string_from_ptr(passphrase).filter(|value| !value.is_empty());
        let known_hosts_path = string_from_ptr(known_hosts_path).filter(|value| !value.is_empty());
        let proxy = ssh::proxy_config_from_json_ptr(proxy_json);
        let server_command = MoshServerCommand {
            command: string_from_ptr(server_command)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "mosh-server new -s -l LANG=en_US.UTF-8".to_owned()),
        };
        with_session_manager(false, |manager| {
            manager.reconnect_mosh(
                session_id,
                &host,
                port,
                &username,
                password.as_deref(),
                private_key.as_deref(),
                certificate.as_deref(),
                passphrase.as_deref(),
                known_hosts_path.as_deref(),
                HostKeyTrustMode::from_u32(host_key_trust_mode),
                proxy,
                server_command,
            )
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_create_telnet_configured(
    columns: u32,
    rows: u32,
    emulator_backend: u32,
    scrollback_lines: u32,
    terminal_type: *const c_char,
    color_term: u32,
    osc52_mode: u32,
    cursor_shape: u32,
    cursor_blinking: bool,
    default_foreground: u32,
    default_background: u32,
    default_cursor: u32,
    host: *const c_char,
    port: u16,
    encoding: *const c_char,
    environment: *const c_char,
) -> SessionId {
    guard(0, || {
        let options = terminal_options_from_args(
            emulator_backend,
            scrollback_lines,
            terminal_type,
            color_term,
            osc52_mode,
            cursor_shape,
            cursor_blinking,
            default_foreground,
            default_background,
            default_cursor,
            ptr::null(),
            ptr::null(),
            environment,
        );
        let host = string_from_ptr(host).unwrap_or_else(|| "unknown-host".to_owned());
        let encoding = string_from_ptr(encoding).unwrap_or_else(|| "UTF-8".to_owned());
        with_session_manager(0, |manager| {
            manager.create_telnet(
                columns as usize,
                rows as usize,
                options,
                &host,
                port,
                &encoding,
            )
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_create_serial_configured(
    columns: u32,
    rows: u32,
    emulator_backend: u32,
    scrollback_lines: u32,
    terminal_type: *const c_char,
    color_term: u32,
    osc52_mode: u32,
    cursor_shape: u32,
    cursor_blinking: bool,
    default_foreground: u32,
    default_background: u32,
    default_cursor: u32,
    serial_port: *const c_char,
    baud_rate: u32,
    data_bits: u32,
    parity: u32,
    stop_bits: u32,
    flow_control: u32,
) -> SessionId {
    guard(0, || {
        let options = terminal_options_from_args(
            emulator_backend,
            scrollback_lines,
            terminal_type,
            color_term,
            osc52_mode,
            cursor_shape,
            cursor_blinking,
            default_foreground,
            default_background,
            default_cursor,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        );
        let serial_port = string_from_ptr(serial_port).unwrap_or_else(|| "unknown-port".to_owned());
        let config =
            match serial_config_from_args(baud_rate, data_bits, parity, stop_bits, flow_control) {
                Ok(config) => config,
                Err(error) => {
                    return with_session_manager(0, |manager| {
                        manager.create_serial_error(
                            columns as usize,
                            rows as usize,
                            options,
                            &serial_port,
                            baud_rate,
                            error,
                        )
                    });
                }
            };
        with_session_manager(0, |manager| {
            manager.create_serial(
                columns as usize,
                rows as usize,
                options,
                &serial_port,
                config,
            )
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_close(session_id: SessionId) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| manager.close(session_id))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_resize(
    session_id: SessionId,
    columns: u32,
    rows: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| {
            manager.resize(
                session_id,
                columns as usize,
                rows as usize,
                cell_width_px,
                cell_height_px,
            )
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_read_shell_history(session_id: SessionId) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let request =
            with_session_manager(Err("session manager unavailable".to_owned()), |manager| {
                manager.request_ssh_shell_history(session_id)
            });
        let result = request.and_then(|receiver| {
            receiver
                .recv_timeout(Duration::from_secs(8))
                .map_err(|_| "SSH shell history request timed out.".to_owned())?
        });
        let value = match result {
            Ok((shell, content)) => serde_json::json!({
                "shell": shell,
                "content": content,
                "error": null,
            }),
            Err(error) => serde_json::json!({
                "shell": null,
                "content": "",
                "error": error,
            }),
        };
        string_to_c_ptr(value.to_string())
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_notify_network_changed(session_id: SessionId) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| manager.notify_network_changed(session_id))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_exit_alternate_screen(session_id: SessionId) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| manager.exit_alternate_screen(session_id))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_scroll_lines(session_id: SessionId, lines: i32) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| manager.scroll_lines(session_id, lines))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_scroll_page_up(session_id: SessionId) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| manager.scroll_page_up(session_id))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_scroll_page_down(session_id: SessionId) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| manager.scroll_page_down(session_id))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_scroll_to_bottom(session_id: SessionId) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| manager.scroll_to_bottom(session_id))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_search(
    session_id: SessionId,
    query: *const c_char,
    direction: u32,
    origin_row: u32,
    origin_column: u32,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let query = string_from_ptr(query).unwrap_or_default();
        let result = with_session_manager(
            crate::terminal::TerminalSearchResult::not_found(0, 0),
            |manager| {
                manager.search(
                    session_id,
                    &query,
                    TerminalSearchDirection::from_u32(direction),
                    origin_row as usize,
                    origin_column as usize,
                )
            },
        );
        string_to_c_ptr(serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_owned()))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_selection_text(
    session_id: SessionId,
    start: i64,
    end: i64,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        with_session_manager(None, |manager| {
            manager.selection_text(session_id, start, end)
        })
        .map(string_to_c_ptr)
        .unwrap_or(ptr::null_mut())
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_command_block_at(
    session_id: SessionId,
    offset: i64,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let block =
            with_session_manager(None, |manager| manager.command_block_at(session_id, offset));
        string_to_c_ptr(serde_json::to_string(&block).unwrap_or_else(|_| "null".to_owned()))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_prompt_click_move(
    session_id: SessionId,
    offset: i64,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let movement = with_session_manager(None, |manager| {
            manager.prompt_click_move(session_id, offset)
        });
        string_to_c_ptr(serde_json::to_string(&movement).unwrap_or_else(|_| "null".to_owned()))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_set_wakeup_callback(
    session_id: SessionId,
    callback: Option<extern "C" fn(*mut c_void)>,
    user_data: *mut c_void,
) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| {
            manager.set_wakeup_callback(
                session_id,
                callback.map(|callback| WakeupCallback::new(callback, user_data)),
            )
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_poll(session_id: SessionId) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| manager.poll(session_id))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_is_exited(session_id: SessionId) -> bool {
    guard(true, || {
        with_session_manager(true, |manager| manager.is_exited(session_id))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_write_codepoint(session_id: SessionId, codepoint: u32) -> bool {
    guard(false, || {
        let Some(character) = char::from_u32(codepoint) else {
            return false;
        };

        let mut buffer = [0; 4];
        let bytes = character.encode_utf8(&mut buffer).as_bytes();
        with_session_manager(false, |manager| manager.write_bytes(session_id, bytes))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_send_input_codepoint(
    session_id: SessionId,
    codepoint: u32,
) -> bool {
    guard(false, || {
        let Some(character) = char::from_u32(codepoint) else {
            return false;
        };

        let mut buffer = [0; 4];
        let bytes = character.encode_utf8(&mut buffer).as_bytes();
        with_session_manager(false, |manager| manager.send_input_bytes(session_id, bytes))
    })
}

/// # Safety
///
/// When `len` is non-zero, `bytes` must point to at least `len` readable bytes
/// for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn nauterm_session_write_bytes(
    session_id: SessionId,
    bytes: *const u8,
    len: usize,
) -> bool {
    guard(false, || {
        if len == 0 {
            return true;
        }
        if bytes.is_null() {
            return false;
        }

        let bytes = unsafe { slice::from_raw_parts(bytes, len) };
        with_session_manager(false, |manager| manager.write_bytes(session_id, bytes))
    })
}

/// # Safety
///
/// When `len` is non-zero, `bytes` must point to at least `len` readable bytes
/// for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn nauterm_session_send_input_bytes(
    session_id: SessionId,
    bytes: *const u8,
    len: usize,
) -> bool {
    guard(false, || {
        if len == 0 {
            return true;
        }
        if bytes.is_null() {
            return false;
        }

        let bytes = unsafe { slice::from_raw_parts(bytes, len) };
        with_session_manager(false, |manager| manager.send_input_bytes(session_id, bytes))
    })
}

/// # Safety
///
/// When `len` is non-zero, `bytes` must point to at least `len` readable bytes
/// for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn nauterm_session_send_input_bytes_status(
    session_id: SessionId,
    bytes: *const u8,
    len: usize,
) -> u32 {
    guard(crate::session::INPUT_STATUS_CLOSED, || {
        if len == 0 {
            return crate::session::INPUT_STATUS_INVALID;
        }
        if bytes.is_null() {
            return crate::session::INPUT_STATUS_INVALID;
        }

        let bytes = unsafe { slice::from_raw_parts(bytes, len) };
        with_session_manager(crate::session::INPUT_STATUS_CLOSED, |manager| {
            manager.send_input_bytes_status(session_id, bytes)
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_snapshot(session_id: SessionId) -> *mut FfiTerminalSnapshot {
    guard(ptr::null_mut(), || {
        with_session_manager(ptr::null_mut(), |manager| {
            let Some(snapshot) = manager.snapshot(session_id) else {
                return ptr::null_mut();
            };

            Box::into_raw(Box::new(snapshot_into_ffi(snapshot)))
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_bell_count(session_id: SessionId) -> u64 {
    guard(0, || {
        with_session_manager(0, |manager| manager.bell_count(session_id).unwrap_or(0))
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_clipboard(session_id: SessionId) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let clipboard = with_session_manager(String::new(), |manager| {
            manager.clipboard(session_id).unwrap_or_default()
        });
        string_to_c_ptr(clipboard)
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_drain_events(session_id: SessionId) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let events = with_session_manager(Vec::new(), |manager| manager.drain_events(session_id));
        let json = serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_owned());
        string_to_c_ptr(json)
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_drain_output_capture(session_id: SessionId) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let capture = with_session_manager(Vec::new(), |manager| {
            manager.drain_output_capture(session_id)
        });
        string_to_c_ptr(hex_encode(&capture))
    })
}

/// # Safety
///
/// When `len` is non-zero, `marker` must point to at least `len` readable bytes
/// for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn nauterm_session_suppress_output_until(
    session_id: SessionId,
    marker: *const u8,
    len: usize,
) -> bool {
    guard(false, || {
        if len == 0 || marker.is_null() {
            return false;
        }
        let marker = unsafe { slice::from_raw_parts(marker, len) };
        with_session_manager(false, |manager| {
            manager.suppress_output_until(session_id, marker)
        })
    })
}

#[no_mangle]
pub extern "C" fn nauterm_session_cancel_output_suppression(session_id: SessionId) -> bool {
    guard(false, || {
        with_session_manager(false, |manager| {
            manager.cancel_output_suppression(session_id)
        })
    })
}

fn serial_config_from_args(
    baud_rate: u32,
    data_bits: u32,
    parity: u32,
    stop_bits: u32,
    flow_control: u32,
) -> Result<SerialConfig, String> {
    let parity = match parity {
        0 => SerialParity::None,
        1 => SerialParity::Even,
        2 => SerialParity::Odd,
        value => return Err(format!("unsupported serial parity value {value}")),
    };
    let flow_control = match flow_control {
        0 => SerialFlowControl::None,
        1 => SerialFlowControl::Software,
        2 => SerialFlowControl::Hardware,
        value => return Err(format!("unsupported serial flow control value {value}")),
    };
    let data_bits = u8::try_from(data_bits)
        .map_err(|_| format!("unsupported data bits {data_bits}; choose 5, 6, 7, or 8"))?;
    let stop_bits = u8::try_from(stop_bits)
        .map_err(|_| format!("unsupported stop bits {stop_bits}; choose 1 or 2"))?;

    SerialConfig::new(baud_rate, data_bits, parity, stop_bits, flow_control)
        .map_err(|error| error.to_string())
}
