// kernel/src/storage/workspace.rs - Persistent Workspace Restoration (UI-04.5)
// Persists and restores open documents, window geometry, cursor/scroll positions,
// active window focus, and intent bar location safely through INOS-FS.

use crate::graphics::Rect;

pub const WORKSPACE_MAGIC: u32 = 0x574F524B; // "WORK"
pub const WORKSPACE_VERSION: u32 = 1;
pub const WORKSPACE_FILENAME: &str = ".workspace";
pub const MAX_PERSISTED_WINDOWS: usize = 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PersistedWindowState {
    pub is_valid: u8,
    pub filename: [u8; 32],
    pub filename_len: u8,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub state: u8, // 0 = Normal, 1 = Maximized, 2 = Minimized, 3 = Closed
    pub z_order: u8,
    pub cursor_pos: u16,
    pub scroll_line: u16,
}

impl PersistedWindowState {
    pub const fn empty() -> Self {
        Self {
            is_valid: 0,
            filename: [0u8; 32],
            filename_len: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            state: 0,
            z_order: 0,
            cursor_pos: 0,
            scroll_line: 0,
        }
    }

    pub fn set_filename(&mut self, name: &str) {
        let b = name.as_bytes();
        let l = core::cmp::min(b.len(), 32);
        self.filename[..l].copy_from_slice(&b[..l]);
        self.filename_len = l as u8;
    }

    pub fn filename_str(&self) -> &str {
        core::str::from_utf8(&self.filename[..self.filename_len as usize]).unwrap_or("")
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WorkspaceLayoutRecord {
    pub magic: u32,
    pub version: u32,
    pub focused_window_idx: u8,
    pub intent_bar_x: i16,
    pub intent_bar_y: i16,
    pub intent_bar_docked: u8,
    pub intent_bar_collapsed: u8,
    pub window_count: u8,
    pub windows: [PersistedWindowState; MAX_PERSISTED_WINDOWS],
    pub checksum: u32,
}

impl WorkspaceLayoutRecord {
    pub const fn empty() -> Self {
        Self {
            magic: WORKSPACE_MAGIC,
            version: WORKSPACE_VERSION,
            focused_window_idx: 0,
            intent_bar_x: 370,
            intent_bar_y: 736,
            intent_bar_docked: 1,
            intent_bar_collapsed: 0,
            window_count: 0,
            windows: [PersistedWindowState::empty(); MAX_PERSISTED_WINDOWS],
            checksum: 0,
        }
    }

    pub fn compute_checksum(&self) -> u32 {
        let mut sum = 0u32;
        let bytes = unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                core::mem::size_of::<Self>() - 4, // exclude checksum field
            )
        };
        for (i, b) in bytes.iter().enumerate() {
            sum = sum.wrapping_add((*b as u32).wrapping_mul(i as u32 + 1));
        }
        sum ^ 0xA5A5_5A5A
    }

    pub fn is_valid(&self) -> bool {
        self.magic == WORKSPACE_MAGIC && self.version == WORKSPACE_VERSION && self.checksum == self.compute_checksum()
    }

    pub fn capture_from(wm: &crate::graphics::SpatialWindowManager) -> Self {
        let mut rec = Self::empty();
        rec.intent_bar_x = wm.intent_bar.bounds.x as i16;
        rec.intent_bar_y = wm.intent_bar.bounds.y as i16;
        rec.intent_bar_docked = if wm.intent_bar.docked_bottom { 1 } else { 0 };
        rec.intent_bar_collapsed = if wm.intent_bar.collapsed { 1 } else { 0 };

        let mut count = 0u8;
        for slot in wm.windows.iter() {
            if let Some(ref w) = slot {
                if (count as usize) < MAX_PERSISTED_WINDOWS {
                    let mut pw = PersistedWindowState::empty();
                    pw.is_valid = 1;
                    pw.set_filename(w.title_str());
                    pw.x = w.bounds.x as i16;
                    pw.y = w.bounds.y as i16;
                    pw.width = w.bounds.width as u16;
                    pw.height = w.bounds.height as u16;
                    pw.state = match w.state {
                        crate::graphics::WindowState::Normal => 0,
                        crate::graphics::WindowState::Maximized { .. } => 1,
                        crate::graphics::WindowState::Minimized => 2,
                        crate::graphics::WindowState::Closed => 3,
                    };
                    pw.z_order = w.z_order;
                    if let Some(ref ed) = w.editor {
                        pw.cursor_pos = ed.cursor_pos as u16;
                        pw.scroll_line = ed.scroll_line as u16;
                    } else {
                        pw.cursor_pos = w.cursor_pos as u16;
                        pw.scroll_line = w.scroll_y as u16;
                    }
                    rec.windows[count as usize] = pw;
                    if wm.focused_window_id == Some(w.id) {
                        rec.focused_window_idx = count;
                    }
                    count += 1;
                }
            }
        }
        rec.window_count = count;
        rec.checksum = rec.compute_checksum();
        rec
    }

    pub fn save<D: crate::storage::BlockDevice>(&mut self, fs: &mut crate::storage::InosFileSystem<D>) -> Result<(), crate::storage::FsError> {
        self.checksum = self.compute_checksum();
        let bytes = unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        };
        if !fs.file_exists(WORKSPACE_FILENAME) {
            let _ = fs.create_file(WORKSPACE_FILENAME)?;
        }
        fs.write_file(WORKSPACE_FILENAME, bytes)?;
        Ok(())
    }

    pub fn load<D: crate::storage::BlockDevice>(fs: &crate::storage::InosFileSystem<D>) -> Result<Self, crate::storage::FsError> {
        let mut record = Self::empty();
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(
                &mut record as *mut Self as *mut u8,
                core::mem::size_of::<Self>(),
            )
        };
        let read_len = fs.read_file(WORKSPACE_FILENAME, bytes)?;
        if read_len == core::mem::size_of::<Self>() && record.is_valid() {
            Ok(record)
        } else {
            Err(crate::storage::FsError::FileNotFound)
        }
    }

    pub fn apply_to<D: crate::storage::BlockDevice>(
        &self,
        wm: &mut crate::graphics::SpatialWindowManager,
        fs: &crate::storage::InosFileSystem<D>,
    ) {
        if !self.is_valid() {
            return;
        }
        wm.intent_bar.bounds.x = self.intent_bar_x as i32;
        wm.intent_bar.bounds.y = self.intent_bar_y as i32;
        wm.intent_bar.docked_bottom = self.intent_bar_docked == 1;
        wm.intent_bar.collapsed = self.intent_bar_collapsed == 1;

        for slot in wm.windows.iter_mut() {
            *slot = None;
        }

        let mut focused_id = None;
        for i in 0..(self.window_count as usize) {
            let pw = &self.windows[i];
            if pw.is_valid == 1 {
                let fname = pw.filename_str();
                let mut buf = [0u8; crate::graphics::editor::MAX_EDITOR_BYTES];
                let content_str = if let Ok(read_len) = fs.read_file(fname, &mut buf) {
                    core::str::from_utf8(&buf[..read_len]).unwrap_or("")
                } else {
                    ""
                };

                let bounds = crate::graphics::Rect::new(pw.x as i32, pw.y as i32, pw.width as u32, pw.height as u32);
                let mut win = crate::graphics::SpatialWindow::new_with_editor(i as u32, fname, content_str, bounds, pw.z_order);
                win.state = match pw.state {
                    0 => crate::graphics::WindowState::Normal,
                    1 => crate::graphics::WindowState::Maximized { prev_bounds: bounds },
                    2 => crate::graphics::WindowState::Minimized,
                    _ => crate::graphics::WindowState::Closed,
                };
                if let Some(ref mut ed) = win.editor {
                    ed.cursor_pos = pw.cursor_pos as usize;
                    ed.scroll_line = pw.scroll_line as usize;
                    ed.is_dirty = false;
                }
                if i as u8 == self.focused_window_idx {
                    focused_id = Some(i as u32);
                    win.focused = true;
                } else {
                    win.focused = false;
                }
                wm.windows[i] = Some(win);
            }
        }
        wm.focused_window_id = focused_id;
    }
}
