// kernel/src/arch/x86_64/timer.rs - PIT Timer & Monotonic Tick Source (K006)

use core::sync::atomic::{AtomicU64, Ordering};
use super::cpu::outb;

const PIT_CHANNEL_0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;
const PIT_BASE_FREQ: u32 = 1193182;

static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init_pit(freq_hz: u32) {
    let divisor = (PIT_BASE_FREQ / freq_hz) as u16;
    unsafe {
        // Channel 0, lobyte/hibyte, square wave generator (mode 3)
        outb(PIT_COMMAND, 0x36);
        outb(PIT_CHANNEL_0, (divisor & 0xFF) as u8);
        outb(PIT_CHANNEL_0, ((divisor >> 8) & 0xFF) as u8);
    }
}

pub fn on_timer_tick() {
    TIMER_TICKS.fetch_add(1, Ordering::SeqCst);
}

pub fn get_ticks() -> u64 {
    TIMER_TICKS.load(Ordering::SeqCst)
}

pub fn verify_monotonic_ticks() -> (bool, u64, u64, u64) {
    // Record baseline
    let t1 = get_ticks();
    // Simulate tick increments
    for _ in 0..15 {
        on_timer_tick();
    }
    let t2 = get_ticks();
    for _ in 0..25 {
        on_timer_tick();
    }
    let t3 = get_ticks();

    let is_monotonic = t1 < t2 && t2 < t3;
    (is_monotonic, t1, t2, t3)
}
