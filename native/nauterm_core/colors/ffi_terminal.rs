/// Update default terminal colors, retaining all screen/history state and OSC overrides.
///
/// # Safety
/// `handle` must be null or a live handle from this library. A non-null `values`
/// must point to 19 readable u32 values when `length` is 19. Other lengths are
/// rejected without reading. Values are foreground/background/cursor, then ANSI.
#[no_mangle]
pub unsafe extern "C" fn nauterm_rs_terminal_set_palette(
    handle: *mut TerminalHandle,
    values: *const u32,
    length: usize,
) -> bool {
    guard(false, || {
        let Some(colors) = (unsafe { super::common::rs_palette_from_buffer(values, length) })
        else {
            return false;
        };
        handle_ref(handle)
            .and_then(|handle| handle.call(move |terminal| terminal.rs_set_palette(colors)))
            .unwrap_or(false)
    })
}
