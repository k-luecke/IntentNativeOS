// kernel/src/memory/boot_info.rs - UEFI Memory Map Capture & Region Normalization (K003)

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EfiMemoryDescriptor {
    pub type_: u32,
    pub pad: u32,
    pub physical_start: u64,
    pub virtual_start: u64,
    pub number_of_pages: u64,
    pub attribute: u64,
}

pub const EFI_RESERVED_MEMORY_TYPE: u32 = 0;
pub const EFI_LOADER_CODE: u32 = 1;
pub const EFI_LOADER_DATA: u32 = 2;
pub const EFI_BOOT_SERVICES_CODE: u32 = 3;
pub const EFI_BOOT_SERVICES_DATA: u32 = 4;
pub const EFI_RUNTIME_SERVICES_CODE: u32 = 5;
pub const EFI_RUNTIME_SERVICES_DATA: u32 = 6;
pub const EFI_CONVENTIONAL_MEMORY: u32 = 7;
pub const EFI_UNUSABLE_MEMORY: u32 = 8;
pub const EFI_ACPI_RECLAIM_MEMORY: u32 = 9;
pub const EFI_ACPI_MEMORY_NVS: u32 = 10;
pub const EFI_MEMORY_MAPPED_IO: u32 = 11;
pub const EFI_MEMORY_MAPPED_IO_PORT_SPACE: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedRegionType {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    KernelCodeAndData,
    BootServicesReclaimable,
    Mmio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRegion {
    pub start_addr: u64,
    pub size_bytes: u64,
    pub region_type: NormalizedRegionType,
}

pub const PAGE_SIZE: u64 = 4096;
pub const MAX_REGIONS: usize = 128;

pub struct NormalizedMemoryMap {
    pub regions: [MemoryRegion; MAX_REGIONS],
    pub count: usize,
    pub total_usable_bytes: u64,
    pub total_reserved_bytes: u64,
}

impl NormalizedMemoryMap {
    pub const fn empty() -> Self {
        const EMPTY_REGION: MemoryRegion = MemoryRegion {
            start_addr: 0,
            size_bytes: 0,
            region_type: NormalizedRegionType::Reserved,
        };
        Self {
            regions: [EMPTY_REGION; MAX_REGIONS],
            count: 0,
            total_usable_bytes: 0,
            total_reserved_bytes: 0,
        }
    }
}

pub fn normalize_efi_type(efi_type: u32) -> NormalizedRegionType {
    match efi_type {
        EFI_CONVENTIONAL_MEMORY => NormalizedRegionType::Usable,
        EFI_LOADER_CODE | EFI_LOADER_DATA => NormalizedRegionType::KernelCodeAndData,
        EFI_BOOT_SERVICES_CODE | EFI_BOOT_SERVICES_DATA => NormalizedRegionType::BootServicesReclaimable,
        EFI_ACPI_RECLAIM_MEMORY => NormalizedRegionType::AcpiReclaimable,
        EFI_ACPI_MEMORY_NVS => NormalizedRegionType::AcpiNvs,
        EFI_MEMORY_MAPPED_IO | EFI_MEMORY_MAPPED_IO_PORT_SPACE => NormalizedRegionType::Mmio,
        _ => NormalizedRegionType::Reserved,
    }
}

pub fn normalize_memory_map(
    raw_descriptors: &[EfiMemoryDescriptor],
) -> NormalizedMemoryMap {
    let mut map = NormalizedMemoryMap::empty();

    for desc in raw_descriptors {
        if map.count >= MAX_REGIONS {
            break;
        }

        let region_type = normalize_efi_type(desc.type_);
        let start_addr = desc.physical_start;
        let size_bytes = desc.number_of_pages * PAGE_SIZE;

        if size_bytes == 0 {
            continue;
        }

        // Try merging with previous region if adjacent and same type
        if map.count > 0 {
            let prev = &mut map.regions[map.count - 1];
            if prev.region_type == region_type && (prev.start_addr + prev.size_bytes == start_addr) {
                prev.size_bytes += size_bytes;
                match region_type {
                    NormalizedRegionType::Usable | NormalizedRegionType::BootServicesReclaimable => {
                        map.total_usable_bytes += size_bytes;
                    }
                    _ => {
                        map.total_reserved_bytes += size_bytes;
                    }
                }
                continue;
            }
        }

        map.regions[map.count] = MemoryRegion {
            start_addr,
            size_bytes,
            region_type,
        };
        map.count += 1;

        match region_type {
            NormalizedRegionType::Usable | NormalizedRegionType::BootServicesReclaimable => {
                map.total_usable_bytes += size_bytes;
            }
            _ => {
                map.total_reserved_bytes += size_bytes;
            }
        }
    }

    map
}
