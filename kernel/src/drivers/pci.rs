// kernel/src/drivers/pci.rs - Standard PCI Configuration Space & Device Discovery
// Compliant with PCI Local Bus Specification 2.2 / 3.0

use crate::arch::x86_64::cpu::{inb, inl, inw, outb, outl, outw};

const PCI_CONFIG_ADDRESS: u16 = 0x0CF8;
const PCI_CONFIG_DATA: u16 = 0x0CFC;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub bar0: u32,
    pub subsystem_vendor_id: u16,
    pub subsystem_device_id: u16,
}

impl PciDevice {
    pub fn config_addr(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
        0x8000_0000u32
            | ((bus as u32) << 16)
            | (((device & 0x1F) as u32) << 11)
            | (((function & 0x07) as u32) << 8)
            | ((offset & 0xFC) as u32)
    }

    pub unsafe fn read_u32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
        outl(PCI_CONFIG_ADDRESS, Self::config_addr(bus, device, function, offset));
        inl(PCI_CONFIG_DATA)
    }

    pub unsafe fn read_u16(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
        outl(PCI_CONFIG_ADDRESS, Self::config_addr(bus, device, function, offset));
        inw(PCI_CONFIG_DATA + ((offset & 2) as u16))
    }

    pub unsafe fn read_u8(bus: u8, device: u8, function: u8, offset: u8) -> u8 {
        outl(PCI_CONFIG_ADDRESS, Self::config_addr(bus, device, function, offset));
        inb(PCI_CONFIG_DATA + ((offset & 3) as u16))
    }

    pub unsafe fn write_u16(bus: u8, device: u8, function: u8, offset: u8, val: u16) {
        outl(PCI_CONFIG_ADDRESS, Self::config_addr(bus, device, function, offset));
        outw(PCI_CONFIG_DATA + ((offset & 2) as u16), val);
    }

    pub unsafe fn write_u32(bus: u8, device: u8, function: u8, offset: u8, val: u32) {
        outl(PCI_CONFIG_ADDRESS, Self::config_addr(bus, device, function, offset));
        outl(PCI_CONFIG_DATA, val);
    }

    /// Enables I/O space (bit 0), Memory space (bit 1), and Bus Master DMA (bit 2)
    pub unsafe fn enable_bus_master(&self) {
        let cmd = Self::read_u16(self.bus, self.device, self.function, 0x04);
        // Bit 0: I/O Enable, Bit 1: Memory Enable, Bit 2: Bus Master Enable
        Self::write_u16(self.bus, self.device, self.function, 0x04, cmd | 0x07);
    }

    /// Returns the I/O base port if BAR0 is an I/O space BAR
    pub fn io_base(&self) -> Option<u16> {
        if (self.bar0 & 0x01) != 0 {
            Some((self.bar0 & !0x03) as u16)
        } else {
            None
        }
    }
}

/// Scans PCI buses 0..8 for a device matching the specified vendor and device ID.
pub fn find_pci_device(vendor_id: u16, device_id: u16) -> Option<PciDevice> {
    for bus in 0..8 {
        for dev in 0..32 {
            let v = unsafe { PciDevice::read_u16(bus, dev, 0, 0x00) };
            if v == 0xFFFF || v == 0x0000 {
                continue;
            }
            let d = unsafe { PciDevice::read_u16(bus, dev, 0, 0x02) };
            if v == vendor_id && d == device_id {
                let cmd = unsafe { PciDevice::read_u16(bus, dev, 0, 0x04) };
                let status = unsafe { PciDevice::read_u16(bus, dev, 0, 0x06) };
                let class_sub = unsafe { PciDevice::read_u16(bus, dev, 0, 0x0A) };
                let prog_if = unsafe { PciDevice::read_u8(bus, dev, 0, 0x09) };
                let bar0 = unsafe { PciDevice::read_u32(bus, dev, 0, 0x10) };
                let sub_v = unsafe { PciDevice::read_u16(bus, dev, 0, 0x2C) };
                let sub_d = unsafe { PciDevice::read_u16(bus, dev, 0, 0x2E) };

                return Some(PciDevice {
                    bus,
                    device: dev,
                    function: 0,
                    vendor_id: v,
                    device_id: d,
                    command: cmd,
                    status,
                    class_code: (class_sub >> 8) as u8,
                    subclass: (class_sub & 0xFF) as u8,
                    prog_if,
                    bar0,
                    subsystem_vendor_id: sub_v,
                    subsystem_device_id: sub_d,
                });
            }
        }
    }
    None
}
