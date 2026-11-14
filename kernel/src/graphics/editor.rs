// kernel/src/graphics/editor.rs - Multiline Text Editor Engine (UI-04.2 & UI-04.3)
// Implements multiline editing, UTF-8 boundary safety, selection, scrolling,
// native clipboard copy/paste, undo, and dirty-state tracking.

use super::app::{AppCommand, AppEvent, KeyCode, WindowApp, CLIPBOARD_SIZE};
use super::proportional_font::{self, FontFace};
use super::{Color, GopBuffer, Rect};

pub const MAX_EDITOR_BYTES: usize = 4096;
const MAX_LINES: usize = 128;
const LINE_HEIGHT_PX: i32 = 20;

#[derive(Clone, Copy)]
pub struct TextEditorApp {
    pub filename: [u8; 32],
    pub filename_len: usize,
    pub buffer: [u8; MAX_EDITOR_BYTES],
    pub len: usize,
    pub cursor_pos: usize,
    pub selection: Option<(usize, usize)>, // (start, end) byte indices
    pub selection_anchor: Option<usize>,
    pub scroll_line: usize,
    pub is_dirty: bool,
    // Single-level undo snapshot for safe rollback
    undo_buffer: [u8; MAX_EDITOR_BYTES],
    undo_len: usize,
    undo_cursor: usize,
    has_undo: bool,
}

impl TextEditorApp {
    pub fn new(filename_str: &str, initial_content: &str) -> Self {
        let mut app = Self {
            filename: [0u8; 32],
            filename_len: 0,
            buffer: [0u8; MAX_EDITOR_BYTES],
            len: 0,
            cursor_pos: 0,
            selection: None,
            selection_anchor: None,
            scroll_line: 0,
            is_dirty: false,
            undo_buffer: [0u8; MAX_EDITOR_BYTES],
            undo_len: 0,
            undo_cursor: 0,
            has_undo: false,
        };
        app.set_filename(filename_str);
        app.set_content(initial_content);
        app.is_dirty = false;
        app
    }

    pub fn set_filename(&mut self, name: &str) {
        let b = name.as_bytes();
        let l = core::cmp::min(b.len(), 32);
        self.filename[..l].copy_from_slice(&b[..l]);
        self.filename_len = l;
    }

    pub fn filename_str(&self) -> &str {
        core::str::from_utf8(&self.filename[..self.filename_len]).unwrap_or("")
    }

    pub fn set_content(&mut self, s: &str) {
        let b = s.as_bytes();
        let l = core::cmp::min(b.len(), MAX_EDITOR_BYTES);
        self.buffer[..l].copy_from_slice(&b[..l]);
        self.len = l;
        self.cursor_pos = l;
        self.selection = None;
        self.is_dirty = true;
    }

    pub fn content_str(&self) -> &str {
        core::str::from_utf8(&self.buffer[..self.len]).unwrap_or("")
    }

    // --- UTF-8 Boundary Navigation Primitives ---

    pub fn is_char_boundary(&self, idx: usize) -> bool {
        if idx == 0 || idx == self.len {
            return true;
        }
        if idx > self.len {
            return false;
        }
        // In UTF-8, continuation bytes match (b & 0xC0) == 0x80
        (self.buffer[idx] & 0xC0) != 0x80
    }

    pub fn prev_char_boundary(&self, mut pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        pos -= 1;
        while pos > 0 && !self.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    pub fn next_char_boundary(&self, mut pos: usize) -> usize {
        if pos >= self.len {
            return self.len;
        }
        pos += 1;
        while pos < self.len && !self.is_char_boundary(pos) {
            pos += 1;
        }
        pos
    }

    // --- Multiline Line Indexing & Geometry ---

    /// Returns a list of start byte offsets for each newline-separated line
    pub fn line_starts(&self, starts: &mut [usize; MAX_LINES]) -> usize {
        starts[0] = 0;
        let mut count = 1;
        for i in 0..self.len {
            if self.buffer[i] == b'\n' && count < MAX_LINES {
                starts[count] = i + 1;
                count += 1;
            }
        }
        count
    }

    /// Finds line index and column offset (in bytes) of current cursor
    pub fn cursor_line_and_col(&self) -> (usize, usize) {
        let mut line = 0;
        let mut line_start = 0;
        for i in 0..self.cursor_pos {
            if self.buffer[i] == b'\n' {
                line += 1;
                line_start = i + 1;
            }
        }
        (line, self.cursor_pos - line_start)
    }

    /// Moves cursor to a given line and target column
    pub fn move_to_line_and_col(&mut self, target_line: usize, target_col: usize) {
        let mut starts = [0usize; MAX_LINES];
        let total_lines = self.line_starts(&mut starts);
        if target_line >= total_lines {
            self.cursor_pos = self.len;
            return;
        }
        let l_start = starts[target_line];
        let l_end = if target_line + 1 < total_lines {
            starts[target_line + 1] - 1 // before \n
        } else {
            self.len
        };
        let l_len = l_end - l_start;
        let col = core::cmp::min(target_col, l_len);
        let mut target_pos = l_start + col;
        // Align to valid UTF-8 boundary
        while target_pos < self.len && !self.is_char_boundary(target_pos) {
            target_pos += 1;
        }
        self.cursor_pos = core::cmp::min(target_pos, self.len);
    }

    // --- Undo & Snapshot Management ---

    fn save_undo_snapshot(&mut self) {
        self.undo_buffer[..self.len].copy_from_slice(&self.buffer[..self.len]);
        self.undo_len = self.len;
        self.undo_cursor = self.cursor_pos;
        self.has_undo = true;
    }

    pub fn undo(&mut self) -> bool {
        if !self.has_undo {
            return false;
        }
        self.buffer[..self.undo_len].copy_from_slice(&self.undo_buffer[..self.undo_len]);
        self.len = self.undo_len;
        self.cursor_pos = self.undo_cursor;
        self.selection = None;
        self.has_undo = false;
        self.is_dirty = true;
        true
    }

    // --- Editing Operations ---

    pub fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection {
            if start < end && end <= self.len {
                self.save_undo_snapshot();
                let span = end - start;
                for i in end..self.len {
                    self.buffer[i - span] = self.buffer[i];
                }
                self.len -= span;
                self.cursor_pos = start;
                self.selection = None;
                self.is_dirty = true;
                return true;
            }
        }
        false
    }

    pub fn insert_char(&mut self, ch: u8) -> bool {
        self.delete_selection();
        if self.len >= MAX_EDITOR_BYTES {
            return false;
        }
        self.save_undo_snapshot();
        for i in (self.cursor_pos..self.len).rev() {
            self.buffer[i + 1] = self.buffer[i];
        }
        self.buffer[self.cursor_pos] = ch;
        self.len += 1;
        self.cursor_pos += 1;
        self.is_dirty = true;
        self.selection = None;
        true
    }

    pub fn insert_bytes(&mut self, bytes: &[u8]) -> bool {
        self.delete_selection();
        if self.len + bytes.len() > MAX_EDITOR_BYTES {
            return false;
        }
        self.save_undo_snapshot();
        let span = bytes.len();
        for i in (self.cursor_pos..self.len).rev() {
            self.buffer[i + span] = self.buffer[i];
        }
        self.buffer[self.cursor_pos..self.cursor_pos + span].copy_from_slice(bytes);
        self.len += span;
        self.cursor_pos += span;
        self.is_dirty = true;
        self.selection = None;
        true
    }

    pub fn backspace(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor_pos == 0 {
            return false;
        }
        self.save_undo_snapshot();
        let prev = self.prev_char_boundary(self.cursor_pos);
        let span = self.cursor_pos - prev;
        for i in self.cursor_pos..self.len {
            self.buffer[i - span] = self.buffer[i];
        }
        self.len -= span;
        self.cursor_pos = prev;
        self.is_dirty = true;
        true
    }

    pub fn delete_forward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor_pos >= self.len {
            return false;
        }
        self.save_undo_snapshot();
        let next = self.next_char_boundary(self.cursor_pos);
        let span = next - self.cursor_pos;
        for i in next..self.len {
            self.buffer[i - span] = self.buffer[i];
        }
        self.len -= span;
        self.is_dirty = true;
        true
    }

    pub fn select_all(&mut self) {
        if self.len > 0 {
            self.selection = Some((0, self.len));
            self.cursor_pos = self.len;
        }
    }

    pub fn set_selection_range(&mut self, start: usize, end: usize) {
        let s = core::cmp::min(start, end);
        let e = core::cmp::max(start, end);
        let s_clamped = core::cmp::min(s, self.len);
        let e_clamped = core::cmp::min(e, self.len);
        if s_clamped < e_clamped {
            self.selection = Some((s_clamped, e_clamped));
        } else {
            self.selection = None;
        }
    }

    /// Automatically adjusts scroll_line so that the cursor is visible within visible_rows
    pub fn ensure_cursor_visible(&mut self, visible_rows: usize) {
        let (c_line, _) = self.cursor_line_and_col();
        if c_line < self.scroll_line {
            self.scroll_line = c_line;
        } else if c_line >= self.scroll_line + visible_rows {
            self.scroll_line = c_line - visible_rows + 1;
        }
    }
}

impl WindowApp for TextEditorApp {
    fn app_id(&self) -> &'static str {
        "text_editor"
    }

    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn title(&self) -> &str {
        self.filename_str()
    }

    fn subtitle(&self) -> &str {
        if self.is_dirty {
            "Plain Text Document * Unsaved Changes (*)"
        } else {
            "Plain Text Document * INOS-FS * UTF-8"
        }
    }

    fn get_content_bytes(&self) -> &[u8] {
        &self.buffer[..self.len]
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        clipboard: &mut [u8; CLIPBOARD_SIZE],
        clip_len: &mut usize,
    ) -> AppCommand {
        match event {
            AppEvent::InputChar(ch) => {
                if self.insert_char(ch) {
                    AppCommand::RequestRedraw
                } else {
                    AppCommand::None
                }
            }
            AppEvent::Key(code) => match code {
                KeyCode::Left => {
                    self.cursor_pos = self.prev_char_boundary(self.cursor_pos);
                    self.selection = None;
                    AppCommand::RequestRedraw
                }
                KeyCode::Right => {
                    self.cursor_pos = self.next_char_boundary(self.cursor_pos);
                    self.selection = None;
                    AppCommand::RequestRedraw
                }
                KeyCode::Home => {
                    let (_, col) = self.cursor_line_and_col();
                    self.cursor_pos -= col;
                    self.selection = None;
                    AppCommand::RequestRedraw
                }
                KeyCode::End => {
                    while self.cursor_pos < self.len && self.buffer[self.cursor_pos] != b'\n' {
                        self.cursor_pos += 1;
                    }
                    self.selection = None;
                    AppCommand::RequestRedraw
                }
                KeyCode::Up => {
                    let (line, col) = self.cursor_line_and_col();
                    if line > 0 {
                        self.move_to_line_and_col(line - 1, col);
                    } else {
                        self.cursor_pos = 0;
                    }
                    self.selection = None;
                    AppCommand::RequestRedraw
                }
                KeyCode::Down => {
                    let (line, col) = self.cursor_line_and_col();
                    let mut starts = [0usize; MAX_LINES];
                    let total = self.line_starts(&mut starts);
                    if line + 1 < total {
                        self.move_to_line_and_col(line + 1, col);
                    } else {
                        self.cursor_pos = self.len;
                    }
                    self.selection = None;
                    AppCommand::RequestRedraw
                }
                KeyCode::Backspace => {
                    if self.backspace() {
                        AppCommand::RequestRedraw
                    } else {
                        AppCommand::None
                    }
                }
                KeyCode::Delete => {
                    if self.delete_forward() {
                        AppCommand::RequestRedraw
                    } else {
                        AppCommand::None
                    }
                }
                KeyCode::Enter => {
                    if self.insert_char(b'\n') {
                        AppCommand::RequestRedraw
                    } else {
                        AppCommand::None
                    }
                }
                KeyCode::Tab => {
                    // 4 spaces for indentation
                    if self.insert_bytes(b"    ") {
                        AppCommand::RequestRedraw
                    } else {
                        AppCommand::None
                    }
                }
                KeyCode::Ctrl(b'c') | KeyCode::Ctrl(b'C') => {
                    // Copy selection to native in-guest clipboard
                    if let Some((s, e)) = self.selection {
                        let span = e - s;
                        let l = core::cmp::min(span, CLIPBOARD_SIZE);
                        clipboard[..l].copy_from_slice(&self.buffer[s..s + l]);
                        *clip_len = l;
                        AppCommand::SetClipboard { len: l }
                    } else {
                        AppCommand::None
                    }
                }
                KeyCode::Ctrl(b'x') | KeyCode::Ctrl(b'X') => {
                    // Cut selection to native in-guest clipboard
                    if let Some((s, e)) = self.selection {
                        let span = e - s;
                        let l = core::cmp::min(span, CLIPBOARD_SIZE);
                        clipboard[..l].copy_from_slice(&self.buffer[s..s + l]);
                        *clip_len = l;
                        self.delete_selection();
                        AppCommand::RequestRedraw
                    } else {
                        AppCommand::None
                    }
                }
                KeyCode::Ctrl(b'v') | KeyCode::Ctrl(b'V') => {
                    // Paste from native in-guest clipboard
                    if *clip_len > 0 {
                        self.insert_bytes(&clipboard[..*clip_len]);
                        AppCommand::RequestRedraw
                    } else {
                        AppCommand::None
                    }
                }
                KeyCode::Ctrl(b's') | KeyCode::Ctrl(b'S') => {
                    AppCommand::SaveFile
                }
                KeyCode::Ctrl(b'z') | KeyCode::Ctrl(b'Z') => {
                    if self.undo() {
                        AppCommand::RequestRedraw
                    } else {
                        AppCommand::None
                    }
                }
                KeyCode::Ctrl(b'a') | KeyCode::Ctrl(b'A') => {
                    self.select_all();
                    AppCommand::RequestRedraw
                }
                _ => AppCommand::None,
            },
            AppEvent::Scroll { delta_lines } => {
                if delta_lines < 0 && self.scroll_line >= (-delta_lines) as usize {
                    self.scroll_line -= (-delta_lines) as usize;
                } else if delta_lines > 0 {
                    self.scroll_line += delta_lines as usize;
                }
                AppCommand::RequestRedraw
            }
            AppEvent::RequestSave => AppCommand::SaveFile,
            AppEvent::ClipboardCopy => {
                if let Some((s, e)) = self.selection {
                    let span = e - s;
                    let l = core::cmp::min(span, CLIPBOARD_SIZE);
                    clipboard[..l].copy_from_slice(&self.buffer[s..s + l]);
                    *clip_len = l;
                    AppCommand::SetClipboard { len: l }
                } else {
                    AppCommand::None
                }
            }
            AppEvent::ClipboardPaste => {
                if *clip_len > 0 {
                    self.insert_bytes(&clipboard[..*clip_len]);
                    AppCommand::RequestRedraw
                } else {
                    AppCommand::None
                }
            }
            _ => AppCommand::None,
        }
    }

    unsafe fn render_surface(
        &self,
        gop: &mut GopBuffer,
        content_rect: Rect,
        is_focused: bool,
    ) {
        let x = content_rect.x;
        let y = content_rect.y;
        let w = content_rect.width;
        let h = content_rect.height;

        // 1. Surface background: Subdued Ivory (#B5AEA0) with smooth rounded corners
        gop.draw_rounded_rect_alpha(content_rect, 8, Color::DOC_SURFACE, 255);
        gop.draw_rounded_rect_outline(content_rect, 8, Color::BORDER_INACTIVE);

        let pad_x = x + 16;
        let pad_y = y + 16;
        let usable_w = w.saturating_sub(36);
        let usable_h = h.saturating_sub(32);
        let visible_rows = (usable_h as usize) / (LINE_HEIGHT_PX as usize);

        let mut starts = [0usize; MAX_LINES];
        let total_lines = self.line_starts(&mut starts);

        let (c_line, c_col) = self.cursor_line_and_col();
        let clip_rect = Rect::new(pad_x, pad_y, usable_w, usable_h);

        // 2. Render Lines in Visible Viewport
        for row in 0..visible_rows {
            let line_idx = self.scroll_line + row;
            if line_idx >= total_lines {
                break;
            }
            let l_start = starts[line_idx];
            let l_end = if line_idx + 1 < total_lines {
                starts[line_idx + 1] - 1 // exclude \n
            } else {
                self.len
            };

            let row_y = pad_y + (row as i32) * LINE_HEIGHT_PX;

            // Render Selection Highlight for this line if applicable
            if let Some((sel_s, sel_e)) = self.selection {
                if sel_e > l_start && sel_s < l_end {
                    let hl_start = core::cmp::max(sel_s, l_start) - l_start;
                    let hl_end = core::cmp::min(sel_e, l_end) - l_start;
                    let prefix_text = core::str::from_utf8(&self.buffer[l_start..l_start + hl_start]).unwrap_or("");
                    let sel_text = core::str::from_utf8(&self.buffer[l_start + hl_start..l_start + hl_end]).unwrap_or("");
                    let prefix_w = GopBuffer::measure_text_font(prefix_text, FontFace::Serif);
                    let sel_w = GopBuffer::measure_text_font(sel_text, FontFace::Serif);
                    let start_px = pad_x + prefix_w as i32;
                    let span_px = sel_w as i32;
                    gop.fill_rect(
                        Rect::new(start_px, row_y, span_px as u32, LINE_HEIGHT_PX as u32),
                        Color { r: 137, g: 150, b: 110, a: 120 }, // Muted Sage selection tint
                    );
                }
            }

            // Render Text of this Line
            if l_end > l_start {
                let line_str = core::str::from_utf8(&self.buffer[l_start..l_end]).unwrap_or("");
                gop.draw_text_font_clipped(
                    pad_x as usize,
                    row_y as usize,
                    line_str,
                    Color::DOC_TEXT,
                    FontFace::Serif,
                    clip_rect,
                );
            }

            // Render Caret if cursor is on this line and window is focused
            if is_focused && line_idx == c_line {
                let prefix = core::str::from_utf8(&self.buffer[l_start..self.cursor_pos]).unwrap_or("");
                let caret_w = GopBuffer::measure_text_font(prefix, FontFace::Serif);
                let caret_x = pad_x + caret_w as i32;
                gop.fill_rect(
                    Rect::new(caret_x, row_y, 2, 16),
                    Color::ACCENT_WALNUT, // Walnut Caret
                );
            }
        }

        // 3. Render Vertical Scrollbar if Document Exceeds Viewport
        if total_lines > visible_rows {
            let sb_x = x + w as i32 - 12;
            let sb_y = pad_y;
            let sb_h = usable_h;
            // Track
            gop.fill_rect(Rect::new(sb_x, sb_y, 4, sb_h), Color { r: 140, g: 135, b: 124, a: 100 });
            // Thumb
            let thumb_h = core::cmp::max(20, (visible_rows as u32 * sb_h) / total_lines as u32);
            let thumb_y = sb_y + ((self.scroll_line as u32 * (sb_h - thumb_h)) / (total_lines - visible_rows) as u32) as i32;
            gop.draw_rounded_rect_alpha(
                Rect::new(sb_x - 1, thumb_y, 6, thumb_h),
                3,
                Color::WINDOW_FRAME,
                220,
            );
        }
    }
}
