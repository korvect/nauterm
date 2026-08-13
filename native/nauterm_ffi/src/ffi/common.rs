use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::ptr;

use crate::terminal::{
    ColorTerm, Osc52Mode, TerminalDefaultColors, TerminalEnvironmentVariable, TerminalOptions,
    TerminalType,
};

// The native FFI is intentionally strict: old dynamic libraries are not
// compatible with the current Dart bindings.
pub const NAUTERM_FFI_ABI_VERSION: u32 = 3;

#[no_mangle]
pub extern "C" fn nauterm_ffi_abi_version() -> u32 {
    NAUTERM_FFI_ABI_VERSION
}

pub(super) fn guard<T>(fallback: T, work: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or(fallback)
}

pub(super) fn string_to_c_ptr(value: String) -> *mut c_char {
    CString::new(value)
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

// FFI constructors pass these scalar fields individually to keep the ABI
// stable and avoid sharing Rust struct layout with Dart.
#[allow(clippy::too_many_arguments)]
pub(super) fn terminal_options_from_args(
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
) -> TerminalOptions {
    TerminalOptions {
        emulator_backend: crate::terminal::TerminalEmulatorBackend::from_u32(emulator_backend),
        scrollback_lines: normalize_scrollback_lines(scrollback_lines),
        terminal_type: terminal_type_from_ptr(terminal_type),
        color_term: ColorTerm::from_u32(color_term),
        osc52: Osc52Mode::from_u32(osc52_mode),
        cursor_shape: cursor_shape_from_u32(cursor_shape),
        cursor_blinking,
        default_colors: TerminalDefaultColors::from_rgb_values(
            default_foreground,
            default_background,
            default_cursor,
        ),
        shell_path: optional_string_from_ptr(shell_path),
        working_directory: optional_string_from_ptr(working_directory).map(PathBuf::from),
        command: None,
        environment: environment_from_ptr(environment),
    }
}

pub(super) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(super) fn string_from_ptr(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let value = unsafe { CStr::from_ptr(value) }.to_str().ok()?;
    Some(value.to_owned())
}

pub(super) fn optional_string_from_ptr(value: *const c_char) -> Option<String> {
    string_from_ptr(value).filter(|value| !value.trim().is_empty())
}

pub(super) fn string_list_from_ptr(value: *const c_char) -> Vec<String> {
    let Some(value) = string_from_ptr(value) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<String>>(&value).unwrap_or_default()
}

fn normalize_scrollback_lines(value: u32) -> usize {
    if value == 0 {
        1
    } else {
        value as usize
    }
}

fn cursor_shape_from_u32(value: u32) -> alacritty_terminal::vte::ansi::CursorShape {
    match value {
        1 => alacritty_terminal::vte::ansi::CursorShape::Underline,
        2 => alacritty_terminal::vte::ansi::CursorShape::Beam,
        3 => alacritty_terminal::vte::ansi::CursorShape::HollowBlock,
        _ => alacritty_terminal::vte::ansi::CursorShape::Block,
    }
}

fn terminal_type_from_ptr(value: *const c_char) -> TerminalType {
    if value.is_null() {
        return TerminalType::default();
    }

    let Ok(value) = unsafe { CStr::from_ptr(value) }.to_str() else {
        return TerminalType::default();
    };

    TerminalType::from_term(value).unwrap_or_default()
}

fn environment_from_ptr(value: *const c_char) -> Vec<TerminalEnvironmentVariable> {
    let Some(value) = string_from_ptr(value) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<TerminalEnvironmentVariable>>(&value)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !entry.variable.trim().is_empty())
        .collect()
}
