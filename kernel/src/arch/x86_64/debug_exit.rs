// kernel/src/arch/x86_64/debug_exit.rs - QEMU ISA debug exit device for automated test completion

use super::cpu::outl;

const ISA_DEBUG_EXIT_PORT: u16 = 0xF4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    unsafe {
        outl(ISA_DEBUG_EXIT_PORT, exit_code as u32);
    }
    // If debug exit port is not enabled, halt the CPU
    loop {
        super::cpu::hlt();
    }
}
