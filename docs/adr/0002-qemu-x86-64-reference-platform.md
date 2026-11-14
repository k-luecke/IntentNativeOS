# ADR 0002: Reference Hardware Platform: QEMU x86-64 with VirtIO

**Status:** Accepted  
**Date:** 2026-10-09  
**Decision Owner:** Platform Lead  
**Technical Stakeholders:** Kernel Platform, Hypervisor, CI  
**Supersedes:** None  

---

## 1. Context and Problem Statement
Developing a custom operating system requires a standardized, fully reproducible hardware target before deploying to bare-metal silicon. The hardware environment must provide standardized virtual devices, support automated CI execution without physical intervention, and run efficiently within a 16 GiB reference budget.

---

## 2. Alternatives Considered

### Option A: Direct Bare-Metal Prototype
- **Pros:** Real hardware quirks identified early.
- **Cons:** Slow reboot cycles, manual provisioning, breaks automated headless CI, risk of device-specific driver traps.

### Option B: QEMU x86-64 Virt Target with UEFI/OVMF and VirtIO (Selected)
- **Pros:**
  - Standardized Open Virtual Machine Firmware (OVMF) UEFI environment.
  - Standardized VirtIO bus devices (VirtIO-Blk, VirtIO-Net, VirtIO-GPU) with well-documented, lean drivers.
  - Headless serial logging via 16550A UART enables automated CI assert verification.
  - Hardware acceleration support on developer hosts via KVM (Linux) and WHPX (Windows).
- **Cons:** Emulation overhead if running without hardware acceleration.

---

## 3. Decision Outcome & Rationale
**Selected: Option B.**
QEMU x86-64 with UEFI/OVMF and VirtIO is frozen as the normative reference virtual target for Gate G0 through G6.
All kernel smoke tests must boot and emit validation tokens over COM1 serial output.
