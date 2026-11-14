// kernel/src/graphics/mod.rs - Framebuffer, Compositor, Font Rendering, and Widgets (S006, S007, S008, G8)

pub mod font;
pub mod typography;
pub mod proportional_font;
pub mod cursor;
pub mod spatial;
pub mod app;
pub mod editor;

pub use cursor::CursorBackingStore;
pub use spatial::{ClickTarget, SpatialWindow, SpatialWindowManager, WindowState};
pub use app::{AppCommand, AppEvent, KeyCode, WindowApp, CLIPBOARD_SIZE};
pub use editor::TextEditorApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const RED: Self = Self { r: 255, g: 0, b: 0, a: 255 };
    pub const GREEN: Self = Self { r: 0, g: 255, b: 0, a: 255 };
    pub const BLUE: Self = Self { r: 0, g: 0, b: 255, a: 255 };
    pub const GRAY: Self = Self { r: 128, g: 128, b: 128, a: 255 };
    pub const DARK_GRAY: Self = Self { r: 32, g: 32, b: 32, a: 255 };
    pub const LIGHT_BLUE: Self = Self { r: 64, g: 128, b: 240, a: 255 };
    pub const ACCENT_PURPLE: Self = Self { r: 147, g: 51, b: 234, a: 255 };

    // Habitable Computing Palette (UI-01) - Contrast Compliant
    pub const PAPER_BG: Self         = Self { r: 245, g: 242, b: 235, a: 255 }; // #F5F2EB Warm Linen Desk
    pub const CANVAS_SHEET: Self     = Self { r: 252, g: 251, b: 249, a: 255 }; // #FCFBF9 Crisp Ivory Paper
    pub const CARD_BG: Self          = Self { r: 255, g: 255, b: 255, a: 255 }; // #FFFFFF Clean Card Surface
    pub const SHELF_BG: Self         = Self { r: 238, g: 234, b: 226, a: 255 }; // #EEEAE2 Soft Stone Shelf

    pub const INK_PRIMARY: Self      = Self { r: 36,  g: 34,  b: 32,  a: 255 }; // #242220 Deep Charcoal (14.2:1 contrast)
    pub const INK_MUTED: Self        = Self { r: 104, g: 100, b: 94,  a: 255 }; // #68645E Warm Muted Stone (4.8:1 contrast)
    pub const BORDER_HAIRLINE: Self  = Self { r: 221, g: 216, b: 206, a: 255 }; // #DDD8CE Soft Linen Border
    pub const BORDER_SHADOW: Self    = Self { r: 200, g: 194, b: 184, a: 255 }; // #C8C2B8 Drop-shadow edge

    pub const TERRACOTTA: Self       = Self { r: 196, g: 90,  b: 60,  a: 255 }; // #C45A3C Warm Clay / Intent Focus
    pub const SAGE_GREEN: Self       = Self { r: 54,  g: 114, b: 78,  a: 255 }; // #36724E Forest Sage / Safe Affirmation
    pub const WARM_AMBER: Self       = Self { r: 212, g: 130, b: 46,  a: 255 }; // #D4822E Amber / Reversible Notice
    pub const BRICK_RED: Self        = Self { r: 184, g: 62,  b: 50,  a: 255 }; // #B83E32 Brick Red / Destructive Warning
    pub const DEEP_NAVY: Self        = Self { r: 42,  g: 58,  b: 76,  a: 255 }; // #2A3A4C Slate Navy / Details Badge

    // Habitable Computing Palette (UI-03 Approved Dark Charcoal / Ivory Direction)
    pub const DESKTOP_BG: Self      = Self { r: 35,  g: 36,  b: 33,  a: 255 }; // #232421 Warm Charcoal
    pub const WINDOW_FRAME: Self    = Self { r: 73,  g: 71,  b: 63,  a: 255 }; // #49473F Muted Walnut-Slate
    pub const WINDOW_HEADER: Self   = Self { r: 101, g: 97,  b: 88,  a: 255 }; // #656158 Warm Stone
    pub const DOC_SURFACE: Self     = Self { r: 181, g: 174, b: 160, a: 255 }; // #B5AEA0 Subdued Ivory
    pub const DOC_TEXT: Self        = Self { r: 41,  g: 40,  b: 35,  a: 255 }; // #292823 Dark Ink (8.4:1 contrast)
    pub const HEADER_TEXT: Self     = Self { r: 230, g: 224, b: 211, a: 255 }; // #E6E0D3 Soft Ivory
    pub const BORDER_INACTIVE: Self = Self { r: 104, g: 103, b: 93,  a: 255 }; // #68675D Subdued Stone
    pub const BORDER_ACTIVE: Self   = Self { r: 137, g: 150, b: 110, a: 255 }; // #89966E Muted Sage
    pub const ACCENT_WALNUT: Self   = Self { r: 128, g: 99,  b: 75,  a: 255 }; // #80634B Walnut Accent
    pub const ACCENT_BOTANIC: Self  = Self { r: 130, g: 147, b: 110, a: 255 }; // #82936E Botanical Green
    pub const CONTROL_CLOSE: Self   = Self { r: 139, g: 79,  b: 71,  a: 255 }; // #8B4F47 Muted Clay-Red
    pub const CONTROL_MIN: Self     = Self { r: 139, g: 112, b: 67,  a: 255 }; // #8B7043 Muted Amber-Walnut
    pub const CONTROL_MAX: Self     = Self { r: 107, g: 130, b: 88,  a: 255 }; // #6B8258 Muted Sage-Green

    // Habitable Modal Treatment Palette (UI-03.1) - High Contrast & Reassuring Hierarchy
    pub const MODAL_BG: Self         = Self { r: 52,  g: 50,  b: 44,  a: 255 }; // #34322C Rich Walnut Card (96% alpha)
    pub const MODAL_HEADING: Self    = Self { r: 238, g: 234, b: 226, a: 255 }; // #EEEAE2 High contrast cream heading (8.8:1)
    pub const MODAL_REASSURE: Self   = Self { r: 226, g: 220, b: 208, a: 255 }; // #E2DCD0 Reassuring explanation (7.8:1)
    pub const MODAL_DETAIL: Self     = Self { r: 196, g: 190, b: 176, a: 255 }; // #C4BEB0 Action detail (6.2:1)
    pub const MODAL_FOOTER: Self     = Self { r: 154, g: 148, b: 136, a: 255 }; // #9A9488 Subdued footer hint
    pub const BUTTON_PRIMARY_BG: Self= Self { r: 137, g: 150, b: 110, a: 255 }; // #89966E Muted Sage Pill
    pub const BUTTON_PRIMARY_FG: Self= Self { r: 35,  g: 36,  b: 33,  a: 255 }; // #232421 Dark Charcoal Ink (7.8:1 on Sage)
    pub const BUTTON_SECONDARY_BG: Self=Self { r: 65,  g: 63,  b: 56,  a: 255 }; // #413F38 Architectural Card Pill
    pub const BUTTON_SECONDARY_FG: Self=Self { r: 226, g: 220, b: 208, a: 255 }; // #E2DCD0 Cream Ink (8.1:1 on Secondary)
    pub const BUTTON_FOCUS_RING: Self  =Self { r: 218, g: 212, b: 198, a: 255 }; // #DAD4C6 Distinct Focus Ring Outline

    // G8 Theme Palette (Legacy)
    pub const SLATE_900: Self = Self { r: 15, g: 23, b: 42, a: 255 };     // #0f172a
    pub const SLATE_800: Self = Self { r: 30, g: 41, b: 59, a: 255 };     // #1e293b
    pub const SLATE_700: Self = Self { r: 51, g: 65, b: 85, a: 255 };     // #334155
    pub const SLATE_600: Self = Self { r: 71, g: 85, b: 105, a: 255 };    // #475569
    pub const SLATE_400: Self = Self { r: 148, g: 163, b: 184, a: 255 };  // #94a3b8
    pub const CYAN_400: Self = Self { r: 56, g: 189, b: 248, a: 255 };     // #38bdf8
    pub const EMERALD_500: Self = Self { r: 16, g: 185, b: 129, a: 255 }; // #10b981
    pub const AMBER_400: Self = Self { r: 251, g: 191, b: 36, a: 255 };   // #fbbf24
    pub const PURPLE_400: Self = Self { r: 192, g: 132, b: 252, a: 255 }; // #c084fc
    pub const ROSE_500: Self = Self { r: 244, g: 63, b: 94, a: 255 };     // #f43f5e
    pub const TEXT_WHITE: Self = Self { r: 248, g: 250, b: 252, a: 255 }; // #f8fafc

    pub const fn to_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    pub const fn from_u32(val: u32) -> Self {
        Self {
            b: (val & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            r: ((val >> 16) & 0xFF) as u8,
            a: ((val >> 24) & 0xFF) as u8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        let r1_right = self.x + self.width as i32;
        let r1_bottom = self.y + self.height as i32;
        let r2_right = other.x + other.width as i32;
        let r2_bottom = other.y + other.height as i32;

        self.x < r2_right && r1_right > other.x && self.y < r2_bottom && r1_bottom > other.y
    }

    pub fn clip_against(&self, boundary: &Rect) -> Option<Rect> {
        let left = core::cmp::max(self.x, boundary.x);
        let top = core::cmp::max(self.y, boundary.y);
        let right = core::cmp::min(self.x + self.width as i32, boundary.x + boundary.width as i32);
        let bottom = core::cmp::min(self.y + self.height as i32, boundary.y + boundary.height as i32);

        if right > left && bottom > top {
            Some(Rect {
                x: left,
                y: top,
                width: (right - left) as u32,
                height: (bottom - top) as u32,
            })
        } else {
            None
        }
    }
}

// Fixed dimensions for the off-screen compositor buffer in kernel (legacy G3 test compatibility)
pub const FB_WIDTH: usize = 128;
pub const FB_HEIGHT: usize = 96;
pub const FB_PIXELS: usize = FB_WIDTH * FB_HEIGHT;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pitch: usize, // pixels per scanline
    pub buffer: [u32; FB_PIXELS],
}

impl Framebuffer {
    pub const fn new() -> Self {
        Self {
            width: FB_WIDTH,
            height: FB_HEIGHT,
            pitch: FB_WIDTH,
            buffer: [0u32; FB_PIXELS],
        }
    }

    pub fn clear(&mut self, color: Color) {
        let val = color.to_u32();
        self.buffer.fill(val);
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x < self.width && y < self.height {
            self.buffer[y * self.pitch + x] = color.to_u32();
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> Color {
        if x < self.width && y < self.height {
            Color::from_u32(self.buffer[y * self.pitch + x])
        } else {
            Color::BLACK
        }
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let bound = Rect::new(0, 0, self.width as u32, self.height as u32);
        if let Some(clipped) = rect.clip_against(&bound) {
            let val = color.to_u32();
            for y in (clipped.y as usize)..((clipped.y + clipped.height as i32) as usize) {
                let row_start = y * self.pitch + clipped.x as usize;
                let row_end = row_start + clipped.width as usize;
                self.buffer[row_start..row_end].fill(val);
            }
        }
    }
}

// --- S007: Window Surface Compositor ---

pub const MAX_SURFACES: usize = 4;
pub const SURFACE_MAX_W: usize = 64;
pub const SURFACE_MAX_H: usize = 48;
pub const SURFACE_PIXELS: usize = SURFACE_MAX_W * SURFACE_MAX_H;

pub struct WindowSurface {
    pub id: u32,
    pub bounds: Rect,
    pub z_order: u8,
    pub visible: bool,
    pub buffer: [u32; SURFACE_PIXELS],
}

impl WindowSurface {
    pub const fn new(id: u32, bounds: Rect, z_order: u8) -> Self {
        Self {
            id,
            bounds,
            z_order,
            visible: true,
            buffer: [0u32; SURFACE_PIXELS],
        }
    }

    pub fn clear(&mut self, color: Color) {
        let val = color.to_u32();
        self.buffer.fill(val);
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x < self.bounds.width as usize && y < self.bounds.height as usize {
            self.buffer[y * self.bounds.width as usize + x] = color.to_u32();
        }
    }
}

pub struct Compositor {
    pub surfaces: [Option<WindowSurface>; MAX_SURFACES],
}

impl Compositor {
    pub const fn new() -> Self {
        Self {
            surfaces: [None, None, None, None],
        }
    }

    pub fn add_surface(&mut self, surface: WindowSurface) -> Result<usize, ()> {
        for (i, slot) in self.surfaces.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(surface);
                return Ok(i);
            }
        }
        Err(())
    }

    pub fn composite(&self, fb: &mut Framebuffer) {
        let mut sorted_indices = [0usize; MAX_SURFACES];
        let mut count = 0;

        for (i, slot) in self.surfaces.iter().enumerate() {
            if slot.is_some() {
                sorted_indices[count] = i;
                count += 1;
            }
        }

        for i in 0..count {
            for j in 0..(count - 1 - i) {
                let idx_a = sorted_indices[j];
                let idx_b = sorted_indices[j + 1];
                let z_a = self.surfaces[idx_a].as_ref().unwrap().z_order;
                let z_b = self.surfaces[idx_b].as_ref().unwrap().z_order;
                if z_a > z_b {
                    sorted_indices.swap(j, j + 1);
                }
            }
        }

        let fb_bounds = Rect::new(0, 0, fb.width as u32, fb.height as u32);

        for &idx in &sorted_indices[0..count] {
            if let Some(ref surf) = self.surfaces[idx] {
                if !surf.visible {
                    continue;
                }

                if let Some(clipped) = surf.bounds.clip_against(&fb_bounds) {
                    for y in 0..clipped.height {
                        let surf_y = (clipped.y - surf.bounds.y + y as i32) as usize;
                        let fb_y = (clipped.y + y as i32) as usize;

                        for x in 0..clipped.width {
                            let surf_x = (clipped.x - surf.bounds.x + x as i32) as usize;
                            let fb_x = (clipped.x + x as i32) as usize;

                            let pixel = surf.buffer[surf_y * surf.bounds.width as usize + surf_x];
                            if (pixel >> 24) != 0 {
                                fb.buffer[fb_y * fb.pitch + fb_x] = pixel;
                            }
                        }
                    }
                }
            }
        }
    }
}

// --- S008: Bitmap Font & Text Input Widgets ---

pub const FONT_WIDTH: usize = 8;
pub const FONT_HEIGHT: usize = 8;

/// Built-in 8x8 bitmap glyph pattern for ASCII chars (delegates to font module)
pub fn get_glyph_bitmap(ch: char) -> [u8; 8] {
    font::get_ascii_glyph(ch as u8)
}

pub fn draw_text(fb: &mut Framebuffer, x: usize, y: usize, text: &str, color: Color) {
    let mut cursor_x = x;
    for ch in text.chars() {
        let glyph = font::get_ascii_glyph(ch as u8);
        for row in 0..8 {
            let byte = glyph[row];
            for col in 0..8 {
                if (byte & (0x80 >> col)) != 0 {
                    fb.set_pixel(cursor_x + col, y + row, color);
                }
            }
        }
        cursor_x += FONT_WIDTH;
    }
}

pub struct TextInputWidget {
    pub text: [u8; 64],
    pub len: usize,
    pub cursor: usize,
}

impl TextInputWidget {
    pub const fn new() -> Self {
        Self {
            text: [0u8; 64],
            len: 0,
            cursor: 0,
        }
    }

    pub fn insert_char(&mut self, ch: u8) -> bool {
        if self.len >= self.text.len() {
            return false;
        }
        for i in (self.cursor..self.len).rev() {
            self.text[i + 1] = self.text[i];
        }
        self.text[self.cursor] = ch;
        self.cursor += 1;
        self.len += 1;
        true
    }

    pub fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        for i in self.cursor..self.len {
            self.text[i - 1] = self.text[i];
        }
        self.cursor -= 1;
        self.len -= 1;
        true
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.len]).unwrap_or("")
    }
}

// --- G8: Full Resolution UEFI GOP Framebuffer & Desktop Blitter ---

#[derive(Clone, Copy)]
pub struct GopBuffer {
    pub base: *mut u32,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}

impl GopBuffer {
    pub const fn new(base: *mut u32, width: usize, height: usize, pitch: usize) -> Self {
        Self { base, width, height, pitch }
    }

    #[inline(always)]
    pub unsafe fn get_pixel(&self, x: usize, y: usize) -> Color {
        if x < self.width && y < self.height {
            Color::from_u32(*self.base.add(y * self.pitch + x))
        } else {
            Color::BLACK
        }
    }

    #[inline(always)]
    pub fn blend(fg: Color, bg: Color, alpha: u8) -> Color {
        if alpha == 0 {
            return bg;
        }
        if alpha == 255 {
            return fg;
        }
        let a = alpha as u32;
        let inv_a = 255 - a;
        Color {
            r: ((fg.r as u32 * a + bg.r as u32 * inv_a) / 255) as u8,
            g: ((fg.g as u32 * a + bg.g as u32 * inv_a) / 255) as u8,
            b: ((fg.b as u32 * a + bg.b as u32 * inv_a) / 255) as u8,
            a: 255,
        }
    }

    pub unsafe fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x < self.width && y < self.height {
            *self.base.add(y * self.pitch + x) = color.to_u32();
        }
    }

    pub unsafe fn fill_rect(&mut self, rect: Rect, color: Color) {
        let val = color.to_u32();
        let x0 = core::cmp::max(0, rect.x) as usize;
        let y0 = core::cmp::max(0, rect.y) as usize;
        let x1 = core::cmp::min(self.width, (rect.x + rect.width as i32) as usize);
        let y1 = core::cmp::min(self.height, (rect.y + rect.height as i32) as usize);

        for y in y0..y1 {
            let row_start = self.base.add(y * self.pitch + x0);
            for x in 0..(x1.saturating_sub(x0)) {
                *row_start.add(x) = val;
            }
        }
    }

    pub unsafe fn draw_rect_outline(&mut self, rect: Rect, color: Color, border: usize) {
        self.fill_rect(Rect::new(rect.x, rect.y, rect.width, border as u32), color);
        self.fill_rect(Rect::new(rect.x, rect.y + rect.height as i32 - border as i32, rect.width, border as u32), color);
        self.fill_rect(Rect::new(rect.x, rect.y, border as u32, rect.height), color);
        self.fill_rect(Rect::new(rect.x + rect.width as i32 - border as i32, rect.y, border as u32, rect.height), color);
    }

    pub unsafe fn fill_rect_alpha(&mut self, rect: Rect, color: Color, alpha: u8) {
        if alpha == 0 {
            return;
        }
        if alpha == 255 {
            self.fill_rect(rect, color);
            return;
        }
        let x0 = core::cmp::max(0, rect.x) as usize;
        let y0 = core::cmp::max(0, rect.y) as usize;
        let x1 = core::cmp::min(self.width, (rect.x + rect.width as i32) as usize);
        let y1 = core::cmp::min(self.height, (rect.y + rect.height as i32) as usize);

        for y in y0..y1 {
            let row_start = self.base.add(y * self.pitch);
            for x in x0..x1 {
                let bg = Color::from_u32(*row_start.add(x));
                let blended = Self::blend(color, bg, alpha);
                *row_start.add(x) = blended.to_u32();
            }
        }
    }

    pub unsafe fn draw_rounded_rect_alpha(&mut self, rect: Rect, radius: i32, color: Color, alpha: u8) {
        if radius <= 0 {
            self.fill_rect_alpha(rect, color, alpha);
            return;
        }
        let rx = rect.x;
        let ry = rect.y;
        let rw = rect.width as i32;
        let rh = rect.height as i32;
        let r = radius;
        let r2 = r * r;
        let r_sub = r * 4;
        let r_sub2 = r_sub * r_sub;

        let x0 = core::cmp::max(0, rx) as usize;
        let y0 = core::cmp::max(0, ry) as usize;
        let x1 = core::cmp::min(self.width, (rx + rw) as usize);
        let y1 = core::cmp::min(self.height, (ry + rh) as usize);

        for y in y0..y1 {
            let py = y as i32;
            let row_start = self.base.add(y * self.pitch);
            for x in x0..x1 {
                let px = x as i32;
                let mut in_corner = false;
                let mut dx = 0i32;
                let mut dy = 0i32;

                if px < rx + r && py < ry + r {
                    in_corner = true;
                    dx = (rx + r) - 1 - px;
                    dy = (ry + r) - 1 - py;
                } else if px >= rx + rw - r && py < ry + r {
                    in_corner = true;
                    dx = px - (rx + rw - r);
                    dy = (ry + r) - 1 - py;
                } else if px < rx + r && py >= ry + rh - r {
                    in_corner = true;
                    dx = (rx + r) - 1 - px;
                    dy = py - (ry + rh - r);
                } else if px >= rx + rw - r && py >= ry + rh - r {
                    in_corner = true;
                    dx = px - (rx + rw - r);
                    dy = py - (ry + rh - r);
                }

                let eff_alpha = if in_corner {
                    let dist2_max = (dx + 1) * (dx + 1) + (dy + 1) * (dy + 1);
                    if dist2_max <= r2 {
                        alpha
                    } else if dx * dx + dy * dy >= (r + 1) * (r + 1) {
                        0
                    } else {
                        let mut count = 0u32;
                        for sy in 0..4 {
                            let sdy = dy * 4 + sy;
                            let sdy2 = sdy * sdy;
                            for sx in 0..4 {
                                let sdx = dx * 4 + sx;
                                if sdx * sdx + sdy2 <= r_sub2 {
                                    count += 1;
                                }
                            }
                        }
                        ((alpha as u32 * count + 8) / 16) as u8
                    }
                } else {
                    alpha
                };

                if eff_alpha > 0 {
                    if eff_alpha == 255 {
                        *row_start.add(x) = color.to_u32();
                    } else {
                        let bg = Color::from_u32(*row_start.add(x));
                        let blended = Self::blend(color, bg, eff_alpha);
                        *row_start.add(x) = blended.to_u32();
                    }
                }
            }
        }
    }

    pub unsafe fn draw_rounded_rect_outline(&mut self, rect: Rect, radius: i32, color: Color) {
        let rx = rect.x;
        let ry = rect.y;
        let rw = rect.width as i32;
        let rh = rect.height as i32;
        let r = radius;
        let inner_r = core::cmp::max(0, r - 1);
        let r_out_sub = r * 4;
        let r_out_sub2 = r_out_sub * r_out_sub;
        let r_in_sub = inner_r * 4;
        let r_in_sub2 = r_in_sub * r_in_sub;

        let x0 = core::cmp::max(0, rx) as usize;
        let y0 = core::cmp::max(0, ry) as usize;
        let x1 = core::cmp::min(self.width, (rx + rw) as usize);
        let y1 = core::cmp::min(self.height, (ry + rh) as usize);

        for y in y0..y1 {
            let py = y as i32;
            let row_start = self.base.add(y * self.pitch);
            for x in x0..x1 {
                let px = x as i32;
                let mut in_corner = false;
                let mut dx = 0i32;
                let mut dy = 0i32;

                if px < rx + r && py < ry + r {
                    in_corner = true;
                    dx = (rx + r) - 1 - px;
                    dy = (ry + r) - 1 - py;
                } else if px >= rx + rw - r && py < ry + r {
                    in_corner = true;
                    dx = px - (rx + rw - r);
                    dy = (ry + r) - 1 - py;
                } else if px < rx + r && py >= ry + rh - r {
                    in_corner = true;
                    dx = (rx + r) - 1 - px;
                    dy = py - (ry + rh - r);
                } else if px >= rx + rw - r && py >= ry + rh - r {
                    in_corner = true;
                    dx = px - (rx + rw - r);
                    dy = py - (ry + rh - r);
                }

                if in_corner {
                    let dist2_min = dx * dx + dy * dy;
                    if dist2_min >= (r + 1) * (r + 1) {
                        continue;
                    }
                    let mut count = 0u32;
                    for sy in 0..4 {
                        let sdy = dy * 4 + sy;
                        let sdy2 = sdy * sdy;
                        for sx in 0..4 {
                            let sdx = dx * 4 + sx;
                            let d2 = sdx * sdx + sdy2;
                            if d2 <= r_out_sub2 && d2 >= r_in_sub2 {
                                count += 1;
                            }
                        }
                    }
                    if count > 0 {
                        let eff_alpha = ((255 * count + 8) / 16) as u8;
                        let bg = Color::from_u32(*row_start.add(x));
                        let blended = Self::blend(color, bg, eff_alpha);
                        *row_start.add(x) = blended.to_u32();
                    }
                } else {
                    if px == rx || px == rx + rw - 1 || py == ry || py == ry + rh - 1 {
                        *row_start.add(x) = color.to_u32();
                    }
                }
            }
        }
    }

    pub unsafe fn draw_oval_pill(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        let radius = (height as i32) / 2;
        self.draw_rounded_rect_alpha(Rect::new(x, y, width, height), radius, color, 255);
    }

    pub unsafe fn draw_circle_alpha(&mut self, cx: i32, cy: i32, radius: i32, color: Color, alpha: u8) {
        let r2 = radius * radius;
        let r_sub = radius * 4;
        let r_sub2 = r_sub * r_sub;
        let x0 = core::cmp::max(0, cx - radius - 1) as usize;
        let y0 = core::cmp::max(0, cy - radius - 1) as usize;
        let x1 = core::cmp::min(self.width, (cx + radius + 2) as usize);
        let y1 = core::cmp::min(self.height, (cy + radius + 2) as usize);

        for y in y0..y1 {
            let dy = (y as i32 - cy).abs();
            let row_start = self.base.add(y * self.pitch);
            for x in x0..x1 {
                let dx = (x as i32 - cx).abs();
                let dist2_min = dx * dx + dy * dy;
                let dist2_max = (dx + 1) * (dx + 1) + (dy + 1) * (dy + 1);

                let eff_alpha = if dist2_max <= r2 {
                    alpha
                } else if dist2_min >= (radius + 1) * (radius + 1) {
                    0
                } else {
                    let mut count = 0u32;
                    for sy in 0..4 {
                        let sdy = dy * 4 + sy;
                        let sdy2 = sdy * sdy;
                        for sx in 0..4 {
                            let sdx = dx * 4 + sx;
                            if sdx * sdx + sdy2 <= r_sub2 {
                                count += 1;
                            }
                        }
                    }
                    ((alpha as u32 * count + 8) / 16) as u8
                };

                if eff_alpha > 0 {
                    if eff_alpha == 255 {
                        *row_start.add(x) = color.to_u32();
                    } else {
                        let bg = Color::from_u32(*row_start.add(x));
                        let blended = Self::blend(color, bg, eff_alpha);
                        *row_start.add(x) = blended.to_u32();
                    }
                }
            }
        }
    }

    /// Renders text with first-class proportional spacing, baseline alignment, and anti-aliased coverage
    pub unsafe fn draw_text_font(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        color: Color,
        face: proportional_font::FontFace,
    ) -> usize {
        let ascent = proportional_font::font_ascent(face);
        let mut cur_x = x;

        for ch in text.chars() {
            if let Some(glyph) = proportional_font::lookup_proportional_glyph(face, ch) {
                let g_w = glyph.width as usize;
                let g_h = glyph.height as usize;
                for r in 0..g_h {
                    for c in 0..g_w {
                        let alpha = glyph.data[r * g_w + c];
                        if alpha > 15 {
                            let px = cur_x as i32 + glyph.bearing_x as i32 + c as i32;
                            let py = y as i32 - glyph.bearing_y as i32 + r as i32 + ascent as i32;
                            if px >= 0 && (px as usize) < self.width && py >= 0 && (py as usize) < self.height {
                                let bg = self.get_pixel(px as usize, py as usize);
                                let blended = Self::blend(color, bg, alpha);
                                self.set_pixel(px as usize, py as usize, blended);
                            }
                        }
                    }
                }
                cur_x += glyph.advance as usize;
            } else {
                let b = ch as u8;
                let glyph = font::get_ascii_glyph(b);
                for row in 0..8 {
                    let byte = glyph[row];
                    for col in 0..8 {
                        if (byte & (0x80 >> col)) != 0 {
                            let px = cur_x + col;
                            let py = y + row;
                            if px < self.width && py < self.height {
                                self.set_pixel(px, py, color);
                            }
                        }
                    }
                }
                cur_x += 8;
            }
        }
        cur_x - x
    }

    /// Renders text strictly clipped to a bounding rectangle
    pub unsafe fn draw_text_font_clipped(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        color: Color,
        face: proportional_font::FontFace,
        clip: Rect,
    ) -> usize {
        let ascent = proportional_font::font_ascent(face);
        let mut cur_x = x;

        for ch in text.chars() {
            if let Some(glyph) = proportional_font::lookup_proportional_glyph(face, ch) {
                let g_w = glyph.width as usize;
                let g_h = glyph.height as usize;
                for r in 0..g_h {
                    for c in 0..g_w {
                        let alpha = glyph.data[r * g_w + c];
                        if alpha > 15 {
                            let px = cur_x as i32 + glyph.bearing_x as i32 + c as i32;
                            let py = y as i32 - glyph.bearing_y as i32 + r as i32 + ascent as i32;
                            if px >= clip.x && px < (clip.x + clip.width as i32) &&
                               py >= clip.y && py < (clip.y + clip.height as i32) &&
                               px >= 0 && (px as usize) < self.width &&
                               py >= 0 && (py as usize) < self.height {
                                let bg = self.get_pixel(px as usize, py as usize);
                                let blended = Self::blend(color, bg, alpha);
                                self.set_pixel(px as usize, py as usize, blended);
                            }
                        }
                    }
                }
                cur_x += glyph.advance as usize;
            } else {
                let b = ch as u8;
                let glyph = font::get_ascii_glyph(b);
                for row in 0..8 {
                    let byte = glyph[row];
                    for col in 0..8 {
                        if (byte & (0x80 >> col)) != 0 {
                            let px = cur_x + col;
                            let py = y + row;
                            if (px as i32) >= clip.x && (px as i32) < (clip.x + clip.width as i32) &&
                               (py as i32) >= clip.y && (py as i32) < (clip.y + clip.height as i32) &&
                               px < self.width && py < self.height {
                                self.set_pixel(px, py, color);
                            }
                        }
                    }
                }
                cur_x += 8;
            }
        }
        cur_x - x
    }

    /// Measures the pixel width of text rendered with the specified proportional font face
    pub fn measure_text_font(text: &str, face: proportional_font::FontFace) -> usize {
        let mut total_w = 0usize;
        for ch in text.chars() {
            if let Some(glyph) = proportional_font::lookup_proportional_glyph(face, ch) {
                total_w += glyph.advance as usize;
            } else {
                total_w += 8;
            }
        }
        total_w
    }


    pub unsafe fn draw_text(&mut self, x: usize, y: usize, text: &str, color: Color) {
        let mut cursor_x = x;
        for b in text.bytes() {
            let glyph = font::get_ascii_glyph(b);
            for row in 0..8 {
                let byte = glyph[row];
                for col in 0..8 {
                    if (byte & (0x80 >> col)) != 0 {
                        self.set_pixel(cursor_x + col, y + row, color);
                    }
                }
            }
            cursor_x += FONT_WIDTH;
        }
    }

    pub unsafe fn draw_text_scale2(&mut self, x: usize, y: usize, text: &str, color: Color) {
        let mut cursor_x = x;
        for b in text.bytes() {
            let glyph = font::get_ascii_glyph(b);
            for row in 0..8 {
                let byte = glyph[row];
                for col in 0..8 {
                    if (byte & (0x80 >> col)) != 0 {
                        self.set_pixel(cursor_x + col * 2, y + row * 2, color);
                        self.set_pixel(cursor_x + col * 2 + 1, y + row * 2, color);
                        self.set_pixel(cursor_x + col * 2, y + row * 2 + 1, color);
                        self.set_pixel(cursor_x + col * 2 + 1, y + row * 2 + 1, color);
                    }
                }
            }
            cursor_x += FONT_WIDTH * 2;
        }
    }

    pub unsafe fn render_complete_desktop(
        &mut self,
        doc_name: &str,
        doc_content: &str,
        operation_stage: &str,
        interactive_status: &str,
        storage_status: &str,
    ) {
        let width = self.width;
        let height = self.height;

        // 1. Root background
        self.fill_rect(Rect::new(0, 0, width as u32, height as u32), Color::SLATE_900);

        // 2. Top Navigation & Telemetry Bar
        self.fill_rect(Rect::new(0, 0, width as u32, 34), Color::SLATE_800);
        self.fill_rect(Rect::new(0, 33, width as u32, 1), Color::SLATE_700);

        self.draw_text(16, 13, "INOS [v0.1.0] Intent-Native Operating System", Color::TEXT_WHITE);

        // Dynamic position for mode buttons and policy indicator
        let badge_start = if width > 900 { width - 440 } else { 450 };
        self.fill_rect(Rect::new(badge_start as i32, 7, 72, 20), Color::EMERALD_500);
        self.draw_text(badge_start + 8, 13, "EXECUTE*", Color::BLACK);

        self.fill_rect(Rect::new((badge_start + 80) as i32, 7, 240, 20), Color::SLATE_700);
        self.draw_text(badge_start + 88, 13, "[POLICY: PQ-STRICT (KAT/STUB)]", Color::EMERALD_500);

        // 3. Primary Intent & Active Document Capsule
        let cap_margin = 20;
        let cap_w = width.saturating_sub(cap_margin * 2);
        let cap_y = 44;
        let cap_h = 44;
        self.fill_rect(Rect::new(cap_margin as i32, cap_y, cap_w as u32, cap_h), Color::SLATE_800);
        self.draw_rect_outline(Rect::new(cap_margin as i32, cap_y, cap_w as u32, cap_h), Color::CYAN_400, 2);

        // Chevron & active document label
        self.draw_text_scale2((cap_margin + 12) as usize, (cap_y + 8) as usize, ">", Color::CYAN_400);
        self.draw_text((cap_margin + 36) as usize, (cap_y + 18) as usize, "Doc: ", Color::SLATE_400);
        self.draw_text((cap_margin + 76) as usize, (cap_y + 18) as usize, doc_name, Color::TEXT_WHITE);
        self.draw_text((cap_margin + 260) as usize, (cap_y + 18) as usize, "Mode: ", Color::SLATE_400);
        self.draw_text((cap_margin + 308) as usize, (cap_y + 18) as usize, operation_stage, Color::EMERALD_500);

        // Right side badge inside capsule
        let right_badge_x = cap_margin + cap_w.saturating_sub(230);
        self.fill_rect(Rect::new(right_badge_x as i32, cap_y + 12, 218, 20), Color::SLATE_700);
        self.draw_text(right_badge_x + 8, (cap_y + 18) as usize, "INPUT: PS/2 8042 (PORT 0x60)", Color::CYAN_400);

        // 4. Windows Layout (Dynamic 2-column split)
        let win_top = 98;
        let dock_h = if height > 750 { 160 } else { 130 };
        let win_h = height.saturating_sub(win_top + dock_h + 16);
        let side_margin = 20;
        let col_gap = 16;
        let available_w = width.saturating_sub(side_margin * 2 + col_gap);
        let win_w = available_w / 2;

        let w1_x = side_margin as i32;
        let w1_y = win_top as i32;
        let w2_x = (side_margin + win_w + col_gap) as i32;
        let w2_y = win_top as i32;

        // --- Window 1: Native Document Editor ---
        self.fill_rect(Rect::new(w1_x, w1_y, win_w as u32, 26), Color::SLATE_700);
        self.fill_rect(Rect::new(w1_x + 10, w1_y + 9, 8, 8), Color::ROSE_500);
        self.fill_rect(Rect::new(w1_x + 22, w1_y + 9, 8, 8), Color::AMBER_400);
        self.fill_rect(Rect::new(w1_x + 34, w1_y + 9, 8, 8), Color::EMERALD_500);
        self.draw_text((w1_x + 48) as usize, (w1_y + 9) as usize, "W1: Document Editor & Filesystem View", Color::TEXT_WHITE);

        self.fill_rect(Rect::new(w1_x, w1_y + 26, win_w as u32, (win_h - 26) as u32), Color::SLATE_800);
        self.draw_rect_outline(Rect::new(w1_x, w1_y, win_w as u32, win_h as u32), Color::SLATE_700, 1);

        self.draw_text((w1_x + 14) as usize, (w1_y + 36) as usize, "File Target: /data/", Color::SLATE_400);
        self.draw_text((w1_x + 158) as usize, (w1_y + 36) as usize, doc_name, Color::CYAN_400);
        self.draw_text((w1_x + 14) as usize, (w1_y + 52) as usize, "FS Engine:   INOS-FS (Persistent Inodes + WAL)", Color::EMERALD_500);

        // Document Content Sub-box
        let edit_box_y = w1_y + 72;
        let edit_box_h = win_h.saturating_sub(108);
        self.fill_rect(Rect::new(w1_x + 14, edit_box_y, (win_w - 28) as u32, edit_box_h as u32), Color::SLATE_900);
        self.draw_rect_outline(Rect::new(w1_x + 14, edit_box_y, (win_w - 28) as u32, edit_box_h as u32), Color::SLATE_700, 1);

        self.draw_text((w1_x + 24) as usize, (edit_box_y + 12) as usize, "--- Content Buffer (Decoded from Hardware Scancodes) ---", Color::SLATE_400);

        // Render document content lines
        if doc_content.is_empty() {
            self.draw_text((w1_x + 24) as usize, (edit_box_y + 36) as usize, "[Empty document - awaiting input...]", Color::SLATE_700);
        } else {
            // Draw up to 2 lines of text
            let max_line = (win_w - 48) / FONT_WIDTH;
            if doc_content.len() <= max_line {
                self.draw_text((w1_x + 24) as usize, (edit_box_y + 36) as usize, doc_content, Color::TEXT_WHITE);
                let cur_x = (w1_x + 24) as usize + doc_content.len() * FONT_WIDTH;
                self.fill_rect(Rect::new(cur_x as i32, edit_box_y + 34, 8, 14), Color::CYAN_400);
            } else {
                let (line1, line2) = doc_content.split_at(core::cmp::min(doc_content.len(), max_line));
                self.draw_text((w1_x + 24) as usize, (edit_box_y + 36) as usize, line1, Color::TEXT_WHITE);
                self.draw_text((w1_x + 24) as usize, (edit_box_y + 54) as usize, line2, Color::TEXT_WHITE);
                let cur_x = (w1_x + 24) as usize + line2.len() * FONT_WIDTH;
                self.fill_rect(Rect::new(cur_x as i32, edit_box_y + 52, 8, 14), Color::CYAN_400);
            }
        }

        self.draw_text((w1_x + 24) as usize, (edit_box_y + edit_box_h as i32 - 24) as usize, "Snapshot Engine: LBA 12..15 reserved for persistent rollback", Color::SLATE_400);

        // --- Window 2: Hardware Controller & Verification Monitor ---
        self.fill_rect(Rect::new(w2_x, w2_y, win_w as u32, 26), Color::SLATE_700);
        self.fill_rect(Rect::new(w2_x + 10, w2_y + 9, 8, 8), Color::ROSE_500);
        self.fill_rect(Rect::new(w2_x + 22, w2_y + 9, 8, 8), Color::AMBER_400);
        self.fill_rect(Rect::new(w2_x + 34, w2_y + 9, 8, 8), Color::EMERALD_500);
        self.draw_text((w2_x + 48) as usize, (w2_y + 9) as usize, "W2: Hardware Verification & Storage Telemetry", Color::TEXT_WHITE);

        self.fill_rect(Rect::new(w2_x, w2_y + 26, win_w as u32, (win_h - 26) as u32), Color::SLATE_800);
        self.draw_rect_outline(Rect::new(w2_x, w2_y, win_w as u32, win_h as u32), Color::SLATE_700, 1);

        self.draw_text((w2_x + 14) as usize, (w2_y + 38) as usize, "Input Pipeline: Intel 8042 PS/2 Controller", Color::CYAN_400);
        self.draw_text((w2_x + 14) as usize, (w2_y + 54) as usize, "Scancode Port:  0x60 (Data) / 0x64 (Status Register)", Color::TEXT_WHITE);
        self.draw_text((w2_x + 14) as usize, (w2_y + 72) as usize, interactive_status, Color::AMBER_400);

        let diag_y = w2_y + 96;
        self.fill_rect(Rect::new(w2_x + 14, diag_y, (win_w - 28) as u32, (win_h - 130) as u32), Color::SLATE_900);
        self.draw_rect_outline(Rect::new(w2_x + 14, diag_y, (win_w - 28) as u32, (win_h - 130) as u32), Color::SLATE_700, 1);

        self.draw_text((w2_x + 24) as usize, (diag_y + 12) as usize, "Verification Invariants:", Color::TEXT_WHITE);
        self.draw_text((w2_x + 24) as usize, (diag_y + 30) as usize, "* Input Origin: QEMU sendkey -> i8042 -> inb(0x60)", Color::EMERALD_500);
        self.draw_text((w2_x + 24) as usize, (diag_y + 48) as usize, "* Arbitrary Data: Decoded ASCII string, non-hardcoded", Color::EMERALD_500);
        self.draw_text((w2_x + 24) as usize, (diag_y + 66) as usize, "* Persistence: Inodes & Dirs written to raw disk sectors", Color::EMERALD_500);
        self.draw_text((w2_x + 24) as usize, (diag_y + 84) as usize, "* Undo Verification: Cold reboot preserves rollback", Color::EMERALD_500);
        self.draw_text((w2_x + 24) as usize, (diag_y + 102) as usize, "* Host Protection: Strictly virtual devices; 0 host touch", Color::CYAN_400);

        // 5. Bottom Native Dock & Storage Status
        let dock_y = height.saturating_sub(dock_h + 8) as i32;
        let dock_w = width.saturating_sub(side_margin * 2) as u32;

        self.fill_rect(Rect::new(side_margin as i32, dock_y, dock_w, 24), Color::SLATE_700);
        self.draw_text((side_margin + 12) as usize, (dock_y + 8) as usize, "INOS-FS Native Directory & Physical Drive Status", Color::TEXT_WHITE);

        self.fill_rect(Rect::new(side_margin as i32, dock_y + 24, dock_w, (dock_h - 24) as u32), Color::SLATE_800);
        self.draw_rect_outline(Rect::new(side_margin as i32, dock_y, dock_w, dock_h as u32), Color::SLATE_700, 1);

        self.draw_text((side_margin + 16) as usize, (dock_y + 36) as usize, storage_status, Color::EMERALD_500);

        let file_row_y = dock_y + 54;
        self.draw_text((side_margin + 16) as usize, file_row_y as usize, "[INOS-FS] report.txt          Type: TextDocument  WAL: Committed", Color::TEXT_WHITE);
        self.draw_text((side_margin + 16) as usize, (file_row_y + 18) as usize, "[INOS-FS] kernel_spec.txt     Type: SpecDoc       WAL: Clean", Color::SLATE_400);

        let btn_y = dock_y + dock_h as i32 - 28;
        self.fill_rect(Rect::new(side_margin as i32 + 12, btn_y, 110, 22), Color::SLATE_700);
        self.draw_text((side_margin + 20) as usize, (btn_y + 7) as usize, "F1: Create", Color::TEXT_WHITE);

        self.fill_rect(Rect::new(side_margin as i32 + 130, btn_y, 110, 22), Color::SLATE_700);
        self.draw_text((side_margin + 138) as usize, (btn_y + 7) as usize, "F2: Save", Color::TEXT_WHITE);

        self.fill_rect(Rect::new(side_margin as i32 + 248, btn_y, 110, 22), Color::SLATE_700);
        self.draw_text((side_margin + 256) as usize, (btn_y + 7) as usize, "F3: Undo", Color::TEXT_WHITE);

        self.fill_rect(Rect::new(side_margin as i32 + 366, btn_y, 130, 22), Color::SLATE_700);
        self.draw_text((side_margin + 374) as usize, (btn_y + 7) as usize, "F10: Reboot", Color::ROSE_500);
    }

    pub unsafe fn render_clarification_dialog(
        &mut self,
        prompt: &str,
        ambiguity_reason: &str,
        option1: &str,
        option2: &str,
        option3: &str,
    ) {
        let width = self.width;
        let height = self.height;

        let card_w = 680u32;
        let card_h = 340u32;
        let card_x = ((width as u32).saturating_sub(card_w)) / 2;
        let card_y = ((height as u32).saturating_sub(card_h)) / 2;

        // Modal shadow & body
        self.fill_rect(Rect::new(card_x as i32 + 6, card_y as i32 + 6, card_w, card_h), Color::BLACK);
        self.fill_rect(Rect::new(card_x as i32, card_y as i32, card_w, card_h), Color::SLATE_800);
        self.draw_rect_outline(Rect::new(card_x as i32, card_y as i32, card_w, card_h), Color::AMBER_400, 2);

        // Header bar
        self.fill_rect(Rect::new(card_x as i32, card_y as i32, card_w, 36), Color::SLATE_900);
        self.draw_rect_outline(Rect::new(card_x as i32, card_y as i32, card_w, 36), Color::AMBER_400, 1);
        self.draw_text_scale2((card_x + 16) as usize, (card_y + 10) as usize, "!", Color::AMBER_400);
        self.draw_text((card_x + 40) as usize, (card_y + 12) as usize, "INTENT CLARIFICATION REQUIRED (Arbitration Intercept)", Color::AMBER_400);

        // Ambiguity description
        self.draw_text((card_x + 24) as usize, (card_y + 52) as usize, "User Directive:", Color::SLATE_400);
        self.draw_text((card_x + 140) as usize, (card_y + 52) as usize, prompt, Color::TEXT_WHITE);

        self.draw_text((card_x + 24) as usize, (card_y + 74) as usize, "Arbiter Notice:", Color::SLATE_400);
        self.draw_text((card_x + 140) as usize, (card_y + 74) as usize, ambiguity_reason, Color::CYAN_400);

        // Options sub-panel
        let opt_y = card_y + 106;
        let opt_h = 160u32;
        let opt_w = card_w - 48;
        self.fill_rect(Rect::new((card_x + 24) as i32, opt_y as i32, opt_w, opt_h), Color::SLATE_900);
        self.draw_rect_outline(Rect::new((card_x + 24) as i32, opt_y as i32, opt_w, opt_h), Color::SLATE_700, 1);

        self.draw_text((card_x + 36) as usize, (opt_y + 16) as usize, "Select execution path to proceed:", Color::SLATE_400);

        // Option 1
        self.fill_rect(Rect::new((card_x + 36) as i32, (opt_y + 38) as i32, opt_w - 24, 28), Color::SLATE_800);
        self.draw_rect_outline(Rect::new((card_x + 36) as i32, (opt_y + 38) as i32, opt_w - 24, 28), Color::EMERALD_500, 1);
        self.draw_text((card_x + 46) as usize, (opt_y + 46) as usize, "[1] ", Color::EMERALD_500);
        self.draw_text((card_x + 72) as usize, (opt_y + 46) as usize, option1, Color::TEXT_WHITE);

        // Option 2
        self.fill_rect(Rect::new((card_x + 36) as i32, (opt_y + 74) as i32, opt_w - 24, 28), Color::SLATE_800);
        self.draw_text((card_x + 46) as usize, (opt_y + 82) as usize, "[2] ", Color::CYAN_400);
        self.draw_text((card_x + 72) as usize, (opt_y + 82) as usize, option2, Color::TEXT_WHITE);

        // Option 3
        self.fill_rect(Rect::new((card_x + 36) as i32, (opt_y + 110) as i32, opt_w - 24, 28), Color::SLATE_800);
        self.draw_text((card_x + 46) as usize, (opt_y + 118) as usize, "[3] ", Color::ROSE_500);
        self.draw_text((card_x + 72) as usize, (opt_y + 118) as usize, option3, Color::TEXT_WHITE);

        // Footer instructions
        self.draw_text((card_x + 24) as usize, (card_y + card_h - 26) as usize, "Press [1], [2], or [3] via PS/2 Keyboard | Zero Capabilities Issued Until Confirmed", Color::SLATE_400);
    }

    // =========================================================================
    // === GATE UI-01: HABITABLE COMPUTING FOUNDATION DESKTOP ENGINE ===
    // =========================================================================

    /// Renders the serene, human-centered Habitable Computing Desktop
    pub unsafe fn render_habitable_desktop(
        &mut self,
        doc_name: &str,
        doc_content: &str,
        status_hint: &str,
        storage_status: &str,
        intent_input: &str,
        show_intent_bar: bool,
    ) {
        let width = self.width;
        let height = self.height;

        // 1. Root canvas background: Warm Linen Desk (#F5F2EB)
        self.fill_rect(Rect::new(0, 0, width as u32, height as u32), Color::PAPER_BG);

        // 2. Top Navigation Shelf (Height 44px)
        self.fill_rect(Rect::new(0, 0, width as u32, 44), Color::SHELF_BG);
        self.fill_rect(Rect::new(0, 43, width as u32, 1), Color::BORDER_HAIRLINE);

        // Breadcrumb & App Identity
        let mut cur_x = 28;
        cur_x += self.draw_text_proportional(cur_x, 14, "INOS", Color::INK_PRIMARY, 2) + 12;
        cur_x += self.draw_text_proportional(cur_x, 18, "/", Color::INK_MUTED, 1) + 12;
        cur_x += self.draw_text_proportional(cur_x, 18, "Documents", Color::INK_MUTED, 1) + 8;
        cur_x += self.draw_text_proportional(cur_x, 18, "/", Color::INK_MUTED, 1) + 8;
        self.draw_text_proportional(cur_x, 18, doc_name, Color::INK_PRIMARY, 1);

        // Center / Right Status Badge
        let status_x = if width > 900 { width - 480 } else { 450 };
        self.draw_text_proportional(status_x, 18, status_hint, Color::SAGE_GREEN, 1);

        // Details / Engineer View Hint Badge
        let details_x = width.saturating_sub(140);
        self.draw_card(details_x, 10, 116, 24, Color::PAPER_BG, Color::BORDER_HAIRLINE, false);
        self.draw_text_proportional(details_x + 8, 15, "Tab: Details", Color::DEEP_NAVY, 1);

        // 3. Central Document Canvas Sheet ("Paper on a wooden desk")
        let canvas_w = 840usize;
        let canvas_h = height.saturating_sub(44 + 48 + 32); // e.g. 800 - 124 = 676
        let canvas_x = (width.saturating_sub(canvas_w)) / 2;
        let canvas_y = 60usize;

        // Draw ivory document sheet with shadow and hairline border
        self.draw_card(canvas_x, canvas_y, canvas_w, canvas_h, Color::CANVAS_SHEET, Color::BORDER_HAIRLINE, true);

        // Document Title (Scale 2 Proportional Typography)
        let doc_title_y = canvas_y + 36;
        self.draw_text_proportional(canvas_x + 48, doc_title_y, doc_name, Color::INK_PRIMARY, 2);

        // Document Metadata
        let meta_y = doc_title_y + 26;
        self.draw_text_proportional(
            canvas_x + 48,
            meta_y,
            "Plain Text Document  *  INOS-FS (Persistent)  *  UTF-8",
            Color::INK_MUTED,
            1,
        );

        // Subtle divider hairline
        let line_y = meta_y + 20;
        self.fill_rect(
            Rect::new(
                (canvas_x + 48) as i32,
                line_y as i32,
                (canvas_w.saturating_sub(96)) as u32,
                1,
            ),
            Color::BORDER_HAIRLINE,
        );

        // Document Content Body
        let content_y = line_y + 24;
        if doc_content.is_empty() {
            self.draw_text_proportional(
                canvas_x + 48,
                content_y,
                "[Empty document - start typing on keyboard to write...]",
                Color::INK_MUTED,
                1,
            );
            // Blinking bar cursor
            self.fill_rect(Rect::new((canvas_x + 48) as i32, content_y as i32 - 1, 2, 14), Color::TERRACOTTA);
        } else {
            // Render lines with comfortable line height
            let text_w = self.draw_text_proportional(canvas_x + 48, content_y, doc_content, Color::INK_PRIMARY, 1);
            // Bar cursor after content
            self.fill_rect(
                Rect::new((canvas_x + 48 + text_w + 2) as i32, content_y as i32 - 1, 2, 14),
                Color::TERRACOTTA,
            );
        }

        // 4. Intent Bar / Pill Capsule (if active or awaiting input)
        if show_intent_bar {
            let intent_y = canvas_y + canvas_h.saturating_sub(84);
            let intent_w = canvas_w.saturating_sub(96);
            let intent_x = canvas_x + 48;

            self.draw_card(intent_x, intent_y, intent_w, 42, Color::CARD_BG, Color::TERRACOTTA, true);
            let mut ix = intent_x + 16;
            ix += self.draw_text_proportional(ix, intent_y + 14, "Intent: ", Color::TERRACOTTA, 1) + 6;
            let typed_w = self.draw_text_proportional(ix, intent_y + 14, intent_input, Color::INK_PRIMARY, 1);
            // Intent cursor
            self.fill_rect(Rect::new((ix + typed_w + 2) as i32, (intent_y + 13) as i32, 2, 14), Color::TERRACOTTA);
        }

        // 5. Bottom System Shelf (Height 40px)
        let shelf_y = height.saturating_sub(40);
        self.fill_rect(Rect::new(0, shelf_y as i32, width as u32, 40), Color::SHELF_BG);
        self.fill_rect(Rect::new(0, shelf_y as i32, width as u32, 1), Color::BORDER_HAIRLINE);

        // Storage / WAL Status
        self.draw_text_proportional(28, shelf_y + 14, storage_status, Color::INK_MUTED, 1);

        // Action Keycap Badges
        let mut key_x = width.saturating_sub(480);
        key_x += self.draw_keycap_badge(key_x, shelf_y + 10, "F1", "New", Color::INK_PRIMARY);
        key_x += self.draw_keycap_badge(key_x, shelf_y + 10, "F2", "Save", Color::SAGE_GREEN);
        key_x += self.draw_keycap_badge(key_x, shelf_y + 10, "F3", "Undo", Color::WARM_AMBER);
        self.draw_keycap_badge(key_x, shelf_y + 10, "F10", "Reboot", Color::BRICK_RED);
    }

    /// Renders the Natural Language Confirmation Modal (Human Communication)
    pub unsafe fn render_natural_confirmation_modal(
        &mut self,
        title: &str,
        explanation_line1: &str,
        explanation_line2: &str,
        opt1_label: &str,
        opt2_label: &str,
    ) {
        let width = self.width;
        let height = self.height;

        let modal_w = 680usize;
        let modal_h = 320usize;
        let modal_x = (width.saturating_sub(modal_w)) / 2;
        let modal_y = (height.saturating_sub(modal_h)) / 2;

        // Card with soft shadow
        self.draw_card(modal_x, modal_y, modal_w, modal_h, Color::CARD_BG, Color::BORDER_HAIRLINE, true);

        // Top decorative accent strip (Terracotta)
        self.fill_rect(
            Rect::new(modal_x as i32 + 1, modal_y as i32 + 1, (modal_w - 2) as u32, 4),
            Color::TERRACOTTA,
        );

        // Natural Header / Question (Scale 2 Proportional)
        self.draw_text_proportional(modal_x + 40, modal_y + 36, title, Color::INK_PRIMARY, 2);

        // Plain English explanation lines
        self.draw_text_proportional(modal_x + 40, modal_y + 74, explanation_line1, Color::INK_MUTED, 1);
        self.draw_text_proportional(modal_x + 40, modal_y + 96, explanation_line2, Color::INK_MUTED, 1);

        // Option [1]: Affirmative Action (Sage Green Outline, Safe Execution)
        let opt1_y = modal_y + 136;
        let opt_w = modal_w - 80;
        self.draw_card(modal_x + 40, opt1_y, opt_w, 44, Color::SHELF_BG, Color::SAGE_GREEN, false);
        self.draw_text_proportional(modal_x + 56, opt1_y + 14, "[1]", Color::SAGE_GREEN, 1);
        self.draw_text_proportional(modal_x + 92, opt1_y + 14, opt1_label, Color::INK_PRIMARY, 1);

        // Option [2]: Cancel / Decline Action
        let opt2_y = modal_y + 192;
        self.draw_card(modal_x + 40, opt2_y, opt_w, 44, Color::SHELF_BG, Color::BORDER_HAIRLINE, false);
        self.draw_text_proportional(modal_x + 56, opt2_y + 14, "[2]", Color::INK_MUTED, 1);
        self.draw_text_proportional(modal_x + 92, opt2_y + 14, opt2_label, Color::INK_PRIMARY, 1);

        // Footer / Engineer View Hint
        let footer_y = modal_y + modal_h - 32;
        self.draw_text_proportional(
            modal_x + 40,
            footer_y,
            "Press [1] to make copy  *  Press [2] to cancel  *  Press [Tab] for details",
            Color::INK_MUTED,
            1,
        );
    }

    /// Renders the Natural Language Transaction Receipt Card
    pub unsafe fn render_natural_receipt_card(
        &mut self,
        headline: &str,
        subline: &str,
        tx_id: &str,
    ) {
        let width = self.width;
        let height = self.height;

        let card_w = 720usize;
        let card_h = 190usize;
        let card_x = (width.saturating_sub(card_w)) / 2;
        let card_y = height.saturating_sub(card_h + 90);

        // Receipt Card with Sage Green Border
        self.draw_card(card_x, card_y, card_w, card_h, Color::CARD_BG, Color::SAGE_GREEN, true);

        // Header Accent Strip
        self.fill_rect(
            Rect::new(card_x as i32 + 1, card_y as i32 + 1, (card_w - 2) as u32, 4),
            Color::SAGE_GREEN,
        );

        // Success Headline (Scale 2 Proportional)
        self.draw_text_proportional(card_x + 36, card_y + 28, headline, Color::SAGE_GREEN, 2);

        // Subline
        self.draw_text_proportional(card_x + 36, card_y + 64, subline, Color::INK_PRIMARY, 1);

        // Tx ID and durability
        let tx_prefix = "Durability: Committed to ATA Disk  *  Receipt ID: ";
        self.draw_text_proportional(card_x + 36, card_y + 90, tx_prefix, Color::INK_MUTED, 1);
        let id_offset = crate::graphics::typography::measure_text_width(tx_prefix, 1, true);
        self.draw_text_proportional(card_x + 36 + id_offset, card_y + 90, tx_id, Color::INK_PRIMARY, 1);

        // Hairline separator
        self.fill_rect(
            Rect::new((card_x + 36) as i32, (card_y + 116) as i32, (card_w - 72) as u32, 1),
            Color::BORDER_HAIRLINE,
        );

        // Undo Notice and Affordance Badge
        self.draw_text_proportional(
            card_x + 36,
            card_y + 136,
            "Press [F3] at any time to undo this operation safely.",
            Color::INK_PRIMARY,
            1,
        );
        self.draw_keycap_badge(card_x + card_w - 140, card_y + 132, "F3", "Undo", Color::WARM_AMBER);
    }

    /// Renders the Engineer Diagnostics View (Detailed Machine Telemetry)
    pub unsafe fn render_engineer_diagnostics_overlay(
        &mut self,
        doc_name: &str,
        model_name: &str,
        ring3_status: &str,
        tokens_info: &str,
        provenance_info: &str,
        grant_info: &str,
        wal_status: &str,
    ) {
        let width = self.width;
        let height = self.height;

        let diag_w = 980usize;
        let diag_h = 580usize;
        let diag_x = (width.saturating_sub(diag_w)) / 2;
        let diag_y = (height.saturating_sub(diag_h)) / 2;

        // Dark Blueprint / Systems Card
        self.draw_card(diag_x, diag_y, diag_w, diag_h, Color::SLATE_900, Color::SLATE_600, true);

        // Header bar
        self.fill_rect(Rect::new(diag_x as i32, diag_y as i32, diag_w as u32, 36), Color::SLATE_800);
        self.draw_text(diag_x + 20, diag_y + 12, "=== INOS SYSTEM TELEMETRY & CAPABILITY PROVENANCE ===", Color::CYAN_400);

        let mut row_y = diag_y + 54;
        let left_x = diag_x + 28;

        self.draw_text(left_x, row_y, "Target Document:    ", Color::SLATE_400);
        self.draw_text(left_x + 180, row_y, doc_name, Color::TEXT_WHITE);
        row_y += 24;

        self.draw_text(left_x, row_y, "Neural Model:       ", Color::SLATE_400);
        self.draw_text(left_x + 180, row_y, model_name, Color::EMERALD_500);
        row_y += 24;

        self.draw_text(left_x, row_y, "Execution Domain:   ", Color::SLATE_400);
        self.draw_text(left_x + 180, row_y, ring3_status, Color::AMBER_400);
        row_y += 24;

        self.draw_text(left_x, row_y, "Token Generation:   ", Color::SLATE_400);
        self.draw_text(left_x + 180, row_y, tokens_info, Color::TEXT_WHITE);
        row_y += 24;

        self.draw_text(left_x, row_y, "Field Provenance:   ", Color::SLATE_400);
        self.draw_text(left_x + 180, row_y, provenance_info, Color::CYAN_400);
        row_y += 24;

        self.draw_text(left_x, row_y, "Capability Grant:   ", Color::SLATE_400);
        self.draw_text(left_x + 180, row_y, grant_info, Color::EMERALD_500);
        row_y += 24;

        self.draw_text(left_x, row_y, "Storage & WAL:      ", Color::SLATE_400);
        self.draw_text(left_x + 180, row_y, wal_status, Color::TEXT_WHITE);
        row_y += 36;

        // Invariants Panel
        self.draw_card(left_x, row_y, diag_w - 56, 170, Color::SLATE_800, Color::SLATE_700, false);
        self.draw_text(left_x + 16, row_y + 14, "Security & Verification Guarantees:", Color::AMBER_400);
        self.draw_text(left_x + 16, row_y + 38, "[T1] Unprivileged Execution: Neural weights mapped non-executable, ring 3 CPL=3", Color::TEXT_WHITE);
        self.draw_text(left_x + 16, row_y + 60, "[T2] Capability Discipline: Operations require single-use HMAC-SHA-256 tokens", Color::TEXT_WHITE);
        self.draw_text(left_x + 16, row_y + 82, "[T3] Untrusted Proposal: Raw model tokens are untrusted candidate directives", Color::TEXT_WHITE);
        self.draw_text(left_x + 16, row_y + 104, "[T4] Crash Consistency: Atomic 2-phase WAL updates across cold reboot", Color::TEXT_WHITE);
        self.draw_text(left_x + 16, row_y + 126, "[T5] Habitable Privacy: Zero telemetry transmitted to external host", Color::EMERALD_500);

        // Footer
        self.draw_text(
            left_x,
            diag_y + diag_h - 26,
            "Press [Tab] or [Esc] to return to Habitable Document Workspace",
            Color::SLATE_400,
        );
    }
}

pub static mut GLOBAL_GOP: Option<GopBuffer> = None;

pub unsafe fn register_global_gop(base: *mut u32, width: usize, height: usize, pitch: usize) {
    GLOBAL_GOP = Some(GopBuffer::new(base, width, height, pitch));
}


