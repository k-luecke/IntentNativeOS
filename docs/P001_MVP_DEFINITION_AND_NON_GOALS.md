# P001: Intent-Native OS — Acceptance Boundary & Non-Goals Statement

**Task ID:** P001  
**Gate:** G0 (Foundational Controls)  
**Status:** Approved Baseline  
**Authority:** `START_HERE_IMPLEMENTATION_HANDOFF.md` & `intent_native_os_engineering_spec_v0_2.md`  

---

## 1. Scope & System Objective

The Intent-Native Operating System (INOS) is an original, offline-first operating system designed to elevate natural human intent, direct manipulation, and high-level programming into typed, verified, and capability-confinement execution.

To ensure disciplined execution without scope creep, implementation is partitioned into three distinct phases with strict non-goals.

---

## 2. Phase Distinctions

### Phase 1: Hosted Semantic Vertical Slice (Gate G1)
- **Objective:** Validate the core intent compiler, clarification gate, and Model A/B collaboration loop running as a verified control plane hosted on a standard developer OS.
- **In Scope:**
  - OCaml 5.x / Dune control plane implementation.
  - Strict Typed Intent IR: modes (`Explore`, `Design`, `Simulate`, `Execute`, `Maintain`), origins, bindings, and material ambiguity tracking.
  - Clarification State Machine: deterministic transition gating without silent escalation.
  - Model A/B Adapter Protocol: structured, content-addressed handoff envelopes with immutable receipts.
  - Reversible execution engine: staging, journaling, and atomic rollback for local file operations.
  - Complete operation when both local models are unloaded.
- **Out of Scope (Deferred to Phase 2):** Bootloader, physical page tables, bare-metal QEMU execution, hardware interrupts.

### Phase 2: Bootable Custom-OS MVP (Gates G2–G6)
- **Objective:** Deploy an original, minimal protected kernel on x86-64 hardware virtualization (QEMU / UEFI) that establishes memory isolation, capability-based IPC, and executes the OCaml control plane.
- **In Scope:**
  - UEFI (OVMF) x86-64 bootloader in Rust/assembly.
  - Microkernel / protected supervisor: physical page allocator, paging, descriptor tables (GDT/IDT), APIC/timer interrupts.
  - Ring-based / capability-isolated address spaces separating T0 (Kernel), T1 (Capability Broker), T2 (OCaml Control Plane), and T3 (User Capsules & Untrusted Models).
  - Synchronous / asynchronous capability IPC message passing over versioned wire envelopes.
  - VirtIO-Block, VirtIO-Net, and 16550A serial console drivers.
  - Offline model residency management within a strict 16 GiB reference memory envelope.

### Phase 3: Future Consumer Release (Non-Goals for G0–G6)
- **Status:** Explicit Non-Goals. Must not delay or complicate Phase 1 or Phase 2.
- **Non-Goals:**
  - Full Windows binary driver compatibility or arbitrary Win32 API re-implementation.
  - Broad consumer graphics/GPU virtualization matrix (NVIDIA/AMD bare-metal hypervisor passthrough).
  - Commercial Windows guest licensing automation or bypass mechanisms.
  - Polished consumer desktop distribution / app store.
  - Cloud-dependent inference, remote telemetry, or mandatory SaaS bridges.
  - Experimental third model (Model C) or novel unverified zero-knowledge proof cryptography.

---

## 3. Irreversible Architectural Invariants

1. **Non-Erosion Rule:** Natural language is a high-level programming interface, never a conversational chat agent.
2. **Clarification Gate:** Destructive or consequential actions MUST halt at the clarification gate before execution.
3. **No Model Self-Authorization:** Local models (A and B) are untrusted workers (T3). They cannot issue capability grants or approve their own plans.
4. **Offline Autonomy:** The core OS, file access, recovery, and precompiled tools MUST remain 100% operational when both models are shut down or unloaded.
5. **No Python Runtime:** Python is strictly excluded from the build system, kernel, IPC, and test harnesses.

---

## 4. Acceptance Criteria & Sign-off

- [x] Clear demarcation between Hosted Vertical Slice (G1), Custom-OS MVP (G2-G6), and Consumer Non-Goals.
- [x] Hardware baseline pinned to x86-64 UEFI / QEMU with 16 GiB reference budget.
- [x] Irreversible invariants codified.
