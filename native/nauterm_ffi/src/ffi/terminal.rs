use std::ffi::{c_char, c_void};
use std::ptr;
use std::slice;
use std::sync::mpsc::{self, SyncSender};
use std::thread::{self, JoinHandle};

use crate::pty::WakeupCallback;
use crate::session::create_terminal_emulator;
use crate::terminal::{
    TerminalEmulator, TerminalOptions, TerminalSearchDirection, TerminalSearchResult,
};

use super::common::{guard, string_from_ptr, string_to_c_ptr, terminal_options_from_args};
use super::snapshot::{free_snapshot, snapshot_into_ffi, FfiTerminalSnapshot};

type TerminalWork = Box<dyn FnOnce(&mut dyn TerminalEmulator) + Send + 'static>;

enum TerminalMessage {
    Run(TerminalWork),
    Shutdown,
}

pub struct TerminalHandle {
    sender: SyncSender<TerminalMessage>,
    worker: Option<JoinHandle<()>>,
}

impl TerminalHandle {
    fn spawn(columns: usize, rows: usize, options: TerminalOptions) -> Option<Self> {
        let (sender, receiver) = mpsc::sync_channel::<TerminalMessage>(256);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("nauterm-terminal".to_owned())
            .spawn(move || {
                let Ok(mut terminal) = create_terminal_emulator(columns, rows, options) else {
                    let _ = ready_sender.send(false);
                    return;
                };
                if ready_sender.send(true).is_err() {
                    return;
                }
                while let Ok(message) = receiver.recv() {
                    match message {
                        TerminalMessage::Run(work) => work(terminal.as_mut()),
                        TerminalMessage::Shutdown => break,
                    }
                }
                terminal.set_wakeup_callback(None);
            })
            .ok()?;
        if ready_receiver.recv().ok() != Some(true) {
            let _ = worker.join();
            return None;
        }
        Some(Self {
            sender,
            worker: Some(worker),
        })
    }

    fn call<R: Send + 'static>(
        &self,
        work: impl FnOnce(&mut dyn TerminalEmulator) -> R + Send + 'static,
    ) -> Option<R> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.sender
            .send(TerminalMessage::Run(Box::new(move |terminal| {
                let _ = reply_sender.send(work(terminal));
            })))
            .ok()?;
        reply_receiver.recv().ok()
    }
}

impl Drop for TerminalHandle {
    fn drop(&mut self) {
        let _ = self.sender.send(TerminalMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn handle_ref<'a>(handle: *mut TerminalHandle) -> Option<&'a TerminalHandle> {
    if handle.is_null() {
        None
    } else {
        Some(unsafe { &*handle })
    }
}

#[no_mangle]
pub extern "C" fn nauterm_terminal_create(columns: u32, rows: u32) -> *mut TerminalHandle {
    guard(ptr::null_mut(), || {
        TerminalHandle::spawn(columns as usize, rows as usize, TerminalOptions::default())
            .map(|handle| Box::into_raw(Box::new(handle)))
            .unwrap_or(ptr::null_mut())
    })
}

#[no_mangle]
pub extern "C" fn nauterm_terminal_create_configured(
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
) -> *mut TerminalHandle {
    guard(ptr::null_mut(), || {
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
        TerminalHandle::spawn(columns as usize, rows as usize, options)
            .map(|handle| Box::into_raw(Box::new(handle)))
            .unwrap_or(ptr::null_mut())
    })
}

/// # Safety
///
/// `handle` must either be null or a pointer returned by `nauterm_terminal_create`.
/// After this call returns, the handle must not be used again.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_destroy(handle: *mut TerminalHandle) {
    guard((), || {
        if !handle.is_null() {
            drop(unsafe { Box::from_raw(handle) });
        }
    });
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_resize(
    handle: *mut TerminalHandle,
    columns: u32,
    rows: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) {
    guard((), || {
        if let Some(handle) = handle_ref(handle) {
            let _ = handle.call(move |terminal| {
                terminal.resize(
                    columns as usize,
                    rows as usize,
                    cell_width_px,
                    cell_height_px,
                );
            });
        }
    });
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_scroll_lines(
    handle: *mut TerminalHandle,
    lines: i32,
) -> bool {
    guard(false, || {
        let Some(handle) = handle_ref(handle) else {
            return false;
        };
        handle
            .call(move |terminal| terminal.scroll_lines(lines))
            .is_some()
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_scroll_page_up(handle: *mut TerminalHandle) -> bool {
    guard(false, || {
        let Some(handle) = handle_ref(handle) else {
            return false;
        };
        handle.call(|terminal| terminal.scroll_page_up()).is_some()
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_scroll_page_down(handle: *mut TerminalHandle) -> bool {
    guard(false, || {
        let Some(handle) = handle_ref(handle) else {
            return false;
        };
        handle
            .call(|terminal| terminal.scroll_page_down())
            .is_some()
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`. `query` must either be null or a valid
/// null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_search(
    handle: *mut TerminalHandle,
    query: *const c_char,
    direction: u32,
    origin_row: u32,
    origin_column: u32,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let Some(handle) = handle_ref(handle) else {
            return string_to_c_ptr(
                serde_json::to_string(&TerminalSearchResult::not_found(0, 0))
                    .unwrap_or_else(|_| "{}".to_owned()),
            );
        };

        let query = string_from_ptr(query).unwrap_or_default();
        let result = handle
            .call(move |terminal| {
                terminal.search(
                    &query,
                    TerminalSearchDirection::from_u32(direction),
                    origin_row as usize,
                    origin_column as usize,
                )
            })
            .unwrap_or_else(|| TerminalSearchResult::not_found(0, 0));
        string_to_c_ptr(serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_owned()))
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`. The returned string must be released with
/// `nauterm_string_free`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_plain_text(handle: *mut TerminalHandle) -> *mut c_char {
    guard(ptr::null_mut(), || {
        handle_ref(handle)
            .and_then(|handle| handle.call(|terminal| terminal.plain_text()))
            .map(string_to_c_ptr)
            .unwrap_or(ptr::null_mut())
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`. The returned string must be released with
/// `nauterm_string_free`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_selection_text(
    handle: *mut TerminalHandle,
    start: i64,
    end: i64,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        handle_ref(handle)
            .and_then(|handle| handle.call(move |terminal| terminal.selection_text(start, end)))
            .map(string_to_c_ptr)
            .unwrap_or(ptr::null_mut())
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`. The returned string must be released with
/// `nauterm_string_free`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_command_block_at(
    handle: *mut TerminalHandle,
    offset: i64,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let Some(handle) = handle_ref(handle) else {
            return ptr::null_mut();
        };
        let command_block = handle
            .call(move |terminal| terminal.command_block_at(offset))
            .flatten();
        string_to_c_ptr(serde_json::to_string(&command_block).unwrap_or_else(|_| "null".to_owned()))
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_start_local_pty(handle: *mut TerminalHandle) -> bool {
    guard(false, || {
        let Some(handle) = handle_ref(handle) else {
            return false;
        };
        handle
            .call(|terminal| terminal.start_local_pty())
            .unwrap_or(false)
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`. `callback`, when present, must remain valid until
/// this function is called again with `None` or the terminal handle is destroyed.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_set_wakeup_callback(
    handle: *mut TerminalHandle,
    callback: Option<extern "C" fn(*mut c_void)>,
    user_data: *mut c_void,
) {
    guard((), || {
        if let Some(handle) = handle_ref(handle) {
            let wakeup = callback.map(|callback| WakeupCallback::new(callback, user_data));
            let _ = handle.call(move |terminal| terminal.set_wakeup_callback(wakeup));
        }
    });
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_poll_local_pty(handle: *mut TerminalHandle) -> bool {
    guard(false, || {
        let Some(handle) = handle_ref(handle) else {
            return false;
        };
        handle
            .call(|terminal| terminal.pump_local_pty())
            .unwrap_or(false)
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_write_codepoint(
    handle: *mut TerminalHandle,
    codepoint: u32,
) -> bool {
    guard(false, || {
        let Some(handle) = handle_ref(handle) else {
            return false;
        };
        let Some(character) = char::from_u32(codepoint) else {
            return false;
        };
        let bytes = character.to_string().into_bytes();

        handle
            .call(move |terminal| terminal.write_bytes(&bytes))
            .is_some()
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_send_input_codepoint(
    handle: *mut TerminalHandle,
    codepoint: u32,
) -> bool {
    guard(false, || {
        let Some(handle) = handle_ref(handle) else {
            return false;
        };
        let Some(character) = char::from_u32(codepoint) else {
            return false;
        };
        let bytes = character.to_string().into_bytes();

        handle
            .call(move |terminal| terminal.send_input_bytes(&bytes))
            .unwrap_or(false)
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`. When `len` is non-zero, `bytes` must point to at
/// least `len` readable bytes for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_write_bytes(
    handle: *mut TerminalHandle,
    bytes: *const u8,
    len: usize,
) -> bool {
    guard(false, || {
        let Some(handle) = handle_ref(handle) else {
            return false;
        };
        if len == 0 {
            return true;
        }
        if bytes.is_null() {
            return false;
        }

        let bytes = unsafe { slice::from_raw_parts(bytes, len) }.to_vec();
        handle
            .call(move |terminal| terminal.write_bytes_without_capture(&bytes))
            .is_some()
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`. When `len` is non-zero, `bytes` must point to at
/// least `len` readable bytes for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_send_input_bytes(
    handle: *mut TerminalHandle,
    bytes: *const u8,
    len: usize,
) -> bool {
    guard(false, || {
        let Some(handle) = handle_ref(handle) else {
            return false;
        };
        if len == 0 {
            return true;
        }
        if bytes.is_null() {
            return false;
        }

        let bytes = unsafe { slice::from_raw_parts(bytes, len) }.to_vec();
        handle
            .call(move |terminal| terminal.send_input_bytes(&bytes))
            .unwrap_or(false)
    })
}

/// # Safety
///
/// `handle` must either be null or a live pointer returned by
/// `nauterm_terminal_create`. The returned snapshot must be released with
/// `nauterm_terminal_free_snapshot`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_snapshot(
    handle: *mut TerminalHandle,
) -> *mut FfiTerminalSnapshot {
    guard(ptr::null_mut(), || {
        let Some(handle) = handle_ref(handle) else {
            return ptr::null_mut();
        };
        let Some(snapshot) = handle.call(|terminal| terminal.snapshot()) else {
            return ptr::null_mut();
        };
        Box::into_raw(Box::new(snapshot_into_ffi(snapshot)))
    })
}

/// # Safety
///
/// `snapshot` must either be null or a pointer returned by
/// `nauterm_terminal_snapshot`. After this call returns, the snapshot pointer
/// must not be used again.
#[no_mangle]
pub unsafe extern "C" fn nauterm_terminal_free_snapshot(snapshot: *mut FfiTerminalSnapshot) {
    guard((), || unsafe { free_snapshot(snapshot) });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_snapshot_round_trip() {
        let handle = nauterm_terminal_create(4, 1);
        assert!(!handle.is_null());

        unsafe {
            assert!(nauterm_terminal_write_codepoint(handle, 'A' as u32));

            let snapshot = nauterm_terminal_snapshot(handle);
            assert!(!snapshot.is_null());
            assert_eq!((*snapshot).columns, 4);
            assert_eq!((*snapshot).rows, 1);
            assert_eq!((*snapshot).cells_len, 4);
            assert_eq!((*snapshot).alternate_screen, 0);

            let cells = slice::from_raw_parts((*snapshot).cells, (*snapshot).cells_len);
            assert_eq!(cells[0].text_len, 1);

            nauterm_terminal_free_snapshot(snapshot);
            nauterm_terminal_destroy(handle);
        }
    }

    #[test]
    fn ffi_create_configured_uses_options() {
        let terminal_type = c"xterm-16color";
        let handle = nauterm_terminal_create_configured(
            4,
            1,
            0,
            2000,
            terminal_type.as_ptr(),
            1,
            0,
            2,
            true,
            0x383a42,
            0xfafafa,
            0x526fff,
        );
        assert!(!handle.is_null());

        unsafe {
            let snapshot = nauterm_terminal_snapshot(handle);
            assert!(!snapshot.is_null());
            assert_eq!((*snapshot).cursor_shape, 2);
            assert_eq!((*snapshot).cursor_blinking, 1);

            nauterm_terminal_free_snapshot(snapshot);
            nauterm_terminal_destroy(handle);
        }
    }

    #[cfg(feature = "terminal-ghostty")]
    #[test]
    fn ffi_create_configured_selects_ghostty_and_exports_graphics() {
        let terminal_type = c"xterm-256color";
        let handle = nauterm_terminal_create_configured(
            8,
            2,
            1,
            2000,
            terminal_type.as_ptr(),
            1,
            0,
            0,
            true,
            0xffffff,
            0,
            0xffffff,
        );
        assert!(!handle.is_null());
        let kitty = b"\x1b_Ga=T,f=100,q=2;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAA\
            DUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==\x1b\\";
        unsafe {
            nauterm_terminal_resize(handle, 8, 2, 8, 16);
            assert!(nauterm_terminal_write_bytes(
                handle,
                kitty.as_ptr(),
                kitty.len()
            ));
            let snapshot = nauterm_terminal_snapshot(handle);
            assert!(!snapshot.is_null());
            assert_eq!((*snapshot).emulator_backend, 1);
            assert_eq!((*snapshot).graphic_images_len, 1);
            assert_eq!((*snapshot).graphic_placements_len, 1);
            assert_eq!((*snapshot).graphic_data_len, 4);
            nauterm_terminal_free_snapshot(snapshot);
            nauterm_terminal_destroy(handle);
        }
    }

    #[cfg(feature = "terminal-ghostty")]
    #[test]
    fn ffi_ghostty_exports_text_selection_search_and_shell_blocks() {
        let terminal_type = c"xterm-256color";
        let handle = nauterm_terminal_create_configured(
            20,
            4,
            1,
            2000,
            terminal_type.as_ptr(),
            1,
            0,
            0,
            false,
            0xffffff,
            0,
            0xffffff,
        );
        assert!(!handle.is_null());
        let input = concat!(
            "\x1b]7;file://localhost/tmp/project\x07",
            "\x1b]133;A\x07$ ",
            "\x1b]4545;CommandStarted;ZWNobyBoaQ==\x07",
            "echo hi\r\nhi\r\n",
            "\x1b]4545;CommandExited;0\x07",
        )
        .as_bytes();

        unsafe {
            assert!(nauterm_terminal_write_bytes(
                handle,
                input.as_ptr(),
                input.len()
            ));

            let text = nauterm_terminal_plain_text(handle);
            assert!(!text.is_null());
            assert!(std::ffi::CStr::from_ptr(text)
                .to_string_lossy()
                .contains("echo hi"));
            crate::database::nauterm_string_free(text);

            let selection = nauterm_terminal_selection_text(handle, 2, 9);
            assert_eq!(
                std::ffi::CStr::from_ptr(selection).to_string_lossy(),
                "echo hi"
            );
            crate::database::nauterm_string_free(selection);

            let search = nauterm_terminal_search(handle, c"echo hi".as_ptr(), 0, 0, 0);
            assert!(std::ffi::CStr::from_ptr(search)
                .to_string_lossy()
                .contains("\"found\":true"));
            crate::database::nauterm_string_free(search);

            let block = nauterm_terminal_command_block_at(handle, 0);
            let block_json = std::ffi::CStr::from_ptr(block).to_string_lossy();
            assert!(block_json.contains("\"command\":\"echo hi\""));
            assert!(block_json.contains("\"working_directory\":\"/tmp/project\""));
            crate::database::nauterm_string_free(block);

            nauterm_terminal_destroy(handle);
        }
    }
}
