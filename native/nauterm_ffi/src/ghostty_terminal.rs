use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::{c_int, c_void};
use std::io::Cursor;
use std::marker::PhantomData;
use std::ptr;
use std::rc::Rc;
use std::slice;
use std::sync::Mutex;
use std::sync::Once;

use crate::ffi::FfiTerminalCell;
use crate::pty::{LocalPty, WakeupCallback};
use crate::terminal::{
    TerminalCommandBlock, TerminalEmulator, TerminalEmulatorBackend, TerminalGeometry,
    TerminalGraphicImage, TerminalGraphicPlacement, TerminalOptions, TerminalSearchDirection,
    TerminalSearchResult, TerminalSnapshot,
};
use base64::Engine as _;

const GHOSTTY_SUCCESS: c_int = 0;
const GHOSTTY_OUT_OF_SPACE: c_int = -3;

const TERMINAL_OPT_COLOR_FOREGROUND: c_int = 11;
const TERMINAL_OPT_USERDATA: c_int = 0;
const TERMINAL_OPT_WRITE_PTY: c_int = 1;
const TERMINAL_OPT_BELL: c_int = 2;
const TERMINAL_OPT_COLOR_BACKGROUND: c_int = 12;
const TERMINAL_OPT_COLOR_CURSOR: c_int = 13;
const TERMINAL_OPT_COLOR_PALETTE: c_int = 14;
const TERMINAL_OPT_KITTY_IMAGE_STORAGE_LIMIT: c_int = 15;
const TERMINAL_OPT_DEFAULT_CURSOR_STYLE: c_int = 22;
const TERMINAL_OPT_DEFAULT_CURSOR_BLINK: c_int = 23;
const TERMINAL_OPT_CLIPBOARD_WRITE: c_int = 26;
const TERMINAL_OPT_SCROLLBACK_MAX_LINES: c_int = 28;

const TERMINAL_DATA_ACTIVE_SCREEN: c_int = 6;
const TERMINAL_DATA_SCROLLBAR: c_int = 9;
const TERMINAL_DATA_TITLE: c_int = 12;
const TERMINAL_DATA_TOTAL_ROWS: c_int = 14;
const TERMINAL_DATA_SCROLLBACK_ROWS: c_int = 15;
const TERMINAL_DATA_COLOR_CURSOR: c_int = 20;
const TERMINAL_DATA_KITTY_GRAPHICS: c_int = 30;
const TERMINAL_DATA_MODE: c_int = 37;

const KITTY_GRAPHICS_DATA_PLACEMENT_ITERATOR: c_int = 1;
const KITTY_PLACEMENT_DATA_IMAGE_ID: c_int = 1;
const KITTY_PLACEMENT_DATA_PLACEMENT_ID: c_int = 2;
const KITTY_PLACEMENT_DATA_IS_VIRTUAL: c_int = 3;
const KITTY_PLACEMENT_DATA_COLUMNS: c_int = 10;
const KITTY_PLACEMENT_DATA_ROWS: c_int = 11;
const KITTY_PLACEMENT_DATA_Z: c_int = 12;
const KITTY_IMAGE_DATA_WIDTH: c_int = 3;
const KITTY_IMAGE_DATA_HEIGHT: c_int = 4;
const KITTY_IMAGE_DATA_FORMAT: c_int = 5;
const KITTY_IMAGE_DATA_DATA_PTR: c_int = 7;
const KITTY_IMAGE_DATA_DATA_LEN: c_int = 8;
const KITTY_IMAGE_DATA_GENERATION: c_int = 9;

const RENDER_DATA_COLS: c_int = 1;
const RENDER_DATA_ROWS: c_int = 2;
const RENDER_DATA_ROW_ITERATOR: c_int = 4;
const RENDER_DATA_CURSOR_VISUAL_STYLE: c_int = 10;
const RENDER_DATA_CURSOR_VISIBLE: c_int = 11;
const RENDER_DATA_CURSOR_BLINKING: c_int = 12;
const RENDER_DATA_CURSOR_VIEWPORT_HAS_VALUE: c_int = 14;
const RENDER_DATA_CURSOR_VIEWPORT_X: c_int = 15;
const RENDER_DATA_CURSOR_VIEWPORT_Y: c_int = 16;

const RENDER_ROW_DATA_CELLS: c_int = 3;
const RENDER_CELL_DATA_RAW: c_int = 1;
const RENDER_CELL_DATA_STYLE: c_int = 2;
const RENDER_CELL_DATA_BG_COLOR: c_int = 5;
const RENDER_CELL_DATA_FG_COLOR: c_int = 6;
const RENDER_CELL_DATA_GRAPHEMES_UTF8: c_int = 9;

const CELL_DATA_WIDE: c_int = 3;
const ROW_DATA_SEMANTIC_PROMPT: c_int = 6;
const ROW_SEMANTIC_PROMPT: c_int = 1;
const CELL_WIDE_WIDE: c_int = 1;
const CELL_WIDE_SPACER_TAIL: c_int = 2;
const CELL_WIDE_SPACER_HEAD: c_int = 3;

const SCROLL_VIEWPORT_BOTTOM: c_int = 1;
const SCROLL_VIEWPORT_DELTA: c_int = 2;
const SCROLL_VIEWPORT_ROW: c_int = 3;

const POINT_TAG_VIEWPORT: c_int = 1;
const POINT_TAG_SCREEN: c_int = 2;
const FORMATTER_FORMAT_PLAIN: c_int = 0;
const MODE_DECCKM: u16 = 1;
const MODE_X10_MOUSE: u16 = 9;
const MODE_KEYPAD_KEYS: u16 = 66;
const MODE_NORMAL_MOUSE: u16 = 1000;
const MODE_BUTTON_MOUSE: u16 = 1002;
const MODE_ANY_MOUSE: u16 = 1003;
const MODE_FOCUS_EVENT: u16 = 1004;
const MODE_SGR_MOUSE: u16 = 1006;
const MODE_BRACKETED_PASTE: u16 = 2004;

const MAX_POLL_OUTPUT_CHUNKS: usize = 32;
const MAX_OSC_BYTES: usize = 64 * 1024;
const MAX_COMMAND_BLOCK_METADATA: usize = 20_000;

const FLAG_INVERSE: u16 = 0x0001;
const FLAG_BOLD: u16 = 0x0002;
const FLAG_ITALIC: u16 = 0x0004;
const FLAG_UNDERLINE: u16 = 0x0008;
const FLAG_WIDE_CHAR: u16 = 0x0020;
const FLAG_WIDE_CHAR_SPACER: u16 = 0x0040;
const FLAG_DIM: u16 = 0x0080;
const FLAG_HIDDEN: u16 = 0x0100;
const FLAG_STRIKEOUT: u16 = 0x0200;
const FLAG_LEADING_WIDE_CHAR_SPACER: u16 = 0x0400;
const FLAG_DOUBLE_UNDERLINE: u16 = 0x0800;
const FLAG_UNDERCURL: u16 = 0x1000;
const FLAG_DOTTED_UNDERLINE: u16 = 0x2000;
const FLAG_DASHED_UNDERLINE: u16 = 0x4000;

const STYLE_COLOR_PALETTE: c_int = 1;
const STYLE_COLOR_RGB: c_int = 2;
const KITTY_UNICODE_PLACEHOLDER: char = '\u{10eeee}';

type GhosttyTerminal = *mut c_void;
type GhosttyRenderState = *mut c_void;
type GhosttyRowIterator = *mut c_void;
type GhosttyRowCells = *mut c_void;
type GhosttyKittyGraphics = *mut c_void;
type GhosttyKittyGraphicsImage = *const c_void;
type GhosttyKittyGraphicsPlacementIterator = *mut c_void;
type GhosttyTrackedGridRef = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GhosttyColorRgb {
    r: u8,
    g: u8,
    b: u8,
}

impl GhosttyColorRgb {
    fn from_u32(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xff) as u8,
            g: ((value >> 8) & 0xff) as u8,
            b: (value & 0xff) as u8,
        }
    }

    fn as_u32(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
union GhosttyStyleColorValue {
    palette: u8,
    rgb: GhosttyColorRgb,
    padding: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GhosttyStyleColor {
    tag: c_int,
    value: GhosttyStyleColorValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GhosttyStyle {
    size: usize,
    fg_color: GhosttyStyleColor,
    bg_color: GhosttyStyleColor,
    underline_color: GhosttyStyleColor,
    bold: bool,
    italic: bool,
    faint: bool,
    blink: bool,
    inverse: bool,
    invisible: bool,
    strikethrough: bool,
    overline: bool,
    underline: c_int,
}

impl Default for GhosttyStyle {
    fn default() -> Self {
        // The C API initializes every field when queried. Only the sized ABI
        // prefix must be populated by the caller.
        let mut value: Self = unsafe { std::mem::zeroed() };
        value.size = std::mem::size_of::<Self>();
        value
    }
}

#[derive(Clone, Copy, Debug)]
struct VirtualPlaceholder {
    viewport_column: i32,
    viewport_row: i32,
    image_id: u32,
    placement_id: u32,
    fragment_column: u32,
    fragment_row: u32,
}

#[derive(Clone, Copy, Debug)]
struct VirtualPlacementDefinition {
    image_id: u32,
    placement_id: u32,
    columns: u32,
    rows: u32,
    z_index: i32,
}

struct ExportCellContext<'a> {
    terminal: GhosttyTerminal,
    text: &'a mut Vec<u8>,
    hyperlink_text: &'a mut Vec<u8>,
    default_foreground: u32,
    default_background: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GhosttyString {
    ptr: *const u8,
    len: usize,
}

impl Default for GhosttyString {
    fn default() -> Self {
        Self {
            ptr: ptr::null(),
            len: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GhosttyPointCoordinate {
    x: u16,
    y: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
union GhosttyPointValue {
    coordinate: GhosttyPointCoordinate,
    padding: [u64; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GhosttyPoint {
    tag: c_int,
    value: GhosttyPointValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GhosttyGridRef {
    size: usize,
    node: *mut c_void,
    x: u16,
    y: u16,
}

impl Default for GhosttyGridRef {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>(),
            node: ptr::null_mut(),
            x: 0,
            y: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GhosttySelection {
    size: usize,
    start: GhosttyGridRef,
    end: GhosttyGridRef,
    rectangle: bool,
}

impl Default for GhosttySelection {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>(),
            start: GhosttyGridRef::default(),
            end: GhosttyGridRef::default(),
            rectangle: false,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GhosttySelectionFormatOptions {
    size: usize,
    emit: c_int,
    unwrap: bool,
    trim: bool,
    selection: *const GhosttySelection,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GhosttyTerminalModeConfig {
    mode: u16,
    value: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GhosttyClipboardContent {
    mime: GhosttyString,
    data: GhosttyString,
}

#[repr(C)]
struct GhosttyClipboardWrite {
    size: usize,
    location: c_int,
    contents: *const GhosttyClipboardContent,
    contents_len: usize,
}

#[derive(Default)]
struct GhosttyCallbackState {
    writes: Vec<u8>,
    clipboard: String,
    bell_count: u64,
}

struct OutputSuppression {
    marker: Vec<u8>,
    pending: Vec<u8>,
}

struct GhosttyCommandBlockRecord {
    id: u64,
    anchor: GhosttyTrackedGridRef,
    working_directory: Option<String>,
    command: Option<String>,
    exit_code: Option<i32>,
}

#[repr(C)]
struct GhosttyBuffer {
    ptr: *mut u8,
    cap: usize,
    len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GhosttyTerminalScrollbar {
    total: u64,
    offset: u64,
    len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
union GhosttyScrollValue {
    delta: isize,
    row: usize,
    padding: [u64; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GhosttyScrollViewport {
    tag: c_int,
    value: GhosttyScrollValue,
}

#[repr(C)]
struct GhosttyKittyPlacementRenderInfo {
    size: usize,
    pixel_width: u32,
    pixel_height: u32,
    grid_cols: u32,
    grid_rows: u32,
    viewport_col: i32,
    viewport_row: i32,
    viewport_visible: bool,
    source_x: u32,
    source_y: u32,
    source_width: u32,
    source_height: u32,
}

#[repr(C)]
struct GhosttySysImage {
    width: u32,
    height: u32,
    data: *mut u8,
    data_len: usize,
}

#[cfg_attr(windows, link(name = "ghostty-vt", kind = "raw-dylib"))]
unsafe extern "C" {
    fn ghostty_sys_set(option: c_int, value: *const c_void) -> c_int;
    fn ghostty_alloc(allocator: *const c_void, len: usize) -> *mut u8;
    fn ghostty_terminal_new(
        allocator: *const c_void,
        terminal: *mut GhosttyTerminal,
        columns: u16,
        rows: u16,
    ) -> c_int;
    fn ghostty_terminal_free(terminal: GhosttyTerminal);
    fn ghostty_terminal_resize(
        terminal: GhosttyTerminal,
        columns: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> c_int;
    fn ghostty_terminal_set(
        terminal: GhosttyTerminal,
        option: c_int,
        value: *const c_void,
    ) -> c_int;
    fn ghostty_terminal_get(terminal: GhosttyTerminal, data: c_int, value: *mut c_void) -> c_int;
    fn ghostty_terminal_vt_write(terminal: GhosttyTerminal, data: *const u8, len: usize);
    fn ghostty_terminal_scroll_viewport(terminal: GhosttyTerminal, behavior: GhosttyScrollViewport);
    fn ghostty_terminal_grid_ref(
        terminal: GhosttyTerminal,
        point: GhosttyPoint,
        out_ref: *mut GhosttyGridRef,
    ) -> c_int;
    fn ghostty_terminal_grid_ref_track(
        terminal: GhosttyTerminal,
        point: GhosttyPoint,
        out_ref: *mut GhosttyTrackedGridRef,
    ) -> c_int;
    fn ghostty_tracked_grid_ref_free(grid_ref: GhosttyTrackedGridRef);
    fn ghostty_tracked_grid_ref_point(
        grid_ref: GhosttyTrackedGridRef,
        tag: c_int,
        point: *mut GhosttyPointCoordinate,
    ) -> c_int;
    fn ghostty_grid_ref_graphemes(
        grid_ref: *const GhosttyGridRef,
        buffer: *mut u32,
        buffer_len: usize,
        out_len: *mut usize,
    ) -> c_int;
    fn ghostty_grid_ref_row(grid_ref: *const GhosttyGridRef, row: *mut u64) -> c_int;
    fn ghostty_row_get(row: u64, data: c_int, value: *mut c_void) -> c_int;
    fn ghostty_grid_ref_hyperlink_uri(
        grid_ref: *const GhosttyGridRef,
        buffer: *mut u8,
        buffer_len: usize,
        out_len: *mut usize,
    ) -> c_int;
    fn ghostty_terminal_select_all(
        terminal: GhosttyTerminal,
        selection: *mut GhosttySelection,
    ) -> c_int;
    fn ghostty_terminal_selection_format_buf(
        terminal: GhosttyTerminal,
        options: GhosttySelectionFormatOptions,
        buffer: *mut u8,
        buffer_len: usize,
        out_written: *mut usize,
    ) -> c_int;

    fn ghostty_render_state_new(allocator: *const c_void, state: *mut GhosttyRenderState) -> c_int;
    fn ghostty_render_state_free(state: GhosttyRenderState);
    fn ghostty_render_state_update(state: GhosttyRenderState, terminal: GhosttyTerminal) -> c_int;
    fn ghostty_render_state_get(
        state: GhosttyRenderState,
        data: c_int,
        value: *mut c_void,
    ) -> c_int;
    fn ghostty_render_state_row_iterator_new(
        allocator: *const c_void,
        iterator: *mut GhosttyRowIterator,
    ) -> c_int;
    fn ghostty_render_state_row_iterator_free(iterator: GhosttyRowIterator);
    fn ghostty_render_state_row_iterator_next(iterator: GhosttyRowIterator) -> bool;
    fn ghostty_render_state_row_get(
        iterator: GhosttyRowIterator,
        data: c_int,
        value: *mut c_void,
    ) -> c_int;
    fn ghostty_render_state_row_cells_new(
        allocator: *const c_void,
        cells: *mut GhosttyRowCells,
    ) -> c_int;
    fn ghostty_render_state_row_cells_free(cells: GhosttyRowCells);
    fn ghostty_render_state_row_cells_next(cells: GhosttyRowCells) -> bool;
    fn ghostty_render_state_row_cells_get(
        cells: GhosttyRowCells,
        data: c_int,
        value: *mut c_void,
    ) -> c_int;
    fn ghostty_cell_get(cell: u64, data: c_int, value: *mut c_void) -> c_int;
    fn ghostty_kitty_graphics_get(
        graphics: GhosttyKittyGraphics,
        data: c_int,
        value: *mut c_void,
    ) -> c_int;
    fn ghostty_kitty_graphics_image(
        graphics: GhosttyKittyGraphics,
        image_id: u32,
    ) -> GhosttyKittyGraphicsImage;
    fn ghostty_kitty_graphics_image_get(
        image: GhosttyKittyGraphicsImage,
        data: c_int,
        value: *mut c_void,
    ) -> c_int;
    fn ghostty_kitty_graphics_placement_iterator_new(
        allocator: *const c_void,
        iterator: *mut GhosttyKittyGraphicsPlacementIterator,
    ) -> c_int;
    fn ghostty_kitty_graphics_placement_iterator_free(
        iterator: GhosttyKittyGraphicsPlacementIterator,
    );
    fn ghostty_kitty_graphics_placement_next(
        iterator: GhosttyKittyGraphicsPlacementIterator,
    ) -> bool;
    fn ghostty_kitty_graphics_placement_get(
        iterator: GhosttyKittyGraphicsPlacementIterator,
        data: c_int,
        value: *mut c_void,
    ) -> c_int;
    fn ghostty_kitty_graphics_placement_render_info(
        iterator: GhosttyKittyGraphicsPlacementIterator,
        image: GhosttyKittyGraphicsImage,
        terminal: GhosttyTerminal,
        info: *mut GhosttyKittyPlacementRenderInfo,
    ) -> c_int;
}

pub struct GhosttyTerminalEngine {
    terminal: GhosttyTerminal,
    render_state: GhosttyRenderState,
    size: TerminalGeometry,
    options: TerminalOptions,
    default_foreground: u32,
    default_background: u32,
    default_cursor: u32,
    cell_width_px: u32,
    cell_height_px: u32,
    callbacks: Box<Mutex<GhosttyCallbackState>>,
    pty: Option<LocalPty>,
    wakeup_callback: Option<WakeupCallback>,
    input_echo_override: Option<bool>,
    exited: bool,
    output_capture: Vec<u8>,
    output_suppression: Option<OutputSuppression>,
    osc_pending: Vec<u8>,
    command_block_id: u64,
    command_blocks: VecDeque<GhosttyCommandBlockRecord>,
    current_working_directory: Option<String>,
    last_snapshot: RefCell<Option<TerminalSnapshot>>,
    #[cfg(test)]
    force_snapshot_failure: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl GhosttyTerminalEngine {
    pub fn new(columns: usize, rows: usize, options: TerminalOptions) -> Result<Self, String> {
        install_png_decoder();
        let columns = columns.clamp(1, u16::MAX as usize) as u16;
        let rows = rows.clamp(1, u16::MAX as usize) as u16;
        let mut terminal = ptr::null_mut();
        let result = unsafe { ghostty_terminal_new(ptr::null(), &mut terminal, columns, rows) };
        if result != GHOSTTY_SUCCESS || terminal.is_null() {
            return Err(format!("ghostty_terminal_new failed with result {result}"));
        }

        let mut render_state = ptr::null_mut();
        let result = unsafe { ghostty_render_state_new(ptr::null(), &mut render_state) };
        if result != GHOSTTY_SUCCESS || render_state.is_null() {
            unsafe { ghostty_terminal_free(terminal) };
            return Err(format!(
                "ghostty_render_state_new failed with result {result}"
            ));
        }

        let default_foreground = options.default_colors.foreground_u32();
        let default_background = options.default_colors.background_u32();
        let default_cursor = options.default_colors.cursor_u32();
        let mut engine = Self {
            terminal,
            render_state,
            size: TerminalGeometry::new(columns as usize, rows as usize),
            options,
            default_foreground,
            default_background,
            default_cursor,
            cell_width_px: 1,
            cell_height_px: 1,
            callbacks: Box::new(Mutex::new(GhosttyCallbackState::default())),
            pty: None,
            wakeup_callback: None,
            input_echo_override: None,
            exited: false,
            output_capture: Vec::new(),
            output_suppression: None,
            osc_pending: Vec::new(),
            command_block_id: 0,
            command_blocks: VecDeque::new(),
            current_working_directory: None,
            last_snapshot: RefCell::new(None),
            #[cfg(test)]
            force_snapshot_failure: false,
            _not_send_sync: PhantomData,
        };
        let options = engine.options.clone();
        engine.configure(&options)?;
        Ok(engine)
    }

    fn configure(&mut self, options: &TerminalOptions) -> Result<(), String> {
        let foreground = GhosttyColorRgb::from_u32(self.default_foreground);
        let background = GhosttyColorRgb::from_u32(self.default_background);
        let cursor = GhosttyColorRgb::from_u32(self.default_cursor);
        let palette = options
            .default_colors
            .palette_u32()
            .map(GhosttyColorRgb::from_u32);
        let scrollback = options.scrollback_lines;
        let kitty_limit: u64 = 64 * 1024 * 1024;
        let cursor_style: c_int = match options.cursor_shape_u32() {
            1 => 2,
            2 => 0,
            3 => 3,
            _ => 1,
        };
        let callbacks = self.callbacks.as_ref() as *const Mutex<GhosttyCallbackState>;
        for (option, value) in [
            (TERMINAL_OPT_USERDATA, callbacks as *const c_void),
            (
                TERMINAL_OPT_WRITE_PTY,
                write_pty as *const () as *const c_void,
            ),
            (TERMINAL_OPT_BELL, bell as *const () as *const c_void),
            (
                TERMINAL_OPT_COLOR_FOREGROUND,
                &foreground as *const _ as *const c_void,
            ),
            (
                TERMINAL_OPT_COLOR_BACKGROUND,
                &background as *const _ as *const c_void,
            ),
            (
                TERMINAL_OPT_COLOR_CURSOR,
                &cursor as *const _ as *const c_void,
            ),
            (
                TERMINAL_OPT_COLOR_PALETTE,
                palette.as_ptr() as *const c_void,
            ),
            (
                TERMINAL_OPT_SCROLLBACK_MAX_LINES,
                &scrollback as *const _ as *const c_void,
            ),
            (
                TERMINAL_OPT_KITTY_IMAGE_STORAGE_LIMIT,
                &kitty_limit as *const _ as *const c_void,
            ),
            (
                TERMINAL_OPT_DEFAULT_CURSOR_STYLE,
                &cursor_style as *const _ as *const c_void,
            ),
            (
                TERMINAL_OPT_DEFAULT_CURSOR_BLINK,
                &options.cursor_blinking as *const _ as *const c_void,
            ),
            (
                TERMINAL_OPT_CLIPBOARD_WRITE,
                clipboard_write as *const () as *const c_void,
            ),
        ] {
            let result = unsafe { ghostty_terminal_set(self.terminal, option, value) };
            if result != GHOSTTY_SUCCESS {
                return Err(format!(
                    "ghostty_terminal_set({option}) failed with result {result}"
                ));
            }
        }
        Ok(())
    }

    fn write_ghostty(&mut self, bytes: &[u8]) {
        if !bytes.is_empty() {
            unsafe { ghostty_terminal_vt_write(self.terminal, bytes.as_ptr(), bytes.len()) };
        }
    }

    fn take_ghostty_writes(&mut self) -> Vec<String> {
        self.callbacks
            .lock()
            .ok()
            .map(|mut state| {
                if state.writes.is_empty() {
                    return Vec::new();
                }
                let output = String::from_utf8_lossy(&state.writes).into_owned();
                state.writes.clear();
                vec![output]
            })
            .unwrap_or_default()
    }

    fn flush_ghostty_writes_to_local_pty(&mut self) {
        let writes = self.take_ghostty_writes();
        let Some(pty) = &mut self.pty else {
            return;
        };
        for write in writes {
            pty.queue_input(write.as_bytes());
        }
    }

    fn scroll(&mut self, tag: c_int, delta: isize) {
        let behavior = GhosttyScrollViewport {
            tag,
            value: GhosttyScrollValue { delta },
        };
        unsafe { ghostty_terminal_scroll_viewport(self.terminal, behavior) };
    }

    fn input_echo_enabled(&self) -> bool {
        self.input_echo_override.unwrap_or_else(|| {
            self.pty
                .as_ref()
                .map(LocalPty::input_visible)
                .unwrap_or(true)
        })
    }

    fn filter_suppressed_output(&mut self, bytes: &[u8]) -> Vec<u8> {
        let Some(suppression) = &mut self.output_suppression else {
            return bytes.to_vec();
        };
        suppression.pending.extend_from_slice(bytes);
        if let Some(index) = find_bytes(&suppression.pending, &suppression.marker) {
            let visible_start = index + suppression.marker.len();
            let visible = suppression.pending[visible_start..].to_vec();
            self.output_suppression = None;
            return visible;
        }
        let retained = suppression.marker.len().saturating_sub(1);
        if suppression.pending.len() > retained {
            suppression
                .pending
                .drain(..suppression.pending.len() - retained);
        }
        Vec::new()
    }

    fn write_visible_bytes(&mut self, bytes: &[u8]) {
        self.osc_pending.extend_from_slice(bytes);
        loop {
            let Some(osc_start) = find_bytes(&self.osc_pending, b"\x1b]") else {
                let retained = usize::from(self.osc_pending.ends_with(b"\x1b"));
                let visible_len = self.osc_pending.len().saturating_sub(retained);
                if visible_len > 0 {
                    let remaining = self.osc_pending.split_off(visible_len);
                    let visible = std::mem::replace(&mut self.osc_pending, remaining);
                    self.write_ghostty(&visible);
                }
                return;
            };
            if osc_start > 0 {
                let remaining = self.osc_pending.split_off(osc_start);
                let visible = std::mem::replace(&mut self.osc_pending, remaining);
                self.write_ghostty(&visible);
                continue;
            }
            let bell = self.osc_pending[2..]
                .iter()
                .position(|byte| *byte == b'\x07')
                .map(|index| index + 2);
            let string_terminator =
                find_bytes(&self.osc_pending[2..], b"\x1b\\").map(|index| index + 2);
            let terminator = match (bell, string_terminator) {
                (Some(left), Some(right)) => {
                    Some((left.min(right), if right <= left { 2 } else { 1 }))
                }
                (Some(index), None) => Some((index, 1)),
                (None, Some(index)) => Some((index, 2)),
                (None, None) => None,
            };
            let Some((terminator, terminator_len)) = terminator else {
                if self.osc_pending.len() > MAX_OSC_BYTES {
                    let pending = std::mem::take(&mut self.osc_pending);
                    self.write_ghostty(&pending);
                }
                return;
            };
            let sequence_len = terminator + terminator_len;
            let remaining = self.osc_pending.split_off(sequence_len);
            let sequence = std::mem::replace(&mut self.osc_pending, remaining);
            self.write_ghostty(&sequence);
            self.handle_shell_integration_osc(&sequence[2..terminator]);
        }
    }

    fn handle_shell_integration_osc(&mut self, payload: &[u8]) {
        if let Some(uri) = payload.strip_prefix(b"7;") {
            self.current_working_directory = shell_integration_directory(uri);
            return;
        }
        if let Some(encoded) = payload.strip_prefix(b"4545;CommandStarted;") {
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) {
                if let Ok(command) = String::from_utf8(decoded) {
                    let command = command.trim();
                    if !command.is_empty() {
                        if let Some(block) = self.command_blocks.back_mut() {
                            block.command = Some(command.to_owned());
                        }
                    }
                }
            }
            return;
        }
        if let Some(status) = payload.strip_prefix(b"4545;CommandExited;") {
            if let Ok(exit_code) = std::str::from_utf8(status)
                .unwrap_or_default()
                .trim()
                .parse::<i32>()
            {
                if let Some(block) = self.command_blocks.back_mut() {
                    block.exit_code = Some(exit_code);
                }
            }
            return;
        }
        if payload != b"133;A" || self.is_alt_screen() {
            return;
        }
        let cursor_y = terminal_get_u16(self.terminal, 4).unwrap_or(0) as usize;
        let point = GhosttyPoint {
            tag: 0,
            value: GhosttyPointValue {
                coordinate: GhosttyPointCoordinate {
                    x: 0,
                    y: cursor_y as u32,
                },
            },
        };
        let mut anchor = ptr::null_mut();
        if unsafe { ghostty_terminal_grid_ref_track(self.terminal, point, &mut anchor) }
            != GHOSTTY_SUCCESS
            || anchor.is_null()
        {
            return;
        }
        self.command_block_id = self.command_block_id.saturating_add(1);
        self.command_blocks.push_back(GhosttyCommandBlockRecord {
            id: self.command_block_id,
            anchor,
            working_directory: self.current_working_directory.clone(),
            command: None,
            exit_code: None,
        });
        while self.command_blocks.len() > MAX_COMMAND_BLOCK_METADATA {
            if let Some(expired) = self.command_blocks.pop_front() {
                unsafe { ghostty_tracked_grid_ref_free(expired.anchor) };
            }
        }
    }

    fn format_selection(&self, selection: &GhosttySelection) -> Option<String> {
        let options = GhosttySelectionFormatOptions {
            size: std::mem::size_of::<GhosttySelectionFormatOptions>(),
            emit: FORMATTER_FORMAT_PLAIN,
            unwrap: true,
            trim: true,
            selection,
        };
        let mut required = 0usize;
        let result = unsafe {
            ghostty_terminal_selection_format_buf(
                self.terminal,
                options,
                ptr::null_mut(),
                0,
                &mut required,
            )
        };
        if result != GHOSTTY_OUT_OF_SPACE && !(result == GHOSTTY_SUCCESS && required == 0) {
            return None;
        }
        let mut bytes = vec![0u8; required];
        let result = unsafe {
            ghostty_terminal_selection_format_buf(
                self.terminal,
                options,
                bytes.as_mut_ptr(),
                bytes.len(),
                &mut required,
            )
        };
        if result != GHOSTTY_SUCCESS {
            return None;
        }
        bytes.truncate(required);
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn selection_for_offsets(&self, start: i64, end: i64) -> Option<GhosttySelection> {
        let columns = self.size.columns.max(1) as i64;
        let scrollback = terminal_get_usize(self.terminal, TERMINAL_DATA_SCROLLBACK_ROWS)? as i64;
        let total_rows = terminal_get_usize(self.terminal, TERMINAL_DATA_TOTAL_ROWS)? as i64;
        let minimum = -scrollback * columns;
        let maximum = (total_rows - scrollback) * columns;
        let first = start.clamp(minimum, maximum);
        let end = end.clamp(minimum, maximum);
        if first >= end {
            return None;
        }
        let last = end - 1;
        let point = |offset: i64| GhosttyPoint {
            tag: POINT_TAG_SCREEN,
            value: GhosttyPointValue {
                coordinate: GhosttyPointCoordinate {
                    x: offset.rem_euclid(columns) as u16,
                    y: (scrollback + offset.div_euclid(columns)) as u32,
                },
            },
        };
        let mut selection = GhosttySelection::default();
        if unsafe { ghostty_terminal_grid_ref(self.terminal, point(first), &mut selection.start) }
            != GHOSTTY_SUCCESS
            || unsafe { ghostty_terminal_grid_ref(self.terminal, point(last), &mut selection.end) }
                != GHOSTTY_SUCCESS
        {
            return None;
        }
        Some(selection)
    }

    fn screen_rows(&self) -> Vec<Vec<String>> {
        let total_rows = terminal_get_usize(self.terminal, TERMINAL_DATA_TOTAL_ROWS).unwrap_or(0);
        (0..total_rows)
            .map(|row| {
                (0..self.size.columns)
                    .map(|column| grid_grapheme(self.terminal, POINT_TAG_SCREEN, column, row))
                    .collect()
            })
            .collect()
    }

    fn keyboard_mode(&self) -> u32 {
        let mode = |value| terminal_mode(self.terminal, value);
        u32::from(mode(MODE_DECCKM))
            | (u32::from(mode(MODE_KEYPAD_KEYS)) << 1)
            | (u32::from(mode(MODE_BRACKETED_PASTE)) << 2)
            | (u32::from(mode(MODE_FOCUS_EVENT)) << 3)
            | (u32::from(mode(MODE_X10_MOUSE) || mode(MODE_NORMAL_MOUSE)) << 4)
            | (u32::from(mode(MODE_BUTTON_MOUSE)) << 5)
            | (u32::from(mode(MODE_ANY_MOUSE)) << 6)
            | (u32::from(mode(MODE_SGR_MOUSE)) << 7)
    }

    fn empty_snapshot(&self) -> TerminalSnapshot {
        let cells = vec![
            empty_cell(self.default_foreground, self.default_background);
            self.size.columns * self.size.rows
        ];
        TerminalSnapshot {
            emulator_backend: TerminalEmulatorBackend::Ghostty,
            columns: self.size.columns,
            rows: self.size.rows,
            history_lines: 0,
            display_offset: 0,
            title: String::new(),
            clipboard: self.clipboard(),
            bell_count: self.bell_count(),
            cursor_column: 0,
            cursor_row: 0,
            cursor_visible: true,
            cursor_shape: 0,
            cursor_color: terminal_color(self.terminal, TERMINAL_DATA_COLOR_CURSOR)
                .unwrap_or(self.default_cursor),
            cursor_blinking: false,
            keyboard_mode: self.keyboard_mode(),
            input_echo_enabled: self.input_echo_enabled(),
            cells,
            text: Vec::new(),
            hyperlink_text: Vec::new(),
            graphic_images: Vec::new(),
            graphic_placements: Vec::new(),
        }
    }

    fn ghostty_snapshot(&self) -> Option<TerminalSnapshot> {
        #[cfg(test)]
        if self.force_snapshot_failure {
            return None;
        }
        if unsafe { ghostty_render_state_update(self.render_state, self.terminal) }
            != GHOSTTY_SUCCESS
        {
            return None;
        }

        let columns = render_get_u16(self.render_state, RENDER_DATA_COLS)? as usize;
        let rows = render_get_u16(self.render_state, RENDER_DATA_ROWS)? as usize;
        let mut scrollbar = GhosttyTerminalScrollbar::default();
        let _ = unsafe {
            ghostty_terminal_get(
                self.terminal,
                TERMINAL_DATA_SCROLLBAR,
                &mut scrollbar as *mut _ as *mut c_void,
            )
        };

        let mut row_iterator = ptr::null_mut();
        if unsafe { ghostty_render_state_row_iterator_new(ptr::null(), &mut row_iterator) }
            != GHOSTTY_SUCCESS
        {
            return None;
        }
        let mut cells_iterator = ptr::null_mut();
        if unsafe { ghostty_render_state_row_cells_new(ptr::null(), &mut cells_iterator) }
            != GHOSTTY_SUCCESS
        {
            unsafe { ghostty_render_state_row_iterator_free(row_iterator) };
            return None;
        }

        let mut cells = Vec::with_capacity(columns * rows);
        let mut text = Vec::new();
        let mut hyperlink_text = Vec::new();
        let mut virtual_placeholders = Vec::new();
        let populated = unsafe {
            ghostty_render_state_get(
                self.render_state,
                RENDER_DATA_ROW_ITERATOR,
                &mut row_iterator as *mut _ as *mut c_void,
            )
        } == GHOSTTY_SUCCESS;
        if populated {
            let mut cell_context = ExportCellContext {
                terminal: self.terminal,
                text: &mut text,
                hyperlink_text: &mut hyperlink_text,
                default_foreground: self.default_foreground,
                default_background: self.default_background,
            };
            let mut row = 0usize;
            while row < rows && unsafe { ghostty_render_state_row_iterator_next(row_iterator) } {
                let row_ready = unsafe {
                    ghostty_render_state_row_get(
                        row_iterator,
                        RENDER_ROW_DATA_CELLS,
                        &mut cells_iterator as *mut _ as *mut c_void,
                    )
                } == GHOSTTY_SUCCESS;
                if !row_ready {
                    break;
                }
                let mut column = 0usize;
                let mut previous_virtual = None;
                while column < columns
                    && unsafe { ghostty_render_state_row_cells_next(cells_iterator) }
                {
                    let (cell, virtual_placeholder) = export_cell(
                        cells_iterator,
                        column,
                        row,
                        previous_virtual.as_ref(),
                        &mut cell_context,
                    );
                    cells.push(cell);
                    if let Some(placeholder) = virtual_placeholder {
                        previous_virtual = Some(placeholder);
                        virtual_placeholders.push(placeholder);
                    } else {
                        previous_virtual = None;
                    }
                    column += 1;
                }
                while column < columns {
                    cells.push(empty_cell(self.default_foreground, self.default_background));
                    column += 1;
                }
                row += 1;
            }
        }
        while cells.len() < columns * rows {
            cells.push(empty_cell(self.default_foreground, self.default_background));
        }
        unsafe {
            ghostty_render_state_row_cells_free(cells_iterator);
            ghostty_render_state_row_iterator_free(row_iterator);
        }

        let cursor_in_viewport =
            render_get_bool(self.render_state, RENDER_DATA_CURSOR_VIEWPORT_HAS_VALUE)
                .unwrap_or(false);
        let cursor_column = if cursor_in_viewport {
            render_get_u16(self.render_state, RENDER_DATA_CURSOR_VIEWPORT_X).unwrap_or(0) as usize
        } else {
            0
        };
        let cursor_row = if cursor_in_viewport {
            render_get_u16(self.render_state, RENDER_DATA_CURSOR_VIEWPORT_Y).unwrap_or(0) as usize
        } else {
            0
        };
        let cursor_shape =
            match render_get_i32(self.render_state, RENDER_DATA_CURSOR_VISUAL_STYLE).unwrap_or(1) {
                0 => 2,
                2 => 1,
                3 => 3,
                _ => 0,
            };
        let history_lines = scrollbar.total.saturating_sub(scrollbar.len) as usize;
        let display_offset = history_lines.saturating_sub(scrollbar.offset as usize);
        let (graphic_images, graphic_placements) = self.graphics_snapshot(&virtual_placeholders);

        Some(TerminalSnapshot {
            emulator_backend: TerminalEmulatorBackend::Ghostty,
            columns,
            rows,
            history_lines,
            display_offset,
            title: terminal_string(self.terminal, TERMINAL_DATA_TITLE),
            clipboard: self.clipboard(),
            bell_count: self.bell_count(),
            cursor_column,
            cursor_row,
            cursor_visible: render_get_bool(self.render_state, RENDER_DATA_CURSOR_VISIBLE)
                .unwrap_or(true),
            cursor_shape,
            cursor_color: self.default_cursor,
            cursor_blinking: render_get_bool(self.render_state, RENDER_DATA_CURSOR_BLINKING)
                .unwrap_or(false),
            keyboard_mode: self.keyboard_mode(),
            input_echo_enabled: self.input_echo_enabled(),
            cells,
            text,
            hyperlink_text,
            graphic_images,
            graphic_placements,
        })
    }

    fn graphics_snapshot(
        &self,
        virtual_placeholders: &[VirtualPlaceholder],
    ) -> (Vec<TerminalGraphicImage>, Vec<TerminalGraphicPlacement>) {
        let mut graphics: GhosttyKittyGraphics = ptr::null_mut();
        if unsafe {
            ghostty_terminal_get(
                self.terminal,
                TERMINAL_DATA_KITTY_GRAPHICS,
                &mut graphics as *mut _ as *mut c_void,
            )
        } != GHOSTTY_SUCCESS
            || graphics.is_null()
        {
            return (Vec::new(), Vec::new());
        }

        let mut iterator: GhosttyKittyGraphicsPlacementIterator = ptr::null_mut();
        if unsafe { ghostty_kitty_graphics_placement_iterator_new(ptr::null(), &mut iterator) }
            != GHOSTTY_SUCCESS
            || iterator.is_null()
        {
            return (Vec::new(), Vec::new());
        }
        if unsafe {
            ghostty_kitty_graphics_get(
                graphics,
                KITTY_GRAPHICS_DATA_PLACEMENT_ITERATOR,
                &mut iterator as *mut _ as *mut c_void,
            )
        } != GHOSTTY_SUCCESS
        {
            unsafe { ghostty_kitty_graphics_placement_iterator_free(iterator) };
            return (Vec::new(), Vec::new());
        }

        let mut seen_images = HashSet::new();
        let mut images = Vec::new();
        let mut placements = Vec::new();
        let mut virtual_placements = HashMap::new();
        while unsafe { ghostty_kitty_graphics_placement_next(iterator) } {
            let mut image_id = 0u32;
            let mut placement_id = 0u32;
            let mut is_virtual = false;
            let mut placement_columns = 0u32;
            let mut placement_rows = 0u32;
            let mut z_index = 0i32;
            if !placement_get(iterator, KITTY_PLACEMENT_DATA_IMAGE_ID, &mut image_id)
                || !placement_get(
                    iterator,
                    KITTY_PLACEMENT_DATA_PLACEMENT_ID,
                    &mut placement_id,
                )
                || !placement_get(iterator, KITTY_PLACEMENT_DATA_IS_VIRTUAL, &mut is_virtual)
                || !placement_get(
                    iterator,
                    KITTY_PLACEMENT_DATA_COLUMNS,
                    &mut placement_columns,
                )
                || !placement_get(iterator, KITTY_PLACEMENT_DATA_ROWS, &mut placement_rows)
                || !placement_get(iterator, KITTY_PLACEMENT_DATA_Z, &mut z_index)
            {
                continue;
            }
            let image = unsafe { ghostty_kitty_graphics_image(graphics, image_id) };
            if image.is_null() {
                continue;
            }
            if is_virtual {
                virtual_placements.insert(
                    (image_id, placement_id),
                    VirtualPlacementDefinition {
                        image_id,
                        placement_id,
                        columns: placement_columns,
                        rows: placement_rows,
                        z_index,
                    },
                );
                continue;
            }
            let mut info = GhosttyKittyPlacementRenderInfo {
                size: std::mem::size_of::<GhosttyKittyPlacementRenderInfo>(),
                pixel_width: 0,
                pixel_height: 0,
                grid_cols: 0,
                grid_rows: 0,
                viewport_col: 0,
                viewport_row: 0,
                viewport_visible: false,
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
            };
            if unsafe {
                ghostty_kitty_graphics_placement_render_info(
                    iterator,
                    image,
                    self.terminal,
                    &mut info,
                )
            } != GHOSTTY_SUCCESS
                || !info.viewport_visible
            {
                continue;
            }
            placements.push(TerminalGraphicPlacement {
                image_id,
                placement_id,
                z_index,
                viewport_column: info.viewport_col,
                viewport_row: info.viewport_row,
                columns: info.grid_cols,
                rows: info.grid_rows,
                source_x: info.source_x,
                source_y: info.source_y,
                source_width: info.source_width,
                source_height: info.source_height,
            });
            if seen_images.insert(image_id) {
                if let Some(image) = export_graphic_image(image_id, image) {
                    images.push(image);
                }
            }
        }
        for placeholder in virtual_placeholders {
            let definition = virtual_placements
                .get(&(placeholder.image_id, placeholder.placement_id))
                .or_else(|| {
                    (placeholder.placement_id == 0).then(|| {
                        virtual_placements
                            .values()
                            .find(|placement| placement.image_id == placeholder.image_id)
                    })?
                });
            let Some(definition) = definition else {
                continue;
            };
            let image = unsafe { ghostty_kitty_graphics_image(graphics, definition.image_id) };
            if image.is_null() {
                continue;
            }
            let Some((image_width, image_height)) = graphic_image_size(image) else {
                continue;
            };
            let grid_columns = if definition.columns > 0 {
                definition.columns
            } else {
                image_width.div_ceil(self.cell_width_px.max(1))
            }
            .max(1);
            let grid_rows = if definition.rows > 0 {
                definition.rows
            } else {
                image_height.div_ceil(self.cell_height_px.max(1))
            }
            .max(1);
            if placeholder.fragment_column >= grid_columns || placeholder.fragment_row >= grid_rows
            {
                continue;
            }
            let source_x = scale_grid_edge(image_width, placeholder.fragment_column, grid_columns);
            let source_right =
                scale_grid_edge(image_width, placeholder.fragment_column + 1, grid_columns);
            let source_y = scale_grid_edge(image_height, placeholder.fragment_row, grid_rows);
            let source_bottom =
                scale_grid_edge(image_height, placeholder.fragment_row + 1, grid_rows);
            placements.push(TerminalGraphicPlacement {
                image_id: definition.image_id,
                placement_id: definition.placement_id,
                z_index: definition.z_index,
                viewport_column: placeholder.viewport_column,
                viewport_row: placeholder.viewport_row,
                columns: 1,
                rows: 1,
                source_x,
                source_y,
                source_width: source_right.saturating_sub(source_x),
                source_height: source_bottom.saturating_sub(source_y),
            });
            if seen_images.insert(definition.image_id) {
                if let Some(image) = export_graphic_image(definition.image_id, image) {
                    images.push(image);
                }
            }
        }
        unsafe { ghostty_kitty_graphics_placement_iterator_free(iterator) };
        (images, placements)
    }
}

impl Drop for GhosttyTerminalEngine {
    fn drop(&mut self) {
        for record in self.command_blocks.drain(..) {
            unsafe { ghostty_tracked_grid_ref_free(record.anchor) };
        }
        unsafe {
            ghostty_render_state_free(self.render_state);
            ghostty_terminal_free(self.terminal);
        }
    }
}

impl TerminalEmulator for GhosttyTerminalEngine {
    fn resize(&mut self, columns: usize, rows: usize, cell_width_px: u32, cell_height_px: u32) {
        self.size = TerminalGeometry::new(columns, rows);
        if let Some(pty) = &mut self.pty {
            pty.resize(self.size);
        }
        self.cell_width_px = cell_width_px.max(1);
        self.cell_height_px = cell_height_px.max(1);
        let _ = unsafe {
            ghostty_terminal_resize(
                self.terminal,
                columns.clamp(1, u16::MAX as usize) as u16,
                rows.clamp(1, u16::MAX as usize) as u16,
                cell_width_px.max(1),
                cell_height_px.max(1),
            )
        };
    }

    fn is_alt_screen(&self) -> bool {
        let mut screen: c_int = 0;
        unsafe {
            ghostty_terminal_get(
                self.terminal,
                TERMINAL_DATA_ACTIVE_SCREEN,
                &mut screen as *mut _ as *mut c_void,
            ) == GHOSTTY_SUCCESS
                && screen == 1
        }
    }

    fn scroll_lines(&mut self, lines: i32) {
        self.scroll(SCROLL_VIEWPORT_DELTA, lines as isize);
    }

    fn scroll_page_up(&mut self) {
        self.scroll(SCROLL_VIEWPORT_DELTA, -(self.size.rows as isize));
    }

    fn scroll_page_down(&mut self) {
        self.scroll(SCROLL_VIEWPORT_DELTA, self.size.rows as isize);
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll(SCROLL_VIEWPORT_BOTTOM, 0);
    }

    fn search(
        &mut self,
        query: &str,
        direction: TerminalSearchDirection,
        origin_row: usize,
        origin_column: usize,
    ) -> TerminalSearchResult {
        if query.is_empty() {
            return TerminalSearchResult::not_found(self.size.columns, self.size.rows);
        }
        let rows = self.screen_rows();
        let scrollback =
            terminal_get_usize(self.terminal, TERMINAL_DATA_SCROLLBACK_ROWS).unwrap_or(0);
        let scrollbar = terminal_scrollbar(self.terminal);
        let viewport_top = scrollbar.offset as usize;
        let origin_absolute_row = viewport_top
            .saturating_add(origin_row)
            .min(rows.len().saturating_sub(1));
        let mut matches = Vec::new();
        for (row_index, cells) in rows.iter().enumerate() {
            let mut text = String::new();
            let mut byte_columns = Vec::new();
            for (column, grapheme) in cells.iter().enumerate() {
                for _ in grapheme.as_bytes() {
                    byte_columns.push(column);
                }
                text.push_str(grapheme);
            }
            for (byte_start, _) in text.match_indices(query) {
                let byte_end = byte_start + query.len();
                let start_column = byte_columns.get(byte_start).copied().unwrap_or(0);
                let end_column = byte_columns
                    .get(byte_end.saturating_sub(1))
                    .copied()
                    .unwrap_or(start_column)
                    .saturating_add(1);
                matches.push((row_index, start_column, end_column));
            }
        }
        let found = match direction {
            TerminalSearchDirection::Forward => matches
                .iter()
                .find(|(row, column, _)| (*row, *column) > (origin_absolute_row, origin_column))
                .or_else(|| matches.first()),
            TerminalSearchDirection::Backward => matches
                .iter()
                .rev()
                .find(|(row, column, _)| (*row, *column) < (origin_absolute_row, origin_column))
                .or_else(|| matches.last()),
        };
        let Some(&(row, start_column, end_column)) = found else {
            return TerminalSearchResult::not_found(self.size.columns, self.size.rows);
        };
        let viewport_row = row.min(scrollback.saturating_add(self.size.rows).saturating_sub(1));
        let target_top = viewport_row
            .saturating_sub(self.size.rows / 2)
            .min(scrollback);
        self.scroll(SCROLL_VIEWPORT_ROW, target_top as isize);
        TerminalSearchResult {
            found: true,
            columns: self.size.columns,
            rows: self.size.rows,
            start_row: row.saturating_sub(target_top),
            start_column,
            end_row: row.saturating_sub(target_top),
            end_column,
            error: None,
        }
    }

    fn start_local_pty(&mut self) -> bool {
        match LocalPty::spawn(self.size, &self.options) {
            Ok(mut pty) => {
                pty.set_wakeup_callback(self.wakeup_callback);
                self.pty = Some(pty);
                self.exited = false;
                true
            }
            Err(error) => {
                self.write_bytes(format!("\r\nFailed to start PTY: {error}\r\n").as_bytes());
                false
            }
        }
    }

    fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>) {
        self.wakeup_callback = callback;
        if let Some(pty) = &mut self.pty {
            pty.set_wakeup_callback(callback);
        }
    }

    fn send_input_bytes(&mut self, bytes: &[u8]) -> bool {
        let Some(pty) = &mut self.pty else {
            return false;
        };
        pty.queue_input(bytes);
        true
    }

    fn pump_local_pty(&mut self) -> bool {
        let mut changed = false;
        let mut exited = false;
        for _ in 0..MAX_POLL_OUTPUT_CHUNKS {
            let pump = {
                let Some(pty) = &mut self.pty else {
                    return changed;
                };
                pty.drain_output()
            };
            exited = pump.exited;
            if !pump.output.is_empty() {
                changed = true;
                self.write_bytes(&pump.output);
            }
            if !pump.has_more {
                break;
            }
        }
        self.flush_ghostty_writes_to_local_pty();
        if exited {
            self.pty = None;
            self.exited = true;
            changed = true;
        }
        changed
    }

    fn is_exited(&self) -> bool {
        self.exited
    }

    fn set_input_echo_enabled(&mut self, enabled: bool) {
        self.input_echo_override = Some(enabled);
    }

    fn mark_exited(&mut self) {
        self.exited = true;
    }

    fn mark_running(&mut self) {
        self.exited = false;
    }

    fn drain_transport_writes(&mut self) -> Vec<String> {
        self.take_ghostty_writes()
    }

    fn drain_output_capture(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output_capture)
    }

    fn suppress_output_until(&mut self, marker: &[u8]) -> bool {
        if marker.is_empty() {
            return false;
        }
        self.output_suppression = Some(OutputSuppression {
            marker: marker.to_vec(),
            pending: Vec::new(),
        });
        true
    }

    fn cancel_output_suppression(&mut self) {
        self.output_suppression = None;
    }

    fn clear_pending_output_for_close(&mut self) {
        self.output_capture.clear();
        self.output_capture.shrink_to_fit();
        if let Some(pty) = &mut self.pty {
            pty.clear_pending_output();
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.output_capture.extend_from_slice(bytes);
        let visible = self.filter_suppressed_output(bytes);
        if !visible.is_empty() {
            self.write_visible_bytes(&visible);
        }
    }

    fn write_bytes_without_capture(&mut self, bytes: &[u8]) {
        self.write_visible_bytes(bytes);
    }

    fn snapshot(&self) -> TerminalSnapshot {
        if let Some(snapshot) = self.ghostty_snapshot() {
            *self.last_snapshot.borrow_mut() = Some(snapshot.clone());
            return snapshot;
        }
        self.last_snapshot
            .borrow()
            .clone()
            .unwrap_or_else(|| self.empty_snapshot())
    }

    fn plain_text(&self) -> String {
        let mut selection = GhosttySelection::default();
        if unsafe { ghostty_terminal_select_all(self.terminal, &mut selection) } != GHOSTTY_SUCCESS
        {
            return String::new();
        }
        self.format_selection(&selection).unwrap_or_default()
    }

    fn selection_text(&self, start: i64, end: i64) -> String {
        self.selection_for_offsets(start, end)
            .and_then(|selection| self.format_selection(&selection))
            .unwrap_or_default()
    }

    fn command_block_at(&self, offset: i64) -> Option<TerminalCommandBlock> {
        if self.is_alt_screen() || self.size.columns == 0 {
            return None;
        }
        let scrollback = terminal_get_usize(self.terminal, TERMINAL_DATA_SCROLLBACK_ROWS)?;
        let total_rows = terminal_get_usize(self.terminal, TERMINAL_DATA_TOTAL_ROWS)?;
        let target_absolute_row: usize = (scrollback as i64
            + offset.div_euclid(self.size.columns as i64))
        .try_into()
        .ok()?;
        let tracked_rows = self
            .command_blocks
            .iter()
            .enumerate()
            .filter_map(|(index, record)| tracked_screen_row(record.anchor).map(|row| (index, row)))
            .collect::<Vec<_>>();
        if let Some(&(index, start_absolute_row)) = tracked_rows
            .iter()
            .rev()
            .find(|(_, row)| *row <= target_absolute_row)
        {
            let record = &self.command_blocks[index];
            let end_absolute_row = tracked_rows
                .iter()
                .find(|(next_index, row)| *next_index > index && *row > start_absolute_row)
                .map(|(_, row)| *row)
                .unwrap_or(total_rows);
            if target_absolute_row < end_absolute_row {
                let line_offset =
                    |row: usize| (row as i64 - scrollback as i64) * self.size.columns as i64;
                return Some(TerminalCommandBlock {
                    id: record.id,
                    start: line_offset(start_absolute_row),
                    end: line_offset(end_absolute_row),
                    working_directory: record.working_directory.clone(),
                    command: record.command.clone(),
                    exit_code: record.exit_code,
                    completed: record.exit_code.is_some(),
                    shell_integrated: true,
                });
            }
        }

        let rows = self.screen_rows();
        let prompt_rows = rows
            .iter()
            .enumerate()
            .filter_map(|(row, cells)| {
                let structured = row_semantic_prompt(self.terminal, row);
                let text = cells.concat();
                (structured || text_looks_like_prompt(&text)).then_some((row, structured))
            })
            .collect::<Vec<_>>();
        let prefer_structured = prompt_rows.iter().any(|(_, structured)| *structured);
        let eligible = |structured: bool| !prefer_structured || structured;
        let start_row = prompt_rows
            .iter()
            .rev()
            .find(|(row, structured)| *row <= target_absolute_row && eligible(*structured))
            .map(|(row, _)| *row)?;
        let end_row = prompt_rows
            .iter()
            .find(|(row, structured)| *row > target_absolute_row && eligible(*structured))
            .map(|(row, _)| *row)
            .unwrap_or(total_rows);
        let line_offset = |row: usize| (row as i64 - scrollback as i64) * self.size.columns as i64;
        Some(TerminalCommandBlock {
            id: 0,
            start: line_offset(start_row),
            end: line_offset(end_row),
            working_directory: None,
            command: None,
            exit_code: None,
            completed: end_row < total_rows,
            shell_integrated: false,
        })
    }

    fn clipboard(&self) -> String {
        self.callbacks
            .lock()
            .map(|state| state.clipboard.clone())
            .unwrap_or_default()
    }

    fn bell_count(&self) -> u64 {
        self.callbacks
            .lock()
            .map(|state| state.bell_count)
            .unwrap_or(0)
    }
}

fn export_cell(
    iterator: GhosttyRowCells,
    viewport_column: usize,
    viewport_row: usize,
    previous_virtual: Option<&VirtualPlaceholder>,
    context: &mut ExportCellContext<'_>,
) -> (FfiTerminalCell, Option<VirtualPlaceholder>) {
    let grapheme = cell_utf8(iterator);

    let mut style = GhosttyStyle::default();
    let _ = unsafe {
        ghostty_render_state_row_cells_get(
            iterator,
            RENDER_CELL_DATA_STYLE,
            &mut style as *mut _ as *mut c_void,
        )
    };
    let virtual_placeholder = parse_virtual_placeholder(
        &grapheme,
        &style,
        viewport_column,
        viewport_row,
        previous_virtual,
    );
    let is_placeholder = starts_with_kitty_unicode_placeholder(&grapheme);
    let text_offset = context.text.len() as u32;
    if !is_placeholder {
        context.text.extend_from_slice(&grapheme);
    }
    let mut foreground = GhosttyColorRgb::from_u32(context.default_foreground);
    let mut background = GhosttyColorRgb::from_u32(context.default_background);
    let _ = unsafe {
        ghostty_render_state_row_cells_get(
            iterator,
            RENDER_CELL_DATA_FG_COLOR,
            &mut foreground as *mut _ as *mut c_void,
        )
    };
    let _ = unsafe {
        ghostty_render_state_row_cells_get(
            iterator,
            RENDER_CELL_DATA_BG_COLOR,
            &mut background as *mut _ as *mut c_void,
        )
    };

    let mut raw_cell: u64 = 0;
    let mut wide: c_int = 0;
    if unsafe {
        ghostty_render_state_row_cells_get(
            iterator,
            RENDER_CELL_DATA_RAW,
            &mut raw_cell as *mut _ as *mut c_void,
        )
    } == GHOSTTY_SUCCESS
    {
        let _ = unsafe {
            ghostty_cell_get(raw_cell, CELL_DATA_WIDE, &mut wide as *mut _ as *mut c_void)
        };
    }

    let mut flags = 0u16;
    flags |= u16::from(style.inverse) * FLAG_INVERSE;
    flags |= u16::from(style.bold) * FLAG_BOLD;
    flags |= u16::from(style.italic) * FLAG_ITALIC;
    flags |= u16::from(style.faint) * FLAG_DIM;
    flags |= u16::from(style.invisible) * FLAG_HIDDEN;
    flags |= u16::from(style.strikethrough) * FLAG_STRIKEOUT;
    flags |= match style.underline {
        1 => FLAG_UNDERLINE,
        2 => FLAG_DOUBLE_UNDERLINE,
        3 => FLAG_UNDERCURL,
        4 => FLAG_DOTTED_UNDERLINE,
        5 => FLAG_DASHED_UNDERLINE,
        _ => 0,
    };
    flags |= match wide {
        CELL_WIDE_WIDE => FLAG_WIDE_CHAR,
        CELL_WIDE_SPACER_TAIL => FLAG_WIDE_CHAR_SPACER,
        CELL_WIDE_SPACER_HEAD => FLAG_LEADING_WIDE_CHAR_SPACER,
        _ => 0,
    };

    let hyperlink_offset = context.hyperlink_text.len() as u32;
    let hyperlink = grid_hyperlink(
        context.terminal,
        POINT_TAG_VIEWPORT,
        viewport_column,
        viewport_row,
    );
    context.hyperlink_text.extend_from_slice(&hyperlink);

    (
        FfiTerminalCell {
            text_offset,
            text_len: if is_placeholder {
                0
            } else {
                grapheme.len().min(u16::MAX as usize) as u16
            },
            flags,
            foreground: foreground.as_u32(),
            background: background.as_u32(),
            hyperlink_offset,
            hyperlink_len: hyperlink.len() as u32,
        },
        virtual_placeholder,
    )
}

fn starts_with_kitty_unicode_placeholder(grapheme: &[u8]) -> bool {
    std::str::from_utf8(grapheme)
        .ok()
        .and_then(|value| value.chars().next())
        == Some(KITTY_UNICODE_PLACEHOLDER)
}

fn parse_virtual_placeholder(
    grapheme: &[u8],
    style: &GhosttyStyle,
    viewport_column: usize,
    viewport_row: usize,
    previous: Option<&VirtualPlaceholder>,
) -> Option<VirtualPlaceholder> {
    let mut characters = std::str::from_utf8(grapheme).ok()?.chars();
    if characters.next()? != KITTY_UNICODE_PLACEHOLDER {
        return None;
    }
    let image_id_low = style_color_id(style.fg_color)?;
    let placement_id = style_color_id(style.underline_color).unwrap_or(0);
    let indices = characters
        .take(3)
        .map(kitty_diacritic_index)
        .collect::<Vec<_>>();
    let same_run = previous.filter(|cell| {
        cell.image_id & 0x00ff_ffff == image_id_low && cell.placement_id == placement_id
    });
    let fragment_row = indices
        .first()
        .copied()
        .flatten()
        .or_else(|| same_run.map(|cell| cell.fragment_row))
        .unwrap_or(0);
    let fragment_column = indices
        .get(1)
        .copied()
        .flatten()
        .or_else(|| same_run.map(|cell| cell.fragment_column + 1))
        .unwrap_or(0);
    let image_id_high = indices
        .get(2)
        .copied()
        .flatten()
        .and_then(|value| u8::try_from(value).ok())
        .or_else(|| same_run.map(|cell| (cell.image_id >> 24) as u8))
        .unwrap_or(0);
    Some(VirtualPlaceholder {
        viewport_column: viewport_column as i32,
        viewport_row: viewport_row as i32,
        image_id: image_id_low | ((image_id_high as u32) << 24),
        placement_id,
        fragment_column,
        fragment_row,
    })
}

fn style_color_id(color: GhosttyStyleColor) -> Option<u32> {
    match color.tag {
        STYLE_COLOR_PALETTE => Some(unsafe { color.value.palette } as u32),
        STYLE_COLOR_RGB => Some(unsafe { color.value.rgb }.as_u32()),
        _ => None,
    }
}

fn scale_grid_edge(image_size: u32, edge: u32, grid_size: u32) -> u32 {
    ((image_size as u64 * edge as u64) / grid_size.max(1) as u64) as u32
}

fn kitty_diacritic_index(character: char) -> Option<u32> {
    KITTY_ROW_COLUMN_DIACRITICS
        .binary_search(&(character as u32))
        .ok()
        .map(|index| index as u32)
}

// Kitty's standardized row/column diacritic table. The array index is the
// encoded grid coordinate (and, for the third mark, the image ID high byte).
const KITTY_ROW_COLUMN_DIACRITICS: &[u32] = &[
    0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F, 0x0346, 0x034A, 0x034B, 0x034C,
    0x0350, 0x0351, 0x0352, 0x0357, 0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
    0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484, 0x0485, 0x0486, 0x0487, 0x0592,
    0x0593, 0x0594, 0x0595, 0x0597, 0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1,
    0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611, 0x0612, 0x0613, 0x0614, 0x0615,
    0x0616, 0x0617, 0x0657, 0x0658, 0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8,
    0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2, 0x06E4, 0x06E7, 0x06E8, 0x06EB,
    0x06EC, 0x0730, 0x0732, 0x0733, 0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743,
    0x0745, 0x0747, 0x0749, 0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE, 0x07EF, 0x07F0, 0x07F1, 0x07F3,
    0x0816, 0x0817, 0x0818, 0x0819, 0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
    0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C, 0x082D, 0x0951, 0x0953, 0x0954,
    0x0F82, 0x0F83, 0x0F86, 0x0F87, 0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75, 0x1A76,
    0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D, 0x1B6E, 0x1B6F, 0x1B70, 0x1B71,
    0x1B72, 0x1B73, 0x1CD0, 0x1CD1, 0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4,
    0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1, 0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5,
    0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9, 0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1,
    0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1, 0x20D4, 0x20D5, 0x20D6, 0x20D7,
    0x20DB, 0x20DC, 0x20E1, 0x20E7, 0x20E9, 0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2,
    0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6, 0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA, 0x2DEB, 0x2DEC, 0x2DED, 0x2DEE,
    0x2DEF, 0x2DF0, 0x2DF1, 0x2DF2, 0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
    0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D, 0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1,
    0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5, 0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
    0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7, 0xAAB8, 0xAABE, 0xAABF, 0xAAC1,
    0xFE20, 0xFE21, 0xFE22, 0xFE23, 0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186,
    0x1D187, 0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243, 0x1D244,
];

fn empty_cell(foreground: u32, background: u32) -> FfiTerminalCell {
    FfiTerminalCell {
        text_offset: 0,
        text_len: 0,
        flags: 0,
        foreground,
        background,
        hyperlink_offset: 0,
        hyperlink_len: 0,
    }
}

fn cell_utf8(iterator: GhosttyRowCells) -> Vec<u8> {
    let mut stack = [0u8; 32];
    let mut buffer = GhosttyBuffer {
        ptr: stack.as_mut_ptr(),
        cap: stack.len(),
        len: 0,
    };
    let result = unsafe {
        ghostty_render_state_row_cells_get(
            iterator,
            RENDER_CELL_DATA_GRAPHEMES_UTF8,
            &mut buffer as *mut _ as *mut c_void,
        )
    };
    if result == GHOSTTY_SUCCESS {
        return stack[..buffer.len.min(stack.len())].to_vec();
    }
    if result != GHOSTTY_OUT_OF_SPACE || buffer.len == 0 {
        return Vec::new();
    }
    let mut bytes = vec![0u8; buffer.len];
    buffer.ptr = bytes.as_mut_ptr();
    buffer.cap = bytes.len();
    buffer.len = 0;
    if unsafe {
        ghostty_render_state_row_cells_get(
            iterator,
            RENDER_CELL_DATA_GRAPHEMES_UTF8,
            &mut buffer as *mut _ as *mut c_void,
        )
    } == GHOSTTY_SUCCESS
    {
        bytes.truncate(buffer.len);
        bytes
    } else {
        Vec::new()
    }
}

fn render_get_u16(state: GhosttyRenderState, data: c_int) -> Option<u16> {
    let mut value = 0u16;
    (unsafe { ghostty_render_state_get(state, data, &mut value as *mut _ as *mut c_void) }
        == GHOSTTY_SUCCESS)
        .then_some(value)
}

fn render_get_i32(state: GhosttyRenderState, data: c_int) -> Option<c_int> {
    let mut value = 0;
    (unsafe { ghostty_render_state_get(state, data, &mut value as *mut _ as *mut c_void) }
        == GHOSTTY_SUCCESS)
        .then_some(value)
}

fn render_get_bool(state: GhosttyRenderState, data: c_int) -> Option<bool> {
    let mut value = false;
    (unsafe { ghostty_render_state_get(state, data, &mut value as *mut _ as *mut c_void) }
        == GHOSTTY_SUCCESS)
        .then_some(value)
}

fn placement_get<T>(
    iterator: GhosttyKittyGraphicsPlacementIterator,
    data: c_int,
    value: &mut T,
) -> bool {
    unsafe {
        ghostty_kitty_graphics_placement_get(iterator, data, value as *mut T as *mut c_void)
            == GHOSTTY_SUCCESS
    }
}

fn image_get<T>(image: GhosttyKittyGraphicsImage, data: c_int, value: &mut T) -> bool {
    unsafe {
        ghostty_kitty_graphics_image_get(image, data, value as *mut T as *mut c_void)
            == GHOSTTY_SUCCESS
    }
}

fn graphic_image_size(image: GhosttyKittyGraphicsImage) -> Option<(u32, u32)> {
    let mut width = 0u32;
    let mut height = 0u32;
    (image_get(image, KITTY_IMAGE_DATA_WIDTH, &mut width)
        && image_get(image, KITTY_IMAGE_DATA_HEIGHT, &mut height))
    .then_some((width, height))
}

fn export_graphic_image(id: u32, image: GhosttyKittyGraphicsImage) -> Option<TerminalGraphicImage> {
    let mut generation = 0u64;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut format = 0i32;
    let mut data_ptr: *const u8 = ptr::null();
    let mut data_len = 0usize;
    if !image_get(image, KITTY_IMAGE_DATA_GENERATION, &mut generation)
        || !image_get(image, KITTY_IMAGE_DATA_WIDTH, &mut width)
        || !image_get(image, KITTY_IMAGE_DATA_HEIGHT, &mut height)
        || !image_get(image, KITTY_IMAGE_DATA_FORMAT, &mut format)
        || !image_get(image, KITTY_IMAGE_DATA_DATA_PTR, &mut data_ptr)
        || !image_get(image, KITTY_IMAGE_DATA_DATA_LEN, &mut data_len)
        || data_ptr.is_null()
    {
        return None;
    }
    let source = unsafe { slice::from_raw_parts(data_ptr, data_len) };
    let rgba = pixels_to_rgba(source, width, height, format)?;
    Some(TerminalGraphicImage {
        id,
        generation,
        width,
        height,
        rgba,
    })
}

fn pixels_to_rgba(source: &[u8], width: u32, height: u32, format: i32) -> Option<Vec<u8>> {
    let pixels = (width as usize).checked_mul(height as usize)?;
    let channels = match format {
        0 => 3,
        1 => 4,
        3 => 2,
        4 => 1,
        _ => return None,
    };
    if source.len() != pixels.checked_mul(channels)? {
        return None;
    }
    if format == 1 {
        return Some(source.to_vec());
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for pixel in source.chunks_exact(channels) {
        match format {
            0 => rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 0xff]),
            3 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]),
            4 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 0xff]),
            _ => unreachable!(),
        }
    }
    Some(rgba)
}

fn terminal_get_u16(terminal: GhosttyTerminal, data: c_int) -> Option<u16> {
    let mut value = 0u16;
    (unsafe { ghostty_terminal_get(terminal, data, &mut value as *mut _ as *mut c_void) }
        == GHOSTTY_SUCCESS)
        .then_some(value)
}

fn terminal_get_usize(terminal: GhosttyTerminal, data: c_int) -> Option<usize> {
    let mut value = 0usize;
    (unsafe { ghostty_terminal_get(terminal, data, &mut value as *mut _ as *mut c_void) }
        == GHOSTTY_SUCCESS)
        .then_some(value)
}

fn terminal_color(terminal: GhosttyTerminal, data: c_int) -> Option<u32> {
    let mut value = GhosttyColorRgb::default();
    (unsafe { ghostty_terminal_get(terminal, data, &mut value as *mut _ as *mut c_void) }
        == GHOSTTY_SUCCESS)
        .then_some(value.as_u32())
}

fn terminal_scrollbar(terminal: GhosttyTerminal) -> GhosttyTerminalScrollbar {
    let mut value = GhosttyTerminalScrollbar::default();
    let _ = unsafe {
        ghostty_terminal_get(
            terminal,
            TERMINAL_DATA_SCROLLBAR,
            &mut value as *mut _ as *mut c_void,
        )
    };
    value
}

fn terminal_string(terminal: GhosttyTerminal, data: c_int) -> String {
    let mut value = GhosttyString::default();
    if unsafe { ghostty_terminal_get(terminal, data, &mut value as *mut _ as *mut c_void) }
        != GHOSTTY_SUCCESS
        || value.ptr.is_null()
        || value.len == 0
    {
        return String::new();
    }
    String::from_utf8_lossy(unsafe { slice::from_raw_parts(value.ptr, value.len) }).into_owned()
}

fn terminal_mode(terminal: GhosttyTerminal, mode: u16) -> bool {
    let mut config = GhosttyTerminalModeConfig { mode, value: false };
    unsafe {
        ghostty_terminal_get(
            terminal,
            TERMINAL_DATA_MODE,
            &mut config as *mut _ as *mut c_void,
        ) == GHOSTTY_SUCCESS
            && config.value
    }
}

fn grid_ref(
    terminal: GhosttyTerminal,
    tag: c_int,
    column: usize,
    row: usize,
) -> Option<GhosttyGridRef> {
    let point = GhosttyPoint {
        tag,
        value: GhosttyPointValue {
            coordinate: GhosttyPointCoordinate {
                x: column.try_into().ok()?,
                y: row.try_into().ok()?,
            },
        },
    };
    let mut grid_ref = GhosttyGridRef::default();
    (unsafe { ghostty_terminal_grid_ref(terminal, point, &mut grid_ref) } == GHOSTTY_SUCCESS)
        .then_some(grid_ref)
}

fn grid_grapheme(terminal: GhosttyTerminal, tag: c_int, column: usize, row: usize) -> String {
    let Some(grid_ref) = grid_ref(terminal, tag, column, row) else {
        return String::new();
    };
    let mut stack = [0u32; 8];
    let mut len = 0usize;
    let result =
        unsafe { ghostty_grid_ref_graphemes(&grid_ref, stack.as_mut_ptr(), stack.len(), &mut len) };
    let codepoints = if result == GHOSTTY_SUCCESS {
        stack[..len.min(stack.len())].to_vec()
    } else if result == GHOSTTY_OUT_OF_SPACE {
        let mut values = vec![0u32; len];
        if unsafe {
            ghostty_grid_ref_graphemes(&grid_ref, values.as_mut_ptr(), values.len(), &mut len)
        } != GHOSTTY_SUCCESS
        {
            return String::new();
        }
        values.truncate(len);
        values
    } else {
        return String::new();
    };
    codepoints.into_iter().filter_map(char::from_u32).collect()
}

fn grid_hyperlink(terminal: GhosttyTerminal, tag: c_int, column: usize, row: usize) -> Vec<u8> {
    let Some(grid_ref) = grid_ref(terminal, tag, column, row) else {
        return Vec::new();
    };
    let mut len = 0usize;
    let result = unsafe { ghostty_grid_ref_hyperlink_uri(&grid_ref, ptr::null_mut(), 0, &mut len) };
    if result != GHOSTTY_OUT_OF_SPACE || len == 0 {
        return Vec::new();
    }
    let mut bytes = vec![0u8; len];
    if unsafe {
        ghostty_grid_ref_hyperlink_uri(&grid_ref, bytes.as_mut_ptr(), bytes.len(), &mut len)
    } != GHOSTTY_SUCCESS
    {
        return Vec::new();
    }
    bytes.truncate(len);
    bytes
}

fn row_semantic_prompt(terminal: GhosttyTerminal, row: usize) -> bool {
    let Some(grid_ref) = grid_ref(terminal, POINT_TAG_SCREEN, 0, row) else {
        return false;
    };
    let mut raw_row = 0u64;
    if unsafe { ghostty_grid_ref_row(&grid_ref, &mut raw_row) } != GHOSTTY_SUCCESS {
        return false;
    }
    let mut semantic = 0i32;
    unsafe {
        ghostty_row_get(
            raw_row,
            ROW_DATA_SEMANTIC_PROMPT,
            &mut semantic as *mut _ as *mut c_void,
        ) == GHOSTTY_SUCCESS
            && semantic == ROW_SEMANTIC_PROMPT
    }
}

fn tracked_screen_row(grid_ref: GhosttyTrackedGridRef) -> Option<usize> {
    if grid_ref.is_null() {
        return None;
    }
    let mut point = GhosttyPointCoordinate::default();
    (unsafe { ghostty_tracked_grid_ref_point(grid_ref, POINT_TAG_SCREEN, &mut point) }
        == GHOSTTY_SUCCESS)
        .then_some(point.y as usize)
}

fn text_looks_like_prompt(text: &str) -> bool {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.char_indices().any(|(index, character)| {
        if !matches!(character, '$' | '%' | '#' | '>') {
            return false;
        }
        let suffix = &trimmed[index + character.len_utf8()..];
        if !suffix.is_empty() && !suffix.chars().next().is_some_and(char::is_whitespace) {
            return false;
        }
        let prefix = trimmed[..index].trim_end();
        prefix.is_empty() || structured_prompt_prefix(prefix)
    })
}

fn structured_prompt_prefix(prefix: &str) -> bool {
    if prefix.starts_with("PS ") || (prefix.starts_with('[') && prefix.ends_with(']')) {
        return true;
    }
    let bytes = prefix.as_bytes();
    let windows_path = bytes.len() >= 3 && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/');
    if !prefix.contains(char::is_whitespace)
        && (prefix.starts_with('/') || prefix.starts_with("~/") || windows_path)
    {
        return true;
    }
    let Some(at) = prefix.find('@') else {
        return false;
    };
    if at == 0 || prefix[..at].contains(char::is_whitespace) {
        return false;
    }
    let host_and_path = &prefix[at + 1..];
    let host_end = host_and_path
        .find([':', ' ', '\t'])
        .unwrap_or(host_and_path.len());
    if host_end == 0 || host_and_path[..host_end].contains(char::is_whitespace) {
        return false;
    }
    let suffix = host_and_path[host_end..].trim_start_matches([' ', '\t']);
    if suffix.is_empty() || suffix.starts_with('~') || suffix.starts_with('/') {
        return true;
    }
    let Some(path) = suffix.strip_prefix(':') else {
        return false;
    };
    let path_bytes = path.as_bytes();
    path.starts_with('~')
        || path.starts_with('/')
        || path_bytes.len() >= 3 && path_bytes[1] == b':' && matches!(path_bytes[2], b'\\' | b'/')
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn shell_integration_directory(uri: &[u8]) -> Option<String> {
    let uri = std::str::from_utf8(uri).ok()?.trim();
    if uri.is_empty() {
        return None;
    }
    url::Url::parse(uri)
        .ok()
        .filter(|url| url.scheme() == "file")
        .map(|url| {
            percent_encoding::percent_decode_str(url.path())
                .decode_utf8_lossy()
                .into_owned()
        })
}

fn install_png_decoder() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| unsafe {
        let _ = ghostty_sys_set(1, decode_png as *const () as *const c_void);
    });
}

unsafe extern "C" fn write_pty(
    _terminal: GhosttyTerminal,
    userdata: *mut c_void,
    data: *const u8,
    len: usize,
) {
    if userdata.is_null() || data.is_null() || len == 0 {
        return;
    }
    let state = unsafe { &*(userdata as *const Mutex<GhosttyCallbackState>) };
    if let Ok(mut state) = state.lock() {
        state
            .writes
            .extend_from_slice(unsafe { slice::from_raw_parts(data, len) });
    }
}

unsafe extern "C" fn bell(_terminal: GhosttyTerminal, userdata: *mut c_void) {
    if userdata.is_null() {
        return;
    }
    let state = unsafe { &*(userdata as *const Mutex<GhosttyCallbackState>) };
    if let Ok(mut state) = state.lock() {
        state.bell_count = state.bell_count.saturating_add(1);
    }
}

unsafe extern "C" fn clipboard_write(
    _terminal: GhosttyTerminal,
    userdata: *mut c_void,
    write: *const GhosttyClipboardWrite,
) -> c_int {
    if userdata.is_null() || write.is_null() {
        return 4;
    }
    let write = unsafe { &*write };
    let contents = if write.contents.is_null() {
        &[][..]
    } else {
        unsafe { slice::from_raw_parts(write.contents, write.contents_len) }
    };
    let text = contents
        .iter()
        .find_map(|content| {
            if content.data.ptr.is_null() {
                return None;
            }
            let mime = if content.mime.ptr.is_null() {
                &[][..]
            } else {
                unsafe { slice::from_raw_parts(content.mime.ptr, content.mime.len) }
            };
            let is_text = mime.is_empty()
                || std::str::from_utf8(mime)
                    .is_ok_and(|value| value.to_ascii_lowercase().starts_with("text/plain"));
            is_text.then(|| {
                String::from_utf8_lossy(unsafe {
                    slice::from_raw_parts(content.data.ptr, content.data.len)
                })
                .into_owned()
            })
        })
        .or_else(|| {
            contents.first().and_then(|content| {
                (!content.data.ptr.is_null()).then(|| {
                    String::from_utf8_lossy(unsafe {
                        slice::from_raw_parts(content.data.ptr, content.data.len)
                    })
                    .into_owned()
                })
            })
        });
    let state = unsafe { &*(userdata as *const Mutex<GhosttyCallbackState>) };
    if let Ok(mut state) = state.lock() {
        state.clipboard = text.unwrap_or_default();
        0
    } else {
        3
    }
}

unsafe extern "C" fn decode_png(
    _userdata: *mut c_void,
    allocator: *const c_void,
    data: *const u8,
    data_len: usize,
    output: *mut GhosttySysImage,
) -> bool {
    std::panic::catch_unwind(|| {
        if data.is_null() || output.is_null() {
            return false;
        }
        let encoded = unsafe { slice::from_raw_parts(data, data_len) };
        let mut decoder = png::Decoder::new(Cursor::new(encoded));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = match decoder.read_info() {
            Ok(reader) => reader,
            Err(_) => return false,
        };
        let Some(buffer_size) = reader.output_buffer_size() else {
            return false;
        };
        let mut decoded = vec![0u8; buffer_size];
        let info = match reader.next_frame(&mut decoded) {
            Ok(info) => info,
            Err(_) => return false,
        };
        decoded.truncate(info.buffer_size());
        let format = match info.color_type {
            png::ColorType::Rgb => 0,
            png::ColorType::Rgba => 1,
            png::ColorType::GrayscaleAlpha => 3,
            png::ColorType::Grayscale => 4,
            png::ColorType::Indexed => return false,
        };
        let Some(rgba) = pixels_to_rgba(&decoded, info.width, info.height, format) else {
            return false;
        };
        let destination = unsafe { ghostty_alloc(allocator, rgba.len()) };
        if destination.is_null() {
            return false;
        }
        unsafe {
            ptr::copy_nonoverlapping(rgba.as_ptr(), destination, rgba.len());
            (*output).width = info.width;
            (*output).height = info.height;
            (*output).data = destination;
            (*output).data_len = rgba.len();
        }
        true
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::{thread, time::Duration, time::Instant};

    use super::*;

    #[test]
    fn renders_styled_wide_and_combining_text() {
        let mut terminal = GhosttyTerminalEngine::new(
            12,
            3,
            TerminalOptions {
                emulator_backend: TerminalEmulatorBackend::Ghostty,
                ..TerminalOptions::default()
            },
        )
        .unwrap();
        terminal.write_bytes("A\x1b[1;31m红\x1b[0me\u{301}".as_bytes());

        let snapshot = terminal.snapshot();
        let rendered = snapshot
            .cells
            .iter()
            .map(|cell| {
                let start = cell.text_offset as usize;
                let end = start + cell.text_len as usize;
                std::str::from_utf8(&snapshot.text[start..end]).unwrap_or("")
            })
            .collect::<String>();
        assert!(rendered.contains("A红e\u{301}"));
        assert_eq!(snapshot.emulator_backend, TerminalEmulatorBackend::Ghostty);
    }

    #[test]
    fn uses_the_configured_ansi_palette_without_a_sidecar() {
        let options = TerminalOptions::default();
        let palette = options.default_colors.palette_u32();
        let mut ghostty = GhosttyTerminalEngine::new(8, 2, options).unwrap();
        let input = b"\x1b[34mA\x1b[94mB\x1b[38;5;208mC";
        ghostty.write_bytes(input);

        let ghostty_snapshot = ghostty.snapshot();
        let ghostty_colors = ghostty_snapshot.cells[..3]
            .iter()
            .map(|cell| cell.foreground)
            .collect::<Vec<_>>();
        assert_eq!(ghostty_colors, vec![palette[4], palette[12], palette[208]]);
    }

    #[test]
    fn tracks_scrollback_and_alternate_screen() {
        let mut terminal = GhosttyTerminalEngine::new(8, 2, TerminalOptions::default()).unwrap();
        terminal.write_bytes(b"one\r\ntwo\r\nthree");
        assert!(terminal.snapshot().history_lines > 0);
        terminal.write_bytes(b"\x1b[?1049halt");
        assert!(terminal.is_alt_screen());
        terminal.write_bytes(b"\x1b[?1049l");
        assert!(!terminal.is_alt_screen());
    }

    #[test]
    fn decodes_and_exports_kitty_png_graphics() {
        let mut terminal = GhosttyTerminalEngine::new(8, 2, TerminalOptions::default()).unwrap();
        terminal.resize(8, 2, 8, 16);
        terminal.write_bytes(
            b"\x1b_Ga=T,f=100,q=2;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAA\
              DUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==\x1b\\",
        );
        let snapshot = terminal.snapshot();
        assert_eq!(snapshot.graphic_images.len(), 1);
        assert_eq!(snapshot.graphic_images[0].width, 1);
        assert_eq!(snapshot.graphic_images[0].height, 1);
        assert_eq!(snapshot.graphic_images[0].rgba.len(), 4);
        assert_eq!(snapshot.graphic_placements.len(), 1);
    }

    #[test]
    fn exports_kitty_unicode_placeholder_without_rendering_a_tofu_glyph() {
        let mut terminal = GhosttyTerminalEngine::new(8, 2, TerminalOptions::default()).unwrap();
        terminal.resize(8, 2, 8, 16);
        terminal.write_bytes(
            concat!(
                "\x1b_Ga=t,f=100,i=1,q=2;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAA",
                "DUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==\x1b\\",
                "\x1b_Ga=p,i=1,p=1,U=1,c=1,r=1,q=2;\x1b\\",
                "\x1b[38;2;0;0;1;58;2;0;0;1m\u{10eeee}\u{0305}\u{0305}\x1b[0m",
            )
            .as_bytes(),
        );

        let snapshot = terminal.snapshot();
        assert_eq!(snapshot.graphic_images.len(), 1);
        assert_eq!(snapshot.graphic_placements.len(), 1);
        assert_eq!(snapshot.graphic_placements[0].viewport_column, 0);
        assert_eq!(snapshot.graphic_placements[0].viewport_row, 0);
        assert_eq!(snapshot.cells[0].text_len, 0);
        assert!(!String::from_utf8_lossy(&snapshot.text).contains(KITTY_UNICODE_PLACEHOLDER));
    }

    #[test]
    fn returns_ghostty_protocol_responses_to_the_transport() {
        let mut terminal = GhosttyTerminalEngine::new(8, 2, TerminalOptions::default()).unwrap();
        terminal.write_bytes(b"\x1b[5n");
        let responses = terminal.drain_transport_writes();
        assert!(responses
            .iter()
            .any(|response| response.contains("\x1b[0n")));
    }

    #[test]
    fn answers_the_kitty_graphics_capability_query_used_by_yazi() {
        let mut terminal = GhosttyTerminalEngine::new(8, 2, TerminalOptions::default()).unwrap();
        terminal.write_bytes(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\");
        let response = terminal.drain_transport_writes().concat();
        assert!(
            response.contains("\x1b_Gi=31;OK\x1b\\"),
            "unexpected Kitty Graphics response: {response:?}"
        );
    }

    #[test]
    fn owns_text_selection_and_search_without_an_alacritty_snapshot() {
        let mut terminal = GhosttyTerminalEngine::new(10, 3, TerminalOptions::default()).unwrap();
        terminal.write_bytes(b"alpha\r\nbeta\r\ngamma");

        assert_eq!(terminal.plain_text(), "alpha\nbeta\ngamma");
        assert_eq!(terminal.selection_text(0, 5), "alpha");

        let result = terminal.search("beta", TerminalSearchDirection::Forward, 0, 0);
        assert!(result.found);
        assert_eq!((result.start_column, result.end_column), (0, 4));
        assert_eq!(
            terminal.snapshot().emulator_backend,
            TerminalEmulatorBackend::Ghostty
        );
    }

    #[test]
    fn exports_ghostty_title_hyperlink_modes_clipboard_and_bell() {
        let mut terminal = GhosttyTerminalEngine::new(12, 3, TerminalOptions::default()).unwrap();
        terminal.write_bytes(
            b"\x1b]2;Ghostty title\x07\x1b]8;;https://example.com\x07link\x1b]8;;\x07\x07\x1b[?1h\x1b[?2004h\x1b]52;c;aGVsbG8=\x07",
        );

        let snapshot = terminal.snapshot();
        assert_eq!(snapshot.title, "Ghostty title");
        assert_eq!(snapshot.clipboard, "hello");
        assert_eq!(snapshot.bell_count, 1);
        assert_ne!(snapshot.keyboard_mode & 0x01, 0);
        assert_ne!(snapshot.keyboard_mode & 0x04, 0);
        assert!(String::from_utf8_lossy(&snapshot.hyperlink_text).contains("https://example.com"));
    }

    #[test]
    fn captures_and_suppresses_output_in_split_chunks() {
        let mut terminal = GhosttyTerminalEngine::new(12, 2, TerminalOptions::default()).unwrap();
        assert!(terminal.suppress_output_until(b"READY"));
        terminal.write_bytes(b"hidden RE");
        terminal.write_bytes(b"ADYvisible");

        assert_eq!(terminal.plain_text(), "visible");
        assert_eq!(terminal.drain_output_capture(), b"hidden READYvisible");
    }

    #[test]
    fn tracks_shell_integration_command_blocks_with_ghostty_coordinates() {
        let mut terminal = GhosttyTerminalEngine::new(20, 4, TerminalOptions::default()).unwrap();
        terminal.write_bytes(
            concat!(
                "\x1b]7;file://localhost/tmp/project\x07",
                "\x1b]133;A\x07$ ",
                "\x1b]4545;CommandStarted;ZWNobyBoaQ==\x07",
                "echo hi\r\nhi\r\n",
                "\x1b]4545;CommandExited;0\x07",
            )
            .as_bytes(),
        );

        let block = terminal
            .command_block_at(0)
            .expect("integrated command block");
        assert!(block.shell_integrated);
        assert_eq!(block.working_directory.as_deref(), Some("/tmp/project"));
        assert_eq!(block.command.as_deref(), Some("echo hi"));
        assert_eq!(block.exit_code, Some(0));
        assert!(block.completed);
    }

    #[test]
    fn keeps_prompt_heuristic_command_blocks_without_shell_integration() {
        let mut terminal = GhosttyTerminalEngine::new(20, 4, TerminalOptions::default()).unwrap();
        terminal.write_bytes(b"user@host:~$ echo hi\r\nhi\r\nuser@host:~$ ");

        let first = terminal.command_block_at(0).expect("prompt command block");
        assert!(!first.shell_integrated);
        assert!(first.completed);
        assert_eq!(first.start, 0);
        assert_eq!(first.end, 40);
    }

    #[test]
    fn integrated_command_block_anchor_survives_reflow() {
        let mut terminal = GhosttyTerminalEngine::new(20, 5, TerminalOptions::default()).unwrap();
        terminal.write_bytes(
            concat!(
                "a long preceding output line\r\n",
                "\x1b]133;A\x07$ ",
                "\x1b]4545;CommandStarted;cHdk\x07",
                "pwd\r\n/tmp\r\n",
                "\x1b]4545;CommandExited;0\x07",
            )
            .as_bytes(),
        );
        let id = terminal.command_blocks.back().unwrap().id;
        terminal.resize(8, 8, 8, 16);

        let row = tracked_screen_row(terminal.command_blocks.back().unwrap().anchor).unwrap();
        let scrollback =
            terminal_get_usize(terminal.terminal, TERMINAL_DATA_SCROLLBACK_ROWS).unwrap();
        let offset = (row as i64 - scrollback as i64) * terminal.size.columns as i64;
        let block = terminal.command_block_at(offset).expect("reflowed block");
        assert_eq!(block.id, id);
        assert_eq!(block.command.as_deref(), Some("pwd"));
    }

    #[test]
    fn scroll_and_resize_are_driven_only_by_ghostty_state() {
        let mut terminal = GhosttyTerminalEngine::new(8, 2, TerminalOptions::default()).unwrap();
        terminal.write_bytes(b"one\r\ntwo\r\nthree\r\nfour");
        let bottom = terminal.snapshot();
        assert!(bottom.history_lines >= 2);
        terminal.scroll_page_up();
        assert!(terminal.snapshot().display_offset > 0);
        terminal.scroll_to_bottom();
        assert_eq!(terminal.snapshot().display_offset, 0);
        terminal.resize(12, 4, 9, 18);
        let resized = terminal.snapshot();
        assert_eq!((resized.columns, resized.rows), (12, 4));
    }

    #[test]
    fn snapshot_failure_reuses_only_the_last_ghostty_frame() {
        let mut terminal = GhosttyTerminalEngine::new(8, 2, TerminalOptions::default()).unwrap();
        terminal.write_bytes(b"ghostty");
        let valid = terminal.snapshot();
        terminal.force_snapshot_failure = true;

        let fallback = terminal.snapshot();
        assert_eq!(fallback.emulator_backend, TerminalEmulatorBackend::Ghostty);
        assert_eq!(fallback.text, valid.text);
        assert_eq!(fallback.cells.len(), valid.cells.len());
    }

    #[cfg(unix)]
    #[test]
    fn forwards_kitty_graphics_capability_response_to_a_local_pty() {
        let script = r#"
import os
import select
import sys
import termios
import time
import tty

fd = sys.stdin.fileno()
original = termios.tcgetattr(fd)
try:
    tty.setraw(fd)
    sys.stdout.buffer.write(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[0c")
    sys.stdout.buffer.flush()
    response = b""
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        readable, _, _ = select.select([fd], [], [], max(0, deadline - time.monotonic()))
        if not readable:
            break
        response += os.read(fd, 128)
        if b"\x1b[?" in response and response.endswith(b"c"):
            break
    kitty = response.find(b"\x1b_Gi=31;OK\x1b\\")
    da1 = response.find(b"\x1b[?")
    marker = b"NAUTERM_KGP_OK" if kitty >= 0 and da1 > kitty else b"NAUTERM_KGP_FAILED:" + response.hex().encode()
    sys.stdout.buffer.write(b"\r\n" + marker + b"\r\n")
    sys.stdout.buffer.flush()
finally:
    termios.tcsetattr(fd, termios.TCSANOW, original)
"#;
        let mut terminal = GhosttyTerminalEngine::new(
            40,
            4,
            TerminalOptions {
                emulator_backend: TerminalEmulatorBackend::Ghostty,
                command: Some(crate::terminal::TerminalCommand {
                    program: "python3".to_owned(),
                    args: vec!["-c".to_owned(), script.to_owned()],
                }),
                ..TerminalOptions::default()
            },
        )
        .unwrap();
        assert!(terminal.start_local_pty());

        let deadline = Instant::now() + Duration::from_secs(4);
        let mut output = String::new();
        while Instant::now() < deadline {
            terminal.pump_local_pty();
            output = terminal.plain_text();
            if output.contains("NAUTERM_KGP_OK") || output.contains("NAUTERM_KGP_FAILED") {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert!(
            output.contains("NAUTERM_KGP_OK"),
            "local PTY did not receive the Kitty Graphics response: {output:?}"
        );
    }
}
