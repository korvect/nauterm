/// Update the live session's default palette on its actor, without reconnecting.
///
/// # Safety
/// When `length` is 19, a non-null `values` must point to 19 readable u32 values.
/// Other lengths are rejected without reading. Values are foreground/background/
/// cursor, then 16 ANSI colors.
#[no_mangle]
pub unsafe extern "C" fn nauterm_rs_session_set_palette(
    id: SessionId,
    values: *const u32,
    length: usize,
) -> bool {
    guard(false, || {
        let Some(colors) = (unsafe { super::common::rs_palette_from_buffer(values, length) })
        else {
            return false;
        };
        with_session_manager(false, |manager| manager.rs_set_palette(id, colors))
    })
}
