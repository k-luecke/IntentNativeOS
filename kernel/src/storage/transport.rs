// kernel/src/storage/transport.rs - Model Weight Transport Abstraction
// Decouples high-level model loading and neural inference from low-level bus transports.

use super::AtaPioBlockDevice;
use crate::drivers::virtio_blk::VirtioBlkDevice;

pub trait ModelTransport {
    fn transport_name(&self) -> &'static str;
    fn read_sectors(&mut self, start_lba: u64, count: u8, dest: *mut u8) -> Result<(), &'static str>;
}

pub struct AtaModelTransport {
    pub dev: AtaPioBlockDevice,
}

impl ModelTransport for AtaModelTransport {
    fn transport_name(&self) -> &'static str {
        "Batched ATA PIO (Port 0x170)"
    }

    fn read_sectors(&mut self, start_lba: u64, count: u8, dest: *mut u8) -> Result<(), &'static str> {
        self.dev.read_multi_blocks(start_lba, count, dest)
            .map_err(|_| "ATA read failure")
    }
}

pub struct VirtioModelTransport {
    pub dev: VirtioBlkDevice,
}

impl ModelTransport for VirtioModelTransport {
    fn transport_name(&self) -> &'static str {
        "VirtIO-blk Legacy PCI (DMA)"
    }

    fn read_sectors(&mut self, start_lba: u64, count: u8, dest: *mut u8) -> Result<(), &'static str> {
        self.dev.read_blocks_dma(start_lba, count, dest)
            .map_err(|_| "VirtIO DMA failure")
    }
}
