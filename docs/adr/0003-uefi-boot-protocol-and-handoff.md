# ADR 0003: UEFI Boot Protocol, Loader Architecture, and Kernel Handoff Specification

## Status
Accepted

## Context
Task `K001` (Gate G2: Custom bootable kernel and user-space foundation) requires defining a documented boot protocol and loader strategy for the Intent-Native Operating System (INOS) targeting x86-64 QEMU/UEFI virtual hardware and eventual bare metal.

The boot sequence must establish a clean, verified, deterministic transition from UEFI firmware into 64-bit long mode kernel code without relying on legacy BIOS or multi-stage real-mode assembly.

## Decision

### 1. Boot Mechanism & Target
- The bootloader is built as a pure UEFI application (`x86_64-unknown-uefi`) located at `/EFI/BOOT/BOOTX64.EFI` on a standard FAT32 EFI System Partition (ESP).
- The kernel is built as an ELF64 binary targeting `x86_64-unknown-none` with `#![no_std]` and `#![no_main]`.
- QEMU executes standard OVMF UEFI firmware (`tools/firmware/OVMF_CODE.fd`).

### 2. Bootloader Responsibilities
1. **Initialize Console & Serial Output:** Configure early 16550 UART (COM1 port `0x3F8`) and UEFI text output for deterministic diagnostic logging from the first instruction.
2. **Locate & Load Kernel:** Read kernel binary into allocated physical memory pages.
3. **Acquire System Descriptors:**
   - Locate ACPI 2.0 RSDP (Root System Description Pointer) via UEFI Configuration Tables.
   - Query Graphics Output Protocol (GOP) for physical framebuffer base, resolution, and scanline stride.
4. **Acquire UEFI Memory Map:** Obtain system memory map descriptor table via `GetMemoryMap`.
5. **Exit Boot Services:** Invoke `ExitBootServices` to terminate UEFI firmware runtime and claim exclusive hardware ownership.
6. **Kernel Handoff Structure (`BootInfo`):** Pass a C-compatible structured pointer in `%rdi`:
   ```rust
   #[repr(C)]
   pub struct BootInfo {
       pub magic: u64,                     // 0x494E4F535F424F4F ("INOS_BOO")
       pub version: u32,                   // 1
       pub memory_map_addr: u64,
       pub memory_map_entries: u64,
       pub memory_map_desc_size: u64,
       pub memory_map_desc_version: u32,
       pub framebuffer_addr: u64,
       pub framebuffer_size: u64,
       pub framebuffer_width: u32,
       pub framebuffer_height: u32,
       pub framebuffer_stride: u32,
       pub rsdp_addr: u64,
       pub command_line_addr: u64,
   }
   ```
7. **Jump to Kernel Entry:** Transition instruction pointer to the kernel's 64-bit entry point with a clean 16-byte aligned kernel stack.

### 3. Memory Region Normalization
The UEFI memory map descriptors are normalized into standard INOS region types:
- `Usable`: Free RAM available to the physical frame allocator (`K007`).
- `Reserved`: Unusable hardware/firmware regions.
- `AcpiReclaimable`: ACPI tables that may be repurposed after table parsing.
- `AcpiNvs`: Non-volatile ACPI storage.
- `KernelCodeAndData`: Memory occupied by the kernel image itself.
- `BootloaderReclaimable`: Memory occupied by bootloader structures reclaimable after page table init.
- `Mmio`: Memory-mapped I/O devices (e.g. LAPIC, IOAPIC, VirtIO bars).

## Consequences
- **Positive:**
  - Standard, clean long mode entry: No 16-bit real mode or 32-bit protected mode thunks needed.
  - Zero Python tooling: Build scripts use standard `cargo`, `mtools`, and POSIX/PowerShell scripts.
  - Testable in automated CI: QEMU boots `OVMF` directly to serial with timeout traps.
- **Negative:**
  - Requires OVMF firmware image in testing harness (packaged in `tools/firmware/`).
