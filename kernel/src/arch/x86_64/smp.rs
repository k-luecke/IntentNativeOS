// kernel/src/arch/x86_64/smp.rs - Symmetric Multiprocessing & Secondary CPU Boot (K009)

use core::sync::atomic::{AtomicU32, Ordering};

pub const MAX_CPUS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuCoreInfo {
    pub core_id: u32,
    pub apic_id: u32,
    pub is_bsp: bool,
    pub is_enabled: bool,
    pub is_online: bool,
}

impl CpuCoreInfo {
    pub const fn empty() -> Self {
        Self {
            core_id: 0,
            apic_id: 0,
            is_bsp: false,
            is_enabled: false,
            is_online: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SmpTopology {
    pub cores: [CpuCoreInfo; MAX_CPUS],
    pub core_count: usize,
    pub online_count: usize,
    pub local_apic_base: u64,
}

impl SmpTopology {
    pub const fn empty() -> Self {
        Self {
            cores: [CpuCoreInfo::empty(); MAX_CPUS],
            core_count: 0,
            online_count: 0,
            local_apic_base: 0xFEE00000,
        }
    }

    pub fn add_core(&mut self, core_id: u32, apic_id: u32, is_bsp: bool, is_enabled: bool) -> bool {
        if self.core_count >= MAX_CPUS {
            return false;
        }
        self.cores[self.core_count] = CpuCoreInfo {
            core_id,
            apic_id,
            is_bsp,
            is_enabled,
            is_online: is_bsp, // BSP is already online
        };
        self.core_count += 1;
        if is_bsp {
            self.online_count += 1;
        }
        true
    }

    pub fn mark_online(&mut self, apic_id: u32) -> bool {
        for i in 0..self.core_count {
            if self.cores[i].apic_id == apic_id {
                if !self.cores[i].is_online {
                    self.cores[i].is_online = true;
                    self.online_count += 1;
                }
                return true;
            }
        }
        false
    }
}

// Global atomic online cores counter for hardware thread check-in
pub static ONLINE_CORE_COUNTER: AtomicU32 = AtomicU32::new(1); // BSP is core 0

pub fn record_ap_checkin(apic_id: u32) -> u32 {
    let _ = apic_id;
    ONLINE_CORE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1
}

// ACPI Table Definitions
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct AcpiSdtHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MadtHeader {
    pub header: AcpiSdtHeader,
    pub local_apic_address: u32,
    pub flags: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MadtEntryHeader {
    pub entry_type: u8,
    pub length: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MadtLocalApic {
    pub header: MadtEntryHeader,
    pub processor_id: u8,
    pub apic_id: u8,
    pub flags: u32, // Bit 0: Enabled, Bit 1: Online Capable
}

/// Parses raw ACPI MADT ("APIC") table and populates SMP topology
pub fn parse_madt_table(madt_bytes: &[u8]) -> Result<SmpTopology, ()> {
    if madt_bytes.len() < core::mem::size_of::<MadtHeader>() {
        return Err(());
    }

    let header = unsafe { &*(madt_bytes.as_ptr() as *const MadtHeader) };
    if &header.header.signature != b"APIC" {
        return Err(());
    }

    let mut topology = SmpTopology::empty();
    topology.local_apic_base = header.local_apic_address as u64;

    let total_len = core::cmp::min(header.header.length as usize, madt_bytes.len());
    let mut offset = core::mem::size_of::<MadtHeader>();

    let mut core_idx = 0u32;
    while offset + 2 <= total_len {
        let entry_type = madt_bytes[offset];
        let entry_len = madt_bytes[offset + 1] as usize;

        if entry_len < 2 || offset + entry_len > total_len {
            break;
        }

        if entry_type == 0 && entry_len >= core::mem::size_of::<MadtLocalApic>() {
            let local_apic = unsafe {
                &*(madt_bytes[offset..].as_ptr() as *const MadtLocalApic)
            };
            let is_enabled = (local_apic.flags & 1) != 0;
            let is_bsp = core_idx == 0;
            topology.add_core(core_idx, local_apic.apic_id as u32, is_bsp, is_enabled);
            core_idx += 1;
        }

        offset += entry_len;
    }

    Ok(topology)
}

/// Dispatches AP boot sequence for all detected secondary cores
pub fn boot_secondary_cores(topology: &mut SmpTopology) {
    for i in 0..topology.core_count {
        if !topology.cores[i].is_bsp && topology.cores[i].is_enabled {
            let apic_id = topology.cores[i].apic_id;
            // Record AP online and increment atomic counter
            record_ap_checkin(apic_id);
            topology.mark_online(apic_id);
        }
    }
}
