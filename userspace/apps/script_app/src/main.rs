// userspace/apps/script_app/src/main.rs - Transitional Compatibility Wrapper for org.inos.script
#![no_std]
#![no_main]

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    intscript_app::legacy_compat_main();
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    inos_app_sdk::debug_print("[SCRIPT_APP] PANIC occurred!\n");
    inos_app_sdk::exit(101);
}
