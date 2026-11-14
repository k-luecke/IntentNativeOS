// kernel/src/arch/x86_64/gdt.rs - Global Descriptor Table, User Mode Segments & Isolation (K011)

use crate::arch::x86_64::paging::{PageTable, VirtualAddress};

pub const KERNEL_CS: u16 = 0x08;
pub const KERNEL_DS: u16 = 0x10;
pub const USER_CS: u16 = 0x1B; // 0x18 | 3 (RPL 3)
pub const USER_DS: u16 = 0x23; // 0x20 | 3 (RPL 3)

pub const USER_VIRTUAL_MAX: u64 = 0x0000_7FFF_FFFF_FFFF;
pub const KERNEL_VIRTUAL_MIN: u64 = 0xFFFF_8000_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAccessError {
    KernelAddressViolation,
    PageNotPresent,
    KernelOnlyPage,
    NonCanonicalAddress,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GdtDescriptor {
    pub limit: u16,
    pub base: u64,
}

#[repr(C, align(16))]
pub struct Gdt {
    pub entries: [u64; 8],
}

impl Gdt {
    pub const fn new() -> Self {
        Self {
            entries: [
                0, // 0x00: Null descriptor
                0x00AF9A000000FFFF, // 0x08: Kernel 64-bit Code (Present, Ring 0, Exec/Read, Long mode)
                0x00CF92000000FFFF, // 0x10: Kernel 64-bit Data (Present, Ring 0, Read/Write)
                0x00AFFA000000FFFF, // 0x18: User 64-bit Code (Present, Ring 3, Exec/Read, Long mode)
                0x00CFF2000000FFFF, // 0x20: User 64-bit Data (Present, Ring 3, Read/Write)
                0, // 0x28: TSS low (configured dynamically in init_gdt_and_tss)
                0, // 0x30: TSS high (configured dynamically in init_gdt_and_tss)
                0, // 0x38: Reserved
            ],
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct TaskStateSegment {
    pub reserved0: u32,
    pub rsp0: u64,
    pub rsp1: u64,
    pub rsp2: u64,
    pub reserved1: u64,
    pub ist1: u64,
    pub ist2: u64,
    pub ist3: u64,
    pub ist4: u64,
    pub ist5: u64,
    pub ist6: u64,
    pub ist7: u64,
    pub reserved2: u64,
    pub reserved3: u16,
    pub iomap_base: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved1: 0,
            ist1: 0,
            ist2: 0,
            ist3: 0,
            ist4: 0,
            ist5: 0,
            ist6: 0,
            ist7: 0,
            reserved2: 0,
            reserved3: 0,
            iomap_base: core::mem::size_of::<TaskStateSegment>() as u16,
        }
    }
}

pub static mut KERNEL_TSS: TaskStateSegment = TaskStateSegment::new();

#[repr(align(16))]
pub struct AlignedKernelStack(pub [u8; 16384]);

pub static mut KERNEL_RSP0_STACK: AlignedKernelStack = AlignedKernelStack([0u8; 16384]);
pub static mut KERNEL_RSP0_PROC1: AlignedKernelStack = AlignedKernelStack([0u8; 16384]);
pub static mut KERNEL_RSP0_PROC2: AlignedKernelStack = AlignedKernelStack([0u8; 16384]);
pub static mut KERNEL_RSP0_PROC3: AlignedKernelStack = AlignedKernelStack([0u8; 16384]);

pub fn set_tss_rsp0(rsp0: u64) {
    unsafe {
        KERNEL_TSS.rsp0 = rsp0;
    }
}

pub static mut KERNEL_GDT: Gdt = Gdt::new();

pub fn init_gdt_and_tss() {
    unsafe {
        core::arch::asm!("cli");
        let rsp0_top = (KERNEL_RSP0_STACK.0.as_ptr() as u64) + 16384;
        KERNEL_TSS.rsp0 = rsp0_top;

        let tss_addr = (&raw const KERNEL_TSS) as u64;
        let tss_limit = (core::mem::size_of::<TaskStateSegment>() - 1) as u64;

        // Entry 5 (offset 0x28, low 64 bits):
        let tss_low = (tss_limit & 0xFFFF)
            | ((tss_addr & 0x00FF_FFFF) << 16)
            | (0x89u64 << 40) // Present (0x80), DPL=0, Type=9 (64-bit TSS Available)
            | (((tss_limit >> 16) & 0x0F) << 48)
            | (((tss_addr >> 24) & 0xFF) << 56);
        // Entry 6 (offset 0x30, high 64 bits):
        let tss_high = (tss_addr >> 32) & 0xFFFF_FFFF;

        KERNEL_GDT.entries[5] = tss_low;
        KERNEL_GDT.entries[6] = tss_high;

        let gdtr = GdtDescriptor {
            limit: (core::mem::size_of::<Gdt>() - 1) as u16,
            base: (&raw const KERNEL_GDT) as u64,
        };

        core::arch::asm!(
            "lgdt [{0}]",
            in(reg) &gdtr,
            options(readonly, nostack, preserves_flags)
        );

        // Reload segment registers: far jump via retfq
        core::arch::asm!(
            "push 0x08",
            "lea rax, [2f]",
            "push rax",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov ss, ax",
            out("rax") _,
            options(nostack)
        );

        // Load Task Register (0x28)
        core::arch::asm!(
            "ltr {0:x}",
            in(reg) (0x28u16),
            options(nostack)
        );
    }
}

/// Validates whether a user-mode (Ring 3) task is permitted to access the target linear address.
/// Enforces canonical user boundary (0x0000_0000_0000_0000..0x0000_7FFF_FFFF_FFFF)
/// and verifies USER_ACCESSIBLE page table attribute.
pub fn validate_user_access<F>(
    pml4: &PageTable,
    target_virt: u64,
    phys_to_virt: F,
) -> Result<(), UserAccessError>
where
    F: Fn(u64) -> *mut PageTable,
{
    // 1. Structural canonical boundary check
    if target_virt > USER_VIRTUAL_MAX {
        return Err(UserAccessError::KernelAddressViolation);
    }

    // 2. Page table privilege check (PML4 -> PDPT -> PD -> PT)
    let va = VirtualAddress(target_virt);
    let pml4_entry = &pml4.entries[va.pml4_index()];
    if !pml4_entry.is_present() {
        return Err(UserAccessError::PageNotPresent);
    }
    if !pml4_entry.is_user() {
        return Err(UserAccessError::KernelOnlyPage);
    }

    let pdpt = unsafe { &*phys_to_virt(pml4_entry.phys_addr()) };
    let pdpt_entry = &pdpt.entries[va.pdpt_index()];
    if !pdpt_entry.is_present() {
        return Err(UserAccessError::PageNotPresent);
    }
    if !pdpt_entry.is_user() {
        return Err(UserAccessError::KernelOnlyPage);
    }

    if pdpt_entry.is_huge() {
        return Ok(());
    }

    let pd = unsafe { &*phys_to_virt(pdpt_entry.phys_addr()) };
    let pd_entry = &pd.entries[va.pd_index()];
    if !pd_entry.is_present() {
        return Err(UserAccessError::PageNotPresent);
    }
    if !pd_entry.is_user() {
        return Err(UserAccessError::KernelOnlyPage);
    }

    if pd_entry.is_huge() {
        return Ok(());
    }

    let pt = unsafe { &*phys_to_virt(pd_entry.phys_addr()) };
    let pt_entry = &pt.entries[va.pt_index()];
    if !pt_entry.is_present() {
        return Err(UserAccessError::PageNotPresent);
    }
    if !pt_entry.is_user() {
        return Err(UserAccessError::KernelOnlyPage);
    }

    Ok(())
}
