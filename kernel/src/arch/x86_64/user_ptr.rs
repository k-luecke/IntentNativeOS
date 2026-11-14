// kernel/src/arch/x86_64/user_ptr.rs - Hardened User-Pointer Validation & Safe Copying (APP-CAP-01)

use crate::arch::x86_64::gdt::USER_VIRTUAL_MAX;
use crate::arch::x86_64::paging::{PageTable, VirtualAddress};
use crate::arch::x86_64::syscall::SyscallError;

/// Validates that a user-mode buffer range is completely canonical, does not overflow,
/// and that every 4 KiB page backing the range is PRESENT and USER_ACCESSIBLE in the current address space (CR3).
pub fn validate_user_buffer(ptr: u64, len: usize, writeable: bool) -> Result<(), SyscallError> {
    if len == 0 {
        return Ok(());
    }

    // Check integer overflow and canonical user bounds
    let end = ptr.checked_add(len as u64).ok_or(SyscallError::InvalidArgument)?;
    if end > USER_VIRTUAL_MAX || ptr > USER_VIRTUAL_MAX {
        return Err(SyscallError::InvalidArgument);
    }

    // Query active CR3 to inspect the running process's page table root
    #[cfg(test)]
    let cr3: u64 = 0;
    #[cfg(not(test))]
    let cr3 = unsafe { crate::arch::x86_64::cpu::read_cr3() & 0x000F_FFFF_FFFF_F000 };
    if cr3 == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    let pml4 = unsafe { &*(cr3 as *const PageTable) };

    // Walk page tables for every 4 KiB page span
    let start_page = ptr & !0xFFF;
    let end_page = (end - 1) & !0xFFF;

    let mut current_page = start_page;
    loop {
        let va = VirtualAddress(current_page);

        // 1. PML4 check
        let pml4_e = &pml4.entries[va.pml4_index()];
        if !pml4_e.is_present() || !pml4_e.is_user() {
            return Err(SyscallError::InvalidArgument);
        }

        // 2. PDPT check
        let pdpt = unsafe { &*(pml4_e.phys_addr() as *const PageTable) };
        let pdpt_e = &pdpt.entries[va.pdpt_index()];
        if !pdpt_e.is_present() || !pdpt_e.is_user() {
            return Err(SyscallError::InvalidArgument);
        }

        if pdpt_e.is_huge() {
            // 1GB huge page
            if writeable && !pdpt_e.is_writable() {
                return Err(SyscallError::InvalidArgument);
            }
        } else {
            // 3. PD check
            let pd = unsafe { &*(pdpt_e.phys_addr() as *const PageTable) };
            let pd_e = &pd.entries[va.pd_index()];
            if !pd_e.is_present() || !pd_e.is_user() {
                return Err(SyscallError::InvalidArgument);
            }

            if pd_e.is_huge() {
                // 2MB huge page
                if writeable && !pd_e.is_writable() {
                    return Err(SyscallError::InvalidArgument);
                }
            } else {
                // 4. PT check
                let pt = unsafe { &*(pd_e.phys_addr() as *const PageTable) };
                let pt_e = &pt.entries[va.pt_index()];
                if !pt_e.is_present() || !pt_e.is_user() {
                    return Err(SyscallError::InvalidArgument);
                }
                if writeable && !pt_e.is_writable() {
                    return Err(SyscallError::InvalidArgument);
                }
            }
        }

        if current_page >= end_page {
            break;
        }
        current_page = current_page.saturating_add(4096);
    }

    Ok(())
}

/// Safely copies a slice from user-space into a bounded kernel-owned buffer
pub fn copy_slice_from_user(ptr: u64, dest: &mut [u8]) -> Result<(), SyscallError> {
    validate_user_buffer(ptr, dest.len(), false)?;
    unsafe {
        core::ptr::copy_nonoverlapping(ptr as *const u8, dest.as_mut_ptr(), dest.len());
    }
    Ok(())
}

/// Safely copies a string from user-space into a bounded kernel buffer and validates UTF-8
pub fn copy_str_from_user<'a>(ptr: u64, len: usize, dest: &'a mut [u8]) -> Result<&'a str, SyscallError> {
    if len > dest.len() || len == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    validate_user_buffer(ptr, len, false)?;
    unsafe {
        core::ptr::copy_nonoverlapping(ptr as *const u8, dest.as_mut_ptr(), len);
    }
    core::str::from_utf8(&dest[..len]).map_err(|_| SyscallError::InvalidArgument)
}

/// Safely copies a slice from kernel space into a validated, writable user-space buffer
pub fn copy_slice_to_user(src: &[u8], dest_ptr: u64) -> Result<(), SyscallError> {
    if src.is_empty() {
        return Ok(());
    }
    validate_user_buffer(dest_ptr, src.len(), true)?;
    unsafe {
        core::ptr::copy_nonoverlapping(src.as_ptr(), dest_ptr as *mut u8, src.len());
    }
    Ok(())
}
