// kernel/src/arch/x86_64/cpu.rs - Low-level CPU instructions and I/O port primitives

use core::arch::asm;

#[inline(always)]
pub unsafe fn outb(port: u16, val: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") val,
        options(nomem, nostack, preserves_flags)
    );
}

#[inline(always)]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") val,
        options(nomem, nostack, preserves_flags)
    );
    val
}

#[inline(always)]
pub unsafe fn outw(port: u16, val: u16) {
    asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") val,
        options(nomem, nostack, preserves_flags)
    );
}

#[inline(always)]
pub unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    asm!(
        "in ax, dx",
        in("dx") port,
        out("ax") val,
        options(nomem, nostack, preserves_flags)
    );
    val
}

#[inline(always)]
pub unsafe fn outl(port: u16, val: u32) {
    asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") val,
        options(nomem, nostack, preserves_flags)
    );
}

#[inline(always)]
pub unsafe fn inl(port: u16) -> u32 {
    let val: u32;
    asm!(
        "in eax, dx",
        in("dx") port,
        out("eax") val,
        options(nomem, nostack, preserves_flags)
    );
    val
}

#[inline(always)]
pub fn hlt() {
    unsafe {
        asm!("hlt", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn cli() {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn sti() {
    unsafe {
        asm!("sti", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub unsafe fn read_cr2() -> u64 {
    let val: u64;
    asm!("mov {}, cr2", out(reg) val, options(nomem, nostack, preserves_flags));
    val
}

#[inline(always)]
pub unsafe fn read_cr3() -> u64 {
    #[cfg(test)]
    {
        0
    }
    #[cfg(not(test))]
    {
        let val: u64;
        asm!("mov {}, cr3", out(reg) val, options(nomem, nostack, preserves_flags));
        val
    }
}

#[inline(always)]
pub unsafe fn write_cr3(val: u64) {
    asm!("mov cr3, {}", in(reg) val, options(nomem, nostack, preserves_flags));
}

#[inline(always)]
pub unsafe fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
    ((high as u64) << 32) | (low as u64)
}

#[inline(always)]
pub unsafe fn enable_sse() {
    let mut cr0: u64;
    asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    cr0 &= !(1 << 2); // Clear EM
    cr0 |= 1 << 1;  // Set MP
    asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));

    let mut cr4: u64;
    asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
    cr4 |= (1 << 9) | (1 << 10); // Set OSFXSR and OSXMMEXCPT
    asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
}
