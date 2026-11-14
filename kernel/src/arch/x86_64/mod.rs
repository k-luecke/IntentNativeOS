// kernel/src/arch/x86_64/mod.rs

pub mod cpu;
pub mod serial;
pub mod debug_exit;
pub mod paging;
pub mod idt;
pub mod pic;
pub mod timer;
pub mod smp;
pub mod scheduler;
pub mod gdt;
pub mod syscall;
pub mod user_ptr;

