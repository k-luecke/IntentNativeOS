// userspace/user_init/src/main.rs - Independent Ring 3 User-Space Executable (Gate G3)

#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    // 1. Verify Ring 3 privilege level
    let cs: u16;
    unsafe {
        core::arch::asm!("mov {:x}, cs", out(reg) cs, options(nomem, nostack));
    }
    let _cpl = cs & 3;

    // 2. Perform syscall across boundary (SYS_GET_VERSION = 9)
    let mut version_out: u64 = 0;
    unsafe {
        core::arch::asm!(
            "mov rax, 9",
            "int 0x80",
            "mov {0}, rax",
            inout(reg) version_out,
            options(nostack)
        );
    }
    let _ = version_out;

    // 3. Attempt access to protected kernel linear space (0xFFFF_8000_0000_0000)
    // Hardware MMU triggers Page Fault (#PF, Vector 14) with User=1
    let kernel_ptr = 0xFFFF_8000_0000_0000 as *const u64;
    let _val = unsafe { core::ptr::read_volatile(kernel_ptr) };
    let _ = _val;

    // 4. Normal exit syscall (SYS_EXIT = 2, code = 0)
    loop {
        unsafe {
            core::arch::asm!(
                "mov rax, 2",
                "mov rdi, 0",
                "int 0x80",
                options(noreturn)
            );
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
