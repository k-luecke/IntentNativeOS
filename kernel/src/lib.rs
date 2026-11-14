// kernel/src/lib.rs - INOS Kernel Core Library & Algorithms

#![cfg_attr(not(test), no_std)]
#![allow(warnings)]

pub mod arch;
pub mod memory;
pub mod ipc;
pub mod capability;
pub mod process;
pub mod storage;
pub mod net;
pub mod input;
pub mod graphics;
pub mod services;
pub mod vm;
pub mod security;
pub mod evaluation;
pub mod drivers;
pub mod crypto;



#[cfg(test)]
mod tests {
    use super::memory::boot_info::*;

    #[test]
    fn test_memory_map_normalization_and_merging() {
        let descriptors = [
            // Region 1: 0x0 - 0x100000 (Usable Conventional RAM, 256 pages = 1MB)
            EfiMemoryDescriptor {
                type_: EFI_CONVENTIONAL_MEMORY,
                pad: 0,
                physical_start: 0x0,
                virtual_start: 0,
                number_of_pages: 256,
                attribute: 0,
            },
            // Region 2: Adjacent Usable RAM (0x100000 - 0x300000, 512 pages = 2MB) -> should merge!
            EfiMemoryDescriptor {
                type_: EFI_CONVENTIONAL_MEMORY,
                pad: 0,
                physical_start: 0x100000,
                virtual_start: 0,
                number_of_pages: 512,
                attribute: 0,
            },
            // Region 3: ACPI Reclaimable (0x300000 - 0x310000, 16 pages = 64KB)
            EfiMemoryDescriptor {
                type_: EFI_ACPI_RECLAIM_MEMORY,
                pad: 0,
                physical_start: 0x300000,
                virtual_start: 0,
                number_of_pages: 16,
                attribute: 0,
            },
            // Region 4: MMIO space (0xFEE00000 - 0xFEE01000, 1 page = 4KB LAPIC)
            EfiMemoryDescriptor {
                type_: EFI_MEMORY_MAPPED_IO,
                pad: 0,
                physical_start: 0xFEE00000,
                virtual_start: 0,
                number_of_pages: 1,
                attribute: 0,
            },
        ];

        let normalized = normalize_memory_map(&descriptors);

        // Expect 3 merged regions (Region 1 and 2 merged into one 3MB region)
        assert_eq!(normalized.count, 3);
        assert_eq!(normalized.regions[0].region_type, NormalizedRegionType::Usable);
        assert_eq!(normalized.regions[0].start_addr, 0x0);
        assert_eq!(normalized.regions[0].size_bytes, 0x300000); // 3 MB

        assert_eq!(normalized.regions[1].region_type, NormalizedRegionType::AcpiReclaimable);
        assert_eq!(normalized.regions[1].start_addr, 0x300000);
        assert_eq!(normalized.regions[1].size_bytes, 16 * 4096);

        assert_eq!(normalized.regions[2].region_type, NormalizedRegionType::Mmio);
        assert_eq!(normalized.regions[2].start_addr, 0xFEE00000);
        assert_eq!(normalized.regions[2].size_bytes, 4096);

        assert_eq!(normalized.total_usable_bytes, 0x300000);
        assert!(normalized.total_reserved_bytes > 0);
    }

    #[test]
    fn test_paging_identity_and_unmapped_translations() {
        use crate::arch::x86_64::paging::*;

        let mut pml4 = Box::new(PageTable::empty());
        let mut pdpt = Box::new(PageTable::empty());
        let mut pd = Box::new(PageTable::empty());
        let mut pts = vec![PageTable::empty(); 4]; // 4 PTs = 8MB mapping

        let pdpt_ptr = &*pdpt as *const PageTable as u64;
        let pd_ptr = &*pd as *const PageTable as u64;
        let pt0_ptr = &pts[0] as *const PageTable as u64;
        let pt1_ptr = &pts[1] as *const PageTable as u64;
        let pt2_ptr = &pts[2] as *const PageTable as u64;
        let pt3_ptr = &pts[3] as *const PageTable as u64;

        setup_identity_page_table(
            &mut pml4,
            &mut pdpt,
            &mut pd,
            &mut pts,
            8 * 1024 * 1024, // 8 MB
        );

        let lookup = move |phys: u64| -> Option<&'static PageTable> {
            if phys == pdpt_ptr {
                Some(unsafe { &*(pdpt_ptr as *const PageTable) })
            } else if phys == pd_ptr {
                Some(unsafe { &*(pd_ptr as *const PageTable) })
            } else if phys == pt0_ptr {
                Some(unsafe { &*(pt0_ptr as *const PageTable) })
            } else if phys == pt1_ptr {
                Some(unsafe { &*(pt1_ptr as *const PageTable) })
            } else if phys == pt2_ptr {
                Some(unsafe { &*(pt2_ptr as *const PageTable) })
            } else if phys == pt3_ptr {
                Some(unsafe { &*(pt3_ptr as *const PageTable) })
            } else {
                None
            }
        };

        // 1. Test identity translation for mapped addresses
        let v1 = 0x0000_1000u64;
        let p1 = translate_addr(&pml4, v1, lookup);
        assert_eq!(p1, Some(0x0000_1000u64));

        let v2 = 0x0020_0000u64; // 2 MB
        let p2 = translate_addr(&pml4, v2, lookup);
        assert_eq!(p2, Some(0x0020_0000u64));

        let v3 = 0x007F_F123u64; // Near 8 MB limit
        let p3 = translate_addr(&pml4, v3, lookup);
        assert_eq!(p3, Some(0x007F_F123u64));

        // 2. Test unmapped address beyond 8 MB returns None
        let unmapped_v = 0x0090_0000u64; // 9 MB
        let unmapped_p = translate_addr(&pml4, unmapped_v, lookup);
        assert_eq!(unmapped_p, None);

        // 3. Test higher half address returns None
        let high_v = 0xFFFF_8000_0000_0000u64;
        let high_p = translate_addr(&pml4, high_v, lookup);
        assert_eq!(high_p, None);
    }

    #[test]
    fn test_frame_allocator_stress_and_reserved_regions() {
        use crate::memory::boot_info::*;
        use crate::memory::frame_allocator::*;

        // Create memory map: 0x100000..0x800000 usable (7MB), others reserved
        let descriptors = [
            EfiMemoryDescriptor {
                type_: EFI_RESERVED_MEMORY_TYPE,
                pad: 0,
                physical_start: 0x0,
                virtual_start: 0,
                number_of_pages: 256, // 0x0..0x100000 reserved
                attribute: 0,
            },
            EfiMemoryDescriptor {
                type_: EFI_CONVENTIONAL_MEMORY,
                pad: 0,
                physical_start: 0x100000,
                virtual_start: 0,
                number_of_pages: 1792, // 7 MB usable (1792 pages)
                attribute: 0,
            },
            EfiMemoryDescriptor {
                type_: EFI_MEMORY_MAPPED_IO,
                pad: 0,
                physical_start: 0xFEE00000,
                virtual_start: 0,
                number_of_pages: 1, // MMIO reserved
                attribute: 0,
            },
        ];

        let mem_map = normalize_memory_map(&descriptors);
        let mut allocator = PhysicalFrameAllocator::empty();
        allocator.init_from_memory_map(&mem_map);

        assert_eq!(allocator.free_frames_count(), 1792);

        // 1. Stress test: Allocate 500 frames continuously
        let mut allocated = Vec::new();
        for _ in 0..500 {
            let frame = allocator.alloc_frame().expect("Allocation should succeed");
            assert_eq!(frame % PAGE_SIZE, 0);
            assert!(frame >= 0x100000, "Frame must not be in reserved lower 1MB");
            assert!(frame < 0x800000, "Frame must be within usable range");
            allocated.push(frame);
        }
        assert_eq!(allocator.free_frames_count(), 1792 - 500);

        // 2. Free alternating 250 frames
        for i in (0..500).step_by(2) {
            allocator.free_frame(allocated[i]).expect("Free should succeed");
        }
        assert_eq!(allocator.free_frames_count(), 1792 - 250);

        // 3. Double free should fail
        let double_free_res = allocator.free_frame(allocated[0]);
        assert_eq!(double_free_res, Err(FrameAllocError::FrameAlreadyFree));

        // 4. Freeing reserved address should fail
        let reserved_free_res = allocator.free_frame(0x0);
        assert_eq!(reserved_free_res, Err(FrameAllocError::ReservedRegionViolation));

        // 5. Re-allocate 250 frames (verifying re-use of freed slots)
        for _ in 0..250 {
            let frame = allocator.alloc_frame().expect("Re-allocation should succeed");
            assert_eq!(frame % PAGE_SIZE, 0);
        }
        assert_eq!(allocator.free_frames_count(), 1792 - 500);
    }

    #[test]
    fn test_address_space_mapping_unmapping_and_unmapped_access() {
        use super::memory::address_space::*;
        use super::memory::frame_allocator::PhysicalFrameAllocator;
        use super::arch::x86_64::paging::{PageTable, PageTableEntry};

        // Simulated physical memory pool: 64 page frames
        static mut POOL: [PageTable; 64] = [PageTable::empty(); 64];

        let descriptors = [
            EfiMemoryDescriptor {
                type_: EFI_CONVENTIONAL_MEMORY,
                pad: 0,
                physical_start: 0x1000, // Starts at frame index 1
                virtual_start: 0,
                number_of_pages: 60,
                attribute: 0,
            },
        ];

        let mem_map = normalize_memory_map(&descriptors);
        let mut allocator = PhysicalFrameAllocator::empty();
        allocator.init_from_memory_map(&mem_map);

        let pml4 = unsafe { &mut POOL[0] };
        *pml4 = PageTable::empty();

        let mut addr_space = AddressSpace::new(pml4);

        let phys_to_virt = |phys: u64| -> *mut PageTable {
            let idx = (phys / 4096) as usize;
            unsafe { &mut POOL[idx] as *mut PageTable }
        };

        let target_virt = 0x0000_7000_5000_0000u64;
        let target_phys = 0x0000_0000_0003_0000u64; // Frame index 48

        // 1. Unmapped access before mapping should return None
        assert_eq!(addr_space.translate(target_virt, phys_to_virt), None);

        // 2. Map page with USER_ACCESSIBLE and WRITABLE
        let map_res = addr_space.map_page(
            target_virt,
            target_phys,
            PageTableEntry::WRITABLE | PageTableEntry::USER_ACCESSIBLE,
            &mut allocator,
            phys_to_virt,
        );
        assert_eq!(map_res, Ok(()));

        // 3. Verify translation matches target_phys
        assert_eq!(addr_space.translate(target_virt, phys_to_virt), Some(target_phys));
        assert_eq!(addr_space.translate(target_virt + 0xABC, phys_to_virt), Some(target_phys + 0xABC));

        // 4. Mapping already mapped page returns AlreadyMapped error
        let re_map = addr_space.map_page(
            target_virt,
            target_phys,
            PageTableEntry::WRITABLE,
            &mut allocator,
            phys_to_virt,
        );
        assert_eq!(re_map, Err(MapError::AlreadyMapped));

        // 5. Unmap page returns previously mapped physical address
        let unmap_res = addr_space.unmap_page(target_virt, phys_to_virt);
        assert_eq!(unmap_res, Ok(target_phys));

        // 6. Accessing unmapped page now returns None
        assert_eq!(addr_space.translate(target_virt, phys_to_virt), None);

        // 7. Unmapping an unmapped page returns NotMapped error
        assert_eq!(addr_space.unmap_page(target_virt, phys_to_virt), Err(UnmapError::NotMapped));
    }

    #[test]
    fn test_smp_madt_parsing_and_core_boot() {
        use super::arch::x86_64::smp::*;

        // Synthetic 2-core MADT table (44 byte header + two 8-byte Local APIC entries = 60 bytes)
        let mut madt = [0u8; 60];
        madt[0..4].copy_from_slice(b"APIC"); // Signature
        madt[4..8].copy_from_slice(&60u32.to_ne_bytes()); // Length
        madt[36..40].copy_from_slice(&0xFEE00000u32.to_ne_bytes()); // Local APIC Base Address

        // Entry 0: Processor Local APIC for Core 0 (BSP)
        madt[44] = 0; // Entry Type: Local APIC
        madt[45] = 8; // Length
        madt[46] = 0; // ACPI Processor ID
        madt[47] = 0; // APIC ID: 0
        madt[48..52].copy_from_slice(&1u32.to_ne_bytes()); // Flags: Enabled (bit 0 = 1)

        // Entry 1: Processor Local APIC for Core 1 (AP)
        madt[52] = 0; // Entry Type: Local APIC
        madt[53] = 8; // Length
        madt[54] = 1; // ACPI Processor ID
        madt[55] = 1; // APIC ID: 1
        madt[56..60].copy_from_slice(&1u32.to_ne_bytes()); // Flags: Enabled (bit 0 = 1)

        let mut topology = parse_madt_table(&madt).expect("MADT table parsing must succeed");
        assert_eq!(topology.core_count, 2);
        assert_eq!(topology.local_apic_base, 0xFEE00000);
        assert_eq!(topology.cores[0].core_id, 0);
        assert_eq!(topology.cores[0].apic_id, 0);
        assert_eq!(topology.cores[0].is_bsp, true);
        assert_eq!(topology.cores[0].is_online, true); // BSP starts online

        assert_eq!(topology.cores[1].core_id, 1);
        assert_eq!(topology.cores[1].apic_id, 1);
        assert_eq!(topology.cores[1].is_bsp, false);
        assert_eq!(topology.cores[1].is_online, false); // AP starts offline

        assert_eq!(topology.online_count, 1);

        // Boot secondary cores
        boot_secondary_cores(&mut topology);

        // Both cores must now be online
        assert_eq!(topology.online_count, 2);
        assert_eq!(topology.cores[1].is_online, true);
    }

    #[test]
    fn test_per_core_scheduler_synthetic_workers_progress_and_yield() {
        use super::arch::x86_64::scheduler::*;

        let mut sched = PerCoreScheduler::new(0);

        // 1. Spawn two synthetic workers
        let w1_idx = sched.spawn_task(101, "worker_a", 1).expect("Worker A spawn must succeed");
        let w2_idx = sched.spawn_task(102, "worker_b", 1).expect("Worker B spawn must succeed");

        assert_eq!(sched.task_count, 2);

        // 2. Schedule initial worker
        let first_idx = sched.schedule_next().expect("First task schedule must succeed");
        assert_eq!(first_idx, w1_idx);

        // 3. Run cooperative yielding loop for 5 rounds
        for round in 0..5 {
            // Current task runs ticks and yields
            sched.tick_current(10);
            let (from_idx, to_idx) = sched.yield_current().expect("Cooperative yield must succeed");

            if round % 2 == 0 {
                assert_eq!(from_idx, w1_idx);
                assert_eq!(to_idx, w2_idx);
            } else {
                assert_eq!(from_idx, w2_idx);
                assert_eq!(to_idx, w1_idx);
            }
        }

        // 4. Verify progress and yields on both workers
        let w1 = sched.tasks[w1_idx].as_ref().unwrap();
        let w2 = sched.tasks[w2_idx].as_ref().unwrap();

        assert!(w1.ticks_run > 0, "Worker A must have accumulated execution ticks");
        assert!(w2.ticks_run > 0, "Worker B must have accumulated execution ticks");
        assert!(w1.yield_count >= 2, "Worker A must have yielded multiple times");
        assert!(w2.yield_count >= 2, "Worker B must have yielded multiple times");
        assert_eq!(sched.total_context_switches, 6); // 1 initial + 5 yields
    }

    #[test]
    fn test_user_mode_address_space_isolation() {
        use super::arch::x86_64::gdt::*;
        use super::arch::x86_64::paging::{PageTable, PageTableEntry};

        static mut POOL: [PageTable; 4] = [PageTable::empty(); 4];
        let pml4 = unsafe { &mut POOL[0] };
        let pdpt = unsafe { &mut POOL[1] };
        let pd = unsafe { &mut POOL[2] };
        let pt = unsafe { &mut POOL[3] };

        *pml4 = PageTable::empty();
        *pdpt = PageTable::empty();
        *pd = PageTable::empty();
        *pt = PageTable::empty();

        let phys_to_virt = |phys: u64| -> *mut PageTable {
            let idx = (phys / 4096) as usize;
            unsafe { &mut POOL[idx] as *mut PageTable }
        };

        // 1. Direct violation: user attempts to access kernel address range (>= 0x8000_0000_0000)
        let kernel_addr = 0xFFFF_8000_0000_0000u64;
        assert_eq!(validate_user_access(pml4, kernel_addr, phys_to_virt), Err(UserAccessError::KernelAddressViolation));

        // 2. Set up kernel-only page mapping (USER_ACCESSIBLE is NOT set)
        pml4.entries[0].set(4096, PageTableEntry::PRESENT | PageTableEntry::WRITABLE);
        pdpt.entries[0].set(8192, PageTableEntry::PRESENT | PageTableEntry::WRITABLE);
        pd.entries[0].set(12288, PageTableEntry::PRESENT | PageTableEntry::WRITABLE);
        pt.entries[0].set(0x10000, PageTableEntry::PRESENT | PageTableEntry::WRITABLE);

        // Attempt user access to kernel-only page -> denied!
        let user_addr = 0x0000_0000_0000_0000u64;
        assert_eq!(validate_user_access(pml4, user_addr, phys_to_virt), Err(UserAccessError::KernelOnlyPage));

        // 3. Grant USER_ACCESSIBLE to all levels
        pml4.entries[0].0 |= PageTableEntry::USER_ACCESSIBLE;
        pdpt.entries[0].0 |= PageTableEntry::USER_ACCESSIBLE;
        pd.entries[0].0 |= PageTableEntry::USER_ACCESSIBLE;
        pt.entries[0].0 |= PageTableEntry::USER_ACCESSIBLE;

        // User access now permitted!
        assert_eq!(validate_user_access(pml4, user_addr, phys_to_virt), Ok(()));
    }

    #[test]
    fn test_syscall_gate_dispatcher() {
        use super::arch::x86_64::syscall::*;

        // 1. Query ABI version
        let res_ver = dispatch_syscall(SyscallFrame {
            nr: SYS_GET_VERSION,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
        });
        assert_eq!(res_ver, Ok(SYSCALL_ABI_VERSION as u64));

        // 2. Known syscall roundtrip (SYS_EXIT) and fail-closed pointer validation on SYS_IPC_SEND
        let res_exit = dispatch_syscall(SyscallFrame {
            nr: SYS_EXIT,
            arg1: 128,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
        });
        assert_eq!(res_exit, Ok(128));

        let res_ipc = dispatch_syscall(SyscallFrame {
            nr: SYS_IPC_SEND,
            arg1: 42, // invalid user pointer in host test environment
            arg2: 128,
            arg3: 0,
            arg4: 0,
            arg5: 0,
        });
        assert_eq!(res_ipc, Err(SyscallError::InvalidArgument));

        // 3. Unknown syscall rejected
        let res_unknown = dispatch_syscall(SyscallFrame {
            nr: 0x9999,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
        });
        assert_eq!(res_unknown, Err(SyscallError::InvalidSyscallNumber));
    }

    #[test]
    fn test_bounded_ipc_queue_send_recv_and_cancellation() {
        use super::ipc::*;

        let mut queue = BoundedIpcQueue::new();
        assert!(queue.is_empty());

        // 1. Fill queue to max capacity (16 messages)
        for i in 1..=16 {
            let msg = IpcMessage::new(
                i as u64,
                100, // sender
                200, // receiver
                IpcMessageType::DirectIntent,
                b"test_payload_chunk",
            );
            assert_eq!(queue.enqueue(msg), Ok(()));
        }

        assert!(queue.is_full());
        assert_eq!(queue.len(), 16);

        // 2. 17th message must fail with QueueFull
        let overflow_msg = IpcMessage::new(17, 100, 200, IpcMessageType::DirectIntent, b"overflow");
        assert_eq!(queue.enqueue(overflow_msg), Err(IpcError::QueueFull));

        // 3. Cancel message ID 5 in flight
        assert_eq!(queue.cancel_message(5), Ok(()));
        // Double cancellation fails
        assert_eq!(queue.cancel_message(5), Err(IpcError::AlreadyCancelled));
        // Non-existent cancellation fails
        assert_eq!(queue.cancel_message(999), Err(IpcError::MessageNotFound));

        // 4. Dequeue messages: message 5 must be skipped automatically
        let mut received_ids = Vec::new();
        while let Ok(msg) = queue.dequeue() {
            received_ids.push(msg.msg_id);
        }

        assert_eq!(received_ids.len(), 15);
        assert!(!received_ids.contains(&5), "Cancelled message 5 must not be delivered");
        assert_eq!(received_ids[0], 1);
        assert_eq!(received_ids[14], 16);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_capability_handles_rights_and_unowned_access() {
        use super::capability::*;

        let mut caps = CapabilitySystem::new();

        // 1. Process 1 creates an IPC channel object with READ | WRITE | DELEGATE
        let (obj_id, h_root) = caps.create_object(
            KernelObjectType::IpcChannel,
            1, // Owner: PID 1
            rights::READ | rights::WRITE | rights::DELEGATE | rights::REVOKE,
        ).expect("Object creation must succeed");

        // 2. Process 1 can access with READ
        assert_eq!(caps.verify_access(1, h_root, rights::READ), Ok(obj_id));

        // 3. Process 2 attempts to use PID 1's handle -> denied (UnownedCapability)
        assert_eq!(caps.verify_access(2, h_root, rights::READ), Err(CapError::UnownedCapability));

        // 4. Process 1 attempts to use non-held right EXECUTE -> denied (InsufficientRights)
        assert_eq!(caps.verify_access(1, h_root, rights::EXECUTE), Err(CapError::InsufficientRights));
    }

    #[test]
    fn test_capability_delegation_and_revocation() {
        use super::capability::*;

        let mut caps = CapabilitySystem::new();

        let (obj_id, h_root) = caps.create_object(
            KernelObjectType::MemoryRegion,
            1, // Owner: PID 1
            rights::READ | rights::WRITE | rights::DELEGATE | rights::REVOKE,
        ).unwrap();

        // 1. PID 1 delegates READ right to PID 2
        let h_child = caps.delegate(h_root, 1, 2, rights::READ).expect("Delegation must succeed");

        // 2. PID 2 can now access with READ
        assert_eq!(caps.verify_access(2, h_child, rights::READ), Ok(obj_id));

        // 3. PID 2 cannot access with WRITE (attenuated)
        assert_eq!(caps.verify_access(2, h_child, rights::WRITE), Err(CapError::InsufficientRights));

        // 4. PID 1 revokes root capability
        let count = caps.revoke(h_root, 1).expect("Revocation must succeed");
        assert_eq!(count, 2); // Both root and child revoked

        // 5. PID 2 immediately loses access!
        assert_eq!(caps.verify_access(2, h_child, rights::READ), Err(CapError::CapabilityRevoked));
        // PID 1 also loses access on the revoked handle
        assert_eq!(caps.verify_access(1, h_root, rights::READ), Err(CapError::CapabilityRevoked));
    }

    #[test]
    fn test_kernel_object_lifecycle_ref_counting() {
        use super::capability::*;

        let mut caps = CapabilitySystem::new();

        // Run 100 repeated synthetic lifecycles
        for iter in 0..100 {
            let (obj_id, h_root) = caps.create_object(
                KernelObjectType::TaskEndpoint,
                1,
                rights::READ | rights::DELEGATE | rights::REVOKE,
            ).unwrap();

            assert_eq!(caps.get_object_ref_count(obj_id), Some(1));
            assert!(caps.is_object_alive(obj_id));

            // Delegate to 4 recipient processes (ref count becomes 5)
            let mut children = [0u64; 4];
            for i in 0..4 {
                children[i] = caps.delegate(h_root, 1, 10 + i as u64, rights::READ).unwrap();
            }
            assert_eq!(caps.get_object_ref_count(obj_id), Some(5));

            // Revoke root handle: cascades to all 4 children, ref count drops to 0, object reclaimed
            let revoked = caps.revoke(h_root, 1).unwrap();
            assert_eq!(revoked, 5);
            assert_eq!(caps.get_object_ref_count(obj_id), Some(0));
            assert!(!caps.is_object_alive(obj_id));

            // Reset system for next synthetic iteration to avoid table exhaustion
            if (iter + 1) % 5 == 0 {
                caps = CapabilitySystem::new();
            }
        }
    }

    #[test]
    fn test_elf_loading_and_approval() {
        use super::process::*;

        // Construct synthetic ELF64 binary with 1 PT_LOAD segment
        let mut elf_bytes = [0u8; 128];
        elf_bytes[0..4].copy_from_slice(&ELF_MAGIC);
        elf_bytes[4] = ELF_CLASS_64;
        elf_bytes[5] = ELF_DATA_LSB;
        elf_bytes[6] = 1; // EV_CURRENT
        elf_bytes[16..18].copy_from_slice(&2u16.to_ne_bytes()); // ET_EXEC
        elf_bytes[18..20].copy_from_slice(&ELF_MACHINE_X86_64.to_ne_bytes()); // x86-64
        elf_bytes[20..24].copy_from_slice(&1u32.to_ne_bytes()); // version
        elf_bytes[24..32].copy_from_slice(&0x0000_0000_0040_0000u64.to_ne_bytes()); // Entry point
        elf_bytes[32..40].copy_from_slice(&64u64.to_ne_bytes()); // phoff (offset 64)
        elf_bytes[54..56].copy_from_slice(&(core::mem::size_of::<Elf64ProgramHeader>() as u16).to_ne_bytes()); // phentsize
        elf_bytes[56..58].copy_from_slice(&1u16.to_ne_bytes()); // phnum (1 segment)

        // Program Header at offset 64
        let ph_offset = 64;
        elf_bytes[ph_offset..ph_offset + 4].copy_from_slice(&PT_LOAD.to_ne_bytes());
        elf_bytes[ph_offset + 4..ph_offset + 8].copy_from_slice(&5u32.to_ne_bytes()); // PF_R | PF_X
        elf_bytes[ph_offset + 8..ph_offset + 16].copy_from_slice(&0u64.to_ne_bytes()); // offset
        elf_bytes[ph_offset + 16..ph_offset + 24].copy_from_slice(&0x0000_0000_0040_0000u64.to_ne_bytes()); // vaddr
        elf_bytes[ph_offset + 32..ph_offset + 40].copy_from_slice(&64u64.to_ne_bytes()); // filesz
        elf_bytes[ph_offset + 40..ph_offset + 48].copy_from_slice(&4096u64.to_ne_bytes()); // memsz

        // Calculate expected checksum
        let mut expected_digest: u32 = 0;
        for &b in elf_bytes.iter() {
            expected_digest = expected_digest.wrapping_add(b as u32);
        }

        // 1. Loading with correct approval digest succeeds
        let res = parse_and_validate_elf(&elf_bytes, expected_digest).expect("Valid approved ELF must parse");
        assert_eq!(res.entry_point, 0x0040_0000);
        assert_eq!(res.load_vaddr, 0x0040_0000);
        assert_eq!(res.mem_size, 4096);

        // 2. Loading with incorrect approval digest fails
        let unapproved = parse_and_validate_elf(&elf_bytes, expected_digest ^ 0xFF);
        assert_eq!(unapproved, Err(ElfError::UnapprovedBinary));
    }

    #[test]
    fn test_process_lifecycle_start_exit_wait_and_orphan_cleanup() {
        use super::process::*;

        let mut pm = ProcessManager::new();

        // 1. Spawn init (PID 1)
        let init_pid = pm.spawn(None, 0x400000, true).unwrap();
        assert_eq!(init_pid, 1);

        // 2. Init spawns Child 1 (PID 2) and Child 2 (PID 3)
        let c1_pid = pm.spawn(Some(1), 0x401000, false).unwrap();
        let c2_pid = pm.spawn(Some(1), 0x402000, false).unwrap();
        assert_eq!(c1_pid, 2);
        assert_eq!(c2_pid, 3);

        // 3. Child 2 spawns Grandchild (PID 4)
        let gc_pid = pm.spawn(Some(3), 0x403000, false).unwrap();
        assert_eq!(gc_pid, 4);

        // 4. Child 1 exits with code 42
        assert_eq!(pm.exit_process(c1_pid, 42), Ok(()));

        // 5. Parent (Init) waits for Child 1 and receives exit code 42
        let exit_code = pm.wait_process(init_pid, c1_pid).expect("Wait on Child 1 must succeed");
        assert_eq!(exit_code, 42);

        // 6. Child 2 exits with code 0 -> Grandchild (PID 4) should be reparented to Init (PID 1)
        assert_eq!(pm.exit_process(c2_pid, 0), Ok(()));
        let gc_slot = pm.processes.iter().position(|p| p.pid == gc_pid).unwrap();
        assert_eq!(pm.processes[gc_slot].parent_pid, Some(1), "Grandchild must be reparented to Init");

        // 7. Waiting on still running process returns StillRunning
        assert_eq!(pm.wait_process(init_pid, gc_pid), Err(ProcessError::StillRunning));
    }

    #[test]
    fn test_kernel_fault_containment_and_user_crash_recovery() {
        use super::process::*;

        let mut pm = ProcessManager::new();
        let _init = pm.spawn(None, 0x400000, true).unwrap();
        let sib1 = pm.spawn(Some(1), 0x401000, false).unwrap();
        let sib2 = pm.spawn(Some(1), 0x402000, false).unwrap();

        // Sibling 1 encounters user fault
        let recover_res = pm.recover_from_user_fault(sib1, 0x02); // Write on non-present
        assert_eq!(recover_res, Ok(()));

        // Sibling 1 must be terminated (Zombie with crash code)
        let s1_slot = pm.processes.iter().position(|p| p.pid == sib1).unwrap();
        assert_eq!(pm.processes[s1_slot].state, ProcessState::Zombie(0xDEAD));

        // Sibling 2 remains running!
        let s2_slot = pm.processes.iter().position(|p| p.pid == sib2).unwrap();
        assert_eq!(pm.processes[s2_slot].state, ProcessState::Running);
    }

    #[test]
    fn test_boot_init_supervisor_critical_service_restart() {
        use super::process::*;

        let mut pm = ProcessManager::new();
        let _init = pm.spawn(None, 0x400000, true).unwrap();

        // Spawn critical user service
        let crit_pid = pm.spawn(Some(1), 0x405000, true).unwrap();

        // Critical service unexpectedly crashes
        pm.exit_process(crit_pid, 1).unwrap();

        // Supervisor tick detects crash and auto-restarts critical service
        let restarted_pid = pm.supervise_tick();
        assert!(restarted_pid.is_some());
        assert_ne!(restarted_pid.unwrap(), crit_pid);

        // Verify service is running again with restart count 1
        let slot = pm.processes.iter().position(|p| p.pid == restarted_pid.unwrap()).unwrap();
        assert_eq!(pm.processes[slot].state, ProcessState::Running);
        assert_eq!(pm.processes[slot].restart_count, 1);
    }

    #[test]
    fn test_deterministic_kernel_event_logging() {
        use super::process::*;

        let mut pm = ProcessManager::new();
        pm.log_event(KernelEventType::ProcessSpawned, 1, 0x400000);
        pm.log_event(KernelEventType::UserFaultRecovered, 1, 0x02);
        pm.log_event(KernelEventType::CriticalServiceRestarted, 2, 1);

        assert_eq!(pm.events[1].unwrap().seq, 1);
        assert_eq!(pm.events[1].unwrap().event_type, KernelEventType::ProcessSpawned);
        assert_eq!(pm.events[2].unwrap().seq, 2);
        assert_eq!(pm.events[2].unwrap().event_type, KernelEventType::UserFaultRecovered);
        assert_eq!(pm.events[3].unwrap().seq, 3);
        assert_eq!(pm.events[3].unwrap().event_type, KernelEventType::CriticalServiceRestarted);
    }

    #[test]
    fn test_syscall_and_ipc_fuzz_corpus() {
        use super::process::fuzz::*;

        let mutations_tested = run_fuzz_corpus();
        assert_eq!(mutations_tested, 64, "All 64 fuzzed mutations must execute without kernel panic");
    }

    #[test]
    fn test_block_device_read_write_verification() {
        use super::storage::*;

        let mut dev = MemoryBlockDevice::new();
        assert_eq!(dev.block_count(), MAX_BLOCKS as u64);

        let mut write_block = [0u8; BLOCK_SIZE];
        write_block[0..8].copy_from_slice(&0x55AA_CAFE_BABE_0001u64.to_ne_bytes());
        write_block[BLOCK_SIZE - 4..BLOCK_SIZE].copy_from_slice(&0x12345678u32.to_ne_bytes());

        assert_eq!(dev.write_block(10, &write_block), Ok(()));

        let mut read_block = [0u8; BLOCK_SIZE];
        assert_eq!(dev.read_block(10, &mut read_block), Ok(()));
        assert_eq!(read_block, write_block);

        // Invalid LBA check
        assert_eq!(dev.read_block(MAX_BLOCKS as u64 + 1, &mut read_block), Err(BlockError::InvalidLba));
    }

    #[test]
    fn test_persistent_filesystem_operations() {
        use super::storage::*;

        let dev = MemoryBlockDevice::new();
        let mut fs = InosFileSystem::new(dev);
        fs.format().expect("Format filesystem");

        // 1. Create file
        let inode_id = fs.create_file("intent.txt").expect("Create file");
        assert_eq!(inode_id, 1);
        assert!(fs.file_exists("intent.txt"));

        // 2. Write file
        let test_data = b"INOS typed contract: operation compile";
        let written = fs.write_file("intent.txt", test_data).expect("Write file");
        assert_eq!(written, test_data.len());

        // 3. Read file
        let mut read_buf = [0u8; 64];
        let bytes_read = fs.read_file("intent.txt", &mut read_buf).expect("Read file");
        assert_eq!(bytes_read, test_data.len());
        assert_eq!(&read_buf[..bytes_read], test_data);

        // 4. Rename file
        fs.rename_file("intent.txt", "intent_v2.txt").expect("Rename file");
        assert!(!fs.file_exists("intent.txt"));
        assert!(fs.file_exists("intent_v2.txt"));

        // Verify content unchanged under new name
        let mut re_read = [0u8; 64];
        let n = fs.read_file("intent_v2.txt", &mut re_read).expect("Re-read renamed file");
        assert_eq!(&re_read[..n], test_data);

        // 5. Delete file
        fs.delete_file("intent_v2.txt").expect("Delete file");
        assert!(!fs.file_exists("intent_v2.txt"));
        assert_eq!(fs.read_file("intent_v2.txt", &mut read_buf), Err(FsError::FileNotFound));
    }

    #[test]
    fn test_atomic_write_wal_crash_recovery() {
        use super::storage::*;

        let mut dev = MemoryBlockDevice::new();
        let journal = AtomicJournal::new(1); // WAL at LBA 1
        let target_lba = 50;

        let original_data = [0xAAu8; BLOCK_SIZE];
        dev.write_block(target_lba, &original_data).unwrap();

        let new_data = [0x55u8; BLOCK_SIZE];

        // Scenario 1: Crash occurs before commit -> uncommitted write must be safely discarded
        let committed = journal.atomic_write(&mut dev, target_lba, &new_data, true).unwrap();
        assert_eq!(committed, false); // Crash injected before commit

        // Reboot / crash recovery
        let recovered = journal.recover_on_boot(&mut dev).unwrap();
        assert_eq!(recovered, false); // Partial write discarded

        // Target block must still hold original data!
        let mut check_buf = [0u8; BLOCK_SIZE];
        dev.read_block(target_lba, &mut check_buf).unwrap();
        assert_eq!(check_buf, original_data);

        // Scenario 2: Normal atomic write commits and flushes to target
        let committed2 = journal.atomic_write(&mut dev, target_lba, &new_data, false).unwrap();
        assert_eq!(committed2, true);

        dev.read_block(target_lba, &mut check_buf).unwrap();
        assert_eq!(check_buf, new_data);
    }

    #[test]
    fn test_network_loopback_ping_and_tcp_stream() {
        use super::net::*;
        let mut loopback = LoopbackDevice::new(Ipv4Address::LOOPBACK);

        // Ping test
        let ping_payload = b"INOS ICMP ECHO PING TEST";
        let ping_res = loopback.process_icmp_ping(ping_payload).expect("ICMP ping");
        assert!(ping_res);

        // TCP stream test
        let tcp_payload = b"INOS TCP STREAM OK";
        let bytes_sent = loopback.simulate_tcp_stream(8080, tcp_payload).expect("TCP stream");
        assert_eq!(bytes_sent, tcp_payload.len());
    }

    #[test]
    fn test_keyboard_pointer_input_event_queue() {
        use super::input::*;
        let mut input = Ps2InputDriver::new(640, 480);

        // Simulate typing 'A' (scancode 0x1E press)
        input.handle_scancode(0x1E);
        assert_eq!(input.queue.len(), 1);

        let ev = input.queue.pop().unwrap();
        match ev {
            InputEvent::Keyboard { code, pressed, .. } => {
                assert_eq!(code, KeyCode::KeyA);
                assert!(pressed);
            }
            _ => panic!("Expected keyboard event"),
        }

        // Simulate pointer motion
        input.handle_mouse_packet(10, -5, 0x01);
        let ptr_ev = input.queue.pop().unwrap();
        match ptr_ev {
            InputEvent::Pointer { dx, dy, buttons } => {
                assert_eq!(dx, 10);
                assert_eq!(dy, -5);
                assert_eq!(buttons, 0x01);
            }
            _ => panic!("Expected pointer event"),
        }
    }

    #[test]
    fn test_framebuffer_drawing_and_pixel_manipulation() {
        use super::graphics::*;
        let mut fb = Box::new(Framebuffer::new());

        fb.clear(Color::DARK_GRAY);
        assert_eq!(fb.get_pixel(50, 50), Color::DARK_GRAY);

        fb.set_pixel(80, 80, Color::RED);
        assert_eq!(fb.get_pixel(80, 80), Color::RED);

        let rect = Rect::new(10, 10, 50, 50);
        fb.fill_rect(rect, Color::LIGHT_BLUE);
        assert_eq!(fb.get_pixel(15, 15), Color::LIGHT_BLUE);
        assert_eq!(fb.get_pixel(59, 59), Color::LIGHT_BLUE);
        assert_eq!(fb.get_pixel(65, 65), Color::DARK_GRAY);
    }

    #[test]
    fn test_compositor_window_surfaces_and_clipping() {
        use super::graphics::*;
        let mut fb = Box::new(Framebuffer::new());
        fb.clear(Color::BLACK);

        let mut comp = Box::new(Compositor::new());

        // Window Surface 1: background window at (10, 10, 40, 40) z=1
        let mut surf1 = WindowSurface::new(1, Rect::new(10, 10, 40, 40), 1);
        surf1.clear(Color::BLUE);

        // Window Surface 2: overlapping foreground window at (30, 30, 40, 40) z=2
        let mut surf2 = WindowSurface::new(2, Rect::new(30, 30, 40, 40), 2);
        surf2.clear(Color::GREEN);

        comp.add_surface(surf1).unwrap();
        comp.add_surface(surf2).unwrap();

        comp.composite(&mut fb);

        // Non-overlapping part of surf1 should be BLUE
        assert_eq!(fb.get_pixel(15, 15), Color::BLUE);

        // Overlapping part where surf2 is on top should be GREEN
        assert_eq!(fb.get_pixel(35, 35), Color::GREEN);

        // Outside both windows should still be BLACK
        assert_eq!(fb.get_pixel(5, 5), Color::BLACK);
    }

    #[test]
    fn test_font_text_rendering_and_text_input_widget() {
        use super::graphics::*;
        let mut fb = Box::new(Framebuffer::new());
        fb.clear(Color::BLACK);

        draw_text(&mut fb, 20, 20, "INOS", Color::WHITE);
        // Glyph 'I' at (20, 20) should have lit pixels
        let mut lit_count = 0;
        for y in 20..28 {
            for x in 20..28 {
                if fb.get_pixel(x, y) == Color::WHITE {
                    lit_count += 1;
                }
            }
        }
        assert!(lit_count > 0);

        let mut widget = TextInputWidget::new();
        widget.insert_char(b'I');
        widget.insert_char(b'N');
        widget.insert_char(b'T');
        assert_eq!(widget.as_str(), "INT");
        widget.backspace();
        assert_eq!(widget.as_str(), "IN");
    }

    #[test]
    fn test_native_file_picker_and_viewer() {
        use super::storage::*;
        use super::services::*;
        use super::input::KeyCode;

        let mut dev = MemoryBlockDevice::new();
        let mut fs = InosFileSystem::new(dev);
        fs.format().unwrap();
        fs.create_file("doc.txt").unwrap();
        fs.create_file("data.csv").unwrap();

        let mut picker = FilePickerWidget::new();
        picker.populate_from_fs(&fs);
        assert_eq!(picker.entry_count, 2);
        assert_eq!(picker.selected_filename(), "doc.txt");

        // Navigate down
        picker.handle_key(KeyCode::ArrowDown);
        assert_eq!(picker.selected_filename(), "data.csv");

        // Confirm
        picker.handle_key(KeyCode::Enter);
        assert!(picker.confirmed);
    }

    #[test]
    fn test_ocaml_ipc_bridge_packet_marshalling() {
        use super::services::*;

        let mut bridge = OcamlIpcBridge::new(1);
        let payload = b"intent: convert to json";
        let mut buf = [0u8; 128];

        let packed_len = bridge.pack_message(OcamlMsgType::IntentSubmission, payload, &mut buf).unwrap();
        assert_eq!(packed_len, 16 + payload.len());

        let header = bridge.unpack_header(&buf[..packed_len]).unwrap();
        assert_eq!(header.magic, OCAML_IPC_MAGIC);
        assert_eq!(header.msg_type, OcamlMsgType::IntentSubmission);
        assert_eq!(header.payload_size as usize, payload.len());
        assert_eq!(&buf[16..packed_len], payload);
    }

    #[test]
    fn test_intent_submission_and_clarification_loop() {
        use super::services::*;

        let mut wf = IntentWorkflow::new();
        wf.submit_intent("transform log data");
        assert_eq!(wf.status, IntentStatus::ReadyForClarification);

        wf.submit_clarification("JSON format");
        assert_eq!(wf.status, IntentStatus::Clarified);
        assert_eq!(&wf.resolution[..wf.resolution_len], b"JSON format");
    }

    #[test]
    fn test_model_ab_arbitration_bridge() {
        use super::services::*;

        let mut arb = ModelArbitrationEngine::new();
        let valid_plan = "PLAN: parse CSV, format JSON, bounded buffer";
        let res = arb.run_ab_arbitration(valid_plan);
        assert_eq!(res, ArbitrationResult::Agreed);
        assert!(arb.model_a_tokens > 0);
        assert!(arb.model_b_tokens > 0);

        let bad_plan = "PLAN: execute MALICIOUS code ESCAPE_CAP";
        let bad_res = arb.run_ab_arbitration(bad_plan);
        assert_eq!(bad_res, ArbitrationResult::Rejected);
    }

    #[test]
    fn test_sandboxed_executor_capability_enforcement() {
        use super::storage::*;
        use super::services::*;

        let mut dev = MemoryBlockDevice::new();
        let mut fs = InosFileSystem::new(dev);
        fs.format().unwrap();
        fs.create_file("source.txt").unwrap();
        fs.write_file("source.txt", b"hello world").unwrap();

        let executor = SandboxedExecutor::new(0xCAFE_BABE);

        fn uppercase_transform(input: &[u8], output: &mut [u8]) -> usize {
            for (i, &b) in input.iter().enumerate() {
                output[i] = if b >= b'a' && b <= b'z' { b - 32 } else { b };
            }
            input.len()
        }

        // Invalid capability token must fail
        let denied = executor.execute_file_transform(&mut fs, "source.txt", 0xDEAD_BEEF, uppercase_transform);
        assert!(denied.is_err());

        // Valid capability token succeeds
        let allowed = executor.execute_file_transform(&mut fs, "source.txt", 0xCAFE_BABE, uppercase_transform).unwrap();
        assert_eq!(allowed, 11);

        let mut read_buf = [0u8; 32];
        let n = fs.read_file("source.txt", &mut read_buf).unwrap();
        assert_eq!(&read_buf[..n], b"HELLO WORLD");
    }

    #[test]
    fn test_snapshot_manager_file_restore_undo() {
        use super::storage::*;
        use super::services::*;

        let mut dev = MemoryBlockDevice::new();
        let mut fs = InosFileSystem::new(dev);
        fs.format().unwrap();
        fs.create_file("target.txt").unwrap();
        let original_content = b"ORIGINAL CONTENT SNAPSHOT v1";
        fs.write_file("target.txt", original_content).unwrap();

        let mut snap = SnapshotManager::new();
        snap.snapshot_file(&fs, "target.txt").unwrap();

        // Mutate file
        fs.delete_file("target.txt").unwrap();
        fs.create_file("target.txt").unwrap();
        fs.write_file("target.txt", b"MODIFIED OVERWRITTEN CONTENT").unwrap();

        // Trigger undo/restore
        let restored_bytes = snap.restore_file(&mut fs).unwrap();
        assert_eq!(restored_bytes, original_content.len());

        let mut verify_buf = [0u8; 64];
        let n = fs.read_file("target.txt", &mut verify_buf).unwrap();
        assert_eq!(&verify_buf[..n], original_content);
    }

    #[test]
    fn test_activity_receipt_generation_and_verification() {
        use super::services::*;

        let data = b"VERIFIED EXECUTION OUTPUT STREAM";
        let receipt = ActivityReceipt::new(101, "FILE_TRANSFORM", data);

        assert_eq!(receipt.receipt_id, 101);
        assert!(receipt.verified);
        assert_ne!(receipt.output_digest, 0);
    }

    #[test]
    fn test_resource_monitor_and_model_unload() {
        use super::services::*;

        let mut monitor = SystemResourceMonitor::new(1024); // 1024 kernel pages
        assert_eq!(monitor.total_allocated_pages(), 1024);

        monitor.load_model_context(2048); // Model takes 2048 pages
        assert_eq!(monitor.total_allocated_pages(), 3072);

        let reclaimed = monitor.unload_model_context();
        assert_eq!(reclaimed, 2048);
        assert_eq!(monitor.total_allocated_pages(), 1024);
    }

    #[test]
    fn test_multi_cycle_stress_harness() {
        use super::storage::*;
        use super::services::*;

        let mut dev = MemoryBlockDevice::new();
        let mut fs = InosFileSystem::new(dev);
        fs.format().unwrap();

        let ok = StressHarness::run_cycles(&mut fs, 50);
        assert!(ok);
    }

    #[test]
    fn test_gate_g3t_native_mvp_complete_workflow() {
        use super::storage::*;
        use super::services::*;
        use super::graphics::*;
        use super::input::KeyCode;

        // 1. Initialize virtual platform storage & filesystem
        let mut dev = MemoryBlockDevice::new();
        let mut fs = InosFileSystem::new(dev);
        fs.format().unwrap();

        let initial_text = b"intent native os specification draft";
        fs.create_file("doc.txt").unwrap();
        fs.write_file("doc.txt", initial_text).unwrap();

        // 2. Boot to graphics & Compositor setup
        let mut fb = Box::new(Framebuffer::new());
        fb.clear(Color::DARK_GRAY);

        // 3. User selects file via FilePicker
        let mut picker = FilePickerWidget::new();
        picker.populate_from_fs(&fs);
        assert_eq!(picker.selected_filename(), "doc.txt");
        picker.handle_key(KeyCode::Enter);
        assert!(picker.confirmed);

        // 4. Intent submission & Clarification
        let mut intent_wf = IntentWorkflow::new();
        intent_wf.submit_intent("uppercase doc.txt");
        intent_wf.submit_clarification("in-place overwrite");
        assert_eq!(intent_wf.status, IntentStatus::Clarified);

        // 5. A/B Model Arbitration
        let mut arb = ModelArbitrationEngine::new();
        let arb_res = arb.run_ab_arbitration("PLAN: Uppercase all ASCII characters in doc.txt");
        assert_eq!(arb_res, ArbitrationResult::Agreed);

        // 6. Snapshot taken before mutation
        let mut snap = SnapshotManager::new();
        snap.snapshot_file(&fs, "doc.txt").unwrap();

        // 7. Sandboxed Execution under capability token
        let executor = SandboxedExecutor::new(0x7777_8888);
        fn uppercase(in_data: &[u8], out_data: &mut [u8]) -> usize {
            for (i, &b) in in_data.iter().enumerate() {
                out_data[i] = if b >= b'a' && b <= b'z' { b - 32 } else { b };
            }
            in_data.len()
        }
        let transformed_len = executor.execute_file_transform(&mut fs, "doc.txt", 0x7777_8888, uppercase).unwrap();
        assert_eq!(transformed_len, initial_text.len());

        let mut read_buf = [0u8; 64];
        let n = fs.read_file("doc.txt", &mut read_buf).unwrap();
        assert_eq!(&read_buf[..n], b"INTENT NATIVE OS SPECIFICATION DRAFT");

        // 8. Activity Receipt generated
        let receipt = ActivityReceipt::new(1, "TRANSFORM_UPPERCASE", &read_buf[..n]);
        assert!(receipt.verified);

        // 9. User requests Undo -> original file restored
        let restored_len = snap.restore_file(&mut fs).unwrap();
        assert_eq!(restored_len, initial_text.len());

        let n2 = fs.read_file("doc.txt", &mut read_buf).unwrap();
        assert_eq!(&read_buf[..n2], initial_text);

        // 10. Resource monitor unloads model cleanly
        let mut monitor = SystemResourceMonitor::new(1024);
        monitor.load_model_context(1024);
        let freed = monitor.unload_model_context();
        assert_eq!(freed, 1024);
        assert_eq!(monitor.total_allocated_pages(), 1024);
    }

    #[test]
    fn test_cpu_virtualization_probe() {
        use super::vm::*;
        let (profile, supported) = VirtualizationProbe::probe_host();
        let _ = profile;
        let _ = supported;
        // Probe returns without crashing on any x86-64 host
    }

    #[test]
    fn test_guest_capsule_scope_and_device_grants() {
        use super::vm::*;
        let scope = CapsuleScope::default_isolated();
        assert!(scope.validate_device_request(DeviceGrant::VirtioConsole));
        assert!(scope.validate_device_request(DeviceGrant::VirtioBlock));
        assert!(!scope.validate_device_request(DeviceGrant::VirtioNet));
        assert!(!scope.validate_device_request(DeviceGrant::DirectPhysicalPci(0x1000)));
    }

    #[test]
    fn test_guest_resource_budget_and_reclaim() {
        use super::vm::*;
        let mut budget = GuestResourceBudget::new(64, 256);
        assert_eq!(budget.guest_allocated_pages, 0);

        assert!(budget.allocate_guest_pages(128).is_ok());
        assert_eq!(budget.guest_allocated_pages, 128);

        // Exceeding max must fail
        assert!(budget.allocate_guest_pages(200).is_err());

        let reclaimed = budget.reclaim_all_guest_pages();
        assert_eq!(reclaimed, 128);
        assert_eq!(budget.guest_allocated_pages, 0);
    }

    #[test]
    fn test_hypervisor_launch_stop_lifecycle() {
        use super::vm::*;
        let mut hv = TestHypervisor::new(42, 64, 256);
        assert_eq!(hv.state, GuestState::Uninitialized);

        assert!(hv.launch(64).is_ok());
        assert_eq!(hv.state, GuestState::Running(42));

        assert!(hv.stop(0).is_ok());
        assert_eq!(hv.state, GuestState::Stopped(0));

        let reclaimed = hv.teardown_and_reclaim();
        assert_eq!(reclaimed, 64);
        assert_eq!(hv.state, GuestState::Reclaimed);
    }

    #[test]
    fn test_virtio_device_abstractions() {
        use super::vm::*;
        let mut virtio = VirtioAbstractions::new();
        virtio.console_write(b"Guest output via VirtIO console");
        assert_eq!(&virtio.console_buf[..virtio.console_len], b"Guest output via VirtIO console");

        virtio.net_send_packet(64);
        assert_eq!(virtio.packets_transmitted, 1);
    }

    #[test]
    fn test_disposable_disk_overlay_isolation() {
        use super::storage::*;
        use super::vm::*;

        let mut base = MemoryBlockDevice::new();
        let base_data = [0xAAu8; BLOCK_SIZE];
        base.write_block(5, &base_data).unwrap();

        let mut overlay_disk = DisposableOverlayDisk::new(base);

        // Read through overlay returns base data
        let mut read_buf = [0u8; BLOCK_SIZE];
        overlay_disk.read_block(5, &mut read_buf).unwrap();
        assert_eq!(read_buf, base_data);

        // Write goes to overlay
        let modified_data = [0x55u8; BLOCK_SIZE];
        overlay_disk.write_block(5, &modified_data).unwrap();

        overlay_disk.read_block(5, &mut read_buf).unwrap();
        assert_eq!(read_buf, modified_data);

        // Base device remains untouched!
        overlay_disk.base_device.read_block(5, &mut read_buf).unwrap();
        assert_eq!(read_buf, base_data);

        // Discard overlay restores base view
        overlay_disk.discard_overlay();
        overlay_disk.read_block(5, &mut read_buf).unwrap();
        assert_eq!(read_buf, base_data);
    }

    #[test]
    fn test_shared_display_and_guest_compositor_window() {
        use super::graphics::*;
        use super::vm::*;

        let mut surf = WindowSurface::new(3, Rect::new(10, 10, 40, 30), 2);
        GuestDisplayAdapter::render_guest_window(&mut surf, "Legacy App", Color::LIGHT_BLUE);
        assert_eq!(surf.buffer[0], Color::DARK_GRAY.to_u32());
    }

    #[test]
    fn test_input_forwarding_and_clipboard_isolation() {
        use super::input::*;
        use super::vm::*;

        let mut fwd = GuestInputForwarder::new();
        let scope = CapsuleScope::default_isolated();

        let ev = InputEvent::Keyboard {
            code: KeyCode::KeyW,
            scancode: 0x11,
            pressed: true,
        };
        assert!(fwd.forward_event(ev, &scope).is_ok());
        assert_eq!(fwd.events_forwarded, 1);

        // Clipboard transfer without grant is forbidden fail-closed
        assert!(fwd.transfer_clipboard(&scope).is_err());
    }

    #[test]
    fn test_capsule_file_broker_document_grants() {
        use super::vm::*;

        let mut broker = CapsuleFileBroker::new();
        assert!(broker.grant_file("budget.xlsx", true).is_ok());
        assert!(broker.grant_file("manual.pdf", false).is_ok());

        assert!(broker.is_access_granted("budget.xlsx", false));
        assert!(broker.is_access_granted("budget.xlsx", true));
        assert!(broker.is_access_granted("manual.pdf", false));
        assert!(!broker.is_access_granted("manual.pdf", true));

        // Unauthorized paths denied
        assert!(!broker.is_access_granted("passwords.kdbx", false));
    }

    #[test]
    fn test_compatibility_resolver_binary_scanner() {
        use super::vm::*;

        let elf_hdr = b"\x7fELF\x02\x01\x01\x00";
        assert_eq!(CompatibilityResolver::scan_binary(elf_hdr), BinaryType::NativeElf64);

        let pe_hdr = b"MZ\x90\x00\x03\x00";
        assert_eq!(CompatibilityResolver::scan_binary(pe_hdr), BinaryType::WindowsPe32Plus);

        let unknown = b"\x00\x00\x00\x00";
        assert_eq!(CompatibilityResolver::scan_binary(unknown), BinaryType::Unknown);
    }

    #[test]
    fn test_lightweight_win32_api_dispatch() {
        use super::vm::*;

        let ver = CompatibilityResolver::dispatch_win32_call(0x01, 0).unwrap();
        assert_eq!(ver, 10); // Windows 10

        let exit = CompatibilityResolver::dispatch_win32_call(0x03, 42).unwrap();
        assert_eq!(exit, 42);

        let err = CompatibilityResolver::dispatch_win32_call(0x999, 0);
        assert!(err.is_err());
    }

    #[test]
    fn test_legacy_app_demonstration_gate_g4t() {
        use super::vm::*;
        use super::storage::*;
        use super::input::*;

        // 1. Probe CPU
        let (prof, _) = VirtualizationProbe::probe_host();
        let _ = prof;

        // 2. Launch ephemeral guest
        let mut hv = TestHypervisor::new(100, 64, 256);
        hv.launch(64).unwrap();

        // 3. Mount overlay disk
        let base = MemoryBlockDevice::new();
        let mut overlay_disk = DisposableOverlayDisk::new(base);
        overlay_disk.write_block(1, &[0x42u8; BLOCK_SIZE]).unwrap();

        // 4. Broker grant
        let mut broker = CapsuleFileBroker::new();
        broker.grant_file("legacy_output.txt", true).unwrap();
        assert!(broker.is_access_granted("legacy_output.txt", true));

        // 5. Forward input
        let mut fwd = GuestInputForwarder::new();
        let scope = CapsuleScope::default_isolated();
        fwd.forward_event(InputEvent::Keyboard { code: KeyCode::Enter, scancode: 0x1C, pressed: true }, &scope).unwrap();

        // 6. Stop and teardown guest
        hv.stop(0).unwrap();
        let freed_pages = hv.teardown_and_reclaim();
        assert_eq!(freed_pages, 64);
        assert_eq!(hv.state, GuestState::Reclaimed);

        // 7. Ephemeral overlay discarded
        overlay_disk.discard_overlay();
        assert_eq!(overlay_disk.overlay_count, 0);
    }

    #[test]
    fn test_gate_g5_security_and_hardening() {
        use super::security::*;

        // A001: Canonical serialization & digest
        let op1 = PrivilegedOperation::new(PrivilegedAction::FileWrite, "/sys/core.cfg", "admin", 1000);
        let op1_dup = PrivilegedOperation::new(PrivilegedAction::FileWrite, "/sys/core.cfg", "admin", 1000);
        let op_diff = PrivilegedOperation::new(PrivilegedAction::FileWrite, "/sys/diff.cfg", "admin", 1000);

        let d1 = op1.compute_canonical_digest();
        let d1_dup = op1_dup.compute_canonical_digest();
        let d_diff = op_diff.compute_canonical_digest();
        assert_eq!(d1, d1_dup);
        assert_ne!(d1, d_diff);

        // A002 & A003: Authorization broker trust and replay resistance
        let mut broker = AuthorizationBroker::new();
        // Model cannot mint
        assert!(broker.mint_grant(CallerRole::UntrustedModel, &op1, 1000, 60).is_err());
        // Guest VM cannot mint
        assert!(broker.mint_grant(CallerRole::UntrustedGuestVm, &op1, 1000, 60).is_err());
        // Admin can mint
        let grant = broker.mint_grant(CallerRole::HostAdministrator, &op1, 1000, 60).unwrap();
        assert_eq!(grant.op_digest, d1);

        // First consume succeeds
        assert!(broker.validate_and_consume(grant.grant_id, &op1, 1010).is_ok());
        // Replay fails
        assert!(broker.validate_and_consume(grant.grant_id, &op1, 1015).is_err());

        // A004: Phone pairing & SAS
        let host_pub = b"x25519_host_key_pub";
        let phone_pub = b"x25519_phone_key_pub";
        let sas1 = PhonePairing::compute_sas(host_pub, phone_pub);
        let sas2 = PhonePairing::compute_sas(host_pub, phone_pub);
        let sas_mitm = PhonePairing::compute_sas(host_pub, b"mitm_eve_key");
        assert_eq!(sas1, sas2);
        assert_ne!(sas1, sas_mitm);

        // A006: Hardware approval verifier
        let mut verifier = HardwareApprovalVerifier::new();
        let dev_key: [u8; 32] = [0x5au8; 32];
        verifier.enroll_device(42, &dev_key).unwrap();

        // Sign assertion
        let mut hasher = Sha256::new();
        hasher.update(b"INOS_ASSERT:v1|");
        hasher.update(&42u32.to_be_bytes());
        hasher.update(&1u64.to_be_bytes());
        hasher.update(&d1);
        hasher.update(&dev_key);
        let valid_sig = hasher.finalize();

        assert!(verifier.verify_assertion(42, 1, &d1, &valid_sig).is_ok());
        // Stale counter rejected
        assert!(verifier.verify_assertion(42, 1, &d1, &valid_sig).is_err());

        // A008 & A009: Crypto agility & downgrade protection
        assert!(CryptoRegistry::is_suite_permitted(SecurityProfile::StandardModern, CryptoSuiteId::Ed25519Sha256).is_ok());
        assert!(CryptoRegistry::is_suite_permitted(SecurityProfile::PostQuantumStrict, CryptoSuiteId::MlKem768).is_ok());
        assert!(CryptoRegistry::is_suite_permitted(SecurityProfile::PostQuantumStrict, CryptoSuiteId::Ed25519Sha256).is_err());
        assert!(CryptoRegistry::run_pq_kat_kem());
        assert!(CryptoRegistry::run_pq_kat_dsa());

        // A011 & A012: Measured boot and rollback prevention
        let mut pcr = PcrBank::new();
        PcrBank::extend(&mut pcr.pcr0, b"MEASURE_FIRMWARE_PAYLOAD");
        assert_ne!(pcr.pcr0, [0u8; 32]);

        let mut update_mgr = UpdateManager::new();
        // Stage update to SlotB
        assert!(update_mgr.stage_update(2, PartitionSlot::SlotB).is_ok());
        assert_eq!(update_mgr.current_slot, PartitionSlot::SlotB);

        // Simulated boot failure -> rollback to SlotA
        let (fallback_slot, success) = update_mgr.finalize_boot(false);
        assert!(!success);
        assert_eq!(fallback_slot, PartitionSlot::SlotA);
        assert_eq!(update_mgr.current_slot, PartitionSlot::SlotA);

        // Anti-rollback check: cannot downgrade below min_allowed_version
        assert!(update_mgr.stage_update(0, PartitionSlot::SlotB).is_err());
    }

    #[test]
    fn test_gate_g6_usability_and_performance() {
        use super::evaluation::*;

        // Q005: 16 GiB reference memory budget validation
        let mem = MemoryBudget16Gb::reference_standard();
        assert!(mem.validate());
        assert_eq!(mem.total_mb, 16384);
        assert!(mem.free_headroom_mb >= 2048);

        // Q007: Performance telemetry satisfaction
        let telem = PerformanceTelemetry::current();
        assert!(telem.satisfies_ci_thresholds());

        // Q009: Safe mode boot detection
        assert_eq!(SystemRecoveryManager::evaluate_boot_mode(false), BootRunMode::NormalProduction);
        assert_eq!(SystemRecoveryManager::evaluate_boot_mode(true), BootRunMode::SafeModeDiagnostic);
    }

    #[test]
    fn test_stories_hash() {
        let path = "../models/pretrained/stories15M.bin";
        if let Ok(bytes) = std::fs::read(path) {
            let mut hasher = crate::security::Sha256::new();
            hasher.update(&bytes);
            let digest = hasher.finalize();
            println!("test_stories_hash: {:02x}{:02x}{:02x}{:02x} (len={})", digest[0], digest[1], digest[2], digest[3], bytes.len());
            assert_eq!(&digest[0..4], &[0xcd, 0x59, 0x06, 0x44]);
        }
    }

    #[test]
    fn test_sentinel_cycle1_suite() {
        use super::capability::*;

        println!("=== SDU-01: Sentinel Cycle 1 Capability & Enforcement Suite ===");

        let mut caps = CapabilitySystem::new();

        // 1. Root Capability Handle Creation (S101)
        let (obj_id, h_root) = caps.create_object(
            KernelObjectType::MemoryRegion,
            1, // Owner: PID 1
            rights::READ | rights::WRITE | rights::DELEGATE | rights::REVOKE,
        ).expect("Root capability handle creation must succeed");
        assert_eq!(caps.verify_access(1, h_root, rights::READ | rights::WRITE), Ok(obj_id));

        // 2. Monotonic Downward Attenuation (S101, S104)
        // PID 1 delegates attenuated READ | DELEGATE rights to PID 10
        let h_child = caps.delegate(h_root, 1, 10, rights::READ | rights::DELEGATE)
            .expect("Attenuation downward must succeed");
        assert_eq!(caps.verify_access(10, h_child, rights::READ), Ok(obj_id));

        // Negative Control: PID 10 cannot exercise WRITE (not held)
        assert_eq!(caps.verify_access(10, h_child, rights::WRITE), Err(CapError::InsufficientRights));

        // Negative Control / Mutation Check: PID 10 attempts upward attenuation (adding WRITE)
        let upward_res = caps.delegate(h_child, 10, 20, rights::READ | rights::WRITE);
        assert_eq!(upward_res, Err(CapError::CannotAttenuateUpward),
            "MUTATION CHECK: Upward attenuation must be strictly rejected");

        // 3. Multilevel Delegation (Root -> Child -> Grandchild)
        let h_grandchild = caps.delegate(h_child, 10, 20, rights::READ)
            .expect("Valid sub-delegation must succeed");
        assert_eq!(caps.verify_access(20, h_grandchild, rights::READ), Ok(obj_id));

        // Verify reference counting across the hierarchy (Root + Child + Grandchild = 3)
        assert_eq!(caps.get_object_ref_count(obj_id), Some(3));
        assert!(caps.is_object_alive(obj_id));

        // 4. Recursive Cascading Revocation (S101)
        // Revoking root handle 1 must cascade down to child (h_child) and grandchild (h_grandchild)
        let revoked_count = caps.revoke(h_root, 1).expect("Revocation must succeed");
        assert_eq!(revoked_count, 3, "Revocation must cascade to all 3 handles");

        // All 3 handles must now return CapabilityRevoked
        assert_eq!(caps.verify_access(1, h_root, rights::READ), Err(CapError::CapabilityRevoked));
        assert_eq!(caps.verify_access(10, h_child, rights::READ), Err(CapError::CapabilityRevoked));
        assert_eq!(caps.verify_access(20, h_grandchild, rights::READ), Err(CapError::CapabilityRevoked));

        // Object ref_count must drop to 0 and is_alive must be false
        assert_eq!(caps.get_object_ref_count(obj_id), Some(0));
        assert!(!caps.is_object_alive(obj_id));

        // 5. Generational Epoch Invalidation (S101)
        let mut caps_epoch = CapabilitySystem::new();
        let (obj_epoch, h_ep1) = caps_epoch.create_object(
            KernelObjectType::IpcChannel,
            50,
            rights::READ | rights::DELEGATE,
        ).unwrap();
        assert_eq!(caps_epoch.verify_access(50, h_ep1, rights::READ), Ok(obj_epoch));

        // Advance epoch
        let new_epoch = caps_epoch.advance_epoch();
        assert_eq!(new_epoch, 2);

        // Access via Epoch 1 handle must now be rejected
        assert_eq!(caps_epoch.verify_access(50, h_ep1, rights::READ), Err(CapError::CapabilityRevoked),
            "Epoch advancement must invalidate prior epoch capability handles");

        // 6. Deny-by-Default Scoped Capability Token Registry (S102)
        let mut reg = CapabilityRegistry::new();
        let fake_token = 0xDEAD_BEEF_CAFE_BABE_0123_4567_89AB_CDEFu128;

        // Negative Control: Forged token ID must fail
        assert_eq!(reg.verify_token(100, "/system/config", fake_token, rights::READ),
            Err(CapError::HandleNotFound));

        // Negative Control: Zero token ID must fail
        assert_eq!(reg.verify_token(100, "/system/config", 0u128, rights::READ),
            Err(CapError::UnownedCapability));

        // Issue authentic token manually to bypass RDRAND hardware dependency in host unit test
        let genuine_token_id = 0x1122_3344_5566_7788_99AA_BBCC_DDEE_FF00u128;
        let mut path_bytes = [0u8; 32];
        let p_str = "/system/vault.key";
        path_bytes[..p_str.len()].copy_from_slice(p_str.as_bytes());
        reg.tokens[0] = Some(ScopedCapabilityToken {
            token_id: genuine_token_id,
            owner_pid: 100,
            owner_generation: super::capability::get_process_generation(100),
            path: path_bytes,
            path_len: p_str.len(),
            rights: rights::READ | rights::WRITE,
            epoch: reg.current_epoch,
            is_revoked: false,
            transaction_bound: 0,
        });

        // Authentic check succeeds
        assert_eq!(reg.verify_token(100, "/system/vault.key", genuine_token_id, rights::READ), Ok(()));

        // Cross-process boundary negative control: PID 200 cannot access PID 100's token
        assert_eq!(reg.verify_token(200, "/system/vault.key", genuine_token_id, rights::READ),
            Err(CapError::UnownedCapability));

        // Path mismatch negative control: Cannot use token for unauthorized path
        assert_eq!(reg.verify_token(100, "/system/other.key", genuine_token_id, rights::READ),
            Err(CapError::InsufficientRights));

        // Insufficient rights negative control: EXECUTE not granted
        assert_eq!(reg.verify_token(100, "/system/vault.key", genuine_token_id, rights::EXECUTE),
            Err(CapError::InsufficientRights));

        // Epoch invalidation negative control: Epoch increment revokes all previous tokens fail-closed
        reg.current_epoch += 1;
        assert_eq!(reg.verify_token(100, "/system/vault.key", genuine_token_id, rights::READ),
            Err(CapError::CapabilityRevoked),
            "Token across epoch increment must be denied fail-closed");

        // Stale process generation negative control: PID recycled after process exit
        reg.current_epoch -= 1; // restore epoch to test generation
        super::capability::increment_process_generation(100);
        assert_eq!(reg.verify_token(100, "/system/vault.key", genuine_token_id, rights::READ),
            Err(CapError::UnownedCapability),
            "PID reuse across process generation must be denied fail-closed");

        println!("=== SDU-01: Sentinel Cycle 1 Capability & Enforcement Suite PASSED ===");
    }

    #[test]
    fn test_sentinel_cycle2_firewall_suite() {
        use super::net::*;
        use super::net::firewall::*;

        println!("=== SDU-01: Sentinel Cycle 2 Native Firewall Suite (S201) ===");

        let mut fw = StatefulFirewall::new();
        fw.set_current_tick(100);

        fn make_packet(src_ip: [u8; 4], dst_ip: [u8; 4], src_port: u16, dst_port: u16, proto: u8) -> [u8; 40] {
            let mut p = [0u8; 40];
            p[0] = 0x45; // IPv4, IHL=5
            p[1] = 0x00;
            p[2] = 0; p[3] = 40; // Total length
            p[8] = 64; // TTL
            p[9] = proto; // TCP=6, UDP=17
            p[12..16].copy_from_slice(&src_ip);
            p[16..20].copy_from_slice(&dst_ip);
            let csum = super::net::ipv4::compute_checksum(&p[..20]);
            p[10..12].copy_from_slice(&csum.to_be_bytes());
            p[20..22].copy_from_slice(&src_port.to_be_bytes());
            p[22..24].copy_from_slice(&dst_port.to_be_bytes());
            p[32] = 0x50; // TCP header len = 20
            p
        }

        let host_ip = [10, 0, 2, 15];
        let remote_ip = [93, 184, 216, 34]; // example.com

        let outbound_pkt = make_packet(host_ip, remote_ip, 49152, 80, 6);
        let inbound_reply = make_packet(remote_ip, host_ip, 80, 49152, 6);
        let unsolicited_inbound = make_packet([198, 51, 100, 1], host_ip, 12345, 22, 6);

        // 1. Default-Deny Inbound Test
        let deny_res = fw.evaluate_inbound(&unsolicited_inbound);
        assert_eq!(deny_res, FirewallVerdict::Drop(DropReason::DefaultDenyInbound),
            "Unsolicited inbound traffic must be dropped by default-deny policy");

        // 2. Capability Mediation Test
        // Process lacking network capability cannot emit packets
        let cap_denied = fw.evaluate_outbound(&outbound_pkt, 101, false);
        assert_eq!(cap_denied, FirewallVerdict::Drop(DropReason::CapabilityMissing),
            "Processes without network capability tokens cannot emit packets");

        // 3. Stateful Outbound Inspection and Conntrack Creation
        let accept_out = fw.evaluate_outbound(&outbound_pkt, 101, true);
        assert_eq!(accept_out, FirewallVerdict::Accept,
            "Authorized outbound packet must be accepted and tracked");

        let telem1 = fw.get_telemetry();
        assert_eq!(telem1.active_connections, 1);
        assert_eq!(telem1.total_accepted, 1);

        // 4. Stateful Inbound Response Matching
        // Now that outbound connection is established, matching inbound response is ACCEPTED
        let accept_in = fw.evaluate_inbound(&inbound_reply);
        assert_eq!(accept_in, FirewallVerdict::Accept,
            "Matching inbound packet belonging to established connection must be accepted");

        // 5. Alternate Path / Port Mismatch Rejection
        let forged_reply = make_packet(remote_ip, host_ip, 8080, 49152, 6); // Port 8080 != 80
        let drop_forged = fw.evaluate_inbound(&forged_reply);
        assert_eq!(drop_forged, FirewallVerdict::Drop(DropReason::DefaultDenyInbound),
            "Inbound packet on unnegotiated port must be dropped");

        // 6. Conntrack Expiration Timers
        // Advance clock past 300 second TCP timeout
        fw.set_current_tick(500); // 100 + 400 > 100 + 300
        let expired_in = fw.evaluate_inbound(&inbound_reply);
        assert_eq!(expired_in, FirewallVerdict::Drop(DropReason::NoMatchingConnection),
            "Stale connection must expire and subsequent inbound packets dropped");

        let telem2 = fw.get_telemetry();
        assert_eq!(telem2.active_connections, 0, "Expired connection must be reclaimed");

        println!("=== SDU-01: Sentinel Cycle 2 Native Firewall Suite PASSED ===");
    }

    #[test]
    fn test_sentinel_cycle3_egress_suite() {
        use super::net::*;
        use super::net::firewall::*;
        use super::net::socket::*;

        println!("=== SDU-01: Sentinel Cycle 3 Network Egress & Authority Suite (S202) ===");

        let mut fw = StatefulFirewall::new();
        fw.set_current_tick(100);

        fn make_packet(src_ip: [u8; 4], dst_ip: [u8; 4], src_port: u16, dst_port: u16, proto: u8) -> [u8; 40] {
            let mut p = [0u8; 40];
            p[0] = 0x45; // IPv4, IHL=5
            p[1] = 0x00;
            p[2] = 0; p[3] = 40; // Total length
            p[8] = 64; // TTL
            p[9] = proto; // TCP=6, UDP=17
            p[12..16].copy_from_slice(&src_ip);
            p[16..20].copy_from_slice(&dst_ip);
            let csum = super::net::ipv4::compute_checksum(&p[..20]);
            p[10..12].copy_from_slice(&csum.to_be_bytes());
            p[20..22].copy_from_slice(&src_port.to_be_bytes());
            p[22..24].copy_from_slice(&dst_port.to_be_bytes());
            p[32] = 0x50; // TCP header len = 20
            p
        }

        let host_ip = [10, 0, 2, 15];
        let permitted_dst = [93, 184, 216, 34]; // example.com
        let forbidden_dst = [1, 1, 1, 1];       // unauthorized egress target
        let cap_tok = 0x8888_1111_2222_3333u64;

        // Register Egress Rule: PID 100 with cap_tok can only access permitted_dst:80
        fw.add_egress_rule(EgressRule {
            pid: 100,
            cap_token: cap_tok,
            allowed_dst_ip: Ipv4Address(permitted_dst),
            allowed_dst_port: 80,
            protocol: 6,
        }).expect("Rule registration succeeds");

        let valid_outbound = make_packet(host_ip, permitted_dst, 49152, 80, 6);
        let forbidden_outbound = make_packet(host_ip, forbidden_dst, 49153, 53, 6);

        // 1. Egress Filtering: Destination not permitted is dropped
        let drop_dest = fw.evaluate_outbound_with_context(&forbidden_outbound, 100, cap_tok, true);
        assert_eq!(drop_dest, FirewallVerdict::Drop(DropReason::DestinationNotPermitted),
            "Egress traffic to unpermitted destination must be dropped fail-closed");

        // 2. Permitted Egress is accepted and tracked
        let accept_valid = fw.evaluate_outbound_with_context(&valid_outbound, 100, cap_tok, true);
        assert_eq!(accept_valid, FirewallVerdict::Accept,
            "Permitted egress destination must be accepted");

        // 3. Process Identity Spoofing Protection:
        // PID 200 attempts to send packets using PID 100's established connection
        let spoof_out = fw.evaluate_outbound_with_context(&valid_outbound, 200, cap_tok, true);
        assert_eq!(spoof_out, FirewallVerdict::Drop(DropReason::ProcessIdentityMismatch),
            "Cross-PID packet injection into existing conntrack flow must be rejected");

        // 4. Active Flow Revocation (S202):
        // Revoking capability token must immediately terminate active flow
        let revoked_flows = fw.revoke_capability_flows(100, cap_tok);
        assert_eq!(revoked_flows, 1, "Exactly 1 active flow must be revoked");

        // Subsequent packets on the connection are now dropped fail-closed
        let reply_after_revocation = make_packet(permitted_dst, host_ip, 80, 49152, 6);
        let drop_revoked = fw.evaluate_inbound(&reply_after_revocation);
        assert_eq!(drop_revoked, FirewallVerdict::Drop(DropReason::DefaultDenyInbound),
            "Inbound reply after flow revocation must be dropped by default-deny");

        // 5. Fail-Closed Protocol & Extension Header Check (IPv6)
        let ipv6_pkt = [0x60, 0x00, 0x00, 0x00, 0x00, 0x14, 0x06, 0x40,
                        0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                        0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];
        let drop_v6 = fw.evaluate_inbound(&ipv6_pkt);
        assert_eq!(drop_v6, FirewallVerdict::Drop(DropReason::UnsupportedProtocolExtension),
            "IPv6 traffic must fail-closed as unsupported protocol extension");

        // 6. Raw Socket Egress Mediation
        assert_eq!(StatefulFirewall::check_raw_egress(false), Err(DropReason::RawSocketUnauthorized),
            "Ring 3 process cannot emit raw frames without Ring 0 mediation");
        assert_eq!(StatefulFirewall::check_raw_egress(true), Ok(()),
            "Ring 0 driver permitted for raw packet transmission");

        // 7. Socket Table Revocation Invalidation
        let mut sock_table = SocketTable::new();
        let sock_id = sock_table.allocate_socket(100, cap_tok, SocketType::Tcp, Ipv4Address(host_ip))
            .expect("Socket allocation succeeds");
        assert!(sock_table.get_socket_mut(sock_id, 100).is_ok());

        // Revoke socket via capability
        let revoked_socks = sock_table.revoke_sockets_for_capability(100, cap_tok);
        assert_eq!(revoked_socks, 1);
        assert!(sock_table.get_socket_mut(sock_id, 100).is_err(),
            "Revoked socket must fail-closed on subsequent operations");

        println!("=== SDU-01: Sentinel Cycle 3 Network Egress & Authority Suite PASSED ===");
    }

    #[test]
    fn test_sentinel_cycle4_dnssec_suite() {
        use super::net::*;
        use super::net::dns::*;

        println!("=== SDU-01: Sentinel Cycle 4 DNS & Network Trust Suite (S203) ===");

        // 1. Positive DNSSEC Validation (S203)
        let mut packet = [0u8; 128];
        packet[0] = 0x12; packet[1] = 0x34; // ID
        packet[2] = 0x81; packet[3] = 0x80; // Flags: Standard response, No error
        packet[4] = 0; packet[5] = 1;      // QDCOUNT 1
        packet[6] = 0; packet[7] = 2;      // ANCOUNT 2 (A record + RRSIG record)

        // Question: example.com
        let mut off = 12;
        packet[off] = 7; off += 1;
        packet[off..off + 7].copy_from_slice(b"example"); off += 7;
        packet[off] = 3; off += 1;
        packet[off..off + 3].copy_from_slice(b"com"); off += 3;
        packet[off] = 0; off += 1;
        packet[off] = 0; packet[off + 1] = 1; off += 2; // TYPE A
        packet[off] = 0; packet[off + 1] = 1; off += 2; // CLASS IN

        // Answer 1: A Record
        packet[off] = 0xC0; packet[off + 1] = 0x0C; off += 2; // Pointer to example.com
        packet[off] = 0; packet[off + 1] = 1; off += 2;      // TYPE A
        packet[off] = 0; packet[off + 1] = 1; off += 2;      // CLASS IN
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4; // TTL 300
        packet[off] = 0; packet[off + 1] = 4; off += 2;      // RDLENGTH 4
        packet[off..off + 4].copy_from_slice(&[93, 184, 216, 34]); off += 4;

        // Answer 2: RRSIG Record
        packet[off] = 0xC0; packet[off + 1] = 0x0C; off += 2; // Pointer
        packet[off] = 0; packet[off + 1] = 46; off += 2;     // TYPE RRSIG (46)
        packet[off] = 0; packet[off + 1] = 1; off += 2;      // CLASS IN
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4;
        packet[off] = 0; packet[off + 1] = 18; off += 2;     // RDLENGTH 18

        // RRSIG RDATA:
        packet[off] = 0; packet[off + 1] = 1; off += 2;      // Type covered: A (1)
        packet[off] = 13; off += 1;                          // Algorithm: ECDSAP256SHA256 (13)
        packet[off] = 2; off += 1;                           // Labels: 2
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4; // Orig TTL
        packet[off..off + 4].copy_from_slice(&2000000000u32.to_be_bytes()); off += 4; // Expiration
        packet[off..off + 4].copy_from_slice(&1000000000u32.to_be_bytes()); off += 4; // Inception
        packet[off..off + 2].copy_from_slice(&0x4455u16.to_be_bytes()); off += 2;     // Key tag

        let (ip, res) = parse_and_validate_response(
            0x1234,
            &packet[..off],
            NetworkTrustProfile::StrictDnssec,
            1500000000,
            false,
            true
        ).expect("DNSSEC validation succeeds");

        assert_eq!(ip, Ipv4Address([93, 184, 216, 34]));
        assert_eq!(res, DnssecValidationResult::LocallyValidated);
        assert!(res.is_locally_validated(), "Must be locally validated");

        // 2. Expired RRSIG signature rejected
        let mut expired_packet = packet;
        expired_packet[off - 10..off - 6].copy_from_slice(&1000000000u32.to_be_bytes()); // Expired in past
        expired_packet[off - 6..off - 2].copy_from_slice(&900000000u32.to_be_bytes());
        let err = parse_and_validate_response(
            0x1234,
            &expired_packet[..off],
            NetworkTrustProfile::StrictDnssec,
            1500000000,
            false,
            true
        ).unwrap_err();
        assert_eq!(err, DnsError::RrsigExpired);

        // 3. Downgrade attack rejected in StrictDnssec mode
        let mut unsigned_packet = [0u8; 64];
        unsigned_packet[0] = 0x55; unsigned_packet[1] = 0x66;
        unsigned_packet[2] = 0x81; unsigned_packet[3] = 0x80; // No AD bit, no RRSIG
        unsigned_packet[4] = 0; unsigned_packet[5] = 1;
        unsigned_packet[6] = 0; unsigned_packet[7] = 1;
        let mut uoff = 12;
        unsigned_packet[uoff] = 3; uoff += 1;
        unsigned_packet[uoff..uoff + 3].copy_from_slice(b"org"); uoff += 3;
        unsigned_packet[uoff] = 0; uoff += 1;
        unsigned_packet[uoff] = 0; unsigned_packet[uoff + 1] = 1; uoff += 2;
        unsigned_packet[uoff] = 0; unsigned_packet[uoff + 1] = 1; uoff += 2;
        unsigned_packet[uoff] = 0xC0; unsigned_packet[uoff + 1] = 0x0C; uoff += 2;
        unsigned_packet[uoff] = 0; unsigned_packet[uoff + 1] = 1; uoff += 2;
        unsigned_packet[uoff] = 0; unsigned_packet[uoff + 1] = 1; uoff += 2;
        unsigned_packet[uoff..uoff + 4].copy_from_slice(&300u32.to_be_bytes()); uoff += 4;
        unsigned_packet[uoff] = 0; unsigned_packet[uoff + 1] = 4; uoff += 2;
        unsigned_packet[uoff..uoff + 4].copy_from_slice(&[8, 8, 8, 8]); uoff += 4;

        let err = parse_and_validate_response(
            0x5566,
            &unsigned_packet[..uoff],
            NetworkTrustProfile::StrictDnssec,
            1500000000,
            false,
            true
        ).unwrap_err();
        assert_eq!(err, DnsError::DowngradeAttackDetected);

        // 4. Upstream AD flag does not claim local validation
        let mut ad_packet = unsigned_packet;
        ad_packet[0] = 0x77; ad_packet[1] = 0x88;
        ad_packet[2] = 0x81; ad_packet[3] = 0xA0; // AD bit SET (0x0020)
        let (ip, res) = parse_and_validate_response(
            0x7788,
            &ad_packet[..uoff],
            NetworkTrustProfile::Opportunistic,
            1500000000,
            false,
            true
        ).expect("Opportunistic resolution succeeds");
        assert_eq!(ip, Ipv4Address([8, 8, 8, 8]));
        assert_eq!(res, DnssecValidationResult::UpstreamInspected);
        assert!(!res.is_locally_validated(), "Upstream inspection must NOT claim local validation");

        // 5. DNS Rebinding defense: public domain returning loopback 127.0.0.1 rejected
        let mut rebind_packet = unsigned_packet;
        rebind_packet[0] = 0x99; rebind_packet[1] = 0x00;
        rebind_packet[uoff - 4..uoff].copy_from_slice(&[127, 0, 0, 1]);
        let err = parse_and_validate_response(
            0x9900,
            &rebind_packet[..uoff],
            NetworkTrustProfile::Opportunistic,
            1500000000,
            false,
            true // Expected public domain
        ).unwrap_err();
        assert_eq!(err, DnsError::RebindingAttackDetected);

        println!("=== SDU-01: Sentinel Cycle 4 DNS & Network Trust Suite PASSED ===");
    }
}









