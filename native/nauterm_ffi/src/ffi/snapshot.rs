use std::ptr;

use crate::terminal::TerminalSnapshot;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FfiTerminalCell {
    pub text_offset: u32,
    pub text_len: u16,
    pub flags: u16,
    pub foreground: u32,
    pub background: u32,
    pub hyperlink_offset: u32,
    pub hyperlink_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FfiTerminalGraphicImage {
    pub id: u32,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    pub data_offset: usize,
    pub data_len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FfiTerminalGraphicPlacement {
    pub image_id: u32,
    pub placement_id: u32,
    pub z_index: i32,
    pub viewport_column: i32,
    pub viewport_row: i32,
    pub columns: u32,
    pub rows: u32,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct FfiTerminalSnapshot {
    pub emulator_backend: u32,
    pub columns: u32,
    pub rows: u32,
    pub history_lines: u32,
    pub display_offset: u32,
    pub title_len: usize,
    pub title: *mut u8,
    pub clipboard_len: usize,
    pub clipboard: *mut u8,
    pub bell_count: u64,
    pub cursor_column: u32,
    pub cursor_row: u32,
    pub cursor_visible: u32,
    pub cursor_shape: u32,
    pub cursor_color: u32,
    pub cursor_blinking: u32,
    pub keyboard_mode: u32,
    pub input_echo_enabled: u32,
    pub cells_len: usize,
    pub cells: *mut FfiTerminalCell,
    pub text_len: usize,
    pub text: *mut u8,
    pub hyperlink_text_len: usize,
    pub hyperlink_text: *mut u8,
    pub graphic_images_len: usize,
    pub graphic_images: *mut FfiTerminalGraphicImage,
    pub graphic_placements_len: usize,
    pub graphic_placements: *mut FfiTerminalGraphicPlacement,
    pub graphic_data_len: usize,
    pub graphic_data: *mut u8,
}

pub(super) fn snapshot_into_ffi(snapshot: TerminalSnapshot) -> FfiTerminalSnapshot {
    let cells_len = snapshot.cells.len();
    let text_len = snapshot.text.len();
    let hyperlink_text_len = snapshot.hyperlink_text.len();
    let mut graphic_data = Vec::new();
    let graphic_images = snapshot
        .graphic_images
        .into_iter()
        .map(|image| {
            let data_offset = graphic_data.len();
            let data_len = image.rgba.len();
            graphic_data.extend_from_slice(&image.rgba);
            FfiTerminalGraphicImage {
                id: image.id,
                generation: image.generation,
                width: image.width,
                height: image.height,
                data_offset,
                data_len,
            }
        })
        .collect::<Vec<_>>();
    let graphic_placements = snapshot
        .graphic_placements
        .into_iter()
        .map(|placement| FfiTerminalGraphicPlacement {
            image_id: placement.image_id,
            placement_id: placement.placement_id,
            z_index: placement.z_index,
            viewport_column: placement.viewport_column,
            viewport_row: placement.viewport_row,
            columns: placement.columns,
            rows: placement.rows,
            source_x: placement.source_x,
            source_y: placement.source_y,
            source_width: placement.source_width,
            source_height: placement.source_height,
        })
        .collect::<Vec<_>>();
    let graphic_images_len = graphic_images.len();
    let graphic_placements_len = graphic_placements.len();
    let graphic_data_len = graphic_data.len();
    let title_bytes = snapshot.title.into_bytes();
    let title_len = title_bytes.len();
    let clipboard_bytes = snapshot.clipboard.into_bytes();
    let clipboard_len = clipboard_bytes.len();

    let cells = Box::into_raw(snapshot.cells.into_boxed_slice()) as *mut FfiTerminalCell;
    let text = Box::into_raw(snapshot.text.into_boxed_slice()) as *mut u8;
    let title = Box::into_raw(title_bytes.into_boxed_slice()) as *mut u8;
    let clipboard = Box::into_raw(clipboard_bytes.into_boxed_slice()) as *mut u8;
    let hyperlink_text = Box::into_raw(snapshot.hyperlink_text.into_boxed_slice()) as *mut u8;
    let graphic_images =
        Box::into_raw(graphic_images.into_boxed_slice()) as *mut FfiTerminalGraphicImage;
    let graphic_placements =
        Box::into_raw(graphic_placements.into_boxed_slice()) as *mut FfiTerminalGraphicPlacement;
    let graphic_data = Box::into_raw(graphic_data.into_boxed_slice()) as *mut u8;

    FfiTerminalSnapshot {
        emulator_backend: snapshot.emulator_backend.as_u32(),
        columns: snapshot.columns as u32,
        rows: snapshot.rows as u32,
        history_lines: snapshot.history_lines as u32,
        display_offset: snapshot.display_offset as u32,
        title_len,
        title,
        clipboard_len,
        clipboard,
        bell_count: snapshot.bell_count,
        cursor_column: snapshot.cursor_column as u32,
        cursor_row: snapshot.cursor_row as u32,
        cursor_visible: u32::from(snapshot.cursor_visible),
        cursor_shape: snapshot.cursor_shape,
        cursor_color: snapshot.cursor_color,
        cursor_blinking: u32::from(snapshot.cursor_blinking),
        keyboard_mode: snapshot.keyboard_mode,
        input_echo_enabled: u32::from(snapshot.input_echo_enabled),
        cells_len,
        cells,
        text_len,
        text,
        hyperlink_text_len,
        hyperlink_text,
        graphic_images_len,
        graphic_images,
        graphic_placements_len,
        graphic_placements,
        graphic_data_len,
        graphic_data,
    }
}

pub(super) unsafe fn free_snapshot(snapshot: *mut FfiTerminalSnapshot) {
    if snapshot.is_null() {
        return;
    }

    let snapshot = unsafe { Box::from_raw(snapshot) };
    if !snapshot.cells.is_null() {
        let cells = ptr::slice_from_raw_parts_mut(snapshot.cells, snapshot.cells_len);
        drop(unsafe { Box::from_raw(cells) });
    }
    if !snapshot.text.is_null() {
        let text = ptr::slice_from_raw_parts_mut(snapshot.text, snapshot.text_len);
        drop(unsafe { Box::from_raw(text) });
    }
    if !snapshot.title.is_null() {
        let title = ptr::slice_from_raw_parts_mut(snapshot.title, snapshot.title_len);
        drop(unsafe { Box::from_raw(title) });
    }
    if !snapshot.clipboard.is_null() {
        let clipboard = ptr::slice_from_raw_parts_mut(snapshot.clipboard, snapshot.clipboard_len);
        drop(unsafe { Box::from_raw(clipboard) });
    }
    if !snapshot.hyperlink_text.is_null() {
        let hyperlink_text =
            ptr::slice_from_raw_parts_mut(snapshot.hyperlink_text, snapshot.hyperlink_text_len);
        drop(unsafe { Box::from_raw(hyperlink_text) });
    }
    if !snapshot.graphic_images.is_null() {
        let images =
            ptr::slice_from_raw_parts_mut(snapshot.graphic_images, snapshot.graphic_images_len);
        drop(unsafe { Box::from_raw(images) });
    }
    if !snapshot.graphic_placements.is_null() {
        let placements = ptr::slice_from_raw_parts_mut(
            snapshot.graphic_placements,
            snapshot.graphic_placements_len,
        );
        drop(unsafe { Box::from_raw(placements) });
    }
    if !snapshot.graphic_data.is_null() {
        let data = ptr::slice_from_raw_parts_mut(snapshot.graphic_data, snapshot.graphic_data_len);
        drop(unsafe { Box::from_raw(data) });
    }
}
