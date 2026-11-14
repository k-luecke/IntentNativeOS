# P002: Reference Virtual Hardware Target Specification (QEMU x86-64)

**Task ID:** P002  
**Gate:** G0 (Foundational Controls)  
**Status:** Approved Specification  
**Reference Machine Budget:** 16 GiB System RAM, 4 vCPUs  

---

## 1. Virtual Machine Overview

The primary hardware target for the Intent-Native OS kernel and bare-metal testing is an x86-64 virtual platform orchestrated via QEMU (`qemu-system-x86_64`) with standardized VirtIO devices.

This target provides a deterministic, reproducible, hardware-assisted virtualization environment suitable for automated CI verification and developer testing.

---

## 2. Hardware Subsystem Specifications

### 2.1 Firmware & Boot
- **Firmware:** UEFI 64-bit via standard Open Virtual Machine Firmware (OVMF).
- **OVMF Image:** `OVMF_CODE.fd` (read-only code) + `OVMF_VARS.fd` (writable NVRAM variables).
- **Boot Protocol:** Unified Extensible Firmware Interface (UEFI) specification v2.8+.
- **Boot Entry:** EFI application (`\EFI\BOOT\BOOTX64.EFI`) locating the kernel image on the ESP (EFI System Partition, FAT32).

### 2.2 CPU & Core Topology
- **Architecture:** x86-64 (AMD64 / Intel 64).
- **QEMU CPU Flag:** `-cpu qemu64,+smap,+smep,+rdrand,+rdseed` (portable) or `-cpu host` (local hardware acceleration with KVM or Windows Hypervisor Platform `whpx`).
- **vCPU Count:** 4 cores standard.
- **Security Extensions Required:**
  - Supervisor Mode Execution Prevention (SMEP).
  - Supervisor Mode Access Prevention (SMAP).
  - No-Execute / Execute-Disable (NX / XD bit).
  - Hardware Random Number Generator (`RDRAND` / `RDSEED`).

### 2.3 Physical Memory Budget & Layout
- **Total Reference RAM:** 16 GiB (`-m 16G`).
- **Memory Partitions:**
  - **T0/T1 Kernel & Broker Reserve:** 256 MiB fixed protected physical RAM.
  - **T2 OCaml Control Plane & Semantic Store:** 1.75 GiB reserved ceiling.
  - **T3 Model Pool (A & B):** 10.0 GiB dynamic allocation (quantized 4-bit / 8-bit weights).
  - **T3 User Application & Capsule Space:** 4.0 GiB elastic memory with host reclamation priority.
- **Physical Paging:** 4-level paging (CR3) with 4 KiB small pages and 2 MiB large pages. 5-level paging disabled for simplicity.

### 2.4 Device & Bus Architecture

| Subsystem | QEMU Emulated Device | Bus / Transport | Driver Requirement |
| :--- | :--- | :--- | :--- |
| **Serial Console** | 16550A UART (`-serial stdio`) | ISA (I/O Port `0x3F8`, IRQ 4) | Early boot debug output and deterministic log capture. |
| **Storage (System)** | VirtIO Block (`virtio-blk-pci`) | PCI Express | Block device for OS root, EFI partition, and append journal. |
| **Storage (Staging)** | VirtIO Block (`virtio-blk-pci`) | PCI Express | Copy-on-Write (CoW) disk image for reversible file operations. |
| **Networking** | VirtIO Network (`virtio-net-pci`) | PCI Express | Default-deny; bridged or user-mode SLIRP (`-netdev user,id=net0`). |
| **Display** | Bochs / Standard VGA or VirtIO-GPU | PCI Express | Linear 32-bit framebuffer (`-vga std` or `-device virtio-vga`). |
| **Timer / Interrupts** | Local APIC & IO-APIC | System Bus | TSC (Time Stamp Counter) + APIC timer for preemptive multitasking. |
| **Input** | VirtIO Tablet / PS/2 Controller | USB / ISA | Deterministic mouse and keyboard events. |

---

## 3. Canonical QEMU Invocation Line

```bash
qemu-system-x86_64 \
  -nodefaults \
  -machine q35,accel=kvm:whpx:tcg \
  -cpu qemu64,+smap,+smep,+rdrand \
  -smp 4 \
  -m 16G \
  -drive if=pflash,format=raw,readonly=on,file=OVMF_CODE.fd \
  -drive if=pflash,format=raw,file=OVMF_VARS.fd \
  -drive file=target/esp.img,format=raw,if=virtio,id=drive0 \
  -serial stdio \
  -display none \
  -device virtio-net-pci,netdev=net0 \
  -netdev user,id=net0,restrict=on
```

---

## 4. Verification Check

- [x] All 7 subsystem categories defined (Firmware, CPU, Memory, Storage, Network, Console, Display).
- [x] Conforms to 16 GiB reference budget.
- [x] Explicit serial console port specified for non-interactive automated test execution.
