// kernel/src/arch/x86_64/serial.rs - 16550 UART COM1 Serial Driver

use super::cpu::{inb, outb};

const COM1: u16 = 0x3F8;

pub struct SerialPort {
    base: u16,
}

impl SerialPort {
    pub const fn new(base: u16) -> Self {
        Self { base }
    }

    pub fn init(&self) {
        unsafe {
            // Disable interrupts
            outb(self.base + 1, 0x00);
            // Enable DLAB (set baud rate divisor)
            outb(self.base + 3, 0x80);
            // Set divisor to 3 (38400 baud) or 1 (115200 baud)
            outb(self.base + 0, 0x01); // 115200 baud low byte
            outb(self.base + 1, 0x00); // high byte
            // 8 bits, no parity, one stop bit
            outb(self.base + 3, 0x03);
            // Enable FIFO, clear them, with 14-byte threshold
            outb(self.base + 2, 0xC7);
            // IRQs enabled, RTS/DSR set
            outb(self.base + 4, 0x0B);
        }
    }

    fn line_status(&self) -> u8 {
        unsafe { inb(self.base + 5) }
    }

    pub fn is_transmit_empty(&self) -> bool {
        (self.line_status() & 0x20) != 0
    }

    pub fn write_byte(&self, _byte: u8) {
        #[cfg(not(test))]
        {
            while !self.is_transmit_empty() {
                core::hint::spin_loop();
            }
            unsafe {
                outb(self.base, _byte);
            }
        }
    }

    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
    }

    pub fn write_hex_byte(&self, b: u8) {
        const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
        self.write_byte(HEX_DIGITS[((b >> 4) & 0x0F) as usize]);
        self.write_byte(HEX_DIGITS[(b & 0x0F) as usize]);
    }

    pub fn write_u64_hex(&self, val: u64) {
        self.write_str("0x");
        const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
        for i in (0..16).rev() {
            let digit = ((val >> (i * 4)) & 0xF) as usize;
            self.write_byte(HEX_DIGITS[digit]);
        }
    }

    pub fn write_u64_dec(&self, mut val: u64) {
        if val == 0 {
            self.write_byte(b'0');
            return;
        }
        let mut buf = [0u8; 20];
        let mut i = 0;
        while val > 0 {
            buf[i] = b'0' + (val % 10) as u8;
            val /= 10;
            i += 1;
        }
        while i > 0 {
            i -= 1;
            self.write_byte(buf[i]);
        }
    }
}

pub static SERIAL1: SerialPort = SerialPort::new(COM1);

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        // Simple macro format
    };
}
