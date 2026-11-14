// userspace/apps/spin_app/src/main.rs - Non-Yielding Compute Application (Preemption Test)
#![no_std]
#![no_main]

use inos_app_sdk::{debug_print, exit, get_cpl, get_pid};

pub mod engine;

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    let cpl = get_cpl();
    let pid = get_pid();
    let _ = pid;

    debug_print("[SPIN_APP] Non-Yielding Compute Application started.\n");
    if cpl != 3 {
        debug_print("[SPIN_APP] ERROR: Expected CPL=3!\n");
        exit(1);
    }
    debug_print("[SPIN_APP] Hardware CPL=3 confirmed. Entering intensive compute loop (NO YIELD)...\n");

    // Perform non-yielding compute loop
    let mut sum: u64 = 0;
    for i in 0..5_000_000u64 {
        sum = sum.wrapping_add(i.wrapping_mul(31));
        core::hint::black_box(sum);
    }

    debug_print("[SPIN_APP] Non-yielding compute loop completed successfully. Exiting cleanly.\n");
    debug_print("[SPIN_APP_COMPLETED_MARKER_v1.0]\n");
    exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    debug_print("[SPIN_APP] PANIC occurred!\n");
    exit(103);
}
