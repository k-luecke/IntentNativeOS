// kernel/src/graphics/cursor.rs - Hardware-Style Pointer Cursor with Backing Store (Gate UI-02)
// Provides artifact-free cursor movement and restoration over changing spatial windows.

#![no_std]

use super::Color;
use super::GopBuffer;

pub const CURSOR_WIDTH: usize = 12;
pub const CURSOR_HEIGHT: usize = 18;

// 12x18 Arrow Cursor Pattern
// 0: Transparent, 1: White Border, 2: Terracotta Fill
static CURSOR_PATTERN: [u8; CURSOR_WIDTH * CURSOR_HEIGHT] = [
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0,
    1, 2, 2, 2, 2, 2, 1, 1, 1, 1, 0, 0,
    1, 2, 2, 1, 2, 2, 1, 0, 0, 0, 0, 0,
    1, 2, 1, 0, 1, 2, 2, 1, 0, 0, 0, 0,
    1, 1, 0, 0, 1, 2, 2, 1, 0, 0, 0, 0,
    1, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0,
    0, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub struct CursorBackingStore {
    pub x: usize,
    pub y: usize,
    pub visible: bool,
    pub saved: [Color; CURSOR_WIDTH * CURSOR_HEIGHT],
}

impl CursorBackingStore {
    pub const fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            visible: false,
            saved: [Color::PAPER_BG; CURSOR_WIDTH * CURSOR_HEIGHT],
        }
    }

    /// Restores the original framebuffer pixels under the previous cursor location
    pub unsafe fn restore(&mut self, gop: &mut GopBuffer) {
        if !self.visible {
            return;
        }
        for cy in 0..CURSOR_HEIGHT {
            let py = self.y + cy;
            if py >= gop.height {
                continue;
            }
            for cx in 0..CURSOR_WIDTH {
                let px = self.x + cx;
                if px >= gop.width {
                    continue;
                }
                let color = self.saved[cy * CURSOR_WIDTH + cx];
                gop.set_pixel(px, py, color);
            }
        }
        self.visible = false;
    }

    /// Saves the background at (new_x, new_y) and renders the pointer cursor
    pub unsafe fn render(&mut self, gop: &mut GopBuffer, new_x: usize, new_y: usize) {
        // 1. Restore previous background if visible
        self.restore(gop);

        // 2. Clamp cursor coordinates to screen boundaries
        let max_x = gop.width.saturating_sub(CURSOR_WIDTH);
        let max_y = gop.height.saturating_sub(CURSOR_HEIGHT);
        let cx = core::cmp::min(new_x, max_x);
        let cy = core::cmp::min(new_y, max_y);

        // 3. Save background pixels to backing store
        for r in 0..CURSOR_HEIGHT {
            let py = cy + r;
            for c in 0..CURSOR_WIDTH {
                let px = cx + c;
                let bg = gop.get_pixel(px, py);
                self.saved[r * CURSOR_WIDTH + c] = bg;
            }
        }
        self.x = cx;
        self.y = cy;
        self.visible = true;

        // 4. Blit cursor pixels
        for r in 0..CURSOR_HEIGHT {
            let py = cy + r;
            for c in 0..CURSOR_WIDTH {
                let px = cx + c;
                let val = CURSOR_PATTERN[r * CURSOR_WIDTH + c];
                if val == 1 {
                    gop.set_pixel(px, py, Color::WHITE);
                } else if val == 2 {
                    gop.set_pixel(px, py, Color::TERRACOTTA);
                }
            }
        }
    }
}
