// kernel/src/ipc/compositor.rs - Typed Bounded Compositor IPC Protocol & Surface Isolation (APP-CAP-01)

use crate::arch::x86_64::serial::SERIAL1;
use crate::arch::x86_64::syscall::SyscallError;

pub const COMPOSITOR_MAGIC: u32 = 0x434D5053; // "CMPS"
pub const COMPOSITOR_VERSION: u32 = 1;
pub const MAX_SURFACES: usize = 8;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorOp {
    RegisterSurface = 1,
    DrawText = 2,
    FillRect = 3,
    Present = 4,
    CloseSurface = 5,
    PollInputEvent = 6,
    SendInputEvent = 7,
    SetClipboard = 8,
    GetClipboard = 9,
    SetFocus = 10,
}

impl CompositorOp {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            1 => Some(CompositorOp::RegisterSurface),
            2 => Some(CompositorOp::DrawText),
            3 => Some(CompositorOp::FillRect),
            4 => Some(CompositorOp::Present),
            5 => Some(CompositorOp::CloseSurface),
            6 => Some(CompositorOp::PollInputEvent),
            7 => Some(CompositorOp::SendInputEvent),
            8 => Some(CompositorOp::SetClipboard),
            9 => Some(CompositorOp::GetClipboard),
            10 => Some(CompositorOp::SetFocus),
            _ => None,
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEventType {
    None = 0,
    KeyDown = 1,
    KeyUp = 2,
    MouseDown = 3,
    MouseUp = 4,
    MouseMove = 5,
    Command = 6,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CompositorInputEvent {
    pub event_type: u32,
    pub arg1: u32, // char or mouse_x
    pub arg2: u32, // scancode or mouse_y
    pub arg3: u32, // modifiers or mouse_button
    pub arg4: u32, // extra flags
}

impl CompositorInputEvent {
    pub const fn empty() -> Self {
        Self {
            event_type: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
        }
    }
}

pub const EVENT_QUEUE_CAPACITY: usize = 16;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CompositorMsg {
    pub magic: u32,
    pub version: u32,
    pub op: u32,
    pub surface_id: u32,
    pub arg1: u32, // x or width or event_type
    pub arg2: u32, // y or height or arg1
    pub arg3: u32, // w or color or arg2
    pub arg4: u32, // h or text_len or arg3
    pub payload: [u8; 64], // text or title or clipboard
}

#[derive(Debug, Clone, Copy)]
pub struct CompositorSurface {
    pub surface_id: u32,
    pub owner_pid: u64,
    pub width: u32,
    pub height: u32,
    pub win_x: i32,
    pub win_y: i32,
    pub title: [u8; 32],
    pub title_len: usize,
    pub is_active: bool,
    pub draw_call_count: u32,
    pub last_text: [u8; 64],
    pub last_text_len: usize,
    pub event_queue: [CompositorInputEvent; EVENT_QUEUE_CAPACITY],
    pub event_head: usize,
    pub event_tail: usize,
    pub event_count: usize,
    pub pressed_keys: [u32; 16],
    pub pressed_count: usize,
    pub overflow_dropped_count: u32,
    pub dirty_min_x: u32,
    pub dirty_min_y: u32,
    pub dirty_max_x: u32,
    pub dirty_max_y: u32,
    pub is_dirty: bool,
    pub dirty_pixels_saved: u64,
}

impl CompositorSurface {
    pub const fn empty() -> Self {
        Self {
            surface_id: 0,
            owner_pid: 0,
            width: 0,
            height: 0,
            win_x: 0,
            win_y: 0,
            title: [0u8; 32],
            title_len: 0,
            is_active: false,
            draw_call_count: 0,
            last_text: [0u8; 64],
            last_text_len: 0,
            event_queue: [CompositorInputEvent::empty(); EVENT_QUEUE_CAPACITY],
            event_head: 0,
            event_tail: 0,
            event_count: 0,
            pressed_keys: [0u32; 16],
            pressed_count: 0,
            overflow_dropped_count: 0,
            dirty_min_x: 0,
            dirty_min_y: 0,
            dirty_max_x: 0,
            dirty_max_y: 0,
            is_dirty: false,
            dirty_pixels_saved: 0,
        }
    }

    pub fn mark_dirty_rect(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if x >= self.width || y >= self.height || w == 0 || h == 0 {
            return;
        }
        let rx = x.saturating_add(w).min(self.width);
        let by = y.saturating_add(h).min(self.height);
        if !self.is_dirty {
            self.dirty_min_x = x;
            self.dirty_min_y = y;
            self.dirty_max_x = rx;
            self.dirty_max_y = by;
            self.is_dirty = true;
        } else {
            if x < self.dirty_min_x {
                self.dirty_min_x = x;
            }
            if y < self.dirty_min_y {
                self.dirty_min_y = y;
            }
            if rx > self.dirty_max_x {
                self.dirty_max_x = rx;
            }
            if by > self.dirty_max_y {
                self.dirty_max_y = by;
            }
        }
    }

    pub fn record_key_event(&mut self, ev: &CompositorInputEvent) {
        if ev.event_type == InputEventType::KeyDown as u32 {
            let key = ev.arg2;
            let already = self.pressed_keys[..self.pressed_count].iter().any(|&k| k == key);
            if !already && self.pressed_count < 16 {
                self.pressed_keys[self.pressed_count] = key;
                self.pressed_count += 1;
            }
        } else if ev.event_type == InputEventType::KeyUp as u32 {
            let key = ev.arg2;
            if let Some(pos) = self.pressed_keys[..self.pressed_count].iter().position(|&k| k == key) {
                for i in pos..self.pressed_count - 1 {
                    self.pressed_keys[i] = self.pressed_keys[i + 1];
                }
                self.pressed_keys[self.pressed_count - 1] = 0;
                self.pressed_count -= 1;
            }
        }
    }

    pub fn synthesize_key_releases(&mut self) {
        if self.pressed_count == 0 {
            return;
        }
        SERIAL1.write_str("[COMPOSITOR_INPUT] Synthesizing KeyUp releases for surface ");
        SERIAL1.write_u64_dec(self.surface_id as u64);
        SERIAL1.write_str(" (count=");
        SERIAL1.write_u64_dec(self.pressed_count as u64);
        SERIAL1.write_str(")\n");

        for i in 0..self.pressed_count {
            let key = self.pressed_keys[i];
            let release_ev = CompositorInputEvent {
                event_type: InputEventType::KeyUp as u32,
                arg1: 0,
                arg2: key,
                arg3: 0,
                arg4: 0x8000,
            };
            if self.event_count < EVENT_QUEUE_CAPACITY {
                self.event_queue[self.event_tail] = release_ev;
                self.event_tail = (self.event_tail + 1) % EVENT_QUEUE_CAPACITY;
                self.event_count += 1;
            }
            self.pressed_keys[i] = 0;
        }
        self.pressed_count = 0;
    }

    pub fn push_event(&mut self, ev: CompositorInputEvent) -> bool {
        if self.event_count >= EVENT_QUEUE_CAPACITY {
            if ev.event_type == InputEventType::MouseMove as u32 {
                self.overflow_dropped_count += 1;
                SERIAL1.write_str("[COMPOSITOR_INPUT] Queue overflow: dropped non-critical pointer event on surface ");
                SERIAL1.write_u64_dec(self.surface_id as u64);
                SERIAL1.write_str("\n");
                return false;
            } else {
                self.overflow_dropped_count += 1;
                SERIAL1.write_str("[COMPOSITOR_INPUT] Queue full on critical event! Sanitizing key state on surface ");
                SERIAL1.write_u64_dec(self.surface_id as u64);
                SERIAL1.write_str("\n");
                self.synthesize_key_releases();
                if self.event_count >= EVENT_QUEUE_CAPACITY {
                    let _ = self.pop_event();
                }
                self.record_key_event(&ev);
                self.event_queue[self.event_tail] = ev;
                self.event_tail = (self.event_tail + 1) % EVENT_QUEUE_CAPACITY;
                self.event_count += 1;
                return true;
            }
        }
        self.record_key_event(&ev);
        self.event_queue[self.event_tail] = ev;
        self.event_tail = (self.event_tail + 1) % EVENT_QUEUE_CAPACITY;
        self.event_count += 1;
        true
    }

    pub fn pop_event(&mut self) -> Option<CompositorInputEvent> {
        if self.event_count == 0 {
            return None;
        }
        let ev = self.event_queue[self.event_head];
        self.event_head = (self.event_head + 1) % EVENT_QUEUE_CAPACITY;
        self.event_count -= 1;
        Some(ev)
    }
}

pub struct CompositorServer {
    pub surfaces: [CompositorSurface; MAX_SURFACES],
    pub next_surface_id: u32,
    pub active_surface_id: u32,
    pub clipboard: [u8; 128],
    pub clip_len: usize,
    pub clipboard_epoch: u64,
    pub interactive_mode: bool,
    pub shift_down: bool,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_decoder: crate::input::MousePacketDecoder,
}

impl CompositorServer {
    pub const fn new() -> Self {
        Self {
            surfaces: [CompositorSurface::empty(); MAX_SURFACES],
            next_surface_id: 1,
            active_surface_id: 0,
            clipboard: [0u8; 128],
            clip_len: 0,
            clipboard_epoch: 1,
            interactive_mode: false,
            shift_down: false,
            mouse_x: 640,
            mouse_y: 400,
            mouse_decoder: crate::input::MousePacketDecoder::new(),
        }
    }

    pub fn poll_hardware_into_surface(&mut self, surface_idx: usize) {
        for _ in 0..4 {
            let Some(raw) = crate::input::poll_hardware_input(&mut self.mouse_decoder) else {
                break;
            };
            match raw {
                crate::input::RawHardwareInput::Keyboard(sc) => {
                    let pressed = (sc & 0x80) == 0;
                    let make = sc & 0x7F;
                    if make == 0x2A || make == 0x36 {
                        self.shift_down = pressed;
                        continue;
                    }
                    let ch: u32 = if pressed {
                        match make {
                            0x1C => b'\n' as u32,
                            0x0E => 0x08,
                            0x0F => b'\t' as u32,
                            0x01 => 0x1B,
                            _ => crate::input::scancode_to_ascii(sc, self.shift_down)
                                .map(|c| c as u32)
                                .unwrap_or(0),
                        }
                    } else {
                        0
                    };
                    let ev = CompositorInputEvent {
                        event_type: if pressed {
                            InputEventType::KeyDown as u32
                        } else {
                            InputEventType::KeyUp as u32
                        },
                        arg1: ch,
                        arg2: make as u32,
                        arg3: if self.shift_down { 1 } else { 0 },
                        arg4: 0,
                    };
                    let _ = self.surfaces[surface_idx].push_event(ev);
                }
                crate::input::RawHardwareInput::Mouse(pkt) => {
                    self.mouse_x = (self.mouse_x + pkt.dx).clamp(0, 1279);
                    self.mouse_y = (self.mouse_y - pkt.dy).clamp(0, 799);
                    let wx = self.surfaces[surface_idx].win_x;
                    let wy = self.surfaces[surface_idx].win_y;
                    let ww = self.surfaces[surface_idx].width as i32;
                    // Titlebar Football Close pill (left) or [X] close button (right) hit test
                    let hit_football_close = self.interactive_mode
                        && self.mouse_y >= wy - 20
                        && self.mouse_y <= wy + 24
                        && self.mouse_x >= wx - 4
                        && self.mouse_x <= wx + 38;
                    let hit_right_close = self.mouse_y >= wy - 16
                        && self.mouse_y < wy + 24
                        && self.mouse_x >= wx + ww - 28
                        && self.mouse_x <= wx + ww;
                    if pkt.left_pressed && (hit_football_close || hit_right_close) {
                        let close_ev = CompositorInputEvent {
                            event_type: InputEventType::KeyDown as u32,
                            arg1: 0x1B,
                            arg2: 0x01,
                            arg3: 0,
                            arg4: 0,
                        };
                        let _ = self.surfaces[surface_idx].push_event(close_ev);
                        continue;
                    }
                    let rel_x = (self.mouse_x - wx).max(0) as u32;
                    let rel_y = (self.mouse_y - (wy + 24)).max(0) as u32;
                    if pkt.left_pressed || pkt.right_pressed {
                        let ev = CompositorInputEvent {
                            event_type: InputEventType::MouseDown as u32,
                            arg1: rel_x,
                            arg2: rel_y,
                            arg3: pkt.buttons as u32,
                            arg4: 0,
                        };
                        let _ = self.surfaces[surface_idx].push_event(ev);
                    } else if pkt.left_released {
                        let ev = CompositorInputEvent {
                            event_type: InputEventType::MouseUp as u32,
                            arg1: rel_x,
                            arg2: rel_y,
                            arg3: 0,
                            arg4: 0,
                        };
                        let _ = self.surfaces[surface_idx].push_event(ev);
                    }
                }
            }
        }
    }

    pub fn set_active_surface(&mut self, surface_id: u32) -> Result<(), SyscallError> {
        if self.active_surface_id == surface_id {
            return Ok(());
        }
        let exists = self.surfaces.iter().any(|s| s.is_active && s.surface_id == surface_id);
        if !exists {
            return Err(SyscallError::ObjectNotFound);
        }
        if self.active_surface_id != 0 {
            let prev_id = self.active_surface_id;
            for s in self.surfaces.iter_mut() {
                if s.is_active && s.surface_id == prev_id {
                    s.synthesize_key_releases();
                    break;
                }
            }
        }
        self.active_surface_id = surface_id;
        SERIAL1.write_str("[COMPOSITOR_IPC] Focus switched to surface ");
        SERIAL1.write_u64_dec(surface_id as u64);
        SERIAL1.write_str("\n");
        Ok(())
    }

    /// Handles a verified compositor IPC message with strict surface ownership enforcement
    pub fn handle_ipc_msg(&mut self, trusted_sender_pid: u64, msg: &mut CompositorMsg) -> Result<u64, SyscallError> {
        if msg.magic != COMPOSITOR_MAGIC {
            SERIAL1.write_str("[COMPOSITOR_IPC] REJECT: Invalid magic 0x");
            SERIAL1.write_u64_hex(msg.magic as u64);
            SERIAL1.write_str("\n");
            return Err(SyscallError::InvalidArgument);
        }

        if msg.version != COMPOSITOR_VERSION {
            SERIAL1.write_str("[COMPOSITOR_IPC] REJECT: Unsupported version\n");
            return Err(SyscallError::InvalidArgument);
        }

        let op = CompositorOp::from_u32(msg.op).ok_or(SyscallError::InvalidArgument)?;

        match op {
            CompositorOp::RegisterSurface => {
                let width = msg.arg1;
                let height = msg.arg2;
                if width == 0 || height == 0 || width > 1280 || height > 800 {
                    return Err(SyscallError::InvalidArgument);
                }

                let slot = self.surfaces.iter().position(|s| !s.is_active)
                    .ok_or(SyscallError::QueueFull)?;

                let surface_id = self.next_surface_id;
                self.next_surface_id += 1;

                let mut title = [0u8; 32];
                let mut actual_title_len = 0usize;
                for i in 0..core::cmp::min(msg.payload.len(), 32) {
                    if msg.payload[i] == 0 {
                        break;
                    }
                    title[i] = msg.payload[i];
                    actual_title_len += 1;
                }

                // Window positioning: center on Habitable Spatial Desktop in interactive_mode
                let (win_x, win_y) = if self.interactive_mode {
                    let cx = ((1280i32 - width as i32) / 2).max(60);
                    let cy = ((720i32 - height as i32) / 2).max(76);
                    (cx, cy)
                } else if slot == 0 {
                    if width > 560 { (60, 56) } else { (40, 80) }
                } else {
                    (640, 80)
                };

                self.surfaces[slot] = CompositorSurface {
                    surface_id,
                    owner_pid: trusted_sender_pid,
                    width,
                    height,
                    win_x,
                    win_y,
                    title,
                    title_len: actual_title_len,
                    is_active: true,
                    draw_call_count: 0,
                    last_text: [0u8; 64],
                    last_text_len: 0,
                    event_queue: [CompositorInputEvent::empty(); EVENT_QUEUE_CAPACITY],
                    event_head: 0,
                    event_tail: 0,
                    event_count: 0,
                    pressed_keys: [0u32; 16],
                    pressed_count: 0,
                    overflow_dropped_count: 0,
                    dirty_min_x: width,
                    dirty_min_y: height,
                    dirty_max_x: 0,
                    dirty_max_y: 0,
                    is_dirty: false,
                    dirty_pixels_saved: 0,
                };

                if self.active_surface_id == 0 || self.interactive_mode {
                    self.active_surface_id = surface_id;
                }

                // Paint native window chrome to hardware GOP framebuffer if available
                unsafe {
                    if let Some(mut gop) = crate::graphics::GLOBAL_GOP {
                        if self.interactive_mode {
                            // Habitable Computing Spatial Window Chrome (UI-03 / UI-04)
                            let frame_rect = crate::graphics::Rect::new(win_x - 14, win_y - 20, width + 28, height + 58);
                            let shadow_rect = crate::graphics::Rect::new(win_x - 10, win_y - 16, width + 28, height + 58);
                            gop.draw_rounded_rect_alpha(shadow_rect, 11, crate::graphics::Color::BLACK, 75);
                            gop.draw_rounded_rect_alpha(frame_rect, 11, crate::graphics::Color::WINDOW_FRAME, 248);

                            let hdr_rect = crate::graphics::Rect::new(win_x - 13, win_y - 19, width + 26, 42);
                            gop.draw_rounded_rect_alpha(hdr_rect, 10, crate::graphics::Color::WINDOW_HEADER, 255);
                            gop.fill_rect(crate::graphics::Rect::new(win_x - 13, win_y + 13, width + 26, 10), crate::graphics::Color::WINDOW_FRAME);
                            gop.draw_rounded_rect_outline(frame_rect, 11, crate::graphics::Color { r: 137, g: 150, b: 110, a: 255 });

                            // Football-Shaped Window Controls (Close #8B4F47, Minimize #8B7043, Maximize #6B8258)
                            gop.draw_oval_pill(win_x + 2, win_y - 2, 15, 8, crate::graphics::Color::CONTROL_CLOSE);
                            gop.draw_oval_pill(win_x + 23, win_y - 2, 15, 8, crate::graphics::Color::CONTROL_MIN);
                            gop.draw_oval_pill(win_x + 44, win_y - 2, 15, 8, crate::graphics::Color::CONTROL_MAX);

                            if let Ok(t_str) = core::str::from_utf8(&title[..actual_title_len]) {
                                gop.draw_text_font(
                                    (win_x + 72) as usize,
                                    (win_y - 8) as usize,
                                    t_str,
                                    crate::graphics::Color::HEADER_TEXT,
                                    crate::graphics::proportional_font::FontFace::Title,
                                );
                                let tw = crate::graphics::GopBuffer::measure_text_font(
                                    t_str,
                                    crate::graphics::proportional_font::FontFace::Title,
                                );
                                gop.draw_text_font(
                                    (win_x + 86) as usize + tw,
                                    (win_y - 5) as usize,
                                    "Ring 3 Native Application * CPL=3 Isolated * [Esc] Close",
                                    crate::graphics::Color::BORDER_INACTIVE,
                                    crate::graphics::proportional_font::FontFace::Ui,
                                );
                            }
                            let client = crate::graphics::Rect::new(win_x, win_y + 24, width, height);
                            gop.draw_rounded_rect_alpha(client, 8, crate::graphics::Color::DOC_SURFACE, 255);
                        } else {
                            let outer = crate::graphics::Rect::new(win_x - 2, win_y - 2, width + 4, height + 28);
                            gop.fill_rect(outer, crate::graphics::Color::from_u32(0xFF334155));
                            let title_bar = crate::graphics::Rect::new(win_x, win_y, width, 24);
                            gop.fill_rect(title_bar, crate::graphics::Color::from_u32(0xFF1E293B));
                            if let Ok(t_str) = core::str::from_utf8(&title[..actual_title_len]) {
                                gop.draw_text(win_x as usize + 10, win_y as usize + 8, t_str, crate::graphics::Color::from_u32(0xFFF8FAFC));
                            }
                            // Close button [X]
                            if width > 40 {
                                let close_rect = crate::graphics::Rect::new(win_x + width as i32 - 24, win_y + 4, 20, 16);
                                gop.fill_rect(close_rect, crate::graphics::Color::from_u32(0xFFDC2626));
                                gop.draw_text((win_x + width as i32 - 18) as usize, win_y as usize + 8, "X", crate::graphics::Color::from_u32(0xFFFFFFFF));
                            }
                            let client = crate::graphics::Rect::new(win_x, win_y + 24, width, height);
                            gop.fill_rect(client, crate::graphics::Color::from_u32(0xFFF8FAFC));
                        }
                    }
                }

                SERIAL1.write_str("[COMPOSITOR_IPC] Surface registered: id=");
                SERIAL1.write_u64_dec(surface_id as u64);
                SERIAL1.write_str(" owner_pid=");
                SERIAL1.write_u64_dec(trusted_sender_pid);
                SERIAL1.write_str(" (");
                SERIAL1.write_u64_dec(width as u64);
                SERIAL1.write_str("x");
                SERIAL1.write_u64_dec(height as u64);
                SERIAL1.write_str(")\n");

                Ok(surface_id as u64)
            }

            CompositorOp::DrawText => {
                let interactive = self.interactive_mode;
                let surface = self.get_owned_surface_mut(msg.surface_id, trusted_sender_pid)?;
                let x = msg.arg1;
                let y = msg.arg2;
                let color_raw = msg.arg3;
                let text_len = msg.arg4 as usize;

                if text_len == 0 || text_len > 64 {
                    return Err(SyscallError::InvalidArgument);
                }

                // Integer overflow and surface boundary validation
                let text_pixel_width = (text_len as u32).checked_mul(8).ok_or(SyscallError::InvalidArgument)?;
                if x.checked_add(text_pixel_width).ok_or(SyscallError::InvalidArgument)? > surface.width {
                    // Out-of-bounds drawing is clamped or rejected fail-closed
                    SERIAL1.write_str("[COMPOSITOR_IPC] DrawText clamped to surface bounds\n");
                }
                if y.checked_add(16).ok_or(SyscallError::InvalidArgument)? > surface.height {
                    return Err(SyscallError::InvalidArgument);
                }

                let same_text = text_len == surface.last_text_len
                    && surface.last_text[..text_len] == msg.payload[..text_len];

                surface.draw_call_count += 1;
                if !same_text {
                    surface.last_text_len = text_len;
                    surface.last_text[..text_len].copy_from_slice(&msg.payload[..text_len]);
                }
                surface.mark_dirty_rect(x, y, text_pixel_width, 16);

                let win_x = surface.win_x;
                let win_y = surface.win_y;
                let surf_w = surface.width;
                let surf_h = surface.height;

                SERIAL1.write_str("[COMPOSITOR_IPC] DrawText received for surface ");
                SERIAL1.write_u64_dec(surface.surface_id as u64);
                SERIAL1.write_str(": '");
                if let Ok(s) = core::str::from_utf8(&surface.last_text[..surface.last_text_len]) {
                    SERIAL1.write_str(s);
                    unsafe {
                        if let Some(mut gop) = crate::graphics::GLOBAL_GOP {
                            let max_chars = ((surf_w.saturating_sub(x)) / 8) as usize;
                            let draw_len = core::cmp::min(s.len(), max_chars);
                            if draw_len > 0 {
                                let raw_rgb = color_raw & 0x00FF_FFFF;
                                let col = if interactive {
                                    // Map app text colors into the Habitable Computing palette + Purple Continuity Ribbon
                                    let r = ((raw_rgb >> 16) & 0xFF) as u32;
                                    let g = ((raw_rgb >> 8) & 0xFF) as u32;
                                    let b = (raw_rgb & 0xFF) as u32;
                                    if y >= surf_h.saturating_sub(50) && y < surf_h.saturating_sub(24) {
                                        crate::graphics::Color { r: 243, g: 232, b: 255, a: 255 } // Crisp Lavender-Ivory (#F3E8FF) on Purple Continuity Ribbon
                                    } else if r + g + b > 520 {
                                        crate::graphics::Color::HEADER_TEXT // Soft Ivory (#E6E0D3)
                                    } else if g > r + 30 && g > 140 {
                                        crate::graphics::Color { r: 184, g: 201, b: 163, a: 255 } // Soft Botanical Sage
                                    } else if r > g + 50 && r > 180 {
                                        crate::graphics::Color { r: 212, g: 132, b: 124, a: 255 } // Muted Clay-Coral
                                    } else {
                                        crate::graphics::Color::DOC_TEXT // Dark Ink (#292823)
                                    }
                                } else {
                                    crate::graphics::Color::from_u32(color_raw | 0xFF000000)
                                };
                                gop.draw_text(win_x as usize + x as usize, win_y as usize + 24 + y as usize, &s[..draw_len], col);
                            }
                        }
                    }
                }
                SERIAL1.write_str("'\n");

                Ok(0)
            }

            CompositorOp::FillRect => {
                let interactive = self.interactive_mode;
                let surface = self.get_owned_surface_mut(msg.surface_id, trusted_sender_pid)?;
                let x = msg.arg1;
                let y = msg.arg2;
                let w = msg.arg3;
                let h = msg.arg4;

                // Enforce integer overflow checking
                let right = x.checked_add(w).ok_or(SyscallError::InvalidArgument)?;
                let bottom = y.checked_add(h).ok_or(SyscallError::InvalidArgument)?;

                surface.draw_call_count += 1;
                surface.mark_dirty_rect(x, y, w, h);

                let win_x = surface.win_x;
                let win_y = surface.win_y;
                let surf_w = surface.width;
                let surf_h = surface.height;
                let raw_color = u32::from_le_bytes([msg.payload[0], msg.payload[1], msg.payload[2], msg.payload[3]]);

                unsafe {
                    if let Some(mut gop) = crate::graphics::GLOBAL_GOP {
                        if x < surf_w && y < surf_h {
                            let cw = core::cmp::min(right, surf_w).saturating_sub(x);
                            let ch = core::cmp::min(bottom, surf_h).saturating_sub(y);
                            if cw > 0 && ch > 0 {
                                let rect = crate::graphics::Rect::new(win_x + x as i32, win_y + 24 + y as i32, cw, ch);
                                let col = if interactive {
                                    // Harmonize app surface & toolbar fills into the Habitable Computing palette + Purple Continuity Ribbon
                                    let rgb = raw_color & 0x00FF_FFFF;
                                    let r = ((rgb >> 16) & 0xFF) as u32;
                                    let g = ((rgb >> 8) & 0xFF) as u32;
                                    let b = (rgb & 0xFF) as u32;
                                    if rgb == 0x0B1325 || (y >= surf_h.saturating_sub(50) && y < surf_h.saturating_sub(24) && w > 200) {
                                        crate::graphics::Color { r: 59, g: 29, b: 84, a: 255 } // Deep Royal Plum (#3B1D54) Continuity Ribbon Bar
                                    } else if rgb == 0x78350F || rgb == 0x064E3B || (y >= surf_h.saturating_sub(50) && y < surf_h.saturating_sub(24) && w <= 200) {
                                        crate::graphics::Color::ACCENT_PURPLE // Vibrant Amethyst Purple (#9333EA) Continuity Ribbon Badge Pill
                                    } else if r > 225 && g > 225 && b > 225 {
                                        crate::graphics::Color::DOC_SURFACE // Subdued Ivory (#B5AEA0)
                                    } else if r > 195 && g > 205 && b > 215 {
                                        crate::graphics::Color { r: 196, g: 190, b: 176, a: 255 } // Muted Cream-Stone (#C4BEB0)
                                    } else if r < 65 && g < 85 && b < 115 {
                                        if y < 56 {
                                            crate::graphics::Color::MODAL_BG // Rich Dark Walnut (#34322C) for Toolbar
                                        } else {
                                            crate::graphics::Color::WINDOW_FRAME // Muted Walnut-Slate (#49473F)
                                        }
                                    } else if b > r + 40 && b > 140 {
                                        crate::graphics::Color { r: 137, g: 150, b: 110, a: 255 } // Muted Sage (#89966E)
                                    } else if g > r + 25 && g > b {
                                        crate::graphics::Color::CONTROL_MAX // Muted Sage-Green (#6B8258)
                                    } else if r > g + 45 && r > b + 45 {
                                        crate::graphics::Color::CONTROL_CLOSE // Muted Clay-Red (#8B4F47)
                                    } else {
                                        crate::graphics::Color::WINDOW_HEADER // Warm Stone (#656158)
                                    }
                                } else {
                                    crate::graphics::Color::from_u32(raw_color | 0xFF000000)
                                };
                                gop.fill_rect(rect, col);
                            }
                        }
                    }
                }

                Ok(0)
            }

            CompositorOp::Present => {
                let surface = self.get_owned_surface_mut(msg.surface_id, trusted_sender_pid)?;
                let full_area = surface.width.saturating_mul(surface.height);
                let dirty_area = if surface.is_dirty {
                    surface.dirty_max_x.saturating_sub(surface.dirty_min_x)
                        .saturating_mul(surface.dirty_max_y.saturating_sub(surface.dirty_min_y))
                } else {
                    0
                };
                surface.dirty_pixels_saved = surface.dirty_pixels_saved
                    .saturating_add(full_area.saturating_sub(dirty_area) as u64);
                surface.is_dirty = false;
                surface.dirty_min_x = surface.width;
                surface.dirty_min_y = surface.height;
                surface.dirty_max_x = 0;
                surface.dirty_max_y = 0;
                SERIAL1.write_str("[COMPOSITOR_IPC] PresentFrame committed for surface ");
                SERIAL1.write_u64_dec(surface.surface_id as u64);
                SERIAL1.write_str("\n");
                Ok(0)
            }

            CompositorOp::CloseSurface => {
                let surface = self.get_owned_surface_mut(msg.surface_id, trusted_sender_pid)?;
                surface.is_active = false;
                surface.synthesize_key_releases();
                if self.active_surface_id == msg.surface_id {
                    self.active_surface_id = self.surfaces.iter().find(|s| s.is_active).map(|s| s.surface_id).unwrap_or(0);
                    SERIAL1.write_str("[COMPOSITOR_IPC] Active surface closed; focus moved to surface ");
                    SERIAL1.write_u64_dec(self.active_surface_id as u64);
                    SERIAL1.write_str("\n");
                }
                SERIAL1.write_str("[COMPOSITOR_IPC] Surface closed: id=");
                SERIAL1.write_u64_dec(msg.surface_id as u64);
                SERIAL1.write_str("\n");
                Ok(0)
            }

            CompositorOp::PollInputEvent => {
                let slot = self.surfaces.iter().position(|s| s.is_active && s.surface_id == msg.surface_id)
                    .ok_or(SyscallError::ObjectNotFound)?;
                if self.surfaces[slot].owner_pid != trusted_sender_pid {
                    return Err(SyscallError::PermissionDenied);
                }
                if self.interactive_mode && self.surfaces[slot].event_count == 0 {
                    self.poll_hardware_into_surface(slot);
                }
                msg.payload[0] = if self.interactive_mode { 1 } else { 0 };
                let surface = &mut self.surfaces[slot];
                if let Some(ev) = surface.pop_event() {
                    msg.arg1 = ev.event_type;
                    msg.arg2 = ev.arg1;
                    msg.arg3 = ev.arg2;
                    msg.arg4 = ev.arg3;
                    Ok(1)
                } else {
                    msg.arg1 = 0;
                    Ok(0)
                }
            }

            CompositorOp::SendInputEvent => {
                let surface = self.get_owned_surface_mut(msg.surface_id, trusted_sender_pid)?;
                let ev = CompositorInputEvent {
                    event_type: msg.arg1,
                    arg1: msg.arg2,
                    arg2: msg.arg3,
                    arg3: msg.arg4,
                    arg4: 0,
                };
                let pushed = surface.push_event(ev);
                Ok(if pushed { 1 } else { 0 })
            }

            CompositorOp::SetFocus => {
                let target_surface_id = msg.surface_id;
                self.set_active_surface(target_surface_id)?;
                Ok(0)
            }

            CompositorOp::SetClipboard => {
                let cap_token_id = (msg.arg2 as u64) | ((msg.arg3 as u64) << 32);
                let cap_token_nonce = msg.arg4 as u64;

                // 1. Verify caller owns active foreground surface
                let is_focused = self.surfaces.iter()
                    .any(|s| s.is_active && s.surface_id == self.active_surface_id && s.owner_pid == trusted_sender_pid);

                if !is_focused {
                    SERIAL1.write_str("[COMPOSITOR_CLIPBOARD] SECURITY VIOLATION: PID=");
                    SERIAL1.write_u64_dec(trusted_sender_pid);
                    SERIAL1.write_str(" is in background. Clipboard write denied fail-closed!\n");
                    return Err(SyscallError::PermissionDenied);
                }

                // 2. Verify capability token with CLIPBOARD_WRITE rights
                let cap_res = unsafe {
                    crate::capability::GLOBAL_CAP_REGISTRY.verify_clipboard_token(
                        trusted_sender_pid,
                        cap_token_id,
                        cap_token_nonce,
                        crate::capability::rights::CLIPBOARD_WRITE,
                        0,
                    )
                };

                if cap_res.is_err() {
                    SERIAL1.write_str("[COMPOSITOR_CLIPBOARD] SECURITY VIOLATION: PID=");
                    SERIAL1.write_u64_dec(trusted_sender_pid);
                    SERIAL1.write_str(" lacks valid CLIPBOARD_WRITE capability token. Write denied fail-closed!\n");
                    return Err(SyscallError::PermissionDenied);
                }

                let len = core::cmp::min(msg.arg1 as usize, 128);
                let copy_len = core::cmp::min(len, msg.payload.len());
                self.clipboard[..copy_len].copy_from_slice(&msg.payload[..copy_len]);
                self.clip_len = copy_len;
                self.clipboard_epoch += 1;

                SERIAL1.write_str("[COMPOSITOR_CLIPBOARD] SetClipboard authorized for PID=");
                SERIAL1.write_u64_dec(trusted_sender_pid);
                SERIAL1.write_str(" len=");
                SERIAL1.write_u64_dec(copy_len as u64);
                SERIAL1.write_str(" epoch=");
                SERIAL1.write_u64_dec(self.clipboard_epoch);
                SERIAL1.write_str("\n");

                Ok(copy_len as u64)
            }

            CompositorOp::GetClipboard => {
                let cap_token_id = (msg.arg2 as u64) | ((msg.arg3 as u64) << 32);
                let cap_token_nonce = msg.arg4 as u64;

                // 1. Verify caller owns active foreground surface
                let is_focused = self.surfaces.iter()
                    .any(|s| s.is_active && s.surface_id == self.active_surface_id && s.owner_pid == trusted_sender_pid);

                if !is_focused {
                    SERIAL1.write_str("[COMPOSITOR_CLIPBOARD] SECURITY VIOLATION: PID=");
                    SERIAL1.write_u64_dec(trusted_sender_pid);
                    SERIAL1.write_str(" is in background. Clipboard read denied fail-closed!\n");
                    return Err(SyscallError::PermissionDenied);
                }

                // 2. Verify capability token with CLIPBOARD_READ rights and ephemeral epoch match
                let cap_res = unsafe {
                    crate::capability::GLOBAL_CAP_REGISTRY.verify_clipboard_token(
                        trusted_sender_pid,
                        cap_token_id,
                        cap_token_nonce,
                        crate::capability::rights::CLIPBOARD_READ,
                        self.clipboard_epoch,
                    )
                };

                if cap_res.is_err() {
                    SERIAL1.write_str("[COMPOSITOR_CLIPBOARD] SECURITY VIOLATION: PID=");
                    SERIAL1.write_u64_dec(trusted_sender_pid);
                    SERIAL1.write_str(" presented invalid or stale CLIPBOARD_READ capability token. Read denied fail-closed!\n");
                    return Err(SyscallError::PermissionDenied);
                }

                let copy_len = core::cmp::min(self.clip_len, msg.payload.len());
                msg.payload[..copy_len].copy_from_slice(&self.clipboard[..copy_len]);
                msg.arg1 = copy_len as u32;

                SERIAL1.write_str("[COMPOSITOR_CLIPBOARD] GetClipboard authorized for PID=");
                SERIAL1.write_u64_dec(trusted_sender_pid);
                SERIAL1.write_str(" len=");
                SERIAL1.write_u64_dec(copy_len as u64);
                SERIAL1.write_str(" epoch=");
                SERIAL1.write_u64_dec(self.clipboard_epoch);
                SERIAL1.write_str("\n");

                Ok(copy_len as u64)
            }
        }
    }

    fn get_owned_surface_mut(&mut self, surface_id: u32, caller_pid: u64) -> Result<&mut CompositorSurface, SyscallError> {
        let surface = self.surfaces.iter_mut()
            .find(|s| s.is_active && s.surface_id == surface_id)
            .ok_or(SyscallError::ObjectNotFound)?;

        // Enforce strict surface ownership
        if surface.owner_pid != caller_pid {
            SERIAL1.write_str("[COMPOSITOR_IPC] SECURITY VIOLATION: PID=");
            SERIAL1.write_u64_dec(caller_pid);
            SERIAL1.write_str(" attempted draw on surface=");
            SERIAL1.write_u64_dec(surface_id as u64);
            SERIAL1.write_str(" owned by PID=");
            SERIAL1.write_u64_dec(surface.owner_pid);
            SERIAL1.write_str(". Access denied fail-closed!\n");
            return Err(SyscallError::PermissionDenied);
        }

        Ok(surface)
    }

    pub fn close_all_surfaces_for_pid(&mut self, target_pid: u64) -> usize {
        let mut count = 0;
        for surface in self.surfaces.iter_mut() {
            if surface.is_active && surface.owner_pid == target_pid {
                surface.is_active = false;
                count += 1;
            }
        }
        count
    }

    pub fn count_active_surfaces_for_pid(&self, target_pid: u64) -> usize {
        let mut count = 0;
        for surface in self.surfaces.iter() {
            if surface.is_active && surface.owner_pid == target_pid {
                count += 1;
            }
        }
        count
    }

    pub fn post_event_by_surface_id(&mut self, surface_id: u32, ev: CompositorInputEvent) -> bool {
        for surface in self.surfaces.iter_mut() {
            if surface.is_active && surface.surface_id == surface_id {
                return surface.push_event(ev);
            }
        }
        false
    }

    pub fn post_event_to_active(&mut self, ev: CompositorInputEvent) -> bool {
        if self.active_surface_id == 0 {
            return false;
        }
        let active_id = self.active_surface_id;
        for surface in self.surfaces.iter_mut() {
            if surface.is_active && surface.surface_id == active_id {
                return surface.push_event(ev);
            }
        }
        false
    }
}

pub static mut GLOBAL_COMPOSITOR_SERVER: CompositorServer = CompositorServer::new();
