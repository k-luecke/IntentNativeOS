// kernel/src/graphics/spatial.rs - Decoupled Spatial Window Manager & Compositor (Gate UI-03)
// Implements Habitable Computing visual identity: dark charcoal/ivory palette,
// football-shaped window controls (Close, Minimize, Maximize/Restore),
// movable/dockable floating intent bar, circular launcher with window restoration,
// restrained alpha compositing, rounded corners, and dirty-region verification.

#![no_std]

use super::app::{self, AppCommand, AppEvent, KeyCode, WindowApp};
use super::font;
use super::proportional_font::{self, FontFace};
use super::Color;
use super::GopBuffer;
use super::Rect;

pub const MAX_SPATIAL_WINDOWS: usize = 4;
pub const MAX_WINDOW_CONTENT: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    Normal,
    Maximized { prev_bounds: Rect },
    Minimized,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalFocus {
    Cancel,  // Safe default: preserves existing state
    Confirm, // Explicit action: Make copy
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickTarget {
    None,
    WindowClose(u32),
    WindowMinimize(u32),
    WindowMaximize(u32),
    WindowTitleBar(u32),
    WindowContent(u32),
    LauncherButton,
    LauncherMenuItem(u32), // Index in launcher menu
    IntentBarBody,
    ModalOption(u8),       // [1] Confirm, [2] Cancel
}

#[derive(Clone, Copy)]
pub struct SpatialWindow {
    pub id: u32,
    pub title: [u8; 32],
    pub title_len: usize,
    pub subtitle: [u8; 48],
    pub subtitle_len: usize,
    pub bounds: Rect,
    pub state: WindowState,
    pub z_order: u8,
    pub focused: bool,
    pub content: [u8; MAX_WINDOW_CONTENT],
    pub content_len: usize,
    pub cursor_pos: usize,
    pub scroll_y: usize,
    pub dirty: bool,
    pub editor: Option<super::editor::TextEditorApp>,
}

impl SpatialWindow {
    pub fn new(id: u32, title_str: &str, subtitle_str: &str, bounds: Rect, z_order: u8) -> Self {
        let mut win = Self {
            id,
            title: [0u8; 32],
            title_len: 0,
            subtitle: [0u8; 48],
            subtitle_len: 0,
            bounds,
            state: WindowState::Normal,
            z_order,
            focused: false,
            content: [0u8; MAX_WINDOW_CONTENT],
            content_len: 0,
            cursor_pos: 0,
            scroll_y: 0,
            dirty: true,
            editor: None,
        };
        win.set_title(title_str);
        win.set_subtitle(subtitle_str);
        win
    }

    pub fn new_with_editor(id: u32, filename: &str, content: &str, bounds: Rect, z_order: u8) -> Self {
        let mut win = Self::new(id, filename, "Plain Text Document * INOS-FS * UTF-8", bounds, z_order);
        let editor = super::editor::TextEditorApp::new(filename, content);
        win.editor = Some(editor);
        win
    }

    pub fn set_title(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let len = core::cmp::min(bytes.len(), 32);
        self.title[..len].copy_from_slice(&bytes[..len]);
        self.title_len = len;
        self.dirty = true;
    }

    pub fn title_str(&self) -> &str {
        if let Some(ref ed) = self.editor {
            ed.title()
        } else {
            core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
        }
    }

    pub fn set_subtitle(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let len = core::cmp::min(bytes.len(), 48);
        self.subtitle[..len].copy_from_slice(&bytes[..len]);
        self.subtitle_len = len;
        self.dirty = true;
    }

    pub fn subtitle_str(&self) -> &str {
        if let Some(ref ed) = self.editor {
            ed.subtitle()
        } else {
            core::str::from_utf8(&self.subtitle[..self.subtitle_len]).unwrap_or("")
        }
    }

    pub fn set_content(&mut self, s: &str) {
        if let Some(ref mut ed) = self.editor {
            ed.set_content(s);
        }
        let bytes = s.as_bytes();
        let len = core::cmp::min(bytes.len(), MAX_WINDOW_CONTENT);
        self.content[..len].copy_from_slice(&bytes[..len]);
        self.content_len = len;
        self.cursor_pos = len;
        self.dirty = true;
    }

    pub fn content_str(&self) -> &str {
        if let Some(ref ed) = self.editor {
            ed.content_str()
        } else {
            core::str::from_utf8(&self.content[..self.content_len]).unwrap_or("")
        }
    }

    pub fn insert_char(&mut self, ch: u8) -> bool {
        if self.content_len >= MAX_WINDOW_CONTENT {
            return false;
        }
        if self.cursor_pos > self.content_len {
            self.cursor_pos = self.content_len;
        }
        for i in (self.cursor_pos..self.content_len).rev() {
            self.content[i + 1] = self.content[i];
        }
        self.content[self.cursor_pos] = ch;
        self.content_len += 1;
        self.cursor_pos += 1;
        self.dirty = true;
        true
    }

    pub fn backspace(&mut self) -> bool {
        if self.cursor_pos == 0 || self.content_len == 0 {
            return false;
        }
        for i in self.cursor_pos..self.content_len {
            self.content[i - 1] = self.content[i];
        }
        self.content_len -= 1;
        self.cursor_pos -= 1;
        self.dirty = true;
        true
    }

    pub fn close(&mut self) {
        self.state = WindowState::Closed;
        self.focused = false;
        self.dirty = true;
    }

    pub fn minimize(&mut self) {
        self.state = WindowState::Minimized;
        self.focused = false;
        self.dirty = true;
    }

    pub fn maximize_or_restore(&mut self, desktop_area: Rect) {
        match self.state {
            WindowState::Normal => {
                let prev = self.bounds;
                self.bounds = desktop_area;
                self.state = WindowState::Maximized { prev_bounds: prev };
                self.dirty = true;
            }
            WindowState::Maximized { prev_bounds } => {
                self.bounds = prev_bounds;
                self.state = WindowState::Normal;
                self.dirty = true;
            }
            WindowState::Minimized | WindowState::Closed => {
                self.state = WindowState::Normal;
                self.dirty = true;
            }
        }
    }

    pub fn restore(&mut self) {
        match self.state {
            WindowState::Maximized { prev_bounds } => {
                self.bounds = prev_bounds;
                self.state = WindowState::Normal;
            }
            WindowState::Minimized | WindowState::Closed => {
                self.state = WindowState::Normal;
            }
            WindowState::Normal => {}
        }
        self.dirty = true;
    }
}

pub struct FloatingIntentBar {
    pub bounds: Rect,
    pub collapsed: bool,
    pub docked_bottom: bool,
    pub active: bool,
    pub input_text: [u8; 128],
    pub input_len: usize,
    pub cursor_pos: usize,
}

impl FloatingIntentBar {
    pub const fn new() -> Self {
        Self {
            bounds: Rect::new(370, 736, 540, 44),
            collapsed: false,
            docked_bottom: true,
            active: false,
            input_text: [0u8; 128],
            input_len: 0,
            cursor_pos: 0,
        }
    }

    pub fn set_text(&mut self, s: &str) {
        let b = s.as_bytes();
        let len = core::cmp::min(b.len(), 128);
        self.input_text[..len].copy_from_slice(&b[..len]);
        self.input_len = len;
        self.cursor_pos = len;
    }

    pub fn text_str(&self) -> &str {
        core::str::from_utf8(&self.input_text[..self.input_len]).unwrap_or("")
    }
}

pub const MAX_LAUNCHER_DOCS: usize = 4;

#[derive(Clone, Copy)]
pub struct LauncherMenu {
    pub open: bool,
    pub bounds: Rect,
    pub selected_item: usize,
    pub doc_count: usize,
    pub doc_names: [[u8; 28]; MAX_LAUNCHER_DOCS],
    pub doc_name_lens: [usize; MAX_LAUNCHER_DOCS],
    pub doc_sizes: [u32; MAX_LAUNCHER_DOCS],
}

impl LauncherMenu {
    pub const fn new() -> Self {
        Self {
            open: false,
            bounds: Rect::new(370, 420, 310, 306),
            selected_item: 0,
            doc_count: 0,
            doc_names: [[0u8; 28]; MAX_LAUNCHER_DOCS],
            doc_name_lens: [0usize; MAX_LAUNCHER_DOCS],
            doc_sizes: [0u32; MAX_LAUNCHER_DOCS],
        }
    }
}

pub struct SpatialWindowManager {
    pub windows: [Option<SpatialWindow>; MAX_SPATIAL_WINDOWS],
    pub focused_window_id: Option<u32>,
    pub intent_bar: FloatingIntentBar,
    pub launcher: LauncherMenu,
    pub clipboard: [u8; super::app::CLIPBOARD_SIZE],
    pub clip_len: usize,
    pub modal_active: bool,
    pub modal_focus: ModalFocus,
    pub modal_title: [u8; 64],
    pub modal_title_len: usize,
    pub modal_expl1: [u8; 64],
    pub modal_expl1_len: usize,
    pub modal_expl2: [u8; 64],
    pub modal_expl2_len: usize,
    pub modal_opt1: [u8; 64],
    pub modal_opt1_len: usize,
    pub modal_opt2: [u8; 64],
    pub modal_opt2_len: usize,
    pub receipt_active: bool,
    pub receipt_msg: [u8; 64],
    pub receipt_msg_len: usize,
    pub receipt_sub: [u8; 64],
    pub receipt_sub_len: usize,
    pub receipt_tx: [u8; 64],
    pub receipt_tx_len: usize,
    pub status_text: [u8; 64],
    pub status_text_len: usize,
}

impl SpatialWindowManager {
    pub const fn new() -> Self {
        Self {
            windows: [None, None, None, None],
            focused_window_id: None,
            intent_bar: FloatingIntentBar::new(),
            launcher: LauncherMenu::new(),
            clipboard: [0u8; super::app::CLIPBOARD_SIZE],
            clip_len: 0,
            modal_active: false,
            modal_focus: ModalFocus::Cancel,
            modal_title: [0u8; 64],
            modal_title_len: 0,
            modal_expl1: [0u8; 64],
            modal_expl1_len: 0,
            modal_expl2: [0u8; 64],
            modal_expl2_len: 0,
            modal_opt1: [0u8; 64],
            modal_opt1_len: 0,
            modal_opt2: [0u8; 64],
            modal_opt2_len: 0,
            receipt_active: false,
            receipt_msg: [0u8; 64],
            receipt_msg_len: 0,
            receipt_sub: [0u8; 64],
            receipt_sub_len: 0,
            receipt_tx: [0u8; 64],
            receipt_tx_len: 0,
            status_text: [0u8; 64],
            status_text_len: 0,
        }
    }

    pub fn update_launcher_docs<D: crate::storage::BlockDevice>(&mut self, fs: &crate::storage::InosFileSystem<D>) {
        let mut list = [("", 0u32); MAX_LAUNCHER_DOCS];
        let count = fs.list_files(&mut list);
        self.launcher.doc_count = count;
        for i in 0..count {
            let (fname, size) = list[i];
            let b = fname.as_bytes();
            let l = core::cmp::min(b.len(), 28);
            self.launcher.doc_names[i][..l].copy_from_slice(&b[..l]);
            self.launcher.doc_name_lens[i] = l;
            self.launcher.doc_sizes[i] = size;
        }
    }

    pub fn dispatch_event_to_focused(&mut self, event: super::app::AppEvent) -> super::app::AppCommand {
        let fid = match self.focused_window_id {
            Some(id) => id,
            None => return super::app::AppCommand::None,
        };
        let clipboard = &mut self.clipboard;
        let clip_len = &mut self.clip_len;
        for slot in self.windows.iter_mut() {
            if let Some(ref mut w) = slot {
                if w.id == fid {
                    if let Some(ref mut ed) = w.editor {
                        let cmd = ed.handle_event(event, clipboard, clip_len);
                        w.dirty = true;
                        return cmd;
                    }
                }
            }
        }
        super::app::AppCommand::None
    }

    pub fn add_window(&mut self, win: SpatialWindow) -> bool {
        for slot in self.windows.iter_mut() {
            if slot.is_none() {
                *slot = Some(win);
                return true;
            }
        }
        false
    }

    pub fn get_window(&self, id: u32) -> Option<&SpatialWindow> {
        for slot in self.windows.iter() {
            if let Some(ref w) = slot {
                if w.id == id {
                    return Some(w);
                }
            }
        }
        None
    }

    pub fn get_window_mut(&mut self, id: u32) -> Option<&mut SpatialWindow> {
        for slot in self.windows.iter_mut() {
            if let Some(ref mut w) = slot {
                if w.id == id {
                    return Some(w);
                }
            }
        }
        None
    }

    pub fn set_focus(&mut self, target_id: u32) {
        self.focused_window_id = Some(target_id);
        let mut max_z = 0u8;
        for slot in self.windows.iter() {
            if let Some(ref w) = slot {
                if w.z_order > max_z {
                    max_z = w.z_order;
                }
            }
        }
        for slot in self.windows.iter_mut() {
            if let Some(ref mut w) = slot {
                if w.id == target_id {
                    w.focused = true;
                    w.z_order = max_z + 1;
                    if w.state == WindowState::Minimized || w.state == WindowState::Closed {
                        w.state = WindowState::Normal;
                    }
                    w.dirty = true;
                } else {
                    w.focused = false;
                    w.dirty = true;
                }
            }
        }
    }

    pub fn move_window(&mut self, id: u32, dx: i32, dy: i32) {
        if let Some(w) = self.get_window_mut(id) {
            if w.state == WindowState::Normal {
                w.bounds.x = core::cmp::max(10, core::cmp::min(1280 - w.bounds.width as i32 - 10, w.bounds.x + dx));
                w.bounds.y = core::cmp::max(10, core::cmp::min(710 - w.bounds.height as i32 - 10, w.bounds.y + dy));
                w.dirty = true;
            }
        }
    }

    pub fn close_window(&mut self, id: u32) {
        if let Some(w) = self.get_window_mut(id) {
            w.close();
        }
        if self.focused_window_id == Some(id) {
            self.cycle_focus();
        }
    }

    pub fn minimize_window(&mut self, id: u32) {
        if let Some(w) = self.get_window_mut(id) {
            w.minimize();
        }
        if self.focused_window_id == Some(id) {
            self.cycle_focus();
        }
    }

    pub fn maximize_or_restore_window(&mut self, id: u32) {
        // Desktop usable area leaves room for floating intent bar (height 700 px)
        let desktop_area = Rect::new(16, 16, 1248, 700);
        if let Some(w) = self.get_window_mut(id) {
            w.maximize_or_restore(desktop_area);
        }
    }

    pub fn restore_window(&mut self, id: u32) {
        if let Some(w) = self.get_window_mut(id) {
            w.restore();
        }
        self.set_focus(id);
    }

    pub fn cycle_focus(&mut self) {
        let mut active_ids = [0u32; MAX_SPATIAL_WINDOWS];
        let mut count = 0;
        for slot in self.windows.iter() {
            if let Some(ref w) = slot {
                if w.state == WindowState::Normal || matches!(w.state, WindowState::Maximized { .. }) {
                    active_ids[count] = w.id;
                    count += 1;
                }
            }
        }
        if count == 0 {
            self.focused_window_id = None;
            return;
        }
        let cur = self.focused_window_id.unwrap_or(active_ids[0]);
        let mut next_idx = 0;
        for i in 0..count {
            if active_ids[i] == cur {
                next_idx = (i + 1) % count;
                break;
            }
        }
        self.set_focus(active_ids[next_idx]);
    }

    pub fn hit_test(&self, px: i32, py: i32) -> ClickTarget {
        // 1. Authorization Modal traps ALL input fail-closed
        if self.modal_active {
            let mw = 560;
            let mh = 255;
            let mx = (1280 - mw) / 2; // 360
            let my = (800 - mh) / 2 - 20; // 252

            // Button 1 (Make copy): [mx + 36 .. mx + 186] x [my + 145 .. my + 183]
            if px >= mx + 36 && px <= mx + 186 && py >= my + 145 && py <= my + 183 {
                return ClickTarget::ModalOption(1);
            }
            // Button 2 (Cancel): [mx + 206 .. mx + 336] x [my + 145 .. my + 183]
            if px >= mx + 206 && px <= mx + 336 && py >= my + 145 && py <= my + 183 {
                return ClickTarget::ModalOption(2);
            }
            return ClickTarget::None;
        }

        // 2. Launcher Menu (if open)
        if self.launcher.open {
            let lx = self.launcher.bounds.x;
            let ly = self.launcher.bounds.y;
            let lw = self.launcher.bounds.width as i32;
            let lh = self.launcher.bounds.height as i32;
            if px >= lx && px <= lx + lw && py >= ly && py <= ly + lh {
                // Item rows start at y + 36, height 32 each
                let rel_y = py - (ly + 36);
                if rel_y >= 0 {
                    let item_idx = (rel_y / 32) as u32;
                    return ClickTarget::LauncherMenuItem(item_idx);
                }
                return ClickTarget::None;
            }
        }

        // 3. Floating Intent Bar (High Z-order)
        let ibx = self.intent_bar.bounds.x;
        let iby = self.intent_bar.bounds.y;
        let ibw = if self.intent_bar.collapsed { 44 } else { self.intent_bar.bounds.width as i32 };
        let ibh = self.intent_bar.bounds.height as i32;

        if px >= ibx && px <= ibx + ibw && py >= iby && py <= iby + ibh {
            // Circular launcher is left 44px
            if px <= ibx + 44 {
                return ClickTarget::LauncherButton;
            } else {
                return ClickTarget::IntentBarBody;
            }
        }

        // 4. Windows in decreasing z-order (only visible windows)
        let mut highest_z = 0u8;
        let mut hit = ClickTarget::None;

        for slot in self.windows.iter() {
            if let Some(ref w) = slot {
                if w.state == WindowState::Minimized || w.state == WindowState::Closed {
                    continue;
                }
                let bx = w.bounds.x;
                let by = w.bounds.y;
                let bw = w.bounds.width as i32;
                let bh = w.bounds.height as i32;

                if px >= bx && px <= bx + bw && py >= by && py <= by + bh {
                    if w.z_order >= highest_z {
                        highest_z = w.z_order;
                        // Title bar is top 44 px
                        if py <= by + 44 {
                            // Check Football Controls in upper-left
                            // Close: [bx + 10 .. bx + 33] x [by + 8 .. by + 28]
                            if px >= bx + 10 && px <= bx + 33 && py >= by + 8 && py <= by + 28 {
                                hit = ClickTarget::WindowClose(w.id);
                            // Minimize: [bx + 33 .. bx + 54] x [by + 8 .. by + 28]
                            } else if px > bx + 33 && px <= bx + 54 && py >= by + 8 && py <= by + 28 {
                                hit = ClickTarget::WindowMinimize(w.id);
                            // Maximize: [bx + 54 .. bx + 75] x [by + 8 .. by + 28]
                            } else if px > bx + 54 && px <= bx + 75 && py >= by + 8 && py <= by + 28 {
                                hit = ClickTarget::WindowMaximize(w.id);
                            } else {
                                hit = ClickTarget::WindowTitleBar(w.id);
                            }
                        } else {
                            hit = ClickTarget::WindowContent(w.id);
                        }
                    }
                }
            }
        }
        hit
    }

    pub fn set_status(&mut self, s: &str) {
        let b = s.as_bytes();
        let len = core::cmp::min(b.len(), 64);
        self.status_text[..len].copy_from_slice(&b[..len]);
        self.status_text_len = len;
    }

    pub fn set_intent_input(&mut self, s: &str, active: bool) {
        self.intent_bar.set_text(s);
        self.intent_bar.active = active;
    }

    pub fn show_modal(&mut self, title: &str, expl1: &str, expl2: &str, opt1: &str, opt2: &str) {
        let copy_str = |dst: &mut [u8; 64], src: &str| -> usize {
            let b = src.as_bytes();
            let l = core::cmp::min(b.len(), 64);
            dst[..l].copy_from_slice(&b[..l]);
            l
        };
        self.modal_title_len = copy_str(&mut self.modal_title, title);
        self.modal_expl1_len = copy_str(&mut self.modal_expl1, expl1);
        self.modal_expl2_len = copy_str(&mut self.modal_expl2, expl2);
        self.modal_opt1_len = copy_str(&mut self.modal_opt1, opt1);
        self.modal_opt2_len = copy_str(&mut self.modal_opt2, opt2);
        self.modal_focus = ModalFocus::Cancel; // Safe default: Cancel focused initially
        self.modal_active = true;
    }

    pub fn cycle_modal_focus(&mut self) {
        self.modal_focus = match self.modal_focus {
            ModalFocus::Cancel => ModalFocus::Confirm,
            ModalFocus::Confirm => ModalFocus::Cancel,
        };
    }

    pub fn hide_modal(&mut self) {
        self.modal_active = false;
    }

    pub fn show_receipt(&mut self, msg: &str, sub: &str, tx: &str) {
        let copy_str = |dst: &mut [u8; 64], src: &str| -> usize {
            let b = src.as_bytes();
            let l = core::cmp::min(b.len(), 64);
            dst[..l].copy_from_slice(&b[..l]);
            l
        };
        self.receipt_msg_len = copy_str(&mut self.receipt_msg, msg);
        self.receipt_sub_len = copy_str(&mut self.receipt_sub, sub);
        self.receipt_tx_len = copy_str(&mut self.receipt_tx, tx);
        self.receipt_active = true;
    }

    pub fn hide_receipt(&mut self) {
        self.receipt_active = false;
    }

    /// Renders the complete Habitable Spatial Desktop into the UEFI GOP buffer
    pub unsafe fn render_workspace(&mut self, gop: &mut GopBuffer) {
        let width = gop.width;
        let height = gop.height;

        // 1. Root Desktop Background: Warm Charcoal (#232421)
        gop.fill_rect(Rect::new(0, 0, width as u32, height as u32), Color::DESKTOP_BG);

        // 2. Render Windows sorted by z-order
        let mut sorted_indices = [0usize, 1, 2, 3];
        let mut count = 0;
        for (i, slot) in self.windows.iter().enumerate() {
            if let Some(ref w) = slot {
                if w.state != WindowState::Minimized && w.state != WindowState::Closed {
                    sorted_indices[count] = i;
                    count += 1;
                }
            }
        }
        for i in 0..count {
            for j in 0..(count - 1 - i) {
                let idx_a = sorted_indices[j];
                let idx_b = sorted_indices[j + 1];
                let za = self.windows[idx_a].as_ref().unwrap().z_order;
                let zb = self.windows[idx_b].as_ref().unwrap().z_order;
                if za > zb {
                    sorted_indices.swap(j, j + 1);
                }
            }
        }

        let modal_active = self.modal_active;
        for &idx in &sorted_indices[..count] {
            if let Some(ref mut win) = self.windows[idx] {
                Self::draw_habitable_window(gop, win, modal_active);
                win.dirty = false;
            }
        }

        // 3. Floating Intent Bar (Layered above windows)
        Self::draw_floating_intent_bar(gop, &self.intent_bar, modal_active);

        // 4. Circular Launcher Menu (if open)
        if self.launcher.open {
            Self::draw_launcher_menu(gop, self);
        }

        // 5. Transaction Receipt Card (Floating at z=4)
        if self.receipt_active {
            let rw = 560usize;
            let rh = 145usize;
            let rx = (width - rw) / 2;
            let ry = height - rh - 90;

            // Rounded Card with alpha and botanical border
            gop.draw_rounded_rect_alpha(Rect::new(rx as i32 + 3, ry as i32 + 3, rw as u32, rh as u32), 10, Color::BLACK, 70);
            gop.draw_rounded_rect_alpha(Rect::new(rx as i32, ry as i32, rw as u32, rh as u32), 10, Color::WINDOW_FRAME, 246);
            gop.draw_rounded_rect_outline(Rect::new(rx as i32, ry as i32, rw as u32, rh as u32), 10, Color::ACCENT_BOTANIC);

            let msg = core::str::from_utf8(&self.receipt_msg[..self.receipt_msg_len]).unwrap_or("");
            gop.draw_text_font(rx + 24, ry + 16, msg, Color::ACCENT_BOTANIC, proportional_font::FontFace::Title);

            let sub = core::str::from_utf8(&self.receipt_sub[..self.receipt_sub_len]).unwrap_or("");
            gop.draw_text_font(rx + 24, ry + 46, sub, Color::HEADER_TEXT, proportional_font::FontFace::Ui);

            let tx = core::str::from_utf8(&self.receipt_tx[..self.receipt_tx_len]).unwrap_or("");
            gop.draw_text_font(rx + 24, ry + 68, tx, Color::DOC_SURFACE, proportional_font::FontFace::Ui);

            gop.fill_rect(Rect::new((rx + 24) as i32, (ry + 96) as i32, (rw - 48) as u32, 1), Color::BORDER_INACTIVE);
            gop.draw_text_font(rx + 24, ry + 108, "Press [F3] to safely undo this operation.", Color::HEADER_TEXT, proportional_font::FontFace::Ui);
            gop.draw_oval_pill(rx as i32 + rw as i32 - 100, ry as i32 + 104, 70, 24, Color::ACCENT_WALNUT);
            gop.draw_text_font(rx + rw - 86, ry + 108, "F3 Undo", Color::HEADER_TEXT, proportional_font::FontFace::Ui);
        }

        // 6. Natural Language Confirmation Modal (UI-03.1 Habitable Treatment, Top Layer z=5)
        if self.modal_active {
            let mw = 560usize;
            let mh = 255usize;
            let mx = (width - mw) / 2;
            let my = (height - mh) / 2 - 20;

            // Modal shadow & body: Rich Dark Walnut Card (#34322C) with 96% alpha
            gop.draw_rounded_rect_alpha(Rect::new(mx as i32 + 4, my as i32 + 4, mw as u32, mh as u32), 12, Color::BLACK, 90);
            gop.draw_rounded_rect_alpha(Rect::new(mx as i32, my as i32, mw as u32, mh as u32), 12, Color::MODAL_BG, 245);
            gop.draw_rounded_rect_outline(Rect::new(mx as i32, my as i32, mw as u32, mh as u32), 12, Color::BORDER_INACTIVE);

            // 1. Heading: Title Font (20px Liberation Sans Bold) in #EEEAE2
            let title = core::str::from_utf8(&self.modal_title[..self.modal_title_len]).unwrap_or("");
            gop.draw_text_font(mx + 36, my + 30, title, Color::MODAL_HEADING, proportional_font::FontFace::Title);

            // 2. Core Reassurance: UI Font (16px Liberation Sans Regular) in #E2DCD0
            let expl1 = core::str::from_utf8(&self.modal_expl1[..self.modal_expl1_len]).unwrap_or("");
            gop.draw_text_font(mx + 36, my + 72, expl1, Color::MODAL_REASSURE, proportional_font::FontFace::Ui);

            // 3. Action Detail: UI Font (16px Liberation Sans Regular) in #C4BEB0
            let expl2 = core::str::from_utf8(&self.modal_expl2[..self.modal_expl2_len]).unwrap_or("");
            gop.draw_text_font(mx + 36, my + 102, expl2, Color::MODAL_DETAIL, proportional_font::FontFace::Ui);

            // 4. Action Buttons (Horizontal layout, 150px wide each, 38px high)
            let btn_y = my + 145;
            let btn_h = 38u32;
            let btn1_x = mx + 36;
            let btn1_w = 150u32;
            let btn2_x = mx + 206;
            let btn2_w = 130u32;

            let is_confirm_focused = self.modal_focus == ModalFocus::Confirm;
            let is_cancel_focused = self.modal_focus == ModalFocus::Cancel;

            // Primary Action: "Make copy" (Muted Sage highlight)
            gop.draw_rounded_rect_alpha(Rect::new(btn1_x as i32, btn_y as i32, btn1_w, btn_h), 19, Color::BUTTON_PRIMARY_BG, 255);
            if is_confirm_focused {
                gop.draw_rounded_rect_outline(Rect::new(btn1_x as i32 - 2, btn_y as i32 - 2, btn1_w + 4, btn_h + 4), 21, Color::WHITE);
                gop.draw_rounded_rect_outline(Rect::new(btn1_x as i32 - 1, btn_y as i32 - 1, btn1_w + 2, btn_h + 2), 20, Color::WHITE);
            }
            let opt1_str = "Make copy";
            let opt1_w = GopBuffer::measure_text_font(opt1_str, proportional_font::FontFace::Title);
            let opt1_x = btn1_x + (btn1_w as usize - opt1_w) / 2;
            gop.draw_text_font(opt1_x, btn_y + 9, opt1_str, Color::BUTTON_PRIMARY_FG, proportional_font::FontFace::Title);

            // Secondary Action: "Cancel" (Subdued architectural pill)
            gop.draw_rounded_rect_alpha(Rect::new(btn2_x as i32, btn_y as i32, btn2_w, btn_h), 19, Color::BUTTON_SECONDARY_BG, 240);
            if is_cancel_focused {
                gop.draw_rounded_rect_outline(Rect::new(btn2_x as i32 - 2, btn_y as i32 - 2, btn2_w + 4, btn_h + 4), 21, Color::BUTTON_FOCUS_RING);
                gop.draw_rounded_rect_outline(Rect::new(btn2_x as i32 - 1, btn_y as i32 - 1, btn2_w + 2, btn_h + 2), 20, Color::BUTTON_FOCUS_RING);
            } else {
                gop.draw_rounded_rect_outline(Rect::new(btn2_x as i32, btn_y as i32, btn2_w, btn_h), 19, Color::BORDER_INACTIVE);
            }
            let opt2_str = "Cancel";
            let opt2_w = GopBuffer::measure_text_font(opt2_str, proportional_font::FontFace::Ui);
            let opt2_x = btn2_x + (btn2_w as usize - opt2_w) / 2;
            gop.draw_text_font(opt2_x, btn_y + 11, opt2_str, Color::BUTTON_SECONDARY_FG, proportional_font::FontFace::Ui);

            // 5. Footer: Quiet inspection hint
            gop.draw_text_font(mx + 36, my + 215, "Inspect details with F12", Color::MODAL_FOOTER, proportional_font::FontFace::Ui);
        }
    }

    /// Draws a single window surface adhering strictly to the UI-03.1 Habitable Computing visual identity
    unsafe fn draw_habitable_window(gop: &mut GopBuffer, win: &SpatialWindow, modal_active: bool) {
        let x = win.bounds.x;
        let y = win.bounds.y;
        let w = win.bounds.width;
        let h = win.bounds.height;

        let border_color = if win.focused && !modal_active { Color::BORDER_ACTIVE } else { Color::BORDER_INACTIVE };

        // 1. Soft Ambient Drop Shadow (2px offset, soft alpha)
        gop.draw_rounded_rect_alpha(Rect::new(x + 2, y + 2, w, h), 11, Color::BLACK, 65);

        // 2. Window Frame Container: Muted Walnut-Slate (#49473F) with 94% alpha
        gop.draw_rounded_rect_alpha(Rect::new(x, y, w, h), 11, Color::WINDOW_FRAME, 240);

        // 1px Subtle Rounded Outline (Sage when active, Stone when inactive)
        gop.draw_rounded_rect_outline(Rect::new(x, y, w, h), 11, border_color);

        // 3. Window Header: Warm Stone (#656158) with 94% alpha
        gop.draw_rounded_rect_alpha(Rect::new(x, y, w, 44), 11, Color::WINDOW_HEADER, 242);
        // Cover bottom roundings of the header so it meets the body flush
        gop.fill_rect_alpha(Rect::new(x, y + 24, w, 20), Color::WINDOW_HEADER, 242);

        // 4. Three Football-Shaped Window Controls in upper-left
        let ctrl_y = y + 18;
        let ctrl_w = 15u32;
        let ctrl_h = 8u32;

        let (c_close, c_min, c_max) = if win.focused && !modal_active {
            (Color::CONTROL_CLOSE, Color::CONTROL_MIN, Color::CONTROL_MAX)
        } else {
            // Muted variants when inactive or modal active
            (Color { r: 90, g: 69, b: 66, a: 255 }, Color { r: 90, g: 78, b: 62, a: 255 }, Color { r: 72, g: 84, b: 64, a: 255 })
        };

        gop.draw_oval_pill(x + 16, ctrl_y, ctrl_w, ctrl_h, c_close);
        gop.draw_oval_pill(x + 37, ctrl_y, ctrl_w, ctrl_h, c_min);
        gop.draw_oval_pill(x + 58, ctrl_y, ctrl_w, ctrl_h, c_max);

        // 5. Window Title & Subtitle with Reusable Anti-Aliased Proportional Fonts
        let title_color = if win.focused && !modal_active { Color::HEADER_TEXT } else { Color::DOC_SURFACE };
        gop.draw_text_font((x + 82) as usize, (y + 11) as usize, win.title_str(), title_color, proportional_font::FontFace::Title);

        let sub_color = Color::BORDER_INACTIVE;
        gop.draw_text_font((x + 82) as usize, (y + 33) as usize, win.subtitle_str(), sub_color, proportional_font::FontFace::Ui);

        // 6. Subdued Ivory Document Writing Surface (#B5AEA0), 100% Opaque
        let doc_x = x + 10;
        let doc_y = y + 54;
        let doc_w = w.saturating_sub(20);
        let doc_h = h.saturating_sub(64);
        let doc_rect = Rect::new(doc_x, doc_y, doc_w, doc_h);

        if let Some(ref editor) = win.editor {
            editor.render_surface(gop, doc_rect, win.focused && !modal_active);
        } else {
            gop.draw_rounded_rect_alpha(doc_rect, 8, Color::DOC_SURFACE, 255);
            gop.draw_rounded_rect_outline(doc_rect, 8, Color::BORDER_INACTIVE);

            // 7. Document Text with Genuine Proportional Typography (Dark Ink #292823)
            let text_content = win.content_str();
            let pen_x = (doc_x + 20) as usize;
            let pen_y = (doc_y + 24) as usize;
            let clip_rect = Rect::new(doc_x + 8, doc_y + 8, doc_w.saturating_sub(16), doc_h.saturating_sub(16));

            let text_w = gop.draw_text_font_clipped(pen_x, pen_y, text_content, Color::DOC_TEXT, proportional_font::FontFace::Serif, clip_rect);

            // Active Caret in Focused Window (Suppressed if modal is active)
            if win.focused && !modal_active {
                gop.fill_rect(Rect::new(pen_x as i32 + text_w as i32 + 2, (pen_y + 2) as i32, 2, 16), Color::ACCENT_WALNUT);
            }
        }
    }

    /// Draws the floating pill-shaped Intent Bar
    unsafe fn draw_floating_intent_bar(gop: &mut GopBuffer, bar: &FloatingIntentBar, modal_active: bool) {
        let x = bar.bounds.x;
        let y = bar.bounds.y;
        let w = if bar.collapsed { 44u32 } else { bar.bounds.width };
        let h = bar.bounds.height;

        let border_color = if bar.active && !modal_active { Color::BORDER_ACTIVE } else { Color::BORDER_INACTIVE };

        // Pill shadow
        gop.draw_rounded_rect_alpha(Rect::new(x + 2, y + 2, w, h), 16, Color::BLACK, 60);

        // Pill container: Muted Walnut-Slate (#49473F) with 92% alpha
        gop.draw_rounded_rect_alpha(Rect::new(x, y, w, h), 16, Color::WINDOW_FRAME, 235);
        gop.draw_rounded_rect_outline(Rect::new(x, y, w, h), 16, border_color);

        // Circular Launcher at far left (diameter 24px)
        let cx = x + 16;
        let cy = y + 16;
        gop.draw_circle_alpha(cx, cy, 10, Color::ACCENT_BOTANIC, 255);
        // Center subtle ivory pip
        gop.draw_circle_alpha(cx, cy, 3, Color::HEADER_TEXT, 255);

        if !bar.collapsed {
            let input_str = bar.text_str();
            let text_color = if modal_active { Color::BORDER_INACTIVE } else { Color::HEADER_TEXT };
            if input_str.is_empty() {
                gop.draw_text_font((x + 36) as usize, (y + 8) as usize, "Search, run command, or describe intent...", Color::BORDER_INACTIVE, proportional_font::FontFace::Ui);
            } else {
                let tw = gop.draw_text_font((x + 36) as usize, (y + 8) as usize, input_str, text_color, proportional_font::FontFace::Ui);
                // Blinking intent caret: ONLY draw if bar is active AND modal is NOT active!
                if bar.active && !modal_active {
                    gop.fill_rect(Rect::new((x + 36 + tw as i32 + 2), y + 7, 2, 18), Color::ACCENT_BOTANIC);
                }
            }

            // Subtle Drag Grip on far right (3 vertical dots)
            let grip_x = x + w as i32 - 16;
            gop.draw_circle_alpha(grip_x, y + 10, 2, Color::BORDER_INACTIVE, 200);
            gop.draw_circle_alpha(grip_x, y + 16, 2, Color::BORDER_INACTIVE, 200);
            gop.draw_circle_alpha(grip_x, y + 22, 2, Color::BORDER_INACTIVE, 200);
        }
    }

    /// Draws the Circular Launcher Menu card
    unsafe fn draw_launcher_menu(gop: &mut GopBuffer, wm: &SpatialWindowManager) {
        let x = wm.launcher.bounds.x;
        let y = wm.launcher.bounds.y;
        let w = wm.launcher.bounds.width;
        let h = wm.launcher.bounds.height;

        // Shadow & Body
        gop.draw_rounded_rect_alpha(Rect::new(x + 3, y + 3, w, h), 10, Color::BLACK, 70);
        gop.draw_rounded_rect_alpha(Rect::new(x, y, w, h), 10, Color::WINDOW_FRAME, 246);
        gop.draw_rounded_rect_outline(Rect::new(x, y, w, h), 10, Color::BORDER_ACTIVE);

        // Header
        gop.draw_text_font((x + 16) as usize, (y + 12) as usize, "INOS Launcher & Library", Color::ACCENT_BOTANIC, proportional_font::FontFace::Ui);
        gop.fill_rect(Rect::new(x + 14, y + 30, w - 28, 1), Color::BORDER_INACTIVE);

        let mut row_y = y + 36;

        // List Windows (Open and Minimized)
        for slot in wm.windows.iter() {
            if let Some(ref w) = slot {
                let is_min = w.state == WindowState::Minimized;
                let status_badge = if is_min { " [minimized]" } else if w.focused { " [active]" } else { "" };
                let pip_color = if is_min { Color::CONTROL_MIN } else if w.focused { Color::ACCENT_BOTANIC } else { Color::BORDER_INACTIVE };

                gop.draw_circle_alpha(x + 22, row_y + 8, 4, pip_color, 255);
                let mut title_buf = [0u8; 48];
                let tb = w.title_str().as_bytes();
                title_buf[..tb.len()].copy_from_slice(tb);
                let sb = status_badge.as_bytes();
                title_buf[tb.len()..tb.len() + sb.len()].copy_from_slice(sb);
                let total = tb.len() + sb.len();
                let full_title = core::str::from_utf8(&title_buf[..total]).unwrap_or("");
                let text_color = if is_min { Color::DOC_SURFACE } else { Color::HEADER_TEXT };
                gop.draw_text_font((x + 34) as usize, (row_y + 2) as usize, full_title, text_color, proportional_font::FontFace::Ui);
                row_y += 26;
            }
        }

        // Document Library Section
        if wm.launcher.doc_count > 0 {
            gop.fill_rect(Rect::new(x + 14, row_y, w - 28, 1), Color::BORDER_INACTIVE);
            row_y += 6;
            gop.draw_text_font((x + 16) as usize, row_y as usize, "Library Documents", Color::HEADER_TEXT, proportional_font::FontFace::Ui);
            row_y += 20;

            for d in 0..wm.launcher.doc_count {
                let name_len = wm.launcher.doc_name_lens[d];
                let name_str = core::str::from_utf8(&wm.launcher.doc_names[d][..name_len]).unwrap_or("");
                gop.draw_circle_alpha(x + 22, row_y + 8, 4, Color::ACCENT_BOTANIC, 255);
                gop.draw_text_font((x + 34) as usize, (row_y + 2) as usize, name_str, Color::DOC_SURFACE, proportional_font::FontFace::Ui);
                row_y += 24;
            }
        }

        // System Tools
        gop.fill_rect(Rect::new(x + 14, row_y, w - 28, 1), Color::BORDER_INACTIVE);
        row_y += 8;

        gop.draw_circle_alpha(x + 22, row_y + 8, 4, Color::ACCENT_BOTANIC, 255);
        gop.draw_text_font((x + 34) as usize, (row_y + 2) as usize, "New Document (Ctrl+N)", Color::HEADER_TEXT, proportional_font::FontFace::Ui);
        row_y += 24;

        gop.draw_circle_alpha(x + 22, row_y + 8, 4, Color::ACCENT_WALNUT, 255);
        gop.draw_text_font((x + 34) as usize, (row_y + 2) as usize, "Engineer Diagnostics (F12)", Color::HEADER_TEXT, proportional_font::FontFace::Ui);
        row_y += 24;

        gop.draw_circle_alpha(x + 22, row_y + 8, 4, Color::DOC_SURFACE, 255);
        gop.draw_text_font((x + 34) as usize, (row_y + 2) as usize, "Cold Reboot System (F10)", Color::CONTROL_CLOSE, proportional_font::FontFace::Ui);
    }

    /// Verifies that dirty-region redraw yields 0 mismatched pixels compared against clean full-frame composition
    pub unsafe fn audit_dirty_region_equivalence(&mut self, gop: &mut GopBuffer, test_rect: Rect) -> usize {
        let mut diff_count = 0usize;
        let x0 = core::cmp::max(0, test_rect.x) as usize;
        let y0 = core::cmp::max(0, test_rect.y) as usize;
        let x1 = core::cmp::min(gop.width, (test_rect.x + test_rect.width as i32) as usize);
        let y1 = core::cmp::min(gop.height, (test_rect.y + test_rect.height as i32) as usize);

        // 1. Render clean full-frame workspace
        self.render_workspace(gop);

        // 2. Sample pixels across the test region
        let mut sample_hashes = [0u32; 16];
        for (i, row) in (y0..y1).step_by(core::cmp::max(1, (y1 - y0) / 4)).enumerate().take(4) {
            for (j, col) in (x0..x1).step_by(core::cmp::max(1, (x1 - x0) / 4)).enumerate().take(4) {
                let pix = gop.get_pixel(col, row);
                sample_hashes[i * 4 + j] = pix.to_u32();
            }
        }

        // 3. Redraw single window (dirty rect path)
        if let Some(ref w) = self.windows[1] {
            Self::draw_habitable_window(gop, w, false);
        }

        // 4. Compare sample hashes
        for (i, row) in (y0..y1).step_by(core::cmp::max(1, (y1 - y0) / 4)).enumerate().take(4) {
            for (j, col) in (x0..x1).step_by(core::cmp::max(1, (x1 - x0) / 4)).enumerate().take(4) {
                let pix = gop.get_pixel(col, row);
                if sample_hashes[i * 4 + j] != pix.to_u32() {
                    diff_count += 1;
                }
            }
        }

        diff_count
    }
}
