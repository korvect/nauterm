//! Check handwritten bindings against the linked library before using them.
use super::*;
use serde_json::Value;
use std::ffi::CStr;
use std::mem::{align_of, offset_of, size_of};
use std::sync::OnceLock;

pub(super) fn verify() -> Result<(), String> {
    static CHECK: OnceLock<Result<(), String>> = OnceLock::new();
    CHECK
        .get_or_init(|| manifest().and_then(|value| validate(&value)))
        .clone()
}

fn manifest() -> Result<Value, String> {
    let pointer = unsafe { ghostty_type_json() };
    if pointer.is_null() {
        return Err("Ghostty ABI manifest is null".into());
    }
    // The library owns this NUL-terminated, process-lifetime string.
    serde_json::from_slice(unsafe { CStr::from_ptr(pointer) }.to_bytes())
        .map_err(|error| format!("Invalid Ghostty ABI manifest: {error}"))
}

fn expect(value: &Value, expected: usize, label: &str) -> Result<(), String> {
    if value.as_u64() == Some(expected as u64) {
        return Ok(());
    }
    Err(format!(
        "Ghostty ABI mismatch for {label}: expected {expected}, got {value}"
    ))
}

fn field_size<T, F>(_: fn(&T) -> &F) -> usize {
    size_of::<F>()
}

fn validate(manifest: &Value) -> Result<(), String> {
    expect(&manifest["schema"], 1, "schema")?;
    expect(
        &manifest["abi"]["pointer_size"],
        size_of::<*const c_void>(),
        "pointer size",
    )?;
    expect(
        &manifest["abi"]["usize_size"],
        size_of::<usize>(),
        "usize size",
    )?;
    let endian = if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    };
    if manifest["abi"]["endian"].as_str() != Some(endian) {
        return Err("Ghostty ABI mismatch for byte order".into());
    }
    let types = &manifest["types"];
    macro_rules! layout {
        ($rust:ty, $name:literal, $($field:ident),* $(,)?) => {{
            let ty = &types[$name];
            expect(&ty["size"], size_of::<$rust>(), concat!($name, ".size"))?;
            expect(&ty["align"], align_of::<$rust>(), concat!($name, ".align"))?;
            $(
                expect(&ty["fields"][stringify!($field)]["offset"], offset_of!($rust, $field), concat!($name, ".", stringify!($field), ".offset"))?;
                expect(&ty["fields"][stringify!($field)]["size"], field_size::<$rust, _>(|v| &v.$field), concat!($name, ".", stringify!($field), ".size"))?;
            )*
        }};
    }
    layout!(GhosttyColorRgb, "GhosttyColorRgb", r, g, b);
    layout!(
        GhosttySelectWordOptions,
        "GhosttyTerminalSelectWordOptions",
        size,
        boundary_codepoints,
        boundary_codepoints_len
    );
    let reference = &types["GhosttyTerminalSelectWordOptions"]["fields"]["ref"];
    expect(
        &reference["offset"],
        offset_of!(GhosttySelectWordOptions, reference),
        "GhosttyTerminalSelectWordOptions.ref.offset",
    )?;
    expect(
        &reference["size"],
        size_of::<GhosttyGridRef>(),
        "GhosttyTerminalSelectWordOptions.ref.size",
    )?;
    layout!(GhosttyString, "GhosttyString", ptr, len);
    layout!(GhosttyBuffer, "GhosttyBuffer", ptr, cap, len);
    layout!(GhosttyStyleColor, "GhosttyStyleColor", tag, value);
    layout!(GhosttyStyleColorValue, "GhosttyStyleColorValue",);
    layout!(
        GhosttyStyle,
        "GhosttyStyle",
        size,
        fg_color,
        bg_color,
        underline_color,
        bold,
        italic,
        faint,
        blink,
        inverse,
        invisible,
        strikethrough,
        overline,
        underline
    );
    layout!(GhosttyPointCoordinate, "GhosttyPointCoordinate", x, y);
    layout!(GhosttyPointValue, "GhosttyPointValue",);
    layout!(GhosttyPoint, "GhosttyPoint", tag, value);
    layout!(GhosttyGridRef, "GhosttyGridRef", size, node, x, y);
    layout!(
        GhosttySelection,
        "GhosttySelection",
        size,
        start,
        end,
        rectangle
    );
    layout!(
        GhosttySelectionBuffer,
        "GhosttySelectionBuffer",
        ptr,
        cap,
        len
    );
    layout!(
        GhosttySelectionFormatOptions,
        "GhosttyTerminalSelectionFormatOptions",
        size,
        emit,
        unwrap,
        trim,
        selection
    );
    layout!(
        GhosttyTerminalModeConfig,
        "GhosttyTerminalModeConfig",
        mode,
        value
    );
    layout!(
        GhosttyClipboardContent,
        "GhosttyClipboardContent",
        mime,
        data
    );
    layout!(
        GhosttyClipboardWrite,
        "GhosttyClipboardWrite",
        size,
        location,
        contents,
        contents_len,
        name,
        granted,
        can_remember,
        ctx,
        reply
    );
    layout!(
        GhosttyClipboardWriteReply,
        "GhosttyClipboardWriteReply",
        size,
        result,
        remember
    );
    layout!(
        GhosttyTerminalScrollbar,
        "GhosttyTerminalScrollbar",
        total,
        offset,
        len
    );
    layout!(GhosttyScrollValue, "GhosttyTerminalScrollViewportValue",);
    layout!(
        GhosttyScrollViewport,
        "GhosttyTerminalScrollViewport",
        tag,
        value
    );
    layout!(
        GhosttyKittyPlacementRenderInfo,
        "GhosttyKittyGraphicsPlacementRenderInfo",
        size,
        pixel_width,
        pixel_height,
        grid_cols,
        grid_rows,
        viewport_col,
        viewport_row,
        viewport_visible,
        source_x,
        source_y,
        source_width,
        source_height
    );
    layout!(
        GhosttySysImage,
        "GhosttySysImage",
        width,
        height,
        data,
        data_len
    );
    layout!(GhosttyWriter, "GhosttyWriter", write, userdata);
    layout!(GhosttyMimeReader, "GhosttyMimeReader", read, userdata);
    layout!(
        GhosttyPaste,
        "GhosttyPaste",
        size,
        location,
        source,
        mimes,
        mimes_len,
        reader,
        allow_unsafe
    );

    // The C API selects operations by integer; a shifted enum can otherwise
    // reinterpret a valid Rust pointer as a completely different value type.
    macro_rules! values {
        ($name:literal, $($member:literal => $value:expr),* $(,)?) => {{
            let ty = &types[$name];
            expect(&ty["size"], size_of::<c_int>(), concat!($name, ".size"))?;
            expect(&ty["align"], align_of::<c_int>(), concat!($name, ".align"))?;
            $(if ty["values"][$member].as_i64() != Some($value as i64) {
                return Err(format!("Ghostty ABI mismatch for {}.{}", $name, $member));
            })*
        }};
    }
    values!("GhosttyResult", "SUCCESS" => GHOSTTY_SUCCESS, "OUT_OF_SPACE" => GHOSTTY_OUT_OF_SPACE);
    values!("GhosttyTerminalOption",
        "USERDATA" => TERMINAL_OPT_USERDATA, "WRITE_PTY" => TERMINAL_OPT_WRITE_PTY,
        "BELL" => TERMINAL_OPT_BELL, "COLOR_FOREGROUND" => TERMINAL_OPT_COLOR_FOREGROUND,
        "COLOR_BACKGROUND" => TERMINAL_OPT_COLOR_BACKGROUND, "COLOR_CURSOR" => TERMINAL_OPT_COLOR_CURSOR,
        "COLOR_PALETTE" => TERMINAL_OPT_COLOR_PALETTE, "KITTY_IMAGE_STORAGE_LIMIT" => TERMINAL_OPT_KITTY_IMAGE_STORAGE_LIMIT,
        "DEFAULT_CURSOR_STYLE" => TERMINAL_OPT_DEFAULT_CURSOR_STYLE, "DEFAULT_CURSOR_BLINK" => TERMINAL_OPT_DEFAULT_CURSOR_BLINK,
        "CLIPBOARD_WRITE" => TERMINAL_OPT_CLIPBOARD_WRITE, "SCROLLBACK_MAX_BYTES" => TERMINAL_OPT_SCROLLBACK_MAX_BYTES,
        "SCROLLBACK_MAX_LINES" => TERMINAL_OPT_SCROLLBACK_MAX_LINES, "MODE" => TERMINAL_OPT_MODE, "RENDER_HOLD" => TERMINAL_OPT_RENDER_HOLD);
    #[cfg(windows)]
    values!("GhosttyTerminalOption", "RESIZE_PULL_SCROLLBACK" => TERMINAL_OPT_RESIZE_PULL_SCROLLBACK);
    values!("GhosttyTerminalData", "ACTIVE_SCREEN" => TERMINAL_DATA_ACTIVE_SCREEN, "SCROLLBAR" => TERMINAL_DATA_SCROLLBAR,
        "TITLE" => TERMINAL_DATA_TITLE, "TOTAL_ROWS" => TERMINAL_DATA_TOTAL_ROWS, "SCROLLBACK_ROWS" => TERMINAL_DATA_SCROLLBACK_ROWS,
        "COLOR_CURSOR" => TERMINAL_DATA_COLOR_CURSOR, "KITTY_GRAPHICS" => TERMINAL_DATA_KITTY_GRAPHICS, "MODE" => TERMINAL_DATA_MODE);
    values!("GhosttySearchOption", "NEEDLE" => 0);
    values!("GhosttySearchData", "MATCHES" => 5);
    values!("GhosttyPasteSource", "CLIPBOARD" => 0);
    values!("GhosttyClipboardLocation", "STANDARD" => 0);
    values!("GhosttyPointTag", "VIEWPORT" => POINT_TAG_VIEWPORT, "SCREEN" => POINT_TAG_SCREEN);
    values!("GhosttyFormatterFormat", "PLAIN" => FORMATTER_FORMAT_PLAIN);
    values!("GhosttyTerminalScrollViewportTag", "BOTTOM" => SCROLL_VIEWPORT_BOTTOM, "DELTA" => SCROLL_VIEWPORT_DELTA, "ROW" => SCROLL_VIEWPORT_ROW);
    values!("GhosttyRenderStateData", "COLS" => RENDER_DATA_COLS, "ROWS" => RENDER_DATA_ROWS,
        "ROW_ITERATOR" => RENDER_DATA_ROW_ITERATOR, "CURSOR_VISUAL_STYLE" => RENDER_DATA_CURSOR_VISUAL_STYLE,
        "CURSOR_VISIBLE" => RENDER_DATA_CURSOR_VISIBLE, "CURSOR_BLINKING" => RENDER_DATA_CURSOR_BLINKING,
        "CURSOR_VIEWPORT_HAS_VALUE" => RENDER_DATA_CURSOR_VIEWPORT_HAS_VALUE,
        "CURSOR_VIEWPORT_X" => RENDER_DATA_CURSOR_VIEWPORT_X, "CURSOR_VIEWPORT_Y" => RENDER_DATA_CURSOR_VIEWPORT_Y);
    values!("GhosttyRenderStateRowData", "RAW" => RENDER_ROW_DATA_RAW, "CELLS" => RENDER_ROW_DATA_CELLS);
    values!("GhosttyRenderStateRowCellsData", "RAW" => RENDER_CELL_DATA_RAW, "STYLE" => RENDER_CELL_DATA_STYLE,
        "BG_COLOR" => RENDER_CELL_DATA_BG_COLOR, "FG_COLOR" => RENDER_CELL_DATA_FG_COLOR, "GRAPHEMES_UTF8" => RENDER_CELL_DATA_GRAPHEMES_UTF8);
    values!("GhosttyCellData", "WIDE" => CELL_DATA_WIDE, "HAS_HYPERLINK" => CELL_DATA_HAS_HYPERLINK, "SEMANTIC_CONTENT" => CELL_DATA_SEMANTIC_CONTENT);
    values!("GhosttyCellSemanticContent", "INPUT" => CELL_SEMANTIC_INPUT);
    values!("GhosttyCellWide", "WIDE" => CELL_WIDE_WIDE, "SPACER_TAIL" => CELL_WIDE_SPACER_TAIL, "SPACER_HEAD" => CELL_WIDE_SPACER_HEAD);
    values!("GhosttyRowData", "WRAP" => ROW_DATA_WRAP, "WRAP_CONTINUATION" => ROW_DATA_WRAP_CONTINUATION,
        "HYPERLINK" => ROW_DATA_HYPERLINK, "SEMANTIC_PROMPT" => ROW_DATA_SEMANTIC_PROMPT);
    values!("GhosttyRowSemanticPrompt", "PROMPT" => ROW_SEMANTIC_PROMPT, "PROMPT_CONTINUATION" => ROW_SEMANTIC_PROMPT_CONTINUATION);
    values!("GhosttyStyleColorTag", "PALETTE" => STYLE_COLOR_PALETTE, "RGB" => STYLE_COLOR_RGB);
    values!("GhosttyKittyGraphicsData", "PLACEMENT_ITERATOR" => KITTY_GRAPHICS_DATA_PLACEMENT_ITERATOR);
    values!("GhosttyKittyGraphicsPlacementData", "IMAGE_ID" => KITTY_PLACEMENT_DATA_IMAGE_ID, "PLACEMENT_ID" => KITTY_PLACEMENT_DATA_PLACEMENT_ID,
        "IS_VIRTUAL" => KITTY_PLACEMENT_DATA_IS_VIRTUAL, "COLUMNS" => KITTY_PLACEMENT_DATA_COLUMNS, "ROWS" => KITTY_PLACEMENT_DATA_ROWS, "Z" => KITTY_PLACEMENT_DATA_Z);
    values!("GhosttyKittyGraphicsImageData", "WIDTH" => KITTY_IMAGE_DATA_WIDTH, "HEIGHT" => KITTY_IMAGE_DATA_HEIGHT,
        "FORMAT" => KITTY_IMAGE_DATA_FORMAT, "DATA_PTR" => KITTY_IMAGE_DATA_DATA_PTR, "DATA_LEN" => KITTY_IMAGE_DATA_DATA_LEN, "GENERATION" => KITTY_IMAGE_DATA_GENERATION);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_manifest_matches_bindings() {
        validate(&manifest().unwrap()).unwrap();
    }

    #[test]
    fn rejects_changed_layout_enum_and_schema() {
        for (path, replacement) in [
            (vec!["schema"], 999),
            (
                vec!["types", "GhosttyPaste", "fields", "reader", "offset"],
                999,
            ),
            (
                vec!["types", "GhosttyTerminalOption", "values", "WRITE_PTY"],
                999,
            ),
        ] {
            let mut value = manifest().unwrap();
            let mut field = &mut value;
            for key in path {
                field = &mut field[key];
            }
            *field = Value::from(replacement);
            assert!(validate(&value)
                .unwrap_err()
                .contains("Ghostty ABI mismatch"));
        }
    }
}
