// kernel/src/graphics/typography.rs - Proportional Typography Subsystem for INOS (Gate UI-01)
// Provides proportional character metrics, baseline alignment, hierarchical scales, and text measurement.

#![no_std]

use super::font;
use super::Color;
use super::GopBuffer;

/// Typographic metrics for a single glyph
#[derive(Debug, Clone, Copy)]
pub struct GlyphMetric {
    pub ascii: u8,
    pub width: u8,        // Tight glyph pixel width
    pub advance: u8,      // Proportional advance width including side bearing
    pub left_bearing: u8, // Pixels before glyph ink
}

/// Proportional advance lookup table for ASCII 32 (' ') through 126 ('~')
pub const fn get_glyph_advance(c: u8) -> u8 {
    match c {
        b' ' => 4,
        b'!' | b'.' | b',' | b':' | b';' | b'\'' | b'`' | b'|' => 3,
        b'i' | b'l' => 4,
        b't' | b'I' | b'[' | b']' | b'(' | b')' | b'{' | b'}' => 5,
        b'r' | b'f' | b'j' | b'"' => 5,
        b'c' | b's' | b'z' | b'v' | b'k' | b'x' | b'-' | b'?' => 6,
        b'a' | b'b' | b'd' | b'e' | b'g' | b'h' | b'n' | b'o' | b'p' | b'q' | b'u' | b'y' => 6,
        b'0'..=b'9' => 6,
        b'A' | b'B' | b'C' | b'D' | b'E' | b'F' | b'G' | b'H' | b'J' | b'K' | b'L' |
        b'N' | b'O' | b'P' | b'Q' | b'R' | b'S' | b'T' | b'U' | b'V' | b'X' | b'Y' | b'Z' => 7,
        b'w' | b'm' | b'M' | b'W' | b'@' | b'%' | b'#' | b'&' | b'=' | b'+' => 8,
        _ => 6,
    }
}

/// Computes the pixel width of a string under proportional or monospaced layout
pub fn measure_text_width(text: &str, scale: usize, proportional: bool) -> usize {
    if !proportional {
        return text.len() * 8 * scale;
    }
    let mut total_w = 0usize;
    for b in text.bytes() {
        total_w += (get_glyph_advance(b) as usize) * scale;
    }
    total_w
}

/// Typography engine extending GopBuffer rendering capabilities
impl GopBuffer {
    /// Renders text with first-class proportional spacing, baseline alignment, and scaling
    pub unsafe fn draw_text_proportional(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        color: Color,
        scale: usize,
    ) -> usize {
        let mut cursor_x = x;
        for b in text.bytes() {
            let glyph = font::get_ascii_glyph(b);
            let adv = get_glyph_advance(b) as usize * scale;

            // Draw glyph rows
            for row in 0..8 {
                let byte = glyph[row];
                for col in 0..8 {
                    if (byte & (0x80 >> col)) != 0 {
                        // Scaled pixel fill
                        for sy in 0..scale {
                            for sx in 0..scale {
                                self.set_pixel(
                                    cursor_x + col * scale + sx,
                                    y + row * scale + sy,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
            cursor_x += adv;
        }
        cursor_x - x
    }

    /// Draws a card with subtle hairline border and soft drop-shadow
    pub unsafe fn draw_card(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        bg: Color,
        border: Color,
        shadow: bool,
    ) {
        if shadow {
            // Soft drop shadow (1px right and bottom)
            let shadow_color = Color::BORDER_SHADOW;
            self.fill_rect(
                super::Rect::new(x as i32 + 2, y as i32 + 2, width as u32, height as u32),
                shadow_color,
            );
        }

        // Card body
        self.fill_rect(
            super::Rect::new(x as i32, y as i32, width as u32, height as u32),
            bg,
        );

        // 1px hairline border
        self.draw_rect_outline(
            super::Rect::new(x as i32, y as i32, width as u32, height as u32),
            border,
            1,
        );
    }

    /// Draws an action keycap badge (e.g., "[F2 Save]" or "[1]")
    pub unsafe fn draw_keycap_badge(
        &mut self,
        x: usize,
        y: usize,
        key_label: &str,
        action_label: &str,
        accent: Color,
    ) -> usize {
        let key_w = measure_text_width(key_label, 1, true) + 12;
        let act_w = if action_label.is_empty() { 0 } else { measure_text_width(action_label, 1, true) + 10 };
        let total_w = key_w + act_w;

        // Keycap background
        self.draw_card(x, y, key_w, 20, Color::SHELF_BG, Color::BORDER_HAIRLINE, false);
        self.draw_text_proportional(x + 6, y + 4, key_label, accent, 1);

        if !action_label.is_empty() {
            self.draw_text_proportional(x + key_w + 4, y + 4, action_label, Color::INK_PRIMARY, 1);
        }

        total_w + 12
    }
}
