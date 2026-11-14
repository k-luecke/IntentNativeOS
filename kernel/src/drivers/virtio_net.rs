// kernel/src/drivers/virtio_net.rs - Legacy VirtIO-net PCI Driver (VirtIO 0.9.5 Specification)
// Provides controlled DMA packet transport using split virtqueues and bounded physical buffers.

use crate::arch::x86_64::cpu::{inb, inl, inw, outb, outl, outw};
use crate::memory::frame_allocator::PhysicalFrameAllocator;
use super::pci::PciDevice;

pub const VIRTIO_VENDOR_ID: u16 = 0x1AF4;
pub const VIRTIO_DEV_NET_LEGACY: u16 = 0x1000;
pub const VIRTIO_DEV_NET_MODERN: u16 = 0x1041;

// VirtIO 0.9.5 Register Offsets from I/O Base
const VIRTIO_REG_DEVICE_FEATURES: u16 = 0x00;
const VIRTIO_REG_GUEST_FEATURES: u16 = 0x04;
const VIRTIO_REG_QUEUE_PFN: u16 = 0x08;
const VIRTIO_REG_QUEUE_NUM: u16 = 0x0C;
const VIRTIO_REG_QUEUE_SEL: u16 = 0x0E;
const VIRTIO_REG_QUEUE_NOTIFY: u16 = 0x10;
const VIRTIO_REG_DEVICE_STATUS: u16 = 0x12;
const VIRTIO_REG_ISR_STATUS: u16 = 0x13;
const VIRTIO_REG_CONFIG_MAC: u16 = 0x14; // 6-byte MAC address

// Device Status Bits
const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
const VIRTIO_STATUS_FAILED: u8 = 128;

// Descriptor Flags
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

pub const NUM_RX_BUFFERS: usize = 16;
pub const PACKET_BUFFER_SIZE: usize = 1536; // VirtioNetHdr (10) + Ethernet Frame (1518) + padding

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct VirtioNetHdr {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: u16,
    pub gso_size: u16,
    pub csum_start: u16,
    pub csum_offset: u16,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioNetError {
    PciNotMatched,
    NoIoBar,
    QueueUnavailable,
    OutOfMemory,
    DeviceError(u8),
    TxTimeout,
    BufferTooSmall,
    PacketTooLarge,
}

pub struct VirtioNetDevice {
    pub io_base: u16,
    pub mac: [u8; 6],
    pub rx_q_size: u16,
    pub tx_q_size: u16,
    pub rx_queue_phys: u64,
    pub tx_queue_phys: u64,
    pub rx_buffers_phys: u64,
    pub tx_buffer_phys: u64,
    rx_used_offset: u64,
    tx_used_offset: u64,
    last_rx_used_idx: u16,
    last_tx_used_idx: u16,
}

impl VirtioNetDevice {
    /// Discovers and initializes a VirtIO Network Device from PCI.
    /// Allocates dedicated guest-physical frames for Rx/Tx VirtQueues and DMA packet buffers.
    pub fn probe_and_init(allocator: &mut PhysicalFrameAllocator) -> Result<Self, VirtioNetError> {
        let pci_dev = super::pci::find_pci_device(VIRTIO_VENDOR_ID, VIRTIO_DEV_NET_LEGACY)
            .or_else(|| super::pci::find_pci_device(VIRTIO_VENDOR_ID, VIRTIO_DEV_NET_MODERN))
            .ok_or(VirtioNetError::PciNotMatched)?;

        let io_base = pci_dev.io_base().ok_or(VirtioNetError::NoIoBar)?;

        unsafe {
            // Enable PCI Bus Mastering and I/O decoding
            pci_dev.enable_bus_master();

            // 1. Reset device
            outb(io_base + VIRTIO_REG_DEVICE_STATUS, 0);

            // 2. Set ACKNOWLEDGE status bit
            outb(io_base + VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);

            // 3. Set DRIVER status bit
            outb(io_base + VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER);

            // 4. Read device features & negotiate
            let _dev_features = inl(io_base + VIRTIO_REG_DEVICE_FEATURES);
            outl(io_base + VIRTIO_REG_GUEST_FEATURES, 0); // basic subset

            // 5. Read MAC address from device configuration registers (0x14..0x1A)
            let mut mac = [0u8; 6];
            for i in 0..6 {
                mac[i] = inb(io_base + VIRTIO_REG_CONFIG_MAC + (i as u16));
            }

            // 6. Setup Receive VirtQueue (Queue 0)
            outw(io_base + VIRTIO_REG_QUEUE_SEL, 0);
            let rx_q_size = inw(io_base + VIRTIO_REG_QUEUE_NUM);
            if rx_q_size == 0 || rx_q_size > 256 {
                outb(io_base + VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_FAILED);
                return Err(VirtioNetError::QueueUnavailable);
            }

            let rx_avail_offset = (rx_q_size as u64) * 16;
            let rx_avail_size = 4 + 2 * (rx_q_size as u64) + 2;
            let rx_used_offset = ((rx_avail_offset + rx_avail_size + 4095) / 4096) * 4096;
            let rx_used_size = 4 + 8 * (rx_q_size as u64) + 2;
            let rx_total_bytes = ((rx_used_offset + rx_used_size + 4095) / 4096) * 4096;
            let rx_total_pages = (rx_total_bytes / 4096) as usize;

            let rx_queue_phys = allocator.alloc_contiguous_frames(rx_total_pages)
                .map_err(|_| VirtioNetError::OutOfMemory)?;
            core::ptr::write_bytes(rx_queue_phys as *mut u8, 0, rx_total_pages * 4096);
            outl(io_base + VIRTIO_REG_QUEUE_PFN, (rx_queue_phys >> 12) as u32);

            // 7. Setup Transmit VirtQueue (Queue 1)
            outw(io_base + VIRTIO_REG_QUEUE_SEL, 1);
            let tx_q_size = inw(io_base + VIRTIO_REG_QUEUE_NUM);
            if tx_q_size == 0 || tx_q_size > 256 {
                outb(io_base + VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_FAILED);
                return Err(VirtioNetError::QueueUnavailable);
            }

            let tx_avail_offset = (tx_q_size as u64) * 16;
            let tx_avail_size = 4 + 2 * (tx_q_size as u64) + 2;
            let tx_used_offset = ((tx_avail_offset + tx_avail_size + 4095) / 4096) * 4096;
            let tx_used_size = 4 + 8 * (tx_q_size as u64) + 2;
            let tx_total_bytes = ((tx_used_offset + tx_used_size + 4095) / 4096) * 4096;
            let tx_total_pages = (tx_total_bytes / 4096) as usize;

            let tx_queue_phys = allocator.alloc_contiguous_frames(tx_total_pages)
                .map_err(|_| VirtioNetError::OutOfMemory)?;
            core::ptr::write_bytes(tx_queue_phys as *mut u8, 0, tx_total_pages * 4096);
            outl(io_base + VIRTIO_REG_QUEUE_PFN, (tx_queue_phys >> 12) as u32);

            // 8. Allocate DMA buffers:
            // Rx buffers: NUM_RX_BUFFERS * PACKET_BUFFER_SIZE (~24 KiB = 6 physical pages)
            let rx_buf_pages = (NUM_RX_BUFFERS * PACKET_BUFFER_SIZE + 4095) / 4096;
            let rx_buffers_phys = allocator.alloc_contiguous_frames(rx_buf_pages)
                .map_err(|_| VirtioNetError::OutOfMemory)?;
            core::ptr::write_bytes(rx_buffers_phys as *mut u8, 0, rx_buf_pages * 4096);

            // Tx buffer: 1 physical page (4096 bytes)
            let tx_buffer_phys = allocator.alloc_frame()
                .map_err(|_| VirtioNetError::OutOfMemory)?;
            core::ptr::write_bytes(tx_buffer_phys as *mut u8, 0, 4096);

            // 9. Populate Rx Queue descriptors and Available Ring
            let rx_desc_table = rx_queue_phys as *mut VirtqDesc;
            let rx_avail_flags = (rx_queue_phys + rx_avail_offset) as *mut u16;
            let rx_avail_idx = rx_avail_flags.add(1);
            let rx_avail_entries = rx_avail_flags.add(2);

            for i in 0..NUM_RX_BUFFERS {
                let desc = &mut *rx_desc_table.add(i);
                desc.addr = rx_buffers_phys + (i * PACKET_BUFFER_SIZE) as u64;
                desc.len = PACKET_BUFFER_SIZE as u32;
                desc.flags = VIRTQ_DESC_F_WRITE; // Device writes into guest buffer
                desc.next = 0;

                core::ptr::write_volatile(rx_avail_entries.add(i), i as u16);
            }

            core::arch::asm!("mfence", options(nostack, preserves_flags));
            core::ptr::write_volatile(rx_avail_idx, NUM_RX_BUFFERS as u16);
            core::arch::asm!("mfence", options(nostack, preserves_flags));

            // 10. Set DRIVER_OK status bit
            outb(
                io_base + VIRTIO_REG_DEVICE_STATUS,
                VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
            );

            // 11. Notify Rx Queue that buffers are populated
            outw(io_base + VIRTIO_REG_QUEUE_NOTIFY, 0);

            Ok(Self {
                io_base,
                mac,
                rx_q_size,
                tx_q_size,
                rx_queue_phys,
                tx_queue_phys,
                rx_buffers_phys,
                tx_buffer_phys,
                rx_used_offset,
                tx_used_offset,
                last_rx_used_idx: 0,
                last_tx_used_idx: 0,
            })
        }
    }

    /// Sends a raw Ethernet frame via the Transmit VirtQueue using bounded DMA buffers.
    pub fn send_packet(&mut self, packet_data: &[u8]) -> Result<(), VirtioNetError> {
        if packet_data.len() > (PACKET_BUFFER_SIZE - 10) {
            return Err(VirtioNetError::PacketTooLarge);
        }

        unsafe {
            // Write VirtioNetHdr (10 bytes of zeroes for basic transmit)
            let hdr_ptr = self.tx_buffer_phys as *mut VirtioNetHdr;
            core::ptr::write_volatile(hdr_ptr, VirtioNetHdr::default());

            // Copy Ethernet packet payload immediately after header
            let payload_dest = (self.tx_buffer_phys + 10) as *mut u8;
            core::ptr::copy_nonoverlapping(packet_data.as_ptr(), payload_dest, packet_data.len());

            let total_len = (10 + packet_data.len()) as u32;

            // Setup Descriptor 0 in Tx VirtQueue
            let tx_desc_table = self.tx_queue_phys as *mut VirtqDesc;
            let d0 = &mut *tx_desc_table.add(0);
            d0.addr = self.tx_buffer_phys;
            d0.len = total_len;
            d0.flags = 0; // Device reads from memory
            d0.next = 0;

            // Put Descriptor 0 into Tx Available Ring
            let tx_avail_offset = (self.tx_q_size as u64) * 16;
            let tx_avail_flags = (self.tx_queue_phys + tx_avail_offset) as *mut u16;
            let tx_avail_idx = tx_avail_flags.add(1);
            let tx_avail_entries = tx_avail_flags.add(2);

            let current_avail = core::ptr::read_volatile(tx_avail_idx);
            let avail_slot = (current_avail % self.tx_q_size) as usize;
            core::ptr::write_volatile(tx_avail_entries.add(avail_slot), 0);

            core::arch::asm!("mfence", options(nostack, preserves_flags));
            core::ptr::write_volatile(tx_avail_idx, current_avail.wrapping_add(1));
            core::arch::asm!("mfence", options(nostack, preserves_flags));

            // Notify Tx Queue (Queue 1)
            outw(self.io_base + VIRTIO_REG_QUEUE_NOTIFY, 1);

            // Poll Tx Used Ring for completion
            let tx_used_flags = (self.tx_queue_phys + self.tx_used_offset) as *const u16;
            let tx_used_idx = tx_used_flags.add(1);

            let mut timeout: u32 = 1_000_000;
            while timeout > 0 {
                let used_idx_val = core::ptr::read_volatile(tx_used_idx);
                if used_idx_val != self.last_tx_used_idx {
                    self.last_tx_used_idx = used_idx_val;
                    return Ok(());
                }
                core::arch::asm!("pause", options(nomem, nostack, preserves_flags));
                timeout -= 1;
            }

            Err(VirtioNetError::TxTimeout)
        }
    }

    /// Polls the Receive VirtQueue for newly arrived Ethernet packets.
    /// If a packet is available, copies it into `dest` (skipping VirtioNetHdr) and re-posts the buffer.
    pub fn poll_receive(&mut self, dest: &mut [u8]) -> Result<Option<usize>, VirtioNetError> {
        unsafe {
            let rx_used_flags = (self.rx_queue_phys + self.rx_used_offset) as *const u16;
            let rx_used_idx = rx_used_flags.add(1);
            let used_idx_val = core::ptr::read_volatile(rx_used_idx);

            if used_idx_val == self.last_rx_used_idx {
                return Ok(None); // No packet arrived
            }

            // Read the completed element from Used Ring
            let used_slot = (self.last_rx_used_idx % self.rx_q_size) as usize;
            let used_ring_elems = (self.rx_queue_phys + self.rx_used_offset + 4) as *const VirtqUsedElem;
            let elem = &*used_ring_elems.add(used_slot);

            let buf_id = elem.id as usize;
            let total_bytes = elem.len as usize;

            if total_bytes < 10 {
                // Malformed packet shorter than VirtIO Net header
                self.last_rx_used_idx = self.last_rx_used_idx.wrapping_add(1);
                self.repost_rx_buffer(buf_id);
                return Ok(None);
            }

            let payload_len = total_bytes - 10;
            if dest.len() < payload_len {
                return Err(VirtioNetError::BufferTooSmall);
            }

            // Copy packet bytes (skipping 10-byte VirtioNetHdr)
            let src_ptr = (self.rx_buffers_phys + (buf_id * PACKET_BUFFER_SIZE) as u64 + 10) as *const u8;
            core::ptr::copy_nonoverlapping(src_ptr, dest.as_mut_ptr(), payload_len);

            self.last_rx_used_idx = self.last_rx_used_idx.wrapping_add(1);

            // Re-post buffer back to Available Ring
            self.repost_rx_buffer(buf_id);

            Ok(Some(payload_len))
        }
    }

    unsafe fn repost_rx_buffer(&mut self, buf_id: usize) {
        let rx_avail_offset = (self.rx_q_size as u64) * 16;
        let rx_avail_flags = (self.rx_queue_phys + rx_avail_offset) as *mut u16;
        let rx_avail_idx = rx_avail_flags.add(1);
        let rx_avail_entries = rx_avail_flags.add(2);

        let current_avail = core::ptr::read_volatile(rx_avail_idx);
        let avail_slot = (current_avail % self.rx_q_size) as usize;
        core::ptr::write_volatile(rx_avail_entries.add(avail_slot), buf_id as u16);

        core::arch::asm!("mfence", options(nostack, preserves_flags));
        core::ptr::write_volatile(rx_avail_idx, current_avail.wrapping_add(1));
        core::arch::asm!("mfence", options(nostack, preserves_flags));

        outw(self.io_base + VIRTIO_REG_QUEUE_NOTIFY, 0);
    }
}
