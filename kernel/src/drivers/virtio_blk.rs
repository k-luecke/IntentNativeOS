// kernel/src/drivers/virtio_blk.rs - Legacy VirtIO-blk PCI Driver (VirtIO 0.9.5 Specification)
// Provides DMA block transport using split virtqueues and bounded physical bounce buffers.

use crate::arch::x86_64::cpu::{inb, inl, inw, outb, outl, outw};
use crate::memory::frame_allocator::PhysicalFrameAllocator;
use super::pci::PciDevice;

pub const VIRTIO_VENDOR_ID: u16 = 0x1AF4;
pub const VIRTIO_DEV_BLOCK_LEGACY: u16 = 0x1001;

// VirtIO 0.9.5 Register Offsets from I/O Base
const VIRTIO_REG_DEVICE_FEATURES: u16 = 0x00;
const VIRTIO_REG_GUEST_FEATURES: u16 = 0x04;
const VIRTIO_REG_QUEUE_PFN: u16 = 0x08;
const VIRTIO_REG_QUEUE_NUM: u16 = 0x0C;
const VIRTIO_REG_QUEUE_SEL: u16 = 0x0E;
const VIRTIO_REG_QUEUE_NOTIFY: u16 = 0x10;
const VIRTIO_REG_DEVICE_STATUS: u16 = 0x12;
const VIRTIO_REG_ISR_STATUS: u16 = 0x13;
const VIRTIO_REG_CONFIG_CAPACITY: u16 = 0x14; // 64-bit sector capacity

// Device Status Bits
const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
const VIRTIO_STATUS_FAILED: u8 = 128;

// Descriptor Flags
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

// Request Types
const VIRTIO_BLK_T_IN: u32 = 0;  // Read request
const VIRTIO_BLK_S_OK: u8 = 0;   // Status success

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtqDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C)]
pub struct VirtqUsedElem {
    pub id: u32,
    pub len: u32,
}

#[repr(C)]
pub struct VirtioBlkReqHeader {
    pub type_: u32,
    pub ioprio: u32,
    pub sector: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioBlkError {
    PciNotMatched,
    NoIoBar,
    QueueUnavailable,
    OutOfMemory,
    DeviceError(u8),
    Timeout,
    InvalidLba,
}

pub struct VirtioBlkDevice {
    pub io_base: u16,
    pub capacity_sectors: u64,
    pub q_size: u16,
    pub used_offset: u64,
    pub queue_phys: u64,
    pub req_phys: u64,
    pub bounce_phys: u64,
    last_used_idx: u16,
}

impl VirtioBlkDevice {
    /// Discovers and initializes a Legacy VirtIO Block Device from PCI.
    /// Allocates dedicated guest-physical frames for Split VirtQueue, request headers, and DMA bounce buffer.
    pub fn probe_and_init(allocator: &mut PhysicalFrameAllocator) -> Result<Self, VirtioBlkError> {
        let pci_dev = super::pci::find_pci_device(VIRTIO_VENDOR_ID, VIRTIO_DEV_BLOCK_LEGACY)
            .ok_or(VirtioBlkError::PciNotMatched)?;

        let io_base = pci_dev.io_base().ok_or(VirtioBlkError::NoIoBar)?;

        unsafe {
            // Enable PCI Bus Mastering and I/O decoding
            pci_dev.enable_bus_master();

            // 1. Reset device
            outb(io_base + VIRTIO_REG_DEVICE_STATUS, 0);

            // 2. Set ACKNOWLEDGE status bit
            outb(io_base + VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);

            // 3. Set DRIVER status bit
            outb(io_base + VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER);

            // 4. Read device features & negotiate (write 0 to acknowledge basic subset)
            let _dev_features = inl(io_base + VIRTIO_REG_DEVICE_FEATURES);
            outl(io_base + VIRTIO_REG_GUEST_FEATURES, 0);

            // 5. Select queue 0 (request queue)
            outw(io_base + VIRTIO_REG_QUEUE_SEL, 0);
            let q_size = inw(io_base + VIRTIO_REG_QUEUE_NUM);
            if q_size == 0 || q_size > 256 {
                outb(io_base + VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_FAILED);
                return Err(VirtioBlkError::QueueUnavailable);
            }

            // 6. Allocate contiguous physical pages for VirtQueue according to VirtIO 0.9.5 alignment rules
            let avail_offset = (q_size as u64) * 16;
            let avail_size = 4 + 2 * (q_size as u64) + 2;
            let used_offset = ((avail_offset + avail_size + 4095) / 4096) * 4096;
            let used_size = 4 + 8 * (q_size as u64) + 2;
            let total_bytes = ((used_offset + used_size + 4095) / 4096) * 4096;
            let total_pages = (total_bytes / 4096) as usize;

            let queue_phys = allocator.alloc_contiguous_frames(total_pages)
                .map_err(|_| VirtioBlkError::OutOfMemory)?;

            // Zero out virtqueue memory
            core::ptr::write_bytes(queue_phys as *mut u8, 0, (total_pages * 4096) as usize);

            // 7. Configure queue physical page frame number (PFN = phys_addr >> 12)
            outl(io_base + VIRTIO_REG_QUEUE_PFN, (queue_phys >> 12) as u32);

            // 8. Allocate 1 physical page for request header and status byte
            let req_phys = allocator.alloc_frame()
                .map_err(|_| VirtioBlkError::OutOfMemory)?;
            core::ptr::write_bytes(req_phys as *mut u8, 0, 4096);

            // 9. Allocate 16 contiguous physical pages (64 KiB = 128 sectors) for DMA bounce buffer
            let bounce_phys = allocator.alloc_contiguous_frames(16)
                .map_err(|_| VirtioBlkError::OutOfMemory)?;
            core::ptr::write_bytes(bounce_phys as *mut u8, 0, 65536);

            // 10. Set DRIVER_OK status bit
            outb(
                io_base + VIRTIO_REG_DEVICE_STATUS,
                VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
            );

            // 11. Read capacity from device config register
            let cap_lo = inl(io_base + VIRTIO_REG_CONFIG_CAPACITY) as u64;
            let cap_hi = inl(io_base + VIRTIO_REG_CONFIG_CAPACITY + 4) as u64;
            let capacity_sectors = cap_lo | (cap_hi << 32);

            Ok(Self {
                io_base,
                capacity_sectors,
                q_size,
                used_offset,
                queue_phys,
                req_phys,
                bounce_phys,
                last_used_idx: 0,
            })
        }
    }

    /// Reads up to 128 sectors (64 KiB) using DMA through a bounded physical bounce buffer.
    pub fn read_blocks_dma(&mut self, start_lba: u64, count: u8, dest: *mut u8) -> Result<(), VirtioBlkError> {
        if count == 0 {
            return Ok(());
        }
        if count > 128 {
            return Err(VirtioBlkError::InvalidLba);
        }
        if start_lba + (count as u64) > self.capacity_sectors && self.capacity_sectors > 0 {
            return Err(VirtioBlkError::InvalidLba);
        }

        let bytes_to_read = (count as u32) * 512;

        unsafe {
            // Setup Request Header at req_phys + 0
            let req_hdr = self.req_phys as *mut VirtioBlkReqHeader;
            (*req_hdr).type_ = VIRTIO_BLK_T_IN;
            (*req_hdr).ioprio = 0;
            (*req_hdr).sector = start_lba;

            // Status byte at req_phys + 32 (set to 0xFF initial sentinel)
            let status_ptr = (self.req_phys + 32) as *mut u8;
            core::ptr::write_volatile(status_ptr, 0xFF);

            // VirtQueue pointer layout
            let desc_table = self.queue_phys as *mut VirtqDesc;
            let avail_ring_flags = (self.queue_phys + (self.q_size as u64) * 16) as *mut u16;
            let avail_ring_idx = avail_ring_flags.add(1);
            let avail_ring_entries = avail_ring_flags.add(2);

            // Used Ring is located at used_offset
            let used_ring_flags = (self.queue_phys + self.used_offset) as *const u16;
            let used_ring_idx = used_ring_flags.add(1);

            // Descriptor 0: Request Header (Device Reads from Guest)
            let d0 = &mut *desc_table.add(0);
            d0.addr = self.req_phys;
            d0.len = 16;
            d0.flags = VIRTQ_DESC_F_NEXT;
            d0.next = 1;

            // Descriptor 1: Bounded Physical DMA Bounce Buffer (Device Writes into Guest)
            let d1 = &mut *desc_table.add(1);
            d1.addr = self.bounce_phys;
            d1.len = bytes_to_read;
            d1.flags = VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE;
            d1.next = 2;

            // Descriptor 2: Status Byte (Device Writes into Guest)
            let d2 = &mut *desc_table.add(2);
            d2.addr = self.req_phys + 32;
            d2.len = 1;
            d2.flags = VIRTQ_DESC_F_WRITE;
            d2.next = 0;

            // Put Descriptor 0 into Available Ring
            let current_avail = core::ptr::read_volatile(avail_ring_idx);
            let avail_slot = (current_avail % self.q_size) as usize;
            core::ptr::write_volatile(avail_ring_entries.add(avail_slot), 0);

            // Memory fence before updating available index
            core::arch::asm!("mfence", options(nostack, preserves_flags));
            core::ptr::write_volatile(avail_ring_idx, current_avail.wrapping_add(1));
            core::arch::asm!("mfence", options(nostack, preserves_flags));

            // Notify Device on Queue 0
            outw(self.io_base + VIRTIO_REG_QUEUE_NOTIFY, 0);

            // Bounded polling for completion
            let target_used_idx = self.last_used_idx.wrapping_add(1);
            let mut timeout = 20_000_000u64;
            while timeout > 0 {
                core::arch::asm!("pause", options(nostack, preserves_flags));
                let current_used = core::ptr::read_volatile(used_ring_idx);
                if current_used == target_used_idx {
                    break;
                }
                timeout -= 1;
            }

            if timeout == 0 {
                return Err(VirtioBlkError::Timeout);
            }
            self.last_used_idx = target_used_idx;

            // Verify device status byte
            let status = core::ptr::read_volatile(status_ptr);
            if status != VIRTIO_BLK_S_OK {
                return Err(VirtioBlkError::DeviceError(status));
            }

            // Controlled ownership transition: copy from verified DMA buffer to target
            core::ptr::copy_nonoverlapping(self.bounce_phys as *const u8, dest, bytes_to_read as usize);
            Ok(())
        }
    }
}
