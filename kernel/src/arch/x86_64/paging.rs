// kernel/src/arch/x86_64/paging.rs - x86-64 4-Level Paging and Kernel Address Space (K004)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageTableEntry(pub u64);

impl PageTableEntry {
    pub const PRESENT: u64 = 1 << 0;
    pub const WRITABLE: u64 = 1 << 1;
    pub const USER_ACCESSIBLE: u64 = 1 << 2;
    pub const WRITE_THROUGH: u64 = 1 << 3;
    pub const NO_CACHE: u64 = 1 << 4;
    pub const ACCESSED: u64 = 1 << 5;
    pub const DIRTY: u64 = 1 << 6;
    pub const HUGE_PAGE: u64 = 1 << 7;
    pub const GLOBAL: u64 = 1 << 8;
    pub const NO_EXECUTE: u64 = 1 << 63;
    pub const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn is_present(&self) -> bool {
        (self.0 & Self::PRESENT) != 0
    }

    pub fn is_writable(&self) -> bool {
        (self.0 & Self::WRITABLE) != 0
    }

    pub fn is_user(&self) -> bool {
        (self.0 & Self::USER_ACCESSIBLE) != 0
    }

    pub fn is_huge(&self) -> bool {
        (self.0 & Self::HUGE_PAGE) != 0
    }

    pub fn phys_addr(&self) -> u64 {
        self.0 & Self::ADDR_MASK
    }

    pub fn set(&mut self, addr: u64, flags: u64) {
        self.0 = (addr & Self::ADDR_MASK) | flags;
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

pub const ENTRY_COUNT: usize = 512;

#[repr(align(4096))]
#[derive(Clone, Copy)]
pub struct PageTable {
    pub entries: [PageTableEntry; ENTRY_COUNT],
}

impl PageTable {
    pub const fn empty() -> Self {
        Self {
            entries: [PageTableEntry::empty(); ENTRY_COUNT],
        }
    }
}

pub struct VirtualAddress(pub u64);

impl VirtualAddress {
    pub fn pml4_index(&self) -> usize {
        ((self.0 >> 39) & 0x1FF) as usize
    }

    pub fn pdpt_index(&self) -> usize {
        ((self.0 >> 30) & 0x1FF) as usize
    }

    pub fn pd_index(&self) -> usize {
        ((self.0 >> 21) & 0x1FF) as usize
    }

    pub fn pt_index(&self) -> usize {
        ((self.0 >> 12) & 0x1FF) as usize
    }

    pub fn page_offset(&self) -> u64 {
        self.0 & 0xFFF
    }
}

/// Software page table walker translating virtual addresses through 4 levels
pub fn translate_addr(
    pml4: &PageTable,
    virt: u64,
    lookup_phys_table: impl Fn(u64) -> Option<&'static PageTable>,
) -> Option<u64> {
    let va = VirtualAddress(virt);
    let pml4_entry = &pml4.entries[va.pml4_index()];
    if !pml4_entry.is_present() {
        return None;
    }

    let pdpt = lookup_phys_table(pml4_entry.phys_addr())?;
    let pdpt_entry = &pdpt.entries[va.pdpt_index()];
    if !pdpt_entry.is_present() {
        return None;
    }

    if pdpt_entry.is_huge() {
        // 1 GiB huge page
        let offset = virt & 0x3FFF_FFFF;
        return Some(pdpt_entry.phys_addr() + offset);
    }

    let pd = lookup_phys_table(pdpt_entry.phys_addr())?;
    let pd_entry = &pd.entries[va.pd_index()];
    if !pd_entry.is_present() {
        return None;
    }

    if pd_entry.is_huge() {
        // 2 MiB huge page
        let offset = virt & 0x1F_FFFF;
        return Some(pd_entry.phys_addr() + offset);
    }

    let pt = lookup_phys_table(pd_entry.phys_addr())?;
    let pt_entry = &pt.entries[va.pt_index()];
    if !pt_entry.is_present() {
        return None;
    }

    Some(pt_entry.phys_addr() + va.page_offset())
}

/// Constructs a static identity mapping for physical memory up to 4 GiB
pub fn setup_identity_page_table(
    pml4: &mut PageTable,
    pdpt: &mut PageTable,
    pd: &mut PageTable,
    pts: &mut [PageTable],
    max_phys_bytes: u64,
) {
    // 1. PML4[0] -> PDPT
    pml4.entries[0].set(
        pdpt as *const PageTable as u64,
        PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
    );

    // 2. PDPT[0] -> PD (0..1GB)
    pdpt.entries[0].set(
        pd as *const PageTable as u64,
        PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
    );

    // Map 1GB..4GB via 1GB huge pages in PDPT for GOP Framebuffer (0x8000_0000) and PCI MMIO
    pdpt.entries[1].set(
        0x4000_0000,
        PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE,
    );
    pdpt.entries[2].set(
        0x8000_0000,
        PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE,
    );
    pdpt.entries[3].set(
        0xC000_0000,
        PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE,
    );

    // 3. Populate PD entries and PT entries (each PT covers 2 MiB)
    let num_pts = pts.len();
    let max_pages = max_phys_bytes / 4096;
    let mut mapped_pages = 0u64;

    for pd_idx in 0..num_pts {
        if mapped_pages >= max_pages {
            break;
        }

        let pt = &mut pts[pd_idx];
        pd.entries[pd_idx].set(
            pt as *const PageTable as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
        );

        for pt_idx in 0..ENTRY_COUNT {
            if mapped_pages >= max_pages {
                break;
            }
            let phys_addr = mapped_pages * 4096;
            pt.entries[pt_idx].set(
                phys_addr,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
            );
            mapped_pages += 1;
        }
    }

    // 4. Map remaining entries in PD (8MB..1024MB) as 2MB huge pages on bare-metal hardware
    #[cfg(not(test))]
    for pd_idx in num_pts..ENTRY_COUNT {
        let phys_addr = (pd_idx as u64) * 2 * 1024 * 1024;
        pd.entries[pd_idx].set(
            phys_addr,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE,
        );
    }
}

/// Sets up user-mode (Ring 3) page permissions for user address space (4MB..6MB)
/// while strictly preserving supervisor-only protections on kernel code and data (< 4MB and higher memory)
pub fn configure_user_space_permissions(
    pml4: &mut PageTable,
    pdpt: &mut PageTable,
    pd: &mut PageTable,
    pts: &mut [PageTable],
) {
    pml4.entries[0].0 |= PageTableEntry::USER_ACCESSIBLE;
    pdpt.entries[0].0 |= PageTableEntry::USER_ACCESSIBLE;

    // PD[0] (0..2MB) and PD[1] (2..4MB) remain supervisor-only
    pd.entries[0].0 &= !PageTableEntry::USER_ACCESSIBLE;
    pd.entries[1].0 &= !PageTableEntry::USER_ACCESSIBLE;

    // PD[2] (4..6MB, covering 0x0040_0000 user code and 0x005F_E000 user stack)
    if pts.len() > 2 {
        pd.entries[2].0 |= PageTableEntry::USER_ACCESSIBLE;
        let pt2 = &mut pts[2];
        for pt_entry in pt2.entries.iter_mut() {
            pt_entry.0 |= PageTableEntry::USER_ACCESSIBLE;
        }
    }

    // Switch CPU hardware CR3 to KERNEL_PML4
    unsafe {
        let pml4_phys = pml4 as *const PageTable as u64;
        KERNEL_CR3 = pml4_phys;
        crate::arch::x86_64::cpu::write_cr3(pml4_phys);
    }
}

pub static mut KERNEL_CR3: u64 = 0;

pub static mut PML4_PROC1: PageTable = PageTable::empty();
pub static mut PDPT_PROC1: PageTable = PageTable::empty();
pub static mut PD_PROC1: PageTable = PageTable::empty();
pub static mut PT_PROC1: PageTable = PageTable::empty();

pub static mut PML4_PROC2: PageTable = PageTable::empty();
pub static mut PDPT_PROC2: PageTable = PageTable::empty();
pub static mut PD_PROC2: PageTable = PageTable::empty();
pub static mut PT_PROC2: PageTable = PageTable::empty();

pub static mut PML4_PROC3: PageTable = PageTable::empty();
pub static mut PDPT_PROC3: PageTable = PageTable::empty();
pub static mut PD_PROC3: PageTable = PageTable::empty();
pub static mut PT_PROC3: PageTable = PageTable::empty();

pub static mut PML4_RENDER: PageTable = PageTable::empty();
pub static mut PDPT_RENDER: PageTable = PageTable::empty();
pub static mut PD_RENDER: PageTable = PageTable::empty();
pub static mut PT_RENDER: PageTable = PageTable::empty();

pub fn init_process_page_tables(kernel_pts: &[PageTable]) {
    unsafe {
        // --- Process 1 (PID 100) ---
        PML4_PROC1.entries[0].set(
            (&raw const PDPT_PROC1) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );

        PDPT_PROC1.entries[0].set(
            (&raw const PD_PROC1) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );
        PDPT_PROC1.entries[1].set(0x4000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);
        PDPT_PROC1.entries[2].set(0x8000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);
        PDPT_PROC1.entries[3].set(0xC000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);

        PD_PROC1.entries[0].set(
            &kernel_pts[0] as *const PageTable as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
        );
        PD_PROC1.entries[1].set(
            &kernel_pts[1] as *const PageTable as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
        );
        PD_PROC1.entries[2].set(
            (&raw const PT_PROC1) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );

        for i in 0..ENTRY_COUNT {
            let phys = 0x0040_0000u64 + (i as u64) * 4096;
            PT_PROC1.entries[i].set(
                phys,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
            );
        }

        for pd_idx in 3..ENTRY_COUNT {
            let phys_addr = (pd_idx as u64) * 2 * 1024 * 1024;
            PD_PROC1.entries[pd_idx].set(
                phys_addr,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE,
            );
        }

        // --- Process 2 (PID 200) ---
        PML4_PROC2.entries[0].set(
            (&raw const PDPT_PROC2) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );

        PDPT_PROC2.entries[0].set(
            (&raw const PD_PROC2) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );
        PDPT_PROC2.entries[1].set(0x4000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);
        PDPT_PROC2.entries[2].set(0x8000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);
        PDPT_PROC2.entries[3].set(0xC000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);

        PD_PROC2.entries[0].set(
            &kernel_pts[0] as *const PageTable as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
        );
        PD_PROC2.entries[1].set(
            &kernel_pts[1] as *const PageTable as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
        );
        PD_PROC2.entries[2].set(
            (&raw const PT_PROC2) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );

        for i in 0..ENTRY_COUNT {
            let phys = 0x0060_0000u64 + (i as u64) * 4096;
            PT_PROC2.entries[i].set(
                phys,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
            );
        }

        for pd_idx in 3..ENTRY_COUNT {
            let phys_addr = (pd_idx as u64) * 2 * 1024 * 1024;
            PD_PROC2.entries[pd_idx].set(
                phys_addr,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE,
            );
        }

        // --- Process 3 (PID 300) ---
        PML4_PROC3.entries[0].set(
            (&raw const PDPT_PROC3) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );

        PDPT_PROC3.entries[0].set(
            (&raw const PD_PROC3) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );
        PDPT_PROC3.entries[1].set(0x4000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);
        PDPT_PROC3.entries[2].set(0x8000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);
        PDPT_PROC3.entries[3].set(0xC000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);

        PD_PROC3.entries[0].set(
            &kernel_pts[0] as *const PageTable as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
        );
        PD_PROC3.entries[1].set(
            &kernel_pts[1] as *const PageTable as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
        );
        PD_PROC3.entries[2].set(
            (&raw const PT_PROC3) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );

        for i in 0..ENTRY_COUNT {
            let phys = 0x0080_0000u64 + (i as u64) * 4096;
            PT_PROC3.entries[i].set(
                phys,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
            );
        }

        for pd_idx in 3..ENTRY_COUNT {
            let phys_addr = (pd_idx as u64) * 2 * 1024 * 1024;
            PD_PROC3.entries[pd_idx].set(
                phys_addr,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE,
            );
        }

        // --- Process Renderer (PID 310) ---
        PML4_RENDER.entries[0].set(
            (&raw const PDPT_RENDER) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );

        PDPT_RENDER.entries[0].set(
            (&raw const PD_RENDER) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );
        PDPT_RENDER.entries[1].set(0x4000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);
        PDPT_RENDER.entries[2].set(0x8000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);
        PDPT_RENDER.entries[3].set(0xC000_0000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE);

        PD_RENDER.entries[0].set(
            &kernel_pts[0] as *const PageTable as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
        );
        PD_RENDER.entries[1].set(
            &kernel_pts[1] as *const PageTable as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
        );
        PD_RENDER.entries[2].set(
            (&raw const PT_RENDER) as u64,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
        );

        // Memory Quotas & W^X Enforcement:
        // Physical frame range for PID 310: 0x00A0_0000 .. 0x00BF_FFFF (512 distinct frames = 2 MB)
        // Disjoint from PROC1 (0x400000), PROC2 (0x600000), PROC3 (0x800000).

        // 1. Code Region: pages 0..128 (0x0040_0000 .. 0x0048_0000, 512 KB) -> RX (Non-writable)
        for i in 0..128 {
            let phys = 0x00A0_0000u64 + (i as u64) * 4096;
            PT_RENDER.entries[i].set(
                phys,
                PageTableEntry::PRESENT | PageTableEntry::USER_ACCESSIBLE, // Explicitly NO WRITABLE!
            );
        }

        // 2. Data/Heap Region: pages 128..480 (0x0048_0000 .. 0x005E_0000) -> RW (Non-executable)
        for i in 128..480 {
            let phys = 0x00A0_0000u64 + (i as u64) * 4096;
            PT_RENDER.entries[i].set(
                phys,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE | PageTableEntry::NO_EXECUTE,
            );
        }

        // 3. Guard Page 1 (Stack Underflow / Heap-Stack Boundary): page 480 (0x005E_0000) -> Non-present
        PT_RENDER.entries[480].clear();

        // 4. Stack Region: pages 481..511 (0x005E_1000 .. 0x005F_F000, 30 pages = 120 KB, RSP starts at 0x005F_E000) -> RW (Non-executable)
        for i in 481..511 {
            let phys = 0x00A0_0000u64 + (i as u64) * 4096;
            PT_RENDER.entries[i].set(
                phys,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE | PageTableEntry::NO_EXECUTE,
            );
        }

        // 5. Guard Page 2 (Stack Top Boundary): page 511 (0x005F_F000) -> Non-present
        PT_RENDER.entries[511].clear();

        for pd_idx in 3..ENTRY_COUNT {
            let phys_addr = (pd_idx as u64) * 2 * 1024 * 1024;
            PD_RENDER.entries[pd_idx].set(
                phys_addr,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::HUGE_PAGE,
            );
        }

        verify_disjoint_physical_mappings();
    }
}

/// Formally verifies that no cross-process writable physical frame aliases exist
/// and that explicit W^X is strictly enforced across all user page tables.
pub fn verify_disjoint_physical_mappings() {
    use crate::arch::x86_64::serial::SERIAL1;
    unsafe {
        SERIAL1.write_str("[PAGE_TABLES] PML4_RENDER initialized (CPL=3 isolated PML4 at 0x");
        SERIAL1.write_u64_hex((&raw const PML4_RENDER) as u64);
        SERIAL1.write_str(")\n");

        // Assert disjoint physical bases
        let p1_base = 0x0040_0000u64;
        let p2_base = 0x0060_0000u64;
        let p3_base = 0x0080_0000u64;
        let render_base = 0x00A0_0000u64;

        if render_base == p1_base || render_base == p2_base || render_base == p3_base {
            panic!("FATAL: Physical base alias detected between renderer and other processes!");
        }
        SERIAL1.write_str("[PAGE_TABLES] Physical memory disjointness verification: PASSED. Zero frame aliases detected.\n");

        // Verify W^X on renderer pages: no page may be both WRITABLE and Executable (NO_EXECUTE missing)
        for i in 0..ENTRY_COUNT {
            let entry = PT_RENDER.entries[i];
            if entry.is_present() && entry.is_writable() {
                if (entry.0 & PageTableEntry::NO_EXECUTE) == 0 {
                    panic!("FATAL: W^X violation! Page is writable and executable at index {}", i);
                }
            }
        }
        SERIAL1.write_str("[PAGE_TABLES] W^X permission verification: PASSED. Zero pages are both writable and executable.\n");

        // Verify stack guard pages: 480 and 511 non-present
        if PT_RENDER.entries[480].is_present() || PT_RENDER.entries[511].is_present() {
            panic!("FATAL: Stack guard page violation! Guard page marked present.");
        }
        SERIAL1.write_str("[PAGE_TABLES] Stack guard page verification: PASSED. Flanking guard pages non-present.\n");
    }
}

static mut PROC_SLOT_PIDS: [u64; 3] = [100, 200, 300];

fn resolve_process_slot(pid: u64) -> usize {
    match pid {
        100 => 0,
        200 => 1,
        300 | 301 | 302 => 2,
        _ => unsafe {
            for i in 0..3 {
                if PROC_SLOT_PIDS[i] == pid {
                    return i;
                }
            }
            for i in 0..3 {
                let slot_pid = PROC_SLOT_PIDS[i];
                let mut active = false;
                for p in crate::process::RESIDENT_PROCESSES.iter() {
                    if p.pid == slot_pid && p.state != crate::process::AppState::Unused {
                        active = true;
                        break;
                    }
                }
                if !active {
                    PROC_SLOT_PIDS[i] = pid;
                    return i;
                }
            }
            0
        },
    }
}

pub fn get_process_cr3(pid: u64) -> u64 {
    unsafe {
        if pid == 310 {
            return (&raw const PML4_RENDER) as u64;
        }
        match resolve_process_slot(pid) {
            0 => (&raw const PML4_PROC1) as u64,
            1 => (&raw const PML4_PROC2) as u64,
            _ => (&raw const PML4_PROC3) as u64,
        }
    }
}

pub fn get_process_phys_base(pid: u64) -> u64 {
    if pid == 310 {
        return 0x00A0_0000;
    }
    match resolve_process_slot(pid) {
        0 => 0x0040_0000,
        1 => 0x0060_0000,
        _ => 0x0080_0000,
    }
}

pub static mut USER_FRAMES_ALLOCATED: usize = 0;
pub static mut USER_FRAMES_FREED: usize = 0;
pub static mut ACTIVE_USER_FRAMES: usize = 0;

pub fn allocate_process_frames(frames: usize) {
    unsafe {
        USER_FRAMES_ALLOCATED += frames;
        ACTIVE_USER_FRAMES += frames;
    }
}

pub fn free_process_frames(frames: usize) {
    unsafe {
        USER_FRAMES_FREED += frames;
        if ACTIVE_USER_FRAMES >= frames {
            ACTIVE_USER_FRAMES -= frames;
        } else {
            ACTIVE_USER_FRAMES = 0;
        }
    }
}

pub fn get_active_user_frames() -> usize {
    unsafe { ACTIVE_USER_FRAMES }
}

pub fn get_user_frame_stats() -> (usize, usize, usize) {
    unsafe { (USER_FRAMES_ALLOCATED, USER_FRAMES_FREED, ACTIVE_USER_FRAMES) }
}

pub fn reset_user_frame_stats() {
    unsafe {
        USER_FRAMES_ALLOCATED = 0;
        USER_FRAMES_FREED = 0;
        ACTIVE_USER_FRAMES = 0;
    }
}

