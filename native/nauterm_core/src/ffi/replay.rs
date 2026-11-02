use super::*;

#[no_mangle]
pub extern "C" fn nauterm_rs_replay_version() -> u32 {
    1
}

/// Feed captured output into a passive VT actor without retaining it as a new capture.
///
/// # Safety
/// A non-null `bytes` must be readable for `length` bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn nauterm_rs_session_write_replay(
    id: SessionId,
    bytes: *const u8,
    length: usize,
) -> bool {
    guard(false, || {
        if bytes.is_null() || length > 4 * 1024 * 1024 {
            return false;
        }
        let bytes = unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec();
        with_session_manager(false, |manager| manager.write_replay(id, bytes))
    })
}

/// Create only a passive VT session; input is rejected by its disconnected transport.
///
/// # Safety
/// When `length` is 19, `values` must be null or readable for 19 u32 values.
#[no_mangle]
pub unsafe extern "C" fn nauterm_rs_session_create_replay(
    columns: u32,
    rows: u32,
    backend: u32,
    scrollback: u32,
    values: *const u32,
    length: usize,
) -> SessionId {
    guard(0, || {
        if !(2..=1000).contains(&columns) || !(1..=500).contains(&rows) || backend > 1 {
            return 0;
        }
        let Some(colors) =
            (unsafe { super::super::common::rs_palette_from_buffer(values, length) })
        else {
            return 0;
        };
        let options = super::super::common::terminal_options_from_args(
            backend,
            scrollback.min(1_000_000),
            c"xterm-256color".as_ptr(),
            1,
            0,
            0,
            false,
            0xffffff,
            0,
            0xffffff,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        );
        with_session_manager(0, |manager| {
            let id = manager.create_replay(columns as usize, rows as usize, options);
            if id != 0 && manager.rs_set_palette(id, colors) {
                id
            } else {
                if id != 0 {
                    manager.close(id);
                }
                0
            }
        })
    })
}
