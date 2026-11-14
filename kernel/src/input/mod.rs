// kernel/src/input/mod.rs - Keyboard & Pointer Input Event Subsystem (S005)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Unknown,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Enter,
    Escape,
    Backspace,
    Tab,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Keyboard {
        code: KeyCode,
        scancode: u8,
        pressed: bool,
    },
    Pointer {
        dx: i32,
        dy: i32,
        buttons: u8, // bit 0: left, bit 1: right, bit 2: middle
    },
}

pub const INPUT_QUEUE_CAPACITY: usize = 32;

pub struct InputEventQueue {
    events: [Option<InputEvent>; INPUT_QUEUE_CAPACITY],
    head: usize,
    tail: usize,
    count: usize,
}

impl InputEventQueue {
    pub const fn new() -> Self {
        Self {
            events: [None; INPUT_QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, ev: InputEvent) -> Result<(), ()> {
        if self.count >= INPUT_QUEUE_CAPACITY {
            return Err(()); // Queue overflow
        }
        self.events[self.tail] = Some(ev);
        self.tail = (self.tail + 1) % INPUT_QUEUE_CAPACITY;
        self.count += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<InputEvent> {
        if self.count == 0 {
            return None;
        }
        let ev = self.events[self.head].take();
        self.head = (self.head + 1) % INPUT_QUEUE_CAPACITY;
        self.count -= 1;
        ev
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

pub struct Ps2InputDriver {
    pub queue: InputEventQueue,
    pub pointer_x: i32,
    pub pointer_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl Ps2InputDriver {
    pub const fn new(max_x: i32, max_y: i32) -> Self {
        Self {
            queue: InputEventQueue::new(),
            pointer_x: max_x / 2,
            pointer_y: max_y / 2,
            max_x,
            max_y,
        }
    }

    /// Decodes a PS/2 scancode set 1 byte into an InputEvent
    pub fn handle_scancode(&mut self, scancode: u8) {
        let pressed = (scancode & 0x80) == 0;
        let make_code = scancode & 0x7F;

        let code = match make_code {
            0x01 => KeyCode::Escape,
            0x02 => KeyCode::Digit1,
            0x03 => KeyCode::Digit2,
            0x04 => KeyCode::Digit3,
            0x05 => KeyCode::Digit4,
            0x06 => KeyCode::Digit5,
            0x07 => KeyCode::Digit6,
            0x08 => KeyCode::Digit7,
            0x09 => KeyCode::Digit8,
            0x0A => KeyCode::Digit9,
            0x0B => KeyCode::Digit0,
            0x0E => KeyCode::Backspace,
            0x0F => KeyCode::Tab,
            0x10 => KeyCode::KeyQ,
            0x11 => KeyCode::KeyW,
            0x12 => KeyCode::KeyE,
            0x13 => KeyCode::KeyR,
            0x14 => KeyCode::KeyT,
            0x15 => KeyCode::KeyY,
            0x16 => KeyCode::KeyU,
            0x17 => KeyCode::KeyI,
            0x18 => KeyCode::KeyO,
            0x19 => KeyCode::KeyP,
            0x1C => KeyCode::Enter,
            0x1E => KeyCode::KeyA,
            0x1F => KeyCode::KeyS,
            0x20 => KeyCode::KeyD,
            0x21 => KeyCode::KeyF,
            0x22 => KeyCode::KeyG,
            0x23 => KeyCode::KeyH,
            0x24 => KeyCode::KeyJ,
            0x25 => KeyCode::KeyK,
            0x26 => KeyCode::KeyL,
            0x2C => KeyCode::KeyZ,
            0x2D => KeyCode::KeyX,
            0x2E => KeyCode::KeyC,
            0x2F => KeyCode::KeyV,
            0x30 => KeyCode::KeyB,
            0x31 => KeyCode::KeyN,
            0x32 => KeyCode::KeyM,
            0x39 => KeyCode::Space,
            0x48 => KeyCode::ArrowUp,
            0x4B => KeyCode::ArrowLeft,
            0x4D => KeyCode::ArrowRight,
            0x50 => KeyCode::ArrowDown,
            _ => KeyCode::Unknown,
        };

        let _ = self.queue.push(InputEvent::Keyboard {
            code,
            scancode,
            pressed,
        });
    }

    /// Handles pointer motion deltas and button states
    pub fn handle_mouse_packet(&mut self, dx: i32, dy: i32, buttons: u8) {
        self.pointer_x = core::cmp::max(0, core::cmp::min(self.max_x, self.pointer_x + dx));
        self.pointer_y = core::cmp::max(0, core::cmp::min(self.max_y, self.pointer_y - dy));

        let _ = self.queue.push(InputEvent::Pointer { dx, dy, buttons });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MousePacket {
    pub dx: i32,
    pub dy: i32,
    pub buttons: u8,
    pub left_pressed: bool,
    pub right_pressed: bool,
    pub left_released: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawHardwareInput {
    Keyboard(u8),
    Mouse(MousePacket),
}

pub struct MousePacketDecoder {
    pub phase: u8,
    pub bytes: [u8; 3],
    pub prev_buttons: u8,
}

impl MousePacketDecoder {
    pub const fn new() -> Self {
        Self {
            phase: 0,
            bytes: [0u8; 3],
            prev_buttons: 0,
        }
    }

    pub fn process_byte(&mut self, b: u8) -> Option<MousePacket> {
        match self.phase {
            0 => {
                // Bit 3 MUST be 1 in standard PS/2 mouse packet byte 0.
                // Discard and recover if out of sync.
                if (b & 0x08) != 0 {
                    self.bytes[0] = b;
                    self.phase = 1;
                }
                None
            }
            1 => {
                self.bytes[1] = b;
                self.phase = 2;
                None
            }
            2 => {
                self.bytes[2] = b;
                self.phase = 0; // ready for next packet

                let flags = self.bytes[0];
                let raw_x = self.bytes[1];
                let raw_y = self.bytes[2];

                let x_overflow = (flags & 0x40) != 0;
                let y_overflow = (flags & 0x80) != 0;

                let dx = if x_overflow {
                    0
                } else if (flags & 0x10) != 0 {
                    (raw_x as i16) - 256
                } else {
                    raw_x as i16
                } as i32;

                let dy = if y_overflow {
                    0
                } else if (flags & 0x20) != 0 {
                    (raw_y as i16) - 256
                } else {
                    raw_y as i16
                } as i32;

                let buttons = flags & 0x07;
                let left_pressed = (buttons & 0x01) != 0 && (self.prev_buttons & 0x01) == 0;
                let right_pressed = (buttons & 0x02) != 0 && (self.prev_buttons & 0x02) == 0;
                let left_released = (buttons & 0x01) == 0 && (self.prev_buttons & 0x01) != 0;

                self.prev_buttons = buttons;

                Some(MousePacket {
                    dx,
                    dy,
                    buttons,
                    left_pressed,
                    right_pressed,
                    left_released,
                })
            }
            _ => {
                self.phase = 0;
                None
            }
        }
    }
}

/// Initializes PS/2 auxiliary device (mouse) via 8042 keyboard controller
pub fn init_ps2_mouse() {
    unsafe {
        let wait_input = || {
            for _ in 0..50_000 {
                if (crate::arch::x86_64::cpu::inb(0x64) & 0x02) == 0 {
                    return true;
                }
            }
            false
        };
        let wait_output = || {
            for _ in 0..50_000 {
                if (crate::arch::x86_64::cpu::inb(0x64) & 0x01) != 0 {
                    return true;
                }
            }
            false
        };

        // 1. Enable auxiliary mouse port
        if wait_input() {
            crate::arch::x86_64::cpu::outb(0x64, 0xA8);
        }

        // 2. Read controller command byte, enable IRQ12, clear clock disable
        if wait_input() {
            crate::arch::x86_64::cpu::outb(0x64, 0x20);
            if wait_output() {
                let mut cb = crate::arch::x86_64::cpu::inb(0x60);
                cb |= 0x02; // Enable auxiliary interrupt
                cb &= !0x20; // Enable auxiliary clock
                if wait_input() {
                    crate::arch::x86_64::cpu::outb(0x64, 0x60);
                    if wait_input() {
                        crate::arch::x86_64::cpu::outb(0x60, cb);
                    }
                }
            }
        }

        // 3. Send 0xF6 (Set Defaults)
        if wait_input() {
            crate::arch::x86_64::cpu::outb(0x64, 0xD4);
            if wait_input() {
                crate::arch::x86_64::cpu::outb(0x60, 0xF6);
                if wait_output() {
                    let _ = crate::arch::x86_64::cpu::inb(0x60);
                }
            }
        }

        // 4. Send 0xF4 (Enable Data Reporting)
        if wait_input() {
            crate::arch::x86_64::cpu::outb(0x64, 0xD4);
            if wait_input() {
                crate::arch::x86_64::cpu::outb(0x60, 0xF4);
                if wait_output() {
                    let _ = crate::arch::x86_64::cpu::inb(0x60);
                }
            }
        }
    }
}

/// Polls hardware PS/2 controller status (Port 0x64) and dispatches either keyboard or mouse input
pub fn poll_hardware_input(decoder: &mut MousePacketDecoder) -> Option<RawHardwareInput> {
    unsafe {
        let status = crate::arch::x86_64::cpu::inb(0x64);
        if (status & 0x01) != 0 {
            let data = crate::arch::x86_64::cpu::inb(0x60);
            if (status & 0x20) == 0 {
                // Keyboard scancode
                Some(RawHardwareInput::Keyboard(data))
            } else {
                // Mouse byte
                if let Some(pkt) = decoder.process_byte(data) {
                    Some(RawHardwareInput::Mouse(pkt))
                } else {
                    None
                }
            }
        } else {
            None
        }
    }
}

/// Polls hardware PS/2 controller status (Port 0x64) and reads Data (Port 0x60)
/// if a real hardware/QEMU keypress is available.
pub fn poll_hardware_scancode() -> Option<u8> {
    unsafe {
        let status = crate::arch::x86_64::cpu::inb(0x64);
        if (status & 0x01) != 0 {
            // Bit 0 = 1: Output buffer has data
            if (status & 0x20) == 0 {
                // Bit 5 = 0: Data from keyboard controller
                Some(crate::arch::x86_64::cpu::inb(0x60))
            } else {
                // Drain auxiliary/mouse byte so keyboard doesn't stall
                let _ = crate::arch::x86_64::cpu::inb(0x60);
                None
            }
        } else {
            None
        }
    }
}

/// Flushes any pending stale bytes in the PS/2 controller buffer
pub fn drain_hardware_buffer() {
    unsafe {
        for _ in 0..64 {
            let status = crate::arch::x86_64::cpu::inb(0x64);
            if (status & 0x01) != 0 {
                let _ = crate::arch::x86_64::cpu::inb(0x60);
            } else {
                break;
            }
        }
    }
}

/// Decodes PS/2 Set 1 make scancodes into printable ASCII characters.
pub fn scancode_to_ascii(scancode: u8, shift: bool) -> Option<char> {
    if (scancode & 0x80) != 0 {
        return None; // Key release / break code
    }
    match scancode & 0x7F {
        0x02 => Some(if shift { '!' } else { '1' }),
        0x03 => Some(if shift { '@' } else { '2' }),
        0x04 => Some(if shift { '#' } else { '3' }),
        0x05 => Some(if shift { '$' } else { '4' }),
        0x06 => Some(if shift { '%' } else { '5' }),
        0x07 => Some(if shift { '^' } else { '6' }),
        0x08 => Some(if shift { '&' } else { '7' }),
        0x09 => Some(if shift { '*' } else { '8' }),
        0x0A => Some(if shift { '(' } else { '9' }),
        0x0B => Some(if shift { ')' } else { '0' }),
        0x0C => Some(if shift { '_' } else { '-' }),
        0x0D => Some(if shift { '+' } else { '=' }),
        0x10 => Some(if shift { 'Q' } else { 'q' }),
        0x11 => Some(if shift { 'W' } else { 'w' }),
        0x12 => Some(if shift { 'E' } else { 'e' }),
        0x13 => Some(if shift { 'R' } else { 'r' }),
        0x14 => Some(if shift { 'T' } else { 't' }),
        0x15 => Some(if shift { 'Y' } else { 'y' }),
        0x16 => Some(if shift { 'U' } else { 'u' }),
        0x17 => Some(if shift { 'I' } else { 'i' }),
        0x18 => Some(if shift { 'O' } else { 'o' }),
        0x19 => Some(if shift { 'P' } else { 'p' }),
        0x1E => Some(if shift { 'A' } else { 'a' }),
        0x1F => Some(if shift { 'S' } else { 's' }),
        0x20 => Some(if shift { 'D' } else { 'd' }),
        0x21 => Some(if shift { 'F' } else { 'f' }),
        0x22 => Some(if shift { 'G' } else { 'g' }),
        0x23 => Some(if shift { 'H' } else { 'h' }),
        0x24 => Some(if shift { 'J' } else { 'j' }),
        0x25 => Some(if shift { 'K' } else { 'k' }),
        0x26 => Some(if shift { 'L' } else { 'l' }),
        0x27 => Some(if shift { ':' } else { ';' }),
        0x28 => Some(if shift { '"' } else { '\'' }),
        0x2B => Some(if shift { '|' } else { '\\' }),
        0x2C => Some(if shift { 'Z' } else { 'z' }),
        0x2D => Some(if shift { 'X' } else { 'x' }),
        0x2E => Some(if shift { 'C' } else { 'c' }),
        0x2F => Some(if shift { 'V' } else { 'v' }),
        0x30 => Some(if shift { 'B' } else { 'b' }),
        0x31 => Some(if shift { 'N' } else { 'n' }),
        0x32 => Some(if shift { 'M' } else { 'm' }),
        0x33 => Some(if shift { '<' } else { ',' }),
        0x34 => Some(if shift { '>' } else { '.' }),
        0x35 => Some(if shift { '?' } else { '/' }),
        0x39 => Some(' '), // Space
        _ => None,
    }
}
