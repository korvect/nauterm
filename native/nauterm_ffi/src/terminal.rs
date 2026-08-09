use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::search::RegexSearch;
use alacritty_terminal::term::{
    point_to_viewport, viewport_to_point, Config, Osc52, Term, TermMode,
};
use alacritty_terminal::vte::ansi::{
    Color, CursorShape, CursorStyle, Handler, NamedColor, Processor, Rgb,
};
use base64::Engine as _;
use percent_encoding::percent_decode_str;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use url::Url;

use crate::ffi::FfiTerminalCell;
use crate::pty::{LocalPty, WakeupCallback};
use crate::theme::{ANSI_PALETTE, DEFAULT_BACKGROUND, DEFAULT_CURSOR, DEFAULT_FOREGROUND};

const MIN_COLUMNS: usize = 2;
const MIN_ROWS: usize = 1;
const DEFAULT_COLUMNS: usize = 80;
const DEFAULT_ROWS: usize = 24;
const DEFAULT_SCROLLBACK_LINES: usize = 10_000;
const MAX_POLL_OUTPUT_CHUNKS: usize = 32;
const MAX_OSC_BYTES: usize = 64 * 1024;
const MAX_COMMAND_BLOCK_METADATA: usize = 20_000;
const COMMAND_BLOCK_MARKER_FLAG: Flags = Flags::from_bits_retain(0x8000);
const COMMAND_BLOCK_MARKER_PREFIX: char = '\u{f0000}';
const COMMAND_BLOCK_MARKER_SAME_LINE: char = '\u{f0010}';
const COMMAND_BLOCK_MARKER_NEXT_LINE: char = '\u{f0011}';
const COMMAND_BLOCK_MARKER_DIGIT_BASE: u32 = 0xf0100;
const COMMAND_BLOCK_MARKER_DIGITS: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct TerminalGeometry {
    pub columns: usize,
    pub rows: usize,
}

#[derive(Clone, Debug)]
pub struct TerminalCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct TerminalEnvironmentVariable {
    pub variable: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct TerminalOptions {
    pub emulator_backend: TerminalEmulatorBackend,
    pub scrollback_lines: usize,
    pub terminal_type: TerminalType,
    pub color_term: ColorTerm,
    pub osc52: Osc52Mode,
    pub cursor_shape: CursorShape,
    pub cursor_blinking: bool,
    pub shell_path: Option<String>,
    pub working_directory: Option<PathBuf>,
    pub command: Option<TerminalCommand>,
    pub environment: Vec<TerminalEnvironmentVariable>,
    pub default_colors: TerminalDefaultColors,
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            emulator_backend: TerminalEmulatorBackend::Alacritty,
            scrollback_lines: DEFAULT_SCROLLBACK_LINES,
            terminal_type: TerminalType::Xterm256Color,
            color_term: ColorTerm::Truecolor,
            osc52: Osc52Mode::OnlyCopy,
            cursor_shape: CursorShape::Block,
            cursor_blinking: false,
            shell_path: None,
            working_directory: None,
            command: None,
            environment: Vec::new(),
            default_colors: TerminalDefaultColors::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerminalEmulatorBackend {
    #[default]
    Alacritty,
    Ghostty,
}

impl TerminalEmulatorBackend {
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::Ghostty,
            _ => Self::Alacritty,
        }
    }

    pub fn as_u32(self) -> u32 {
        match self {
            Self::Alacritty => 0,
            Self::Ghostty => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum Osc52Mode {
    #[default]
    OnlyCopy,
    CopyPaste,
}

impl Osc52Mode {
    fn to_alacritty(self) -> Osc52 {
        match self {
            Self::OnlyCopy => Osc52::OnlyCopy,
            Self::CopyPaste => Osc52::CopyPaste,
        }
    }

    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::CopyPaste,
            _ => Self::OnlyCopy,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum TerminalType {
    #[default]
    Xterm256Color,
    Xterm16Color,
    Xterm,
}

impl TerminalType {
    pub fn from_term(value: &str) -> Option<Self> {
        match value {
            "xterm-256color" => Some(Self::Xterm256Color),
            "xterm-16color" => Some(Self::Xterm16Color),
            "xterm" => Some(Self::Xterm),
            _ => None,
        }
    }

    pub fn term(self) -> &'static str {
        match self {
            Self::Xterm256Color => "xterm-256color",
            Self::Xterm16Color => "xterm-16color",
            Self::Xterm => "xterm",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ColorTerm {
    None,
    Truecolor,
}

impl ColorTerm {
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::Truecolor,
            _ => Self::None,
        }
    }

    pub fn env_value(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Truecolor => Some("truecolor"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalDefaultColors {
    foreground: Rgb,
    background: Rgb,
    cursor: Rgb,
}

impl Default for TerminalDefaultColors {
    fn default() -> Self {
        Self {
            foreground: DEFAULT_FOREGROUND,
            background: DEFAULT_BACKGROUND,
            cursor: DEFAULT_CURSOR,
        }
    }
}

impl TerminalDefaultColors {
    pub fn from_rgb_values(foreground: u32, background: u32, cursor: u32) -> Self {
        Self {
            foreground: u32_to_rgb(foreground, DEFAULT_FOREGROUND),
            background: u32_to_rgb(background, DEFAULT_BACKGROUND),
            cursor: u32_to_rgb(cursor, DEFAULT_CURSOR),
        }
    }

    fn color_for_index(self, index: usize) -> Rgb {
        match index {
            0..=15 => ANSI_PALETTE[index],
            16..=255 => indexed_color(index as u8),
            256 => self.foreground,
            257 => self.background,
            258 => self.cursor,
            _ => self.foreground,
        }
    }

    pub(crate) fn foreground_u32(self) -> u32 {
        rgb_to_u32(self.foreground)
    }

    pub(crate) fn background_u32(self) -> u32 {
        rgb_to_u32(self.background)
    }

    pub(crate) fn cursor_u32(self) -> u32 {
        rgb_to_u32(self.cursor)
    }

    pub(crate) fn palette_u32(self) -> [u32; 256] {
        std::array::from_fn(|index| rgb_to_u32(self.color_for_index(index)))
    }
}

impl TerminalOptions {
    pub(crate) fn cursor_shape_u32(&self) -> u32 {
        cursor_shape_to_u32(self.cursor_shape)
    }
}

impl TerminalGeometry {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            columns: columns.max(MIN_COLUMNS),
            rows: rows.max(MIN_ROWS),
        }
    }
}

impl Dimensions for TerminalGeometry {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

#[derive(Clone, Debug)]
struct TerminalEvents {
    pty_writes: Arc<Mutex<Vec<String>>>,
    title: Arc<Mutex<Option<String>>>,
    clipboard: Arc<Mutex<Option<String>>>,
    bell_count: Arc<Mutex<u64>>,
    default_colors: TerminalDefaultColors,
}

impl TerminalEvents {
    fn new(default_colors: TerminalDefaultColors) -> Self {
        Self {
            pty_writes: Arc::new(Mutex::new(Vec::new())),
            title: Arc::new(Mutex::new(None)),
            clipboard: Arc::new(Mutex::new(None)),
            bell_count: Arc::new(Mutex::new(0)),
            default_colors,
        }
    }

    fn drain_pty_writes(&self) -> Vec<String> {
        self.pty_writes
            .lock()
            .map(|mut writes| writes.drain(..).collect())
            .unwrap_or_default()
    }

    fn title(&self) -> Option<String> {
        self.title.lock().ok().and_then(|title| title.clone())
    }

    fn clipboard(&self) -> Option<String> {
        self.clipboard
            .lock()
            .ok()
            .and_then(|clipboard| clipboard.clone())
    }

    fn bell_count(&self) -> u64 {
        self.bell_count.lock().map(|count| *count).unwrap_or(0)
    }
}

impl EventListener for TerminalEvents {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                if let Ok(mut writes) = self.pty_writes.lock() {
                    writes.push(text);
                }
            }
            Event::Title(title) => {
                if let Ok(mut current_title) = self.title.lock() {
                    *current_title = Some(title);
                }
            }
            Event::ResetTitle => {
                if let Ok(mut current_title) = self.title.lock() {
                    *current_title = None;
                }
            }
            Event::ColorRequest(index, formatter) => {
                let color = self.default_colors.color_for_index(index);
                if let Ok(mut writes) = self.pty_writes.lock() {
                    writes.push(formatter(color));
                }
            }
            Event::ClipboardStore(_, text) => {
                if let Ok(mut clipboard) = self.clipboard.lock() {
                    *clipboard = Some(text);
                }
            }
            Event::ClipboardLoad(_, formatter) => {
                let text = self
                    .clipboard
                    .lock()
                    .ok()
                    .and_then(|clipboard| clipboard.clone())
                    .unwrap_or_default();
                if let Ok(mut writes) = self.pty_writes.lock() {
                    writes.push(formatter(&text));
                }
            }
            Event::TextAreaSizeRequest(_) => {}
            Event::Wakeup => {}
            Event::Bell => {
                if let Ok(mut count) = self.bell_count.lock() {
                    *count = count.saturating_add(1);
                }
            }
            Event::MouseCursorDirty | Event::CursorBlinkingChange | Event::Exit => {}
            Event::ChildExit(_) => {}
        }
    }
}

pub struct TerminalEngine {
    term: Term<TerminalEvents>,
    processor: Processor,
    events: TerminalEvents,
    output_capture: Vec<u8>,
    output_suppression: Option<OutputSuppression>,
    osc_pending: Vec<u8>,
    command_block_id: u64,
    command_block_directories: HashMap<u64, String>,
    command_block_commands: HashMap<u64, String>,
    command_block_exit_codes: HashMap<u64, i32>,
    command_block_order: VecDeque<u64>,
    command_block_marker_cache: RefCell<Option<Vec<(i64, u64)>>>,
    current_working_directory: Option<String>,
    size: TerminalGeometry,
    options: TerminalOptions,
    alt_screen_cursor_style: Option<CursorStyle>,
    pty: Option<LocalPty>,
    input_echo_override: Option<bool>,
    exited: bool,
    wakeup_callback: Option<WakeupCallback>,
}

struct OutputSuppression {
    marker: Vec<u8>,
    pending: Vec<u8>,
}

#[derive(Clone)]
pub struct TerminalSnapshot {
    pub emulator_backend: TerminalEmulatorBackend,
    pub columns: usize,
    pub rows: usize,
    pub history_lines: usize,
    pub display_offset: usize,
    pub title: String,
    pub clipboard: String,
    pub bell_count: u64,
    pub cursor_column: usize,
    pub cursor_row: usize,
    pub cursor_visible: bool,
    pub cursor_shape: u32,
    pub cursor_color: u32,
    pub cursor_blinking: bool,
    pub keyboard_mode: u32,
    pub input_echo_enabled: bool,
    pub alternate_screen: bool,
    pub cells: Vec<FfiTerminalCell>,
    pub text: Vec<u8>,
    pub hyperlink_text: Vec<u8>,
    pub graphic_images: Vec<TerminalGraphicImage>,
    pub graphic_placements: Vec<TerminalGraphicPlacement>,
}

#[derive(Clone, Debug)]
pub struct TerminalGraphicImage {
    pub id: u32,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    /// Decoded RGBA8888 pixels.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalGraphicPlacement {
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

#[derive(Clone, Copy, Debug)]
pub enum TerminalSearchDirection {
    Forward,
    Backward,
}

impl TerminalSearchDirection {
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::Backward,
            _ => Self::Forward,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct TerminalSearchResult {
    pub found: bool,
    pub columns: usize,
    pub rows: usize,
    pub start_row: usize,
    pub start_column: usize,
    pub end_row: usize,
    pub end_column: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct TerminalCommandBlock {
    pub id: u64,
    pub start: i64,
    pub end: i64,
    pub working_directory: Option<String>,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub completed: bool,
    pub shell_integrated: bool,
}

impl TerminalSearchResult {
    pub fn not_found(columns: usize, rows: usize) -> Self {
        Self {
            found: false,
            columns,
            rows,
            start_row: 0,
            start_column: 0,
            end_row: 0,
            end_column: 0,
            error: None,
        }
    }

    fn error(columns: usize, rows: usize, error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::not_found(columns, rows)
        }
    }
}

/// Backend-neutral terminal emulation contract.
///
/// Implementations deliberately do not require `Send` or `Sync`: a terminal
/// and every borrowed handle derived from it are owned by one session actor
/// thread for their full lifetime.
pub trait TerminalEmulator {
    fn resize(&mut self, columns: usize, rows: usize, cell_width_px: u32, cell_height_px: u32);
    fn is_alt_screen(&self) -> bool;
    fn scroll_lines(&mut self, lines: i32);
    fn scroll_page_up(&mut self);
    fn scroll_page_down(&mut self);
    fn scroll_to_bottom(&mut self);
    fn search(
        &mut self,
        query: &str,
        direction: TerminalSearchDirection,
        origin_row: usize,
        origin_column: usize,
    ) -> TerminalSearchResult;
    fn start_local_pty(&mut self) -> bool;
    fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>);
    fn send_input_bytes(&mut self, bytes: &[u8]) -> bool;
    fn pump_local_pty(&mut self) -> bool;
    fn is_exited(&self) -> bool;
    fn set_input_echo_enabled(&mut self, enabled: bool);
    fn mark_exited(&mut self);
    fn mark_running(&mut self);
    fn drain_transport_writes(&mut self) -> Vec<String>;
    fn drain_output_capture(&mut self) -> Vec<u8>;
    fn suppress_output_until(&mut self, marker: &[u8]) -> bool;
    fn cancel_output_suppression(&mut self);
    fn clear_pending_output_for_close(&mut self);
    fn write_bytes(&mut self, bytes: &[u8]);
    fn write_bytes_without_capture(&mut self, bytes: &[u8]);
    fn snapshot(&self) -> TerminalSnapshot;
    fn plain_text(&self) -> String;
    fn selection_text(&self, start: i64, end: i64) -> String;
    fn command_block_at(&self, offset: i64) -> Option<TerminalCommandBlock>;
    fn clipboard(&self) -> String;
    fn bell_count(&self) -> u64;
}

#[derive(Clone, Debug)]
struct ExportCell {
    text: String,
    hyperlink: Option<String>,
    flags: u16,
    foreground: u32,
    background: u32,
}

#[derive(Clone, Debug)]
struct NormalizedCells {
    cells: Vec<ExportCell>,
    column_map: Vec<usize>,
}

impl TerminalEngine {
    #[allow(dead_code)]
    pub fn new(columns: usize, rows: usize) -> Self {
        Self::new_with_options(columns, rows, TerminalOptions::default())
    }

    pub fn new_with_options(columns: usize, rows: usize, options: TerminalOptions) -> Self {
        let size = TerminalGeometry::new(
            normalize_dimension(columns, DEFAULT_COLUMNS),
            normalize_dimension(rows, DEFAULT_ROWS),
        );
        let config = Config {
            scrolling_history: options.scrollback_lines,
            osc52: options.osc52.to_alacritty(),
            default_cursor_style: CursorStyle {
                shape: options.cursor_shape,
                blinking: options.cursor_blinking,
            },
            ..Config::default()
        };

        let events = TerminalEvents::new(options.default_colors);
        let term = Term::new(config, &size, events.clone());

        Self {
            term,
            processor: Processor::new(),
            events,
            output_capture: Vec::new(),
            output_suppression: None,
            osc_pending: Vec::new(),
            command_block_id: 0,
            command_block_directories: HashMap::new(),
            command_block_commands: HashMap::new(),
            command_block_exit_codes: HashMap::new(),
            command_block_order: VecDeque::new(),
            command_block_marker_cache: RefCell::new(None),
            current_working_directory: None,
            size,
            options,
            alt_screen_cursor_style: None,
            pty: None,
            input_echo_override: None,
            exited: false,
            wakeup_callback: None,
        }
    }

    pub fn resize(&mut self, columns: usize, rows: usize) {
        let next_size = TerminalGeometry::new(
            normalize_dimension(columns, self.size.columns),
            normalize_dimension(rows, self.size.rows),
        );
        if self.size.columns == next_size.columns && self.size.rows == next_size.rows {
            return;
        }

        self.size = next_size;
        self.term.resize(next_size);
        *self.command_block_marker_cache.borrow_mut() = None;
        if let Some(pty) = &mut self.pty {
            pty.resize(next_size);
        }
    }

    pub fn is_alt_screen(&self) -> bool {
        self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    pub fn scroll_lines(&mut self, lines: i32) {
        if lines == 0 {
            return;
        }

        self.term.scroll_display(Scroll::Delta(lines));
    }

    pub fn scroll_page_up(&mut self) {
        self.term.scroll_display(Scroll::PageUp);
    }

    pub fn scroll_page_down(&mut self) {
        self.term.scroll_display(Scroll::PageDown);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    pub fn search(
        &mut self,
        query: &str,
        direction: TerminalSearchDirection,
        origin_row: usize,
        origin_column: usize,
    ) -> TerminalSearchResult {
        if query.is_empty() {
            return TerminalSearchResult::not_found(self.size.columns, self.size.rows);
        }

        let pattern = literal_search_regex(query);
        let mut regex = match RegexSearch::new(&pattern) {
            Ok(regex) => regex,
            Err(error) => {
                return TerminalSearchResult::error(
                    self.size.columns,
                    self.size.rows,
                    error.to_string(),
                );
            }
        };

        let row = origin_row.min(self.size.rows.saturating_sub(1));
        let column = origin_column.min(self.size.columns.saturating_sub(1));
        let origin = viewport_to_point(
            self.term.grid().display_offset(),
            Point::new(row, Column(column)),
        );
        let search_direction = match direction {
            TerminalSearchDirection::Forward => Direction::Right,
            TerminalSearchDirection::Backward => Direction::Left,
        };
        let side = match direction {
            TerminalSearchDirection::Forward => Side::Right,
            TerminalSearchDirection::Backward => Side::Left,
        };

        let Some(regex_match) =
            self.term
                .search_next(&mut regex, origin, search_direction, side, None)
        else {
            return TerminalSearchResult::not_found(self.size.columns, self.size.rows);
        };

        let mut start = *regex_match.start();
        let mut end = *regex_match.end();
        if start > end {
            std::mem::swap(&mut start, &mut end);
        }

        self.term.scroll_to_point(start);

        let display_offset = self.term.grid().display_offset();
        let Some(start_viewport) = point_to_viewport(display_offset, start) else {
            return TerminalSearchResult::not_found(self.size.columns, self.size.rows);
        };
        let Some(end_viewport) = point_to_viewport(display_offset, end) else {
            return TerminalSearchResult::not_found(self.size.columns, self.size.rows);
        };
        if start_viewport.line >= self.size.rows || end_viewport.line >= self.size.rows {
            return TerminalSearchResult::not_found(self.size.columns, self.size.rows);
        }

        TerminalSearchResult {
            found: true,
            columns: self.size.columns,
            rows: self.size.rows,
            start_row: start_viewport.line,
            start_column: start_viewport
                .column
                .0
                .min(self.size.columns.saturating_sub(1)),
            end_row: end_viewport.line,
            end_column: (end_viewport.column.0 + 1).min(self.size.columns),
            error: None,
        }
    }

    pub fn start_local_pty(&mut self) -> bool {
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

    pub fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>) {
        self.wakeup_callback = callback;
        if let Some(pty) = &mut self.pty {
            pty.set_wakeup_callback(callback);
        }
    }

    #[allow(dead_code)]
    pub fn send_input_char(&mut self, character: char) -> bool {
        let Some(pty) = &mut self.pty else {
            return false;
        };

        let mut buffer = [0; 4];
        pty.queue_input(character.encode_utf8(&mut buffer).as_bytes());
        true
    }

    pub fn send_input_bytes(&mut self, bytes: &[u8]) -> bool {
        let Some(pty) = &mut self.pty else {
            return false;
        };

        pty.queue_input(bytes);
        true
    }

    pub fn pump_local_pty(&mut self) -> bool {
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

        self.forward_terminal_pty_writes();
        if exited {
            self.pty = None;
            self.exited = true;
            changed = true;
        }

        changed
    }

    pub fn is_exited(&self) -> bool {
        self.exited
    }

    pub fn input_echo_enabled(&self) -> bool {
        self.input_echo_override.unwrap_or_else(|| {
            self.pty
                .as_ref()
                .map(|pty| pty.input_visible())
                .unwrap_or(true)
        })
    }

    pub fn set_input_echo_enabled(&mut self, enabled: bool) {
        self.input_echo_override = Some(enabled);
    }

    pub fn mark_exited(&mut self) {
        self.exited = true;
    }

    pub fn mark_running(&mut self) {
        self.exited = false;
    }

    pub fn drain_transport_writes(&mut self) -> Vec<String> {
        self.events.drain_pty_writes()
    }

    pub fn drain_output_capture(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output_capture)
    }

    pub fn suppress_output_until(&mut self, marker: &[u8]) -> bool {
        if marker.is_empty() {
            return false;
        }
        self.output_suppression = Some(OutputSuppression {
            marker: marker.to_vec(),
            pending: Vec::new(),
        });
        true
    }

    pub fn cancel_output_suppression(&mut self) {
        self.output_suppression = None;
    }

    pub fn clear_pending_output_for_close(&mut self) {
        self.output_capture.clear();
        self.output_capture.shrink_to_fit();
        if let Some(pty) = &mut self.pty {
            pty.clear_pending_output();
        }
    }

    #[allow(dead_code)]
    pub fn write_char(&mut self, character: char) {
        let mut buffer = [0; 4];
        self.write_bytes(character.encode_utf8(&mut buffer).as_bytes());
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        let _ = self.write_bytes_and_return_visible(bytes);
    }

    pub(crate) fn write_bytes_and_return_visible(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.output_capture.extend_from_slice(bytes);
        let visible = self.filter_suppressed_output(bytes);
        if !visible.is_empty() {
            self.write_bytes_without_capture(&visible);
        }
        visible
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

    pub fn write_bytes_without_capture(&mut self, bytes: &[u8]) {
        self.osc_pending.extend_from_slice(bytes);
        loop {
            let Some(osc_start) = find_bytes(&self.osc_pending, b"\x1b]") else {
                let retained = usize::from(self.osc_pending.ends_with(b"\x1b"));
                let visible_len = self.osc_pending.len().saturating_sub(retained);
                if visible_len > 0 {
                    let remaining = self.osc_pending.split_off(visible_len);
                    let visible = std::mem::replace(&mut self.osc_pending, remaining);
                    self.advance_terminal_bytes(&visible);
                }
                return;
            };
            if osc_start > 0 {
                let remaining = self.osc_pending.split_off(osc_start);
                let visible = std::mem::replace(&mut self.osc_pending, remaining);
                self.advance_terminal_bytes(&visible);
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
                    Some((left.min(right), usize::from(right <= left) + 1))
                }
                (Some(index), None) => Some((index, 1)),
                (None, Some(index)) => Some((index, 2)),
                (None, None) => None,
            };
            let Some((terminator, terminator_len)) = terminator else {
                if self.osc_pending.len() > MAX_OSC_BYTES {
                    let pending = std::mem::take(&mut self.osc_pending);
                    self.advance_terminal_bytes(&pending);
                }
                return;
            };
            let sequence_len = terminator + terminator_len;
            let remaining = self.osc_pending.split_off(sequence_len);
            let sequence = std::mem::replace(&mut self.osc_pending, remaining);
            self.advance_terminal_bytes(&sequence);
            self.handle_shell_integration_osc(&sequence[2..terminator]);
        }
    }

    fn advance_terminal_bytes(&mut self, bytes: &[u8]) {
        *self.command_block_marker_cache.borrow_mut() = None;
        let was_alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);
        let cursor_style_before = self.term.cursor_style();

        self.processor.advance(&mut self.term, bytes);
        self.restore_cursor_style_after_alt_screen(was_alt_screen, cursor_style_before);
        self.forward_terminal_pty_writes();
    }

    fn handle_shell_integration_osc(&mut self, payload: &[u8]) {
        if let Some(uri) = payload.strip_prefix(b"7;") {
            self.current_working_directory = shell_integration_directory(uri);
            return;
        }
        if let Some(encoded) = payload.strip_prefix(b"4545;CommandStarted;") {
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) {
                if let Ok(command) = String::from_utf8(decoded) {
                    let command = command.trim().to_owned();
                    if !command.is_empty() && self.command_block_id > 0 {
                        self.command_block_commands
                            .insert(self.command_block_id, command);
                    }
                }
            }
            return;
        }
        if let Some(status) = payload.strip_prefix(b"4545;CommandExited;") {
            if let Ok(status) = std::str::from_utf8(status) {
                if let Ok(exit_code) = status.trim().parse::<i32>() {
                    if self.command_block_id > 0 {
                        self.command_block_exit_codes
                            .insert(self.command_block_id, exit_code);
                    }
                }
            }
            return;
        }
        if payload != b"133;A" || self.is_alt_screen() {
            return;
        }
        self.mark_command_block_start();
    }

    fn mark_command_block_start(&mut self) {
        let grid = self.term.grid();
        let cursor = grid.cursor.point;
        let first_line = -(grid.history_size() as i32);
        let (marker_line, marker_column, starts_on_next_line) = if cursor.column.0 > 0 {
            (cursor.line, cursor.column.0 - 1, false)
        } else if cursor.line.0 > first_line {
            let marker_line = cursor.line - 1i32;
            let marker_column = (0..grid.columns())
                .rev()
                .find(|column| {
                    let cell = &grid[marker_line][Column(*column)];
                    cell.c != ' ' || !cell.flags.is_empty()
                })
                .unwrap_or(0);
            (marker_line, marker_column, true)
        } else {
            (cursor.line, grid.columns().saturating_sub(1), false)
        };

        self.command_block_id = self.command_block_id.saturating_add(1);
        let id = self.command_block_id;
        let marker = command_block_marker(id, starts_on_next_line);
        let cell = &mut self.term.grid_mut()[marker_line][Column(marker_column)];
        cell.flags.insert(COMMAND_BLOCK_MARKER_FLAG);
        for character in marker {
            cell.push_zerowidth(character);
        }

        if let Some(directory) = self.current_working_directory.clone() {
            self.command_block_directories.insert(id, directory);
        }
        self.command_block_order.push_back(id);
        while self.command_block_order.len() > MAX_COMMAND_BLOCK_METADATA {
            if let Some(expired) = self.command_block_order.pop_front() {
                self.command_block_directories.remove(&expired);
                self.command_block_commands.remove(&expired);
                self.command_block_exit_codes.remove(&expired);
            }
        }
    }

    fn restore_cursor_style_after_alt_screen(
        &mut self,
        was_alt_screen: bool,
        cursor_style_before: CursorStyle,
    ) {
        let is_alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);
        if !was_alt_screen && is_alt_screen {
            self.alt_screen_cursor_style = Some(cursor_style_before);
            return;
        }

        if was_alt_screen && !is_alt_screen {
            if let Some(cursor_style) = self.alt_screen_cursor_style.take() {
                self.term.set_cursor_style(Some(cursor_style));
            }
        }
    }

    pub fn snapshot(&self) -> TerminalSnapshot {
        let renderable = self.term.renderable_content();
        let columns = self.size.columns;
        let rows = self.size.rows;
        let history_lines = self
            .term
            .grid()
            .total_lines()
            .saturating_sub(self.term.grid().screen_lines());
        let display_offset = renderable.display_offset;
        let mut raw_cells = vec![ExportCell::empty(); columns * rows];

        for indexed in renderable.display_iter {
            let Some(point) = point_to_viewport(renderable.display_offset, indexed.point) else {
                continue;
            };
            if point.line >= rows || point.column.0 >= columns {
                continue;
            }

            let index = point.line * columns + point.column.0;
            raw_cells[index] = export_cell(indexed.cell, renderable.colors);
        }
        let normalized = normalize_unicode_cells(raw_cells, columns, rows);

        let cursor_point = point_to_viewport(renderable.display_offset, renderable.cursor.point);
        let (cursor_column, cursor_row) = cursor_point
            .filter(|point| point.line < rows && point.column.0 < columns)
            .map(|point| (point.column.0, point.line))
            .unwrap_or((0, 0));
        let cursor_column = normalized.visual_column_for(cursor_column, cursor_row, columns);
        let (cells, text, hyperlink_text) = pack_cells(normalized.cells);
        let cursor_visible = !matches!(renderable.cursor.shape, CursorShape::Hidden);
        let cursor_shape = cursor_shape_to_u32(renderable.cursor.shape);
        let cursor_blinking = self.term.cursor_style().blinking;
        let cursor_color = rgb_to_u32(resolve_color(
            Color::Named(NamedColor::Cursor),
            renderable.colors,
            DEFAULT_CURSOR,
        ));
        let keyboard_mode = keyboard_mode_to_u32(*self.term.mode());
        let input_echo_enabled = self.input_echo_enabled();
        let title = self.events.title().unwrap_or_default();
        let clipboard = self.events.clipboard().unwrap_or_default();
        let bell_count = self.events.bell_count();

        TerminalSnapshot {
            emulator_backend: TerminalEmulatorBackend::Alacritty,
            columns,
            rows,
            history_lines,
            display_offset,
            title,
            clipboard,
            bell_count,
            cursor_column,
            cursor_row,
            cursor_visible,
            cursor_shape,
            cursor_color,
            cursor_blinking,
            keyboard_mode,
            input_echo_enabled,
            alternate_screen: self.is_alt_screen(),
            cells,
            text,
            hyperlink_text,
            graphic_images: Vec::new(),
            graphic_placements: Vec::new(),
        }
    }

    pub fn plain_text(&self) -> String {
        let grid = self.term.grid();
        let history_lines = grid.history_size() as i32;
        let screen_lines = grid.screen_lines() as i32;
        let columns = grid.columns();
        let mut logical_lines = Vec::with_capacity(grid.total_lines());
        let mut current_line = String::new();

        for line_index in -history_lines..screen_lines {
            let line = Line(line_index);
            for column in 0..columns {
                let cell = &grid[line][Column(column)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                if cell.flags.contains(Flags::HIDDEN) {
                    current_line.push(' ');
                } else {
                    current_line.push_str(&cell_text(cell.c, cell));
                }
            }

            while current_line.ends_with(' ') {
                current_line.pop();
            }
            let wrapped = columns > 0
                && grid[line][Column(columns - 1)]
                    .flags
                    .contains(Flags::WRAPLINE);
            if !wrapped {
                logical_lines.push(std::mem::take(&mut current_line));
            }
        }

        if !current_line.is_empty() {
            logical_lines.push(current_line);
        }
        while logical_lines.last().is_some_and(String::is_empty) {
            logical_lines.pop();
        }
        if logical_lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", logical_lines.join("\n"))
        }
    }

    pub fn selection_text(&self, start: i64, end: i64) -> String {
        let grid = self.term.grid();
        let columns = grid.columns() as i64;
        if columns == 0 || start >= end {
            return String::new();
        }

        let history_lines = grid.history_size() as i64;
        let screen_lines = grid.screen_lines() as i64;
        let minimum = -history_lines * columns;
        let maximum = screen_lines * columns;
        let start = start.clamp(minimum, maximum);
        let end = end.clamp(minimum, maximum);
        if start >= end {
            return String::new();
        }
        if start == minimum && end == maximum {
            return self.plain_text().trim_end_matches('\n').to_owned();
        }

        let start_line = start.div_euclid(columns);
        let end_line = (end - 1).div_euclid(columns);
        let mut lines = Vec::with_capacity((end_line - start_line + 1) as usize);
        for line_index in start_line..=end_line {
            let start_column = if line_index == start_line {
                start.rem_euclid(columns) as usize
            } else {
                0
            };
            let end_column = if line_index == end_line {
                ((end - 1).rem_euclid(columns) + 1) as usize
            } else {
                columns as usize
            };
            let line = Line(line_index as i32);
            let mut text = String::new();
            for column in start_column..end_column {
                let cell = &grid[line][Column(column)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                if cell.flags.contains(Flags::HIDDEN) {
                    text.push(' ');
                } else {
                    text.push_str(&cell_text(cell.c, cell));
                }
            }
            while text.ends_with(' ') {
                text.pop();
            }
            lines.push(text);
        }

        lines.join("\n")
    }

    pub fn command_block_at(&self, offset: i64) -> Option<TerminalCommandBlock> {
        if self.is_alt_screen() {
            return None;
        }
        if let Some(block) = self.integrated_command_block_at(offset) {
            return Some(block);
        }
        let grid = self.term.grid();
        let columns = grid.columns() as i64;
        if columns == 0 {
            return None;
        }
        let first_line = -(grid.history_size() as i64);
        let last_line = grid.screen_lines() as i64 - 1;
        let target_line = offset.div_euclid(columns);
        if target_line < first_line || target_line > last_line {
            return None;
        }

        let prompt_lines = (first_line..=last_line)
            .filter_map(|line| terminal_line_prompt_kind(grid, line).map(|kind| (line, kind)))
            .collect::<Vec<_>>();
        let prefer_structured = prompt_lines.iter().any(|(_, structured)| *structured);
        let is_eligible = |structured: bool| !prefer_structured || structured;
        let start_line = prompt_lines
            .iter()
            .rev()
            .find(|(line, structured)| *line <= target_line && is_eligible(*structured))
            .map(|(line, _)| *line)?;
        let end_line = prompt_lines
            .iter()
            .find(|(line, structured)| *line > target_line && is_eligible(*structured))
            .map(|(line, _)| *line)
            .unwrap_or(last_line + 1);

        Some(TerminalCommandBlock {
            id: 0,
            start: start_line * columns,
            end: end_line * columns,
            working_directory: None,
            command: None,
            exit_code: None,
            completed: end_line <= last_line,
            shell_integrated: false,
        })
    }

    fn integrated_command_block_at(&self, offset: i64) -> Option<TerminalCommandBlock> {
        let grid = self.term.grid();
        let columns = grid.columns() as i64;
        if columns == 0 {
            return None;
        }
        let first_line = -(grid.history_size() as i64);
        let last_line = grid.screen_lines() as i64 - 1;
        let target_line = offset.div_euclid(columns);
        if target_line < first_line || target_line > last_line {
            return None;
        }

        if self.command_block_marker_cache.borrow().is_none() {
            let markers = scan_command_block_markers(grid, first_line, last_line);
            *self.command_block_marker_cache.borrow_mut() = Some(markers);
        }
        let marker_cache = self.command_block_marker_cache.borrow();
        let markers = marker_cache.as_deref().unwrap_or_default();
        let (start_line, id) = markers
            .iter()
            .rev()
            .find(|(line, _)| *line <= target_line)
            .copied()?;
        let end_line = markers
            .iter()
            .find(|(line, marker_id)| *line > start_line && *marker_id > id)
            .map(|(line, _)| *line)
            .unwrap_or(last_line + 1);
        Some(TerminalCommandBlock {
            id,
            start: start_line * columns,
            end: end_line * columns,
            working_directory: self.command_block_directories.get(&id).cloned(),
            command: self.command_block_commands.get(&id).cloned(),
            exit_code: self.command_block_exit_codes.get(&id).copied(),
            completed: self.command_block_exit_codes.contains_key(&id),
            shell_integrated: true,
        })
    }

    pub fn clipboard(&self) -> String {
        self.events.clipboard().unwrap_or_default()
    }

    pub fn bell_count(&self) -> u64 {
        self.events.bell_count()
    }

    fn forward_terminal_pty_writes(&mut self) {
        let Some(pty) = &mut self.pty else {
            return;
        };

        for write in self.events.drain_pty_writes() {
            pty.queue_input(write.as_bytes());
        }
    }
}

impl TerminalEmulator for TerminalEngine {
    fn resize(&mut self, columns: usize, rows: usize, _cell_width_px: u32, _cell_height_px: u32) {
        TerminalEngine::resize(self, columns, rows);
    }

    fn is_alt_screen(&self) -> bool {
        TerminalEngine::is_alt_screen(self)
    }

    fn scroll_lines(&mut self, lines: i32) {
        TerminalEngine::scroll_lines(self, lines);
    }

    fn scroll_page_up(&mut self) {
        TerminalEngine::scroll_page_up(self);
    }

    fn scroll_page_down(&mut self) {
        TerminalEngine::scroll_page_down(self);
    }

    fn scroll_to_bottom(&mut self) {
        TerminalEngine::scroll_to_bottom(self);
    }

    fn search(
        &mut self,
        query: &str,
        direction: TerminalSearchDirection,
        origin_row: usize,
        origin_column: usize,
    ) -> TerminalSearchResult {
        TerminalEngine::search(self, query, direction, origin_row, origin_column)
    }

    fn start_local_pty(&mut self) -> bool {
        TerminalEngine::start_local_pty(self)
    }

    fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>) {
        TerminalEngine::set_wakeup_callback(self, callback);
    }

    fn send_input_bytes(&mut self, bytes: &[u8]) -> bool {
        TerminalEngine::send_input_bytes(self, bytes)
    }

    fn pump_local_pty(&mut self) -> bool {
        TerminalEngine::pump_local_pty(self)
    }

    fn is_exited(&self) -> bool {
        TerminalEngine::is_exited(self)
    }

    fn set_input_echo_enabled(&mut self, enabled: bool) {
        TerminalEngine::set_input_echo_enabled(self, enabled);
    }

    fn mark_exited(&mut self) {
        TerminalEngine::mark_exited(self);
    }

    fn mark_running(&mut self) {
        TerminalEngine::mark_running(self);
    }

    fn drain_transport_writes(&mut self) -> Vec<String> {
        TerminalEngine::drain_transport_writes(self)
    }

    fn drain_output_capture(&mut self) -> Vec<u8> {
        TerminalEngine::drain_output_capture(self)
    }

    fn suppress_output_until(&mut self, marker: &[u8]) -> bool {
        TerminalEngine::suppress_output_until(self, marker)
    }

    fn cancel_output_suppression(&mut self) {
        TerminalEngine::cancel_output_suppression(self);
    }

    fn clear_pending_output_for_close(&mut self) {
        TerminalEngine::clear_pending_output_for_close(self);
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        TerminalEngine::write_bytes(self, bytes);
    }

    fn write_bytes_without_capture(&mut self, bytes: &[u8]) {
        TerminalEngine::write_bytes_without_capture(self, bytes);
    }

    fn snapshot(&self) -> TerminalSnapshot {
        TerminalEngine::snapshot(self)
    }

    fn plain_text(&self) -> String {
        TerminalEngine::plain_text(self)
    }

    fn selection_text(&self, start: i64, end: i64) -> String {
        TerminalEngine::selection_text(self, start, end)
    }

    fn command_block_at(&self, offset: i64) -> Option<TerminalCommandBlock> {
        TerminalEngine::command_block_at(self, offset)
    }

    fn clipboard(&self) -> String {
        TerminalEngine::clipboard(self)
    }

    fn bell_count(&self) -> u64 {
        TerminalEngine::bell_count(self)
    }
}

fn normalize_dimension(value: usize, fallback: usize) -> usize {
    if value == 0 {
        fallback
    } else {
        value
    }
}

fn terminal_line_prompt_kind(
    grid: &alacritty_terminal::grid::Grid<Cell>,
    line: i64,
) -> Option<bool> {
    let columns = grid.columns();
    let mut text = String::new();
    for column in 0..columns {
        let cell = &grid[Line(line as i32)][Column(column)];
        if cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }
        if cell.flags.contains(Flags::HIDDEN) {
            text.push(' ');
        } else {
            text.push_str(&cell_text(cell.c, cell));
        }
    }
    terminal_text_prompt_kind(&text)
}

#[cfg(test)]
fn terminal_text_looks_like_prompt(text: &str) -> bool {
    terminal_text_prompt_kind(text).is_some()
}

fn terminal_text_prompt_kind(text: &str) -> Option<bool> {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    for (index, character) in trimmed.char_indices() {
        if !matches!(character, '$' | '%' | '#' | '>') {
            continue;
        }
        let suffix = &trimmed[index + character.len_utf8()..];
        if !suffix.is_empty() && !suffix.chars().next().is_some_and(char::is_whitespace) {
            continue;
        }
        let prefix = trimmed[..index].trim_end();
        if prefix.is_empty() || terminal_structured_prompt_prefix(prefix) {
            return Some(!prefix.is_empty());
        }
    }
    None
}

fn terminal_structured_prompt_prefix(prefix: &str) -> bool {
    if prefix.starts_with("PS ") || (prefix.starts_with('[') && prefix.ends_with(']')) {
        return true;
    }

    let prefix_bytes = prefix.as_bytes();
    let windows_path = prefix_bytes.len() >= 3
        && prefix_bytes[1] == b':'
        && matches!(prefix_bytes[2], b'\\' | b'/');
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

fn shell_integration_directory(payload: &[u8]) -> Option<String> {
    let uri = std::str::from_utf8(payload).ok()?;
    let url = Url::parse(uri).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    let path = percent_decode_str(url.path())
        .decode_utf8()
        .ok()?
        .into_owned();
    #[cfg(windows)]
    let path = {
        let mut path = path;
        if path.starts_with('/')
            && path.as_bytes().get(2) == Some(&b':')
            && path.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic)
        {
            path.remove(0);
        }
        path
    };
    (!path.is_empty()).then_some(path)
}

fn command_block_marker(id: u64, starts_on_next_line: bool) -> [char; 6] {
    let mut marker = ['\0'; 6];
    marker[0] = COMMAND_BLOCK_MARKER_PREFIX;
    for digit in 0..COMMAND_BLOCK_MARKER_DIGITS {
        let shift = (COMMAND_BLOCK_MARKER_DIGITS - digit - 1) * 12;
        marker[digit + 1] =
            char::from_u32(COMMAND_BLOCK_MARKER_DIGIT_BASE + ((id >> shift) & 0xfff) as u32)
                .expect("command block marker digit must be valid Unicode");
    }
    marker[5] = if starts_on_next_line {
        COMMAND_BLOCK_MARKER_NEXT_LINE
    } else {
        COMMAND_BLOCK_MARKER_SAME_LINE
    };
    marker
}

fn command_block_markers(cell: &Cell) -> Vec<(u64, bool)> {
    let Some(characters) = cell.zerowidth() else {
        return Vec::new();
    };
    let mut markers = Vec::new();
    let mut index = 0;
    while index + 5 < characters.len() {
        if characters[index] != COMMAND_BLOCK_MARKER_PREFIX {
            index += 1;
            continue;
        }
        let mut id = 0u64;
        let mut valid = true;
        for digit in 0..COMMAND_BLOCK_MARKER_DIGITS {
            let value = characters[index + digit + 1] as u32;
            if !(COMMAND_BLOCK_MARKER_DIGIT_BASE..COMMAND_BLOCK_MARKER_DIGIT_BASE + 0x1000)
                .contains(&value)
            {
                valid = false;
                break;
            }
            id = (id << 12) | u64::from(value - COMMAND_BLOCK_MARKER_DIGIT_BASE);
        }
        let relation = characters[index + 5];
        if valid
            && matches!(
                relation,
                COMMAND_BLOCK_MARKER_SAME_LINE | COMMAND_BLOCK_MARKER_NEXT_LINE
            )
        {
            markers.push((id, relation == COMMAND_BLOCK_MARKER_NEXT_LINE));
            index += 6;
        } else {
            index += 1;
        }
    }
    markers
}

fn scan_command_block_markers(
    grid: &alacritty_terminal::grid::Grid<Cell>,
    first_line: i64,
    last_line: i64,
) -> Vec<(i64, u64)> {
    let mut markers = Vec::new();
    for line in first_line..=last_line {
        for column in 0..grid.columns() {
            let cell = &grid[Line(line as i32)][Column(column)];
            if !cell.flags.contains(COMMAND_BLOCK_MARKER_FLAG) {
                continue;
            }
            for (id, starts_on_next_line) in command_block_markers(cell) {
                markers.push((line + i64::from(starts_on_next_line), id));
            }
        }
    }
    markers.sort_unstable();
    markers
}

fn is_command_block_marker_character(character: char) -> bool {
    matches!(
        character,
        COMMAND_BLOCK_MARKER_PREFIX
            | COMMAND_BLOCK_MARKER_SAME_LINE
            | COMMAND_BLOCK_MARKER_NEXT_LINE
    ) || (COMMAND_BLOCK_MARKER_DIGIT_BASE..COMMAND_BLOCK_MARKER_DIGIT_BASE + 0x1000)
        .contains(&(character as u32))
}

fn literal_search_regex(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for character in query.chars() {
        match character {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

impl ExportCell {
    fn empty() -> Self {
        Self {
            text: String::new(),
            hyperlink: None,
            flags: 0,
            foreground: rgb_to_u32(DEFAULT_FOREGROUND),
            background: rgb_to_u32(DEFAULT_BACKGROUND),
        }
    }

    fn wide_spacer_like(cell: &Self) -> Self {
        Self {
            text: String::new(),
            hyperlink: cell.hyperlink.clone(),
            flags: Flags::WIDE_CHAR_SPACER.bits(),
            foreground: cell.foreground,
            background: cell.background,
        }
    }

    fn is_wide(&self) -> bool {
        self.flags & Flags::WIDE_CHAR.bits() != 0
    }

    fn is_wide_spacer(&self) -> bool {
        self.flags & (Flags::WIDE_CHAR_SPACER.bits() | Flags::LEADING_WIDE_CHAR_SPACER.bits()) != 0
    }
}

impl NormalizedCells {
    fn visual_column_for(&self, column: usize, row: usize, columns: usize) -> usize {
        if columns == 0 {
            return 0;
        }

        self.column_map
            .get(row * columns + column)
            .copied()
            .unwrap_or(column)
            .min(columns - 1)
    }
}

fn normalize_unicode_cells(
    raw_cells: Vec<ExportCell>,
    columns: usize,
    rows: usize,
) -> NormalizedCells {
    let mut normalized = Vec::with_capacity(columns * rows);
    let mut column_map = vec![0; columns * rows];
    for row in 0..rows {
        let row_start = row * columns;
        let row_cells = &raw_cells[row_start..row_start + columns];
        let mut visual_row = Vec::with_capacity(columns);
        let mut column = 0;

        while column < columns {
            let mut cell = row_cells[column].clone();
            if should_merge_with_previous_cluster(&cell, &visual_row) {
                if let Some(base_index) = previous_base_cell_index(&visual_row) {
                    visual_row[base_index].text.push_str(&cell.text);
                    let caret_column = grapheme_caret_column(&visual_row, base_index, columns);
                    map_raw_column(&mut column_map, row_start, column, caret_column, columns);
                    if cell.is_wide()
                        && column + 1 < columns
                        && row_cells[column + 1].is_wide_spacer()
                    {
                        map_raw_column(
                            &mut column_map,
                            row_start,
                            column + 1,
                            caret_column,
                            columns,
                        );
                    }
                }
                if cell.is_wide() && column + 1 < columns && row_cells[column + 1].is_wide_spacer()
                {
                    column += 2;
                } else {
                    column += 1;
                }
                continue;
            }

            if should_force_emoji_wide(&cell) && !cell.is_wide() {
                let visual_column = visual_row.len();
                cell.flags |= Flags::WIDE_CHAR.bits();
                map_raw_column(&mut column_map, row_start, column, visual_column, columns);
                visual_row.push(cell.clone());
                if visual_row.len() < columns {
                    visual_row.push(ExportCell::wide_spacer_like(&cell));
                }
                column += 1;
                continue;
            }

            map_raw_column(
                &mut column_map,
                row_start,
                column,
                visual_row.len(),
                columns,
            );
            visual_row.push(cell);
            column += 1;
        }

        visual_row.resize_with(columns, ExportCell::empty);
        normalized.extend(visual_row.into_iter().take(columns));
    }

    NormalizedCells {
        cells: normalized,
        column_map,
    }
}

fn map_raw_column(
    column_map: &mut [usize],
    row_start: usize,
    raw_column: usize,
    visual_column: usize,
    columns: usize,
) {
    if raw_column >= columns {
        return;
    }

    column_map[row_start + raw_column] = visual_column.min(columns.saturating_sub(1));
}

fn grapheme_caret_column(cells: &[ExportCell], base_index: usize, columns: usize) -> usize {
    let cell_span = if cells.get(base_index).is_some_and(ExportCell::is_wide)
        && cells
            .get(base_index + 1)
            .is_some_and(ExportCell::is_wide_spacer)
    {
        2
    } else {
        1
    };

    (base_index + cell_span).min(columns.saturating_sub(1))
}

fn should_merge_with_previous_cluster(cell: &ExportCell, visual_row: &[ExportCell]) -> bool {
    if cell.text.is_empty() || visual_row.is_empty() {
        return false;
    }

    let Some(previous_index) = previous_base_cell_index(visual_row) else {
        return false;
    };
    let previous = &visual_row[previous_index];
    previous.text.ends_with('\u{200d}')
        || cell
            .text
            .chars()
            .next()
            .is_some_and(is_emoji_cluster_continuation)
}

fn previous_base_cell_index(cells: &[ExportCell]) -> Option<usize> {
    cells.iter().rposition(|cell| !cell.is_wide_spacer())
}

fn is_emoji_cluster_continuation(character: char) -> bool {
    matches!(
        character as u32,
        0x0300..=0x036f
            | 0x1ab0..=0x1aff
            | 0x1dc0..=0x1dff
            | 0x20d0..=0x20ff
            | 0xfe00..=0xfe0f
            | 0x1f3fb..=0x1f3ff
    )
}

fn should_force_emoji_wide(cell: &ExportCell) -> bool {
    cell.text.contains('\u{fe0f}') && cell.text.chars().any(is_emoji_presentation_base)
}

fn is_emoji_presentation_base(character: char) -> bool {
    matches!(
        character as u32,
        0x00a9
            | 0x00ae
            | 0x203c
            | 0x2049
            | 0x2122
            | 0x2139
            | 0x2194..=0x21aa
            | 0x231a..=0x231b
            | 0x2328
            | 0x23cf
            | 0x23e9..=0x23f3
            | 0x23f8..=0x23fa
            | 0x24c2
            | 0x25aa..=0x25ab
            | 0x25b6
            | 0x25c0
            | 0x25fb..=0x25fe
            | 0x2600..=0x27bf
            | 0x2934..=0x2935
            | 0x2b05..=0x2b55
            | 0x3030
            | 0x303d
            | 0x3297
            | 0x3299
    )
}

fn pack_cells(cells: Vec<ExportCell>) -> (Vec<FfiTerminalCell>, Vec<u8>, Vec<u8>) {
    let mut text = Vec::with_capacity(cells.len());
    let mut hyperlink_text = Vec::new();
    let ffi_cells = cells
        .into_iter()
        .map(|cell| {
            let text_offset = text.len();
            text.extend_from_slice(cell.text.as_bytes());
            let text_len = text.len() - text_offset;
            let hyperlink_offset = hyperlink_text.len();
            if let Some(hyperlink) = &cell.hyperlink {
                hyperlink_text.extend_from_slice(hyperlink.as_bytes());
            }
            let hyperlink_len = hyperlink_text.len() - hyperlink_offset;

            FfiTerminalCell {
                text_offset: text_offset as u32,
                text_len: text_len as u16,
                flags: cell.flags,
                foreground: cell.foreground,
                background: cell.background,
                hyperlink_offset: hyperlink_offset as u32,
                hyperlink_len: hyperlink_len as u32,
            }
        })
        .collect();

    (ffi_cells, text, hyperlink_text)
}

fn export_cell(cell: &Cell, colors: &alacritty_terminal::term::color::Colors) -> ExportCell {
    let text = if cell
        .flags
        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
    {
        String::new()
    } else if cell.flags.contains(Flags::HIDDEN) {
        " ".to_string()
    } else {
        cell_text(cell.c, cell)
    };

    ExportCell {
        text,
        hyperlink: cell.hyperlink().map(|hyperlink| hyperlink.uri().to_owned()),
        flags: (cell.flags - COMMAND_BLOCK_MARKER_FLAG).bits(),
        foreground: rgb_to_u32(resolve_color(cell.fg, colors, DEFAULT_FOREGROUND)),
        background: rgb_to_u32(resolve_color(cell.bg, colors, DEFAULT_BACKGROUND)),
    }
}

fn cell_text(character: char, cell: &Cell) -> String {
    let mut text = String::new();
    text.push(character);

    if let Some(zerowidth) = cell.zerowidth() {
        for character in zerowidth {
            if !is_command_block_marker_character(*character) {
                text.push(*character);
            }
        }
    }

    text
}

fn resolve_color(
    color: Color,
    colors: &alacritty_terminal::term::color::Colors,
    fallback: Rgb,
) -> Rgb {
    match color {
        Color::Spec(rgb) => rgb,
        Color::Indexed(index) => indexed_color(index),
        Color::Named(name) => colors[name].unwrap_or_else(|| default_named_color(name, fallback)),
    }
}

fn default_named_color(name: NamedColor, fallback: Rgb) -> Rgb {
    match name as usize {
        0..=15 => ANSI_PALETTE[name as usize],
        256 => DEFAULT_FOREGROUND,
        257 => DEFAULT_BACKGROUND,
        258 => DEFAULT_CURSOR,
        _ => fallback,
    }
}

fn indexed_color(index: u8) -> Rgb {
    match index {
        0..=15 => ANSI_PALETTE[index as usize],
        16..=231 => {
            let index = index - 16;
            let r = index / 36;
            let g = (index % 36) / 6;
            let b = index % 6;
            Rgb {
                r: color_cube_component(r),
                g: color_cube_component(g),
                b: color_cube_component(b),
            }
        }
        232..=255 => {
            let value = 8 + (index - 232) * 10;
            Rgb {
                r: value,
                g: value,
                b: value,
            }
        }
    }
}

fn color_cube_component(value: u8) -> u8 {
    if value == 0 {
        0
    } else {
        55 + value * 40
    }
}

fn rgb_to_u32(rgb: Rgb) -> u32 {
    u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b)
}

fn u32_to_rgb(value: u32, fallback: Rgb) -> Rgb {
    if value > 0x00ff_ffff {
        return fallback;
    }
    Rgb {
        r: ((value >> 16) & 0xff) as u8,
        g: ((value >> 8) & 0xff) as u8,
        b: (value & 0xff) as u8,
    }
}

fn cursor_shape_to_u32(shape: CursorShape) -> u32 {
    match shape {
        CursorShape::Block => 0,
        CursorShape::Underline => 1,
        CursorShape::Beam => 2,
        CursorShape::HollowBlock => 3,
        CursorShape::Hidden => 0,
    }
}

fn keyboard_mode_to_u32(mode: TermMode) -> u32 {
    u32::from(mode.contains(TermMode::APP_CURSOR))
        | (u32::from(mode.contains(TermMode::APP_KEYPAD)) << 1)
        | (u32::from(mode.contains(TermMode::BRACKETED_PASTE)) << 2)
        | (u32::from(mode.contains(TermMode::FOCUS_IN_OUT)) << 3)
        | (u32::from(mode.contains(TermMode::MOUSE_REPORT_CLICK)) << 4)
        | (u32::from(mode.contains(TermMode::MOUSE_DRAG)) << 5)
        | (u32::from(mode.contains(TermMode::MOUSE_MOTION)) << 6)
        | (u32::from(mode.contains(TermMode::SGR_MOUSE)) << 7)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "integration-tests")]
    use std::thread;
    #[cfg(feature = "integration-tests")]
    use std::time::{Duration, Instant};

    #[test]
    fn writes_plain_text_to_the_grid() {
        let mut terminal = TerminalEngine::new(8, 2);

        terminal.write_bytes(b"hello");
        let snapshot = terminal.snapshot();

        assert_eq!(snapshot.columns, 8);
        assert_eq!(snapshot.rows, 2);
        assert_eq!(cell_text(&snapshot, 0), "h");
        assert_eq!(cell_text(&snapshot, 4), "o");
    }

    #[test]
    fn detects_prompt_and_output_command_blocks() {
        let mut terminal = TerminalEngine::new(24, 6);
        terminal.write_bytes(b"user@host:~$ ls\r\none\r\ntwo\r\nuser@host:~$ pwd\r\n/home");

        assert_eq!(
            terminal
                .command_block_at(24)
                .map(|region| (region.start, region.end)),
            Some((0, 72))
        );
        assert_eq!(
            terminal
                .command_block_at(96)
                .map(|region| (region.start, region.end)),
            Some((72, 144))
        );
        assert!(terminal.command_block_at(-24).is_none());
        assert!(terminal.command_block_at(-10_000).is_none());
    }

    #[test]
    fn shell_integration_defines_command_blocks_and_working_directories() {
        let mut terminal = TerminalEngine::new(48, 6);
        terminal.write_bytes(
            b"\x1b]7;file://localhost/tmp/Project Files\x07\x1b]133;A\x07user@host:/tmp$ cat index.html\r\n</div>\r\n",
        );
        terminal.write_bytes(
            b"\x1b]4545;CommandStarted;Y2F0IGluZGV4Lmh0bWw=\x07\x1b]4545;CommandExited;0\x07",
        );
        terminal.write_bytes(b"\x1b]7;file://localhost/var/tmp\x1b");
        terminal.write_bytes(b"\\\x1b]133;A\x1b\\user@host:/var/tmp$ ");

        let block = terminal.command_block_at(48).expect("integrated block");
        assert_eq!((block.start, block.end), (0, 2 * 48));
        assert_eq!(block.id, 1);
        assert_eq!(
            block.working_directory.as_deref(),
            Some("/tmp/Project Files")
        );
        assert_eq!(block.command.as_deref(), Some("cat index.html"));
        assert_eq!(block.exit_code, Some(0));
        assert!(block.completed);
        assert!(block.shell_integrated);

        let current = terminal
            .command_block_at(2 * 48)
            .expect("current integrated block");
        assert_eq!(current.id, 2);
        assert_eq!((current.start, current.end), (2 * 48, 6 * 48));
        assert_eq!(current.working_directory.as_deref(), Some("/var/tmp"));
        assert_eq!(current.command, None);
        assert_eq!(current.exit_code, None);
        assert!(!current.completed);
        assert!(current.shell_integrated);
        assert!(!terminal.plain_text().contains(COMMAND_BLOCK_MARKER_PREFIX));
        assert!(terminal
            .snapshot()
            .cells
            .iter()
            .all(|cell| cell.flags & COMMAND_BLOCK_MARKER_FLAG.bits() == 0));
    }

    #[test]
    fn shell_integration_markers_follow_scrollback_rows() {
        let mut terminal = TerminalEngine::new_with_options(
            24,
            3,
            TerminalOptions {
                scrollback_lines: 32,
                ..TerminalOptions::default()
            },
        );
        terminal.write_bytes(
            b"\x1b]7;file://localhost/first\x07\x1b]133;A\x07$ one\r\n1\r\n2\r\n3\r\n4\r\n",
        );
        terminal.write_bytes(b"\x1b]7;file://localhost/second\x07\x1b]133;A\x07$ two");

        let grid = terminal.term.grid();
        let first_line = -(grid.history_size() as i64);
        let last_line = grid.screen_lines() as i64 - 1;
        let directories = (first_line..=last_line)
            .filter_map(|line| terminal.command_block_at(line * 24))
            .filter_map(|block| block.working_directory)
            .collect::<Vec<_>>();
        assert!(directories.iter().any(|directory| directory == "/first"));
        assert!(directories.iter().any(|directory| directory == "/second"));
    }

    #[test]
    fn structured_prompts_ignore_prompt_like_cat_output() {
        let mut terminal = TerminalEngine::new(40, 7);
        terminal.write_bytes(
            b"user@host:~$ cat note.md\r\n# Heading\r\n$ price\r\nbody\r\nuser@host:~$ echo done",
        );

        assert_eq!(
            terminal
                .command_block_at(2 * 40)
                .map(|block| (block.start, block.end)),
            Some((0, 4 * 40))
        );
    }

    #[test]
    fn html_closing_tags_do_not_split_command_blocks() {
        let mut terminal = TerminalEngine::new(80, 8);
        terminal.write_bytes(
            b"user@host:~$ cat index.html\r\n<div class=\"wrapper\">\r\n</div>\r\n<script>\r\nwrapper.addEventListener('mouseleave', reset);\r\n</script>\r\nuser@host:~$ ",
        );

        assert_eq!(
            terminal
                .command_block_at(4 * 80)
                .map(|block| (block.start, block.end)),
            Some((0, 6 * 80))
        );
    }

    #[test]
    fn alternate_screen_does_not_expose_command_blocks() {
        let mut terminal = TerminalEngine::new(40, 7);
        terminal.write_bytes(b"user@host:~$ vim note.md\r\n\x1b[?1049h# Heading");

        assert!(terminal.command_block_at(0).is_none());
    }

    #[test]
    fn prompt_detection_accepts_common_shells_without_matching_plain_output() {
        assert!(terminal_text_looks_like_prompt("user@host:/tmp $ ls"));
        assert!(terminal_text_looks_like_prompt("[root@host ~]# dnf update"));
        assert!(terminal_text_looks_like_prompt("PS C:\\Users\\me> dir"));
        assert!(terminal_text_looks_like_prompt("$ echo hello"));
        assert!(!terminal_text_looks_like_prompt("price: $ 5"));
        assert!(!terminal_text_looks_like_prompt("build > output.log"));
        assert!(!terminal_text_looks_like_prompt("</div>"));
        assert!(!terminal_text_looks_like_prompt("</script>"));
    }

    #[test]
    fn exports_rendered_scrollback_as_plain_text() {
        let mut terminal = TerminalEngine::new_with_options(
            8,
            2,
            TerminalOptions {
                scrollback_lines: 20,
                ..TerminalOptions::default()
            },
        );
        terminal.write_bytes(b"\x1b[31mfirst\x1b[0m\r\nsecond\r\nthird");

        assert_eq!(terminal.plain_text(), "first\nsecond\nthird\n");
        assert_eq!(terminal.selection_text(-8, 16), "first\nsecond\nthird");
        assert_eq!(terminal.selection_text(-8, 8), "first\nsecond");
    }

    #[test]
    fn plain_text_export_uses_the_final_alternate_screen_state() {
        let mut terminal = TerminalEngine::new(8, 2);
        terminal.write_bytes(b"shell\r\n\x1b[?1049h\x1b[Htop\x1b[?1049l");

        assert_eq!(terminal.plain_text(), "shell\n");
    }

    #[test]
    fn suppresses_display_output_until_a_split_marker() {
        let mut terminal = TerminalEngine::new(24, 2);
        assert!(terminal.suppress_output_until(b"ready-marker"));

        terminal.write_bytes(b"hidden setup ready-");
        assert!(!snapshot_text(&terminal.snapshot()).contains("hidden"));
        terminal.write_bytes(b"marker\r\nvisible");

        let output = snapshot_text(&terminal.snapshot());
        assert!(!output.contains("hidden"));
        assert!(output.contains("visible"));
        assert_eq!(
            terminal.drain_output_capture(),
            b"hidden setup ready-marker\r\nvisible"
        );
    }

    #[test]
    fn cancelling_output_suppression_discards_hidden_setup() {
        let mut terminal = TerminalEngine::new(24, 2);
        assert!(terminal.suppress_output_until(b"ready-marker"));

        terminal.write_bytes(b"partial hidden setup");
        terminal.cancel_output_suppression();
        terminal.write_bytes(b"visible");

        let output = snapshot_text(&terminal.snapshot());
        assert!(!output.contains("hidden"));
        assert!(output.contains("visible"));
    }

    #[test]
    fn applies_ansi_color_sequences() {
        let mut terminal = TerminalEngine::new(8, 2);

        terminal.write_bytes(b"\x1b[31mR\x1b[37mW");
        let snapshot = terminal.snapshot();

        assert_eq!(cell_text(&snapshot, 0), "R");
        assert_eq!(snapshot.cells[0].foreground, 0xd95f56);
        assert_eq!(cell_text(&snapshot, 1), "W");
        assert_eq!(snapshot.cells[1].foreground, 0x68707d);
        assert_ne!(snapshot.cells[1].foreground, 0xfbfbf8);
    }

    #[test]
    fn preserves_truecolor_bce_and_wrap_boundaries() {
        let mut colors = TerminalEngine::new(4, 2);
        colors.write_bytes(b"\x1b[38;2;18;52;86mF");
        let snapshot = colors.snapshot();
        assert_eq!(snapshot.cells[0].foreground, 0x123456);

        colors.write_bytes(b"\x1b[48;2;171;205;239mB\x1b[2K");
        let snapshot = colors.snapshot();
        assert_eq!(snapshot.cells[0].background, 0xabcdef);
        assert_eq!(snapshot.cells[1].background, 0xabcdef);

        let mut wrapped = TerminalEngine::new(4, 2);
        wrapped.write_bytes(b"abcdE");
        let snapshot = wrapped.snapshot();
        assert_eq!(cell_text(&snapshot, 4), "E");
        assert_eq!(snapshot.cursor_row, 1);
    }

    #[test]
    fn exports_osc_title_updates() {
        let mut terminal = TerminalEngine::new(8, 2);

        terminal.write_bytes(b"\x1b]0;nauterm-title\x07content");

        let snapshot = terminal.snapshot();
        assert_eq!(snapshot.title, "nauterm-title");
        assert_eq!(cell_text(&snapshot, 0), "c");
    }

    #[test]
    fn ignores_unconsumed_osc_sequences_without_leaking_payload() {
        let mut terminal = TerminalEngine::new(32, 2);

        terminal.write_bytes(
            b"\x1b]7;file://remote/tmp\x07\x1b]9;notification\x07\x1b]133;A\x07visible",
        );

        let output = snapshot_text(&terminal.snapshot());
        assert!(output.contains("visible"));
        assert!(!output.contains("file://remote/tmp"));
        assert!(!output.contains("notification"));
    }

    #[test]
    fn preserves_osc8_hyperlink_scope() {
        let mut terminal = TerminalEngine::new(16, 2);

        terminal.write_bytes(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\plain");

        let link = terminal.term.grid()[Point::new(Line(0), Column(0))]
            .hyperlink()
            .expect("OSC 8 should attach a hyperlink to the cell");
        assert_eq!(link.uri(), "https://example.com");
        let snapshot = terminal.snapshot();
        let link_cell = snapshot.cells[0];
        let link_start = link_cell.hyperlink_offset as usize;
        let link_end = link_start + link_cell.hyperlink_len as usize;
        assert_eq!(
            std::str::from_utf8(&snapshot.hyperlink_text[link_start..link_end]).unwrap(),
            "https://example.com"
        );
        assert_eq!(snapshot.cells[4].hyperlink_len, 0);
    }

    #[test]
    fn exports_bell_and_osc52_clipboard_updates() {
        let mut terminal = TerminalEngine::new(8, 2);

        terminal.write_bytes(b"\x07\x1b]52;c;SGVsbG8=\x07");

        let snapshot = terminal.snapshot();
        assert_eq!(snapshot.bell_count, 1);
        assert_eq!(snapshot.clipboard, "Hello");
    }

    #[test]
    fn osc52_paste_requires_explicit_copy_and_paste_mode() {
        let mut copy_only = TerminalEngine::new(8, 2);
        copy_only.write_bytes(b"\x1b]52;c;SGVsbG8=\x07\x1b]52;c;?\x07");
        assert!(copy_only.drain_transport_writes().is_empty());

        let mut copy_and_paste = TerminalEngine::new_with_options(
            8,
            2,
            TerminalOptions {
                osc52: Osc52Mode::CopyPaste,
                ..TerminalOptions::default()
            },
        );
        copy_and_paste.write_bytes(b"\x1b]52;c;SGVsbG8=\x07\x1b]52;c;?\x07");
        assert_eq!(
            copy_and_paste.drain_transport_writes(),
            vec!["\x1b]52;c;SGVsbG8=\x07".to_owned()]
        );
    }

    #[test]
    fn exports_cursor_shape_and_color() {
        let mut terminal = TerminalEngine::new(8, 2);

        terminal.write_bytes(b"\x1b[5 q\x1b]12;#112233\x1b\\");
        let snapshot = terminal.snapshot();

        assert_eq!(snapshot.cursor_shape, 2);
        assert_eq!(snapshot.cursor_color, 0x112233);
    }

    #[test]
    fn applies_configured_cursor_defaults() {
        let terminal = TerminalEngine::new_with_options(
            8,
            2,
            TerminalOptions {
                cursor_shape: CursorShape::Beam,
                cursor_blinking: true,
                ..TerminalOptions::default()
            },
        );

        let snapshot = terminal.snapshot();

        assert_eq!(snapshot.cursor_shape, 2);
        assert!(snapshot.cursor_blinking);
    }

    #[test]
    fn answers_dynamic_color_queries_from_configured_theme() {
        let mut terminal = TerminalEngine::new_with_options(
            8,
            2,
            TerminalOptions {
                default_colors: TerminalDefaultColors::from_rgb_values(
                    0x445566, 0x112233, 0x778899,
                ),
                ..TerminalOptions::default()
            },
        );

        terminal.write_bytes(b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]12;?\x1b\\");

        assert_eq!(
            terminal.drain_transport_writes(),
            vec![
                "\x1b]10;rgb:4444/5555/6666\x1b\\".to_owned(),
                "\x1b]11;rgb:1111/2222/3333\x1b\\".to_owned(),
                "\x1b]12;rgb:7777/8888/9999\x1b\\".to_owned(),
            ]
        );
    }

    #[test]
    fn restores_cursor_style_after_alternate_screen_exit() {
        let mut terminal = TerminalEngine::new_with_options(
            8,
            2,
            TerminalOptions {
                cursor_shape: CursorShape::Beam,
                cursor_blinking: true,
                ..TerminalOptions::default()
            },
        );

        terminal.write_bytes(b"\x1b[?1049h\x1b[?12l\x1b[2 q");
        let snapshot = terminal.snapshot();
        assert!(snapshot.alternate_screen);
        assert_eq!(snapshot.cursor_shape, 0);
        assert!(!snapshot.cursor_blinking);

        terminal.write_bytes(b"\x1b[?1049l");
        let snapshot = terminal.snapshot();
        assert!(!snapshot.alternate_screen);
        assert_eq!(snapshot.cursor_shape, 2);
        assert!(snapshot.cursor_blinking);
    }

    #[test]
    fn maps_terminal_types_to_term_values() {
        assert_eq!(
            TerminalType::from_term("xterm-256color").map(TerminalType::term),
            Some("xterm-256color")
        );
        assert_eq!(
            TerminalType::from_term("xterm-16color").map(TerminalType::term),
            Some("xterm-16color")
        );
        assert_eq!(
            TerminalType::from_term("xterm").map(TerminalType::term),
            Some("xterm")
        );
        assert!(TerminalType::from_term("ansi").is_none());
    }

    #[test]
    fn exports_keyboard_modes() {
        let mut terminal = TerminalEngine::new(8, 2);

        terminal.write_bytes(b"\x1b[?1h\x1b[?2004h");
        let snapshot = terminal.snapshot();

        assert_eq!(snapshot.keyboard_mode & 0x01, 0x01);
        assert_eq!(snapshot.keyboard_mode & 0x04, 0x04);
    }

    #[test]
    fn exports_wide_and_combining_unicode_cells() {
        let mut terminal = TerminalEngine::new(18, 2);

        terminal.write_bytes("表e\u{301}🙂👍🏽✈️👨‍👩‍👧‍👦".as_bytes());
        let snapshot = terminal.snapshot();

        assert_eq!(cell_text(&snapshot, 0), "表");
        assert!(cell_has_flag(&snapshot, 0, Flags::WIDE_CHAR));
        assert!(cell_has_flag(&snapshot, 1, Flags::WIDE_CHAR_SPACER));

        assert_eq!(cell_text(&snapshot, 2), "e\u{301}");
        assert!(!cell_has_flag(&snapshot, 2, Flags::WIDE_CHAR));

        assert_eq!(cell_text(&snapshot, 3), "🙂");
        assert!(cell_has_flag(&snapshot, 3, Flags::WIDE_CHAR));
        assert!(cell_has_flag(&snapshot, 4, Flags::WIDE_CHAR_SPACER));

        assert_eq!(cell_text(&snapshot, 5), "👍🏽");
        assert!(cell_has_flag(&snapshot, 5, Flags::WIDE_CHAR));
        assert!(cell_has_flag(&snapshot, 6, Flags::WIDE_CHAR_SPACER));

        assert_eq!(cell_text(&snapshot, 7), "✈️");
        assert!(cell_has_flag(&snapshot, 7, Flags::WIDE_CHAR));
        assert!(cell_has_flag(&snapshot, 8, Flags::WIDE_CHAR_SPACER));

        assert_eq!(cell_text(&snapshot, 9), "👨‍👩‍👧‍👦");
        assert!(cell_has_flag(&snapshot, 9, Flags::WIDE_CHAR));
        assert!(cell_has_flag(&snapshot, 10, Flags::WIDE_CHAR_SPACER));
        assert_eq!(snapshot.cursor_column, 11);
    }

    #[test]
    fn malformed_utf8_does_not_corrupt_the_terminal_snapshot() {
        let mut terminal = TerminalEngine::new(8, 2);

        terminal.write_bytes(&[0xff, b'A', 0xc3]);
        let snapshot = terminal.snapshot();
        let text = snapshot_text(&snapshot);

        assert!(text.contains('A'));
        assert!(snapshot.cells.len() >= 16);
    }

    #[test]
    fn scrolls_through_history_without_changing_the_viewport_size() {
        let mut terminal = TerminalEngine::new_with_options(
            8,
            2,
            TerminalOptions {
                scrollback_lines: 20,
                ..TerminalOptions::default()
            },
        );

        terminal.write_bytes(b"one\r\ntwo\r\nthree\r\nfour");
        let bottom_snapshot = terminal.snapshot();
        let bottom = snapshot_text(&bottom_snapshot);
        assert!(bottom_snapshot.history_lines >= 2);
        assert_eq!(bottom_snapshot.display_offset, 0);

        terminal.scroll_lines(2);
        let scrolled_snapshot = terminal.snapshot();
        let scrolled = snapshot_text(&scrolled_snapshot);

        assert_ne!(scrolled, bottom);
        assert!(scrolled.contains("two") || scrolled.contains("three"));
        assert_eq!(scrolled_snapshot.display_offset, 2);
        assert!(scrolled_snapshot.display_offset <= scrolled_snapshot.history_lines);

        terminal.scroll_to_bottom();
        let restored_snapshot = terminal.snapshot();
        assert_eq!(snapshot_text(&restored_snapshot), bottom);
        assert_eq!(restored_snapshot.display_offset, 0);
    }

    #[test]
    fn scrollback_disabled_mosh_display_updates_do_not_create_history() {
        let mut terminal = TerminalEngine::new_with_options(
            8,
            2,
            TerminalOptions {
                scrollback_lines: 0,
                ..TerminalOptions::default()
            },
        );

        terminal.write_bytes(b"sh$ ");
        terminal.write_bytes(b"\x1b[?25l\nALT\x1b[?25h");

        let snapshot = terminal.snapshot();
        let visible = snapshot_text(&snapshot);

        assert_eq!(snapshot.history_lines, 0);
        assert_eq!(snapshot.display_offset, 0);
        assert!(visible.contains("ALT"));
    }

    #[test]
    fn search_finds_visible_text() {
        let mut terminal = TerminalEngine::new(12, 3);

        terminal.write_bytes(b"alpha\r\nbeta\r\ngamma");
        let result = terminal.search("beta", TerminalSearchDirection::Forward, 0, 0);

        assert!(result.found);
        assert_eq!(result.start_row, 1);
        assert_eq!(result.start_column, 0);
        assert_eq!(result.end_row, 1);
        assert_eq!(result.end_column, 4);
    }

    #[test]
    fn search_scrolls_scrollback_match_into_view() {
        let mut terminal = TerminalEngine::new_with_options(
            12,
            2,
            TerminalOptions {
                scrollback_lines: 20,
                ..TerminalOptions::default()
            },
        );

        terminal.write_bytes(b"needle\r\nline-2\r\nline-3\r\nline-4");
        let bottom = snapshot_text(&terminal.snapshot());
        let result = terminal.search("needle", TerminalSearchDirection::Forward, 0, 0);
        let scrolled = snapshot_text(&terminal.snapshot());

        assert!(result.found);
        assert_eq!(result.start_row, 0);
        assert_eq!(result.start_column, 0);
        assert_ne!(scrolled, bottom);
        assert!(scrolled.contains("needle"));
    }

    #[cfg(all(not(windows), feature = "integration-tests"))]
    #[test]
    fn local_pty_runs_a_shell_command() {
        let mut terminal = TerminalEngine::new(80, 24);

        assert!(terminal.start_local_pty());
        assert!(terminal.send_input_bytes(b"printf nauterm_pty_ready\\n; exit\n"));

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut output = String::new();
        while Instant::now() < deadline {
            terminal.pump_local_pty();
            output = snapshot_text(&terminal.snapshot());
            if output.contains("nauterm_pty_ready") {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert!(
            output.contains("nauterm_pty_ready"),
            "PTY output did not include marker. Output: {output:?}"
        );
    }

    #[cfg(all(unix, feature = "integration-tests"))]
    #[test]
    fn local_pty_renders_real_tmux_split_panes_and_status_line() {
        let tmux_available = std::process::Command::new("tmux")
            .arg("-V")
            .status()
            .is_ok_and(|status| status.success());
        assert!(
            tmux_available,
            "tmux must be installed for the integration test suite"
        );

        let socket = format!("nauterm-matrix-{}", std::process::id());
        let session = format!("nauterm-matrix-{}", std::process::id());
        let script = format!(
            "tmux -L {socket} -f /dev/null new-session -d -s {session} 'for i in 1 2 3 4 5; do printf LEFT-$i; done; sleep 1'; \
             tmux -L {socket} split-window -h -t {session} 'for i in 1 2 3 4 5; do printf RIGHT-$i; done; sleep 1'; \
             tmux -L {socket} set-option -t {session} status-left TMUX-STATUS; \
             (sleep 2; tmux -L {socket} kill-session -t {session} 2>/dev/null) & \
             exec tmux -L {socket} attach-session -t {session}"
        );
        let mut terminal = TerminalEngine::new_with_options(
            100,
            30,
            TerminalOptions {
                command: Some(TerminalCommand {
                    program: "/bin/sh".to_owned(),
                    args: vec!["-c".to_owned(), script],
                }),
                ..TerminalOptions::default()
            },
        );

        assert!(terminal.start_local_pty());
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = String::new();
        while Instant::now() < deadline {
            terminal.pump_local_pty();
            output = snapshot_text(&terminal.snapshot());
            if output.contains("LEFT-5") && output.contains("RIGHT-5") {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert!(
            output.contains("LEFT-5") && output.contains("RIGHT-5"),
            "tmux split pane output did not converge: {output:?}"
        );
        assert!(
            output.contains("TMUX-STATU"),
            "tmux status line missing: {output:?}"
        );
    }

    #[cfg(all(not(windows), feature = "integration-tests"))]
    #[cfg_attr(
        target_os = "macos",
        ignore = "macOS may coalesce PTY output past transient termios states"
    )]
    #[test]
    fn local_pty_reports_echo_mode_changes() {
        let mut terminal = TerminalEngine::new(80, 24);

        assert!(terminal.start_local_pty());
        wait_for_pty_output(&mut terminal, |output, snapshot| {
            output_looks_ready_for_input(output) && snapshot.input_echo_enabled
        });

        assert!(terminal.send_input_bytes(
            b"stty icanon -echo; printf 'nauterm_echo_off_ready\\n'; sleep 1; stty sane; printf 'nauterm_echo_on_ready\\n'; exit\n"
        ));

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_echo_off = false;
        let mut output = String::new();
        while Instant::now() < deadline {
            terminal.pump_local_pty();
            thread::sleep(Duration::from_millis(20));
            let snapshot = terminal.snapshot();
            output = snapshot_text(&snapshot);
            if output.contains("nauterm_echo_off_ready") && !snapshot.input_echo_enabled {
                saw_echo_off = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert!(
            saw_echo_off,
            "PTY did not report echo off. Output: {output:?}"
        );

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_echo_on = false;
        while Instant::now() < deadline {
            terminal.pump_local_pty();
            thread::sleep(Duration::from_millis(20));
            let snapshot = terminal.snapshot();
            output = snapshot_text(&snapshot);
            if output.contains("nauterm_echo_on_ready") && snapshot.input_echo_enabled {
                saw_echo_on = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            saw_echo_on,
            "PTY did not report echo on. Output: {output:?}"
        );
    }

    #[cfg(all(not(windows), feature = "integration-tests"))]
    #[test]
    fn local_pty_treats_raw_noecho_as_interactive_input() {
        let mut terminal = TerminalEngine::new(80, 24);

        assert!(terminal.start_local_pty());
        wait_for_pty_output(&mut terminal, |output, _| {
            output_looks_ready_for_input(output)
        });
        assert!(terminal.send_input_bytes(
            b"stty raw -echo; printf nauterm_raw_noecho_ready\\r; stty sane; printf nauterm_echo_on_ready\\n; exit\n"
        ));

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_raw_noecho = false;
        let mut output = String::new();
        while Instant::now() < deadline {
            terminal.pump_local_pty();
            let snapshot = terminal.snapshot();
            output = snapshot_text(&snapshot);
            if output.contains("nauterm_raw_noecho_ready") && snapshot.input_echo_enabled {
                saw_raw_noecho = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert!(
            saw_raw_noecho,
            "PTY treated raw noecho as private input. Output: {output:?}"
        );
    }

    #[cfg(all(not(windows), feature = "integration-tests"))]
    fn wait_for_pty_output(
        terminal: &mut TerminalEngine,
        ready: impl Fn(&str, &TerminalSnapshot) -> bool,
    ) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            terminal.pump_local_pty();
            let snapshot = terminal.snapshot();
            let output = snapshot_text(&snapshot);
            if ready(&output, &snapshot) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[cfg(all(not(windows), feature = "integration-tests"))]
    fn output_looks_ready_for_input(output: &str) -> bool {
        output.contains("$ ") || output.contains("% ") || output.contains("# ")
    }

    fn cell_text(snapshot: &TerminalSnapshot, index: usize) -> &str {
        let cell = snapshot.cells[index];
        let start = cell.text_offset as usize;
        let end = start + cell.text_len as usize;
        std::str::from_utf8(&snapshot.text[start..end]).unwrap()
    }

    fn cell_has_flag(snapshot: &TerminalSnapshot, index: usize, flag: Flags) -> bool {
        snapshot.cells[index].flags & flag.bits() != 0
    }

    fn snapshot_text(snapshot: &TerminalSnapshot) -> String {
        let mut output = String::new();
        for row in 0..snapshot.rows {
            for column in 0..snapshot.columns {
                output.push_str(cell_text(snapshot, row * snapshot.columns + column));
            }
            output.push('\n');
        }

        output
    }
}
