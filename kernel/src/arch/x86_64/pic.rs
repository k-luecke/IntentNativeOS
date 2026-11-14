// kernel/src/arch/x86_64/pic.rs - 8259 Programmable Interrupt Controller Remapping (K006)

use super::cpu::{inb, outb};

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x11;
const ICW4_8086: u8 = 0x01;

pub fn remap_pic(offset1: u8, offset2: u8) {
    unsafe {
        // Save interrupt masks
        let mask1 = inb(PIC1_DATA);
        let mask2 = inb(PIC2_DATA);

        // Start initialization sequence in cascade mode
        outb(PIC1_CMD, ICW1_INIT);
        outb(PIC2_CMD, ICW1_INIT);

        // ICW2: Vector offsets
        outb(PIC1_DATA, offset1);
        outb(PIC2_DATA, offset2);

        // ICW3: Cascade wiring
        outb(PIC1_DATA, 4); // Tell Master PIC that Slave PIC is at IRQ2 (0000 0100)
        outb(PIC2_DATA, 2); // Tell Slave PIC its cascade identity (0000 0010)

        // ICW4: 8086 mode
        outb(PIC1_DATA, ICW4_8086);
        outb(PIC2_DATA, ICW4_8086);

        // Mask all hardware IRQs to prevent unhandled timer/hardware interrupts
        outb(PIC1_DATA, 0xFF);
        outb(PIC2_DATA, 0xFF);
    }
}

pub fn send_eoi(irq: u8) {
    unsafe {
        if irq >= 8 {
            outb(PIC2_CMD, 0x20);
        }
        outb(PIC1_CMD, 0x20);
    }
}

pub fn enable_timer_irq() {
    unsafe {
        let mask = inb(PIC1_DATA);
        outb(PIC1_DATA, mask & !0x01);
    }
}

pub fn disable_timer_irq() {
    unsafe {
        let mask = inb(PIC1_DATA);
        outb(PIC1_DATA, mask | 0x01);
    }
}

pub fn init_pit(frequency_hz: u32) {
    unsafe {
        let divisor: u16 = if frequency_hz == 0 {
            0
        } else {
            let d = 1193182 / frequency_hz;
            if d > 65535 { 0 } else { d as u16 }
        };
        outb(0x43, 0x34);
        outb(0x40, (divisor & 0xFF) as u8);
        outb(0x40, ((divisor >> 8) & 0xFF) as u8);
    }
}

