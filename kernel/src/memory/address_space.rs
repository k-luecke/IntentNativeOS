// kernel/src/memory/address_space.rs - Virtual Memory Mapping and Address Space (K008)

use crate::arch::x86_64::paging::{PageTable, PageTableEntry, VirtualAddress};
use crate::memory::frame_allocator::{FrameAllocError, PhysicalFrameAllocator};
use crate::memory::boot_info::PAGE_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    FrameAllocationFailed(FrameAllocError),
    AlreadyMapped,
    InvalidAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnmapError {
    NotMapped,
    InvalidAlignment,
}

pub struct AddressSpace<'a> {
    pub pml4: &'a mut PageTable,
}

impl<'a> AddressSpace<'a> {
    pub fn new(pml4: &'a mut PageTable) -> Self {
        Self { pml4 }
    }

    /// Maps a 4 KiB virtual page to a physical frame with given flags.
    /// Allocates intermediate page tables from the provided frame allocator if missing.
    /// Uses phys_to_virt translation function to access page tables.
    pub fn map_page<F>(
        &mut self,
        virt: u64,
        phys: u64,
        flags: u64,
        allocator: &mut PhysicalFrameAllocator,
        phys_to_virt: F,
    ) -> Result<(), MapError>
    where
        F: Fn(u64) -> *mut PageTable,
    {
        if virt % PAGE_SIZE != 0 || phys % PAGE_SIZE != 0 {
            return Err(MapError::InvalidAlignment);
        }

        let va = VirtualAddress(virt);
        let user_flag = if (flags & PageTableEntry::USER_ACCESSIBLE) != 0 {
            PageTableEntry::USER_ACCESSIBLE
        } else {
            0
        };
        let intermediate_flags = PageTableEntry::PRESENT | PageTableEntry::WRITABLE | user_flag;

        // 1. Traverse or allocate PDPT
        let pml4_entry = &mut self.pml4.entries[va.pml4_index()];
        let pdpt_phys = if !pml4_entry.is_present() {
            let frame = allocator.alloc_frame().map_err(MapError::FrameAllocationFailed)?;
            let table_ptr = phys_to_virt(frame);
            unsafe {
                *table_ptr = PageTable::empty();
            }
            pml4_entry.set(frame, intermediate_flags);
            frame
        } else {
            // Inherit user accessible flag if user mapping is requested
            if (flags & PageTableEntry::USER_ACCESSIBLE) != 0 {
                pml4_entry.0 |= PageTableEntry::USER_ACCESSIBLE;
            }
            pml4_entry.phys_addr()
        };

        // 2. Traverse or allocate PD
        let pdpt = unsafe { &mut *phys_to_virt(pdpt_phys) };
        let pdpt_entry = &mut pdpt.entries[va.pdpt_index()];
        let pd_phys = if !pdpt_entry.is_present() {
            let frame = allocator.alloc_frame().map_err(MapError::FrameAllocationFailed)?;
            let table_ptr = phys_to_virt(frame);
            unsafe {
                *table_ptr = PageTable::empty();
            }
            pdpt_entry.set(frame, intermediate_flags);
            frame
        } else {
            if (flags & PageTableEntry::USER_ACCESSIBLE) != 0 {
                pdpt_entry.0 |= PageTableEntry::USER_ACCESSIBLE;
            }
            pdpt_entry.phys_addr()
        };

        // 3. Traverse or allocate PT
        let pd = unsafe { &mut *phys_to_virt(pd_phys) };
        let pd_entry = &mut pd.entries[va.pd_index()];
        let pt_phys = if !pd_entry.is_present() {
            let frame = allocator.alloc_frame().map_err(MapError::FrameAllocationFailed)?;
            let table_ptr = phys_to_virt(frame);
            unsafe {
                *table_ptr = PageTable::empty();
            }
            pd_entry.set(frame, intermediate_flags);
            frame
        } else {
            if (flags & PageTableEntry::USER_ACCESSIBLE) != 0 {
                pd_entry.0 |= PageTableEntry::USER_ACCESSIBLE;
            }
            pd_entry.phys_addr()
        };

        // 4. Map PT entry to physical page
        let pt = unsafe { &mut *phys_to_virt(pt_phys) };
        let pt_entry = &mut pt.entries[va.pt_index()];
        if pt_entry.is_present() {
            return Err(MapError::AlreadyMapped);
        }

        pt_entry.set(phys, flags | PageTableEntry::PRESENT);
        Ok(())
    }

    /// Unmaps a 4 KiB virtual page.
    /// Invalidates TLB and returns the physical frame address that was mapped.
    pub fn unmap_page<F>(
        &mut self,
        virt: u64,
        phys_to_virt: F,
    ) -> Result<u64, UnmapError>
    where
        F: Fn(u64) -> *mut PageTable,
    {
        if virt % PAGE_SIZE != 0 {
            return Err(UnmapError::InvalidAlignment);
        }

        let va = VirtualAddress(virt);
        let pml4_entry = &self.pml4.entries[va.pml4_index()];
        if !pml4_entry.is_present() {
            return Err(UnmapError::NotMapped);
        }

        let pdpt = unsafe { &*phys_to_virt(pml4_entry.phys_addr()) };
        let pdpt_entry = &pdpt.entries[va.pdpt_index()];
        if !pdpt_entry.is_present() {
            return Err(UnmapError::NotMapped);
        }

        let pd = unsafe { &*phys_to_virt(pdpt_entry.phys_addr()) };
        let pd_entry = &pd.entries[va.pd_index()];
        if !pd_entry.is_present() {
            return Err(UnmapError::NotMapped);
        }

        let pt = unsafe { &mut *phys_to_virt(pd_entry.phys_addr()) };
        let pt_entry = &mut pt.entries[va.pt_index()];
        if !pt_entry.is_present() {
            return Err(UnmapError::NotMapped);
        }

        let phys = pt_entry.phys_addr();
        pt_entry.clear();

        #[cfg(all(target_arch = "x86_64", not(test)))]
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
        }

        Ok(phys)
    }

    /// Translates a virtual address to physical address if mapped.
    pub fn translate<F>(&self, virt: u64, phys_to_virt: F) -> Option<u64>
    where
        F: Fn(u64) -> *mut PageTable,
    {
        let va = VirtualAddress(virt);
        let pml4_entry = &self.pml4.entries[va.pml4_index()];
        if !pml4_entry.is_present() {
            return None;
        }

        let pdpt = unsafe { &*phys_to_virt(pml4_entry.phys_addr()) };
        let pdpt_entry = &pdpt.entries[va.pdpt_index()];
        if !pdpt_entry.is_present() {
            return None;
        }

        if pdpt_entry.is_huge() {
            let offset = virt & 0x3FFF_FFFF;
            return Some(pdpt_entry.phys_addr() + offset);
        }

        let pd = unsafe { &*phys_to_virt(pdpt_entry.phys_addr()) };
        let pd_entry = &pd.entries[va.pd_index()];
        if !pd_entry.is_present() {
            return None;
        }

        if pd_entry.is_huge() {
            let offset = virt & 0x1F_FFFF;
            return Some(pd_entry.phys_addr() + offset);
        }

        let pt = unsafe { &*phys_to_virt(pd_entry.phys_addr()) };
        let pt_entry = &pt.entries[va.pt_index()];
        if !pt_entry.is_present() {
            return None;
        }

        Some(pt_entry.phys_addr() + va.page_offset())
    }

    pub fn map_range<F>(
        &mut self,
        virt_start: u64,
        phys_start: u64,
        count: usize,
        flags: u64,
        allocator: &mut PhysicalFrameAllocator,
        phys_to_virt: F,
    ) -> Result<(), MapError>
    where
        F: Fn(u64) -> *mut PageTable + Copy,
    {
        for i in 0..count {
            let virt = virt_start + (i as u64) * PAGE_SIZE;
            let phys = phys_start + (i as u64) * PAGE_SIZE;
            self.map_page(virt, phys, flags, allocator, phys_to_virt)?;
        }
        Ok(())
    }

    pub fn unmap_range<F>(
        &mut self,
        virt_start: u64,
        count: usize,
        allocator: &mut PhysicalFrameAllocator,
        phys_to_virt: F,
    ) -> Result<(), UnmapError>
    where
        F: Fn(u64) -> *mut PageTable + Copy,
    {
        for i in 0..count {
            let virt = virt_start + (i as u64) * PAGE_SIZE;
            let phys = self.unmap_page(virt, phys_to_virt)?;
            let _ = allocator.free_frame(phys);
        }
        Ok(())
    }

    pub fn protect_range<F>(
        &mut self,
        virt_start: u64,
        count: usize,
        flags_to_clear: u64,
        flags_to_set: u64,
        phys_to_virt: F,
    ) -> Result<(), UnmapError>
    where
        F: Fn(u64) -> *mut PageTable + Copy,
    {
        for i in 0..count {
            let virt = virt_start + (i as u64) * PAGE_SIZE;
            let va = VirtualAddress(virt);
            let pml4_entry = &self.pml4.entries[va.pml4_index()];
            if !pml4_entry.is_present() { return Err(UnmapError::NotMapped); }
            let pdpt = unsafe { &*phys_to_virt(pml4_entry.phys_addr()) };
            let pdpt_entry = &pdpt.entries[va.pdpt_index()];
            if !pdpt_entry.is_present() { return Err(UnmapError::NotMapped); }
            let pd = unsafe { &*phys_to_virt(pdpt_entry.phys_addr()) };
            let pd_entry = &pd.entries[va.pd_index()];
            if !pd_entry.is_present() { return Err(UnmapError::NotMapped); }
            let pt = unsafe { &mut *phys_to_virt(pd_entry.phys_addr()) };
            let pt_entry = &mut pt.entries[va.pt_index()];
            if !pt_entry.is_present() { return Err(UnmapError::NotMapped); }

            let phys = pt_entry.phys_addr();
            let current_flags = pt_entry.0 & !PageTableEntry::ADDR_MASK;
            let updated_flags = (current_flags & !flags_to_clear) | flags_to_set | PageTableEntry::PRESENT;
            pt_entry.set(phys, updated_flags);

            #[cfg(all(target_arch = "x86_64", not(test)))]
            unsafe {
                core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
            }
        }
        Ok(())
    }
}
