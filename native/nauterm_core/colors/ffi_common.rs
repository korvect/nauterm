// Callers validate their FFI pointer contract before entering this helper.
pub(super) unsafe fn rs_palette_from_buffer(
    values: *const u32,
    length: usize,
) -> Option<crate::terminal::TerminalDefaultColors> {
    if values.is_null() || length != 19 {
        return None;
    }
    crate::terminal::TerminalDefaultColors::from_palette(unsafe {
        std::slice::from_raw_parts(values, length)
    })
}
