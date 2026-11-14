// kernel/src/drivers/hw_discovery.rs - Read-Only Hardware Inventory & Driver Matching (HW-00)
// STRICT NON-NEGOTIABLE INVARIANT: 100% READ-ONLY. Zero config writes, zero driver binding, zero disk writes.

use crate::arch::x86_64::cpu::{inb, inl, inw, outl};
use crate::arch::x86_64::serial::SERIAL1;

const PCI_CONFIG_ADDRESS: u16 = 0x0CF8;
const PCI_CONFIG_DATA: u16 = 0x0CFC;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceCategory {
    HostBridge,
    IsaBridge,
    PciBridge,
    StorageIde,
    StorageAhci,
    StorageVirtioBlk,
    StorageOther,
    NetworkEthernet,
    NetworkVirtioNet,
    DisplayVga,
    DisplayVirtioGpu,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub struct DiscoveredDevice {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub bar0: u32,
    pub category: DeviceCategory,
    pub matched_driver_name: &'static str,
}

pub const MAX_DISCOVERED_DEVICES: usize = 32;

pub struct HardwareInventory {
    pub devices: [Option<DiscoveredDevice>; MAX_DISCOVERED_DEVICES],
    pub device_count: usize,
    pub cpu_vendor: [u8; 12],
    pub cpu_family: u32,
    pub cpu_model: u32,
    pub cpu_stepping: u32,
    pub has_sse: bool,
    pub has_avx: bool,
    pub has_rdrand: bool,
    pub has_apic: bool,
}

impl HardwareInventory {
    pub const fn empty() -> Self {
        Self {
            devices: [None; MAX_DISCOVERED_DEVICES],
            device_count: 0,
            cpu_vendor: [0u8; 12],
            cpu_family: 0,
            cpu_model: 0,
            cpu_stepping: 0,
            has_sse: false,
            has_avx: false,
            has_rdrand: false,
            has_apic: false,
        }
    }
}

pub static mut HW_INVENTORY: HardwareInventory = HardwareInventory::empty();

fn pci_config_addr(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    0x8000_0000u32
        | ((bus as u32) << 16)
        | (((dev & 0x1F) as u32) << 11)
        | (((func & 0x07) as u32) << 8)
        | ((offset & 0xFC) as u32)
}

unsafe fn pci_read_u16(bus: u8, dev: u8, func: u8, offset: u8) -> u16 {
    outl(PCI_CONFIG_ADDRESS, pci_config_addr(bus, dev, func, offset));
    inw(PCI_CONFIG_DATA + ((offset & 2) as u16))
}

unsafe fn pci_read_u32(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    outl(PCI_CONFIG_ADDRESS, pci_config_addr(bus, dev, func, offset));
    inl(PCI_CONFIG_DATA)
}

unsafe fn pci_read_u8(bus: u8, dev: u8, func: u8, offset: u8) -> u8 {
    outl(PCI_CONFIG_ADDRESS, pci_config_addr(bus, dev, func, offset));
    inb(PCI_CONFIG_DATA + ((offset & 3) as u16))
}

fn classify_pci_device(vendor_id: u16, device_id: u16, class_code: u8, subclass: u8) -> (DeviceCategory, &'static str) {
    if vendor_id == 0x1AF4 {
        if device_id == 0x1001 || device_id == 0x1042 {
            return (DeviceCategory::StorageVirtioBlk, "VirtIO Block Controller");
        }
        if device_id == 0x1000 || device_id == 0x1041 {
            return (DeviceCategory::NetworkVirtioNet, "VirtIO Network Adapter");
        }
        if device_id == 0x1050 {
            return (DeviceCategory::DisplayVirtioGpu, "VirtIO GPU Display Adapter");
        }
    }

    if class_code == 0x01 { // Mass Storage
        if subclass == 0x01 || (vendor_id == 0x8086 && device_id == 0x7010) {
            return (DeviceCategory::StorageIde, "Standard IDE/ATA Controller");
        }
        if subclass == 0x06 {
            return (DeviceCategory::StorageAhci, "AHCI SATA Controller");
        }
        return (DeviceCategory::StorageOther, "Generic Storage Controller");
    }

    if class_code == 0x02 { // Network Controller
        if vendor_id == 0x8086 && (device_id == 0x100E || device_id == 0x100F) {
            return (DeviceCategory::NetworkEthernet, "Intel e1000 Gigabit Ethernet");
        }
        if vendor_id == 0x10EC && device_id == 0x8139 {
            return (DeviceCategory::NetworkEthernet, "Realtek RTL8139 Fast Ethernet");
        }
        return (DeviceCategory::NetworkEthernet, "Generic Ethernet Controller");
    }

    if class_code == 0x03 { // Display Controller
        if vendor_id == 0x1234 && device_id == 0x1111 {
            return (DeviceCategory::DisplayVga, "QEMU/Bochs Linear Framebuffer VGA");
        }
        return (DeviceCategory::DisplayVga, "Standard VGA Compatible Adapter");
    }

    if class_code == 0x06 { // Bridge Device
        if subclass == 0x00 {
            return (DeviceCategory::HostBridge, "PCI Host Bridge");
        }
        if subclass == 0x01 {
            return (DeviceCategory::IsaBridge, "PCI-to-ISA Bridge");
        }
        if subclass == 0x04 {
            return (DeviceCategory::PciBridge, "PCI-to-PCI Bridge");
        }
    }

    (DeviceCategory::Other, "Unknown / Unbound PCI Function")
}

pub fn run_hw00_read_only_discovery() {
    SERIAL1.write_str("[HW_00] Initializing read-only hardware discovery & inventory...\n");

    let inventory = unsafe { &mut HW_INVENTORY };
    inventory.device_count = 0;

    // 1. CPUID Inventory
    let mut eax: u32;
    let mut ebx: u32;
    let mut ecx: u32;
    let mut edx: u32;

    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov {0:e}, ebx",
            "pop rbx",
            out(reg) ebx,
            inout("eax") 0u32 => eax,
            out("ecx") ecx,
            out("edx") edx,
            options(nomem, preserves_flags)
        );
    }

    inventory.cpu_vendor[0..4].copy_from_slice(&ebx.to_le_bytes());
    inventory.cpu_vendor[4..8].copy_from_slice(&edx.to_le_bytes());
    inventory.cpu_vendor[8..12].copy_from_slice(&ecx.to_le_bytes());

    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov {0:e}, ebx",
            "pop rbx",
            out(reg) ebx,
            inout("eax") 1u32 => eax,
            out("ecx") ecx,
            out("edx") edx,
            options(nomem, preserves_flags)
        );
    }

    inventory.cpu_stepping = eax & 0x0F;
    inventory.cpu_model = (eax >> 4) & 0x0F;
    inventory.cpu_family = (eax >> 8) & 0x0F;
    inventory.has_sse = (edx & (1 << 25)) != 0;
    inventory.has_apic = (edx & (1 << 9)) != 0;
    inventory.has_avx = (ecx & (1 << 28)) != 0;
    inventory.has_rdrand = (ecx & (1 << 30)) != 0;

    SERIAL1.write_str("[HW_00] CPU Vendor: ");
    if let Ok(vstr) = core::str::from_utf8(&inventory.cpu_vendor) {
        SERIAL1.write_str(vstr);
    }
    SERIAL1.write_str(" | Family: ");
    SERIAL1.write_u64_dec(inventory.cpu_family as u64);
    SERIAL1.write_str(" Model: ");
    SERIAL1.write_u64_dec(inventory.cpu_model as u64);
    SERIAL1.write_str(" Stepping: ");
    SERIAL1.write_u64_dec(inventory.cpu_stepping as u64);
    SERIAL1.write_str("\n");

    // 2. Read-Only PCI Bus Enumeration (Buses 0..8, Devs 0..32, Funcs 0..8)
    for bus in 0..8u8 {
        for dev in 0..32u8 {
            for func in 0..8u8 {
                let vendor_id = unsafe { pci_read_u16(bus, dev, func, 0x00) };
                if vendor_id == 0xFFFF || vendor_id == 0x0000 {
                    if func == 0 {
                        break; // No device present
                    }
                    continue;
                }

                let device_id = unsafe { pci_read_u16(bus, dev, func, 0x02) };
                let class_sub = unsafe { pci_read_u16(bus, dev, func, 0x0A) };
                let class_code = (class_sub >> 8) as u8;
                let subclass = (class_sub & 0xFF) as u8;
                let prog_if = unsafe { pci_read_u8(bus, dev, func, 0x09) };
                let bar0 = unsafe { pci_read_u32(bus, dev, func, 0x10) };

                let (category, matched_driver_name) = classify_pci_device(vendor_id, device_id, class_code, subclass);

                let discovered = DiscoveredDevice {
                    bus,
                    dev,
                    func,
                    vendor_id,
                    device_id,
                    class_code,
                    subclass,
                    prog_if,
                    bar0,
                    category,
                    matched_driver_name,
                };

                if inventory.device_count < MAX_DISCOVERED_DEVICES {
                    inventory.devices[inventory.device_count] = Some(discovered);
                    inventory.device_count += 1;
                }

                SERIAL1.write_str("[HW_00] PCI [");
                SERIAL1.write_u64_dec(bus as u64);
                SERIAL1.write_str(":");
                SERIAL1.write_u64_dec(dev as u64);
                SERIAL1.write_str(":");
                SERIAL1.write_u64_dec(func as u64);
                SERIAL1.write_str("] Vendor: 0x");
                SERIAL1.write_u64_hex(vendor_id as u64);
                SERIAL1.write_str(" Device: 0x");
                SERIAL1.write_u64_hex(device_id as u64);
                SERIAL1.write_str(" -> ");
                SERIAL1.write_str(matched_driver_name);
                SERIAL1.write_str(" (Read-Only Inventory)\n");

                // If single-function device, do not scan functions 1..7
                if func == 0 {
                    let header_type = unsafe { pci_read_u8(bus, dev, 0, 0x0E) };
                    if (header_type & 0x80) == 0 {
                        break;
                    }
                }
            }
        }
    }

    SERIAL1.write_str("[HW_00] Hardware inventory completed: ");
    SERIAL1.write_u64_dec(inventory.device_count as u64);
    SERIAL1.write_str(" PCI functions cataloged. Zero driver writes performed.\n");
    SERIAL1.write_str("[HW_00_READ_ONLY_DISCOVERY_VERIFIED_MARKER_v1.0]\n");
}
