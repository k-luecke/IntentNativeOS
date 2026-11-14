# L401 — Minimal Native Inference System-Service Requirements & Gap Analysis

**Document ID:** SPEC-L401-KERNEL-INFERENCE-REQ-01  
**Target Gate:** Gate L4 (Kernel Ownership and Native Service Integration)  
**Date:** October 9, 2026  
**Status:** Canonical Kernel Service Specification

---

## 1. Executive Summary

Running neural language model inference inside an operating system kernel or native Ring 3 user process introduces distinct architectural demands that differ markedly from standard desktop utilities:
1. **Memory Scale:** Streaming or mapping 58 MiB to 1.5 GiB of continuous FP32/Q4_0 weights.
2. **Computational Intensity:** Sustained matrix-vector products (GEMV) requiring uninterrupted CPU cycles and SIMD vector registers without triggering kernel watchdogs.
3. **Privilege Boundary:** Inference execution must remain strictly non-privileged (Ring 3), unable to forge capabilities or bypass access controls.

This document inventories the minimal kernel primitives required for full native execution and provides an empirical gap analysis against the current Intent-Native OS kernel.

---

## 2. Minimal Required System Services

### 2.1 Memory Management & Allocation
| Primitive | Purpose | Minimal Specification |
|---|---|---|
| **Virtual Memory Mapping (`sys_mmap` / `sys_brk`)** | Dynamic heap allocation for KV cache and activation buffers | Page-aligned anonymous memory allocations up to 64 MiB |
| **Direct File/Block Streaming** | Loading model weight blobs (`.bin`) without intermediate memory copies | Read-only contiguous sector mapping or DMA directly into user pages |
| **Page Table & NX Protection** | Enforcing W^X (Write XOR Execute) security | Read-only permissions on model weights (`PROT_READ`); non-executable stack/heap |
| **Warm-Reclaim / Unload** | Freeing activation buffers and weight pages upon intent resolution | Explicit unmap syscall (`sys_munmap`) returning physical frames to kernel pool |

### 2.2 Execution & Scheduling
| Primitive | Purpose | Minimal Specification |
|---|---|---|
| **Preemptive Time-Slicing** | Preventing autoregressive decoding loops from starving OS input/UI | APIC timer interrupts yielding control every 10–20 ms |
| **FPU / SIMD State Preservation** | Preserving AVX2 / SSE / NEON registers across context switches | `xsave` / `xrstor` or `fxsave` / `fxrstor` in task switch interrupt handler |
| **Cancellation & Bounds Checking** | Halting run-away generation loops exceeding token budgets | Per-process generation deadlines and monotonic cycle counter queries |

### 2.3 Inter-Process Communication (IPC) & Storage
| Primitive | Purpose | Minimal Specification |
|---|---|---|
| **Typed Message Pipes / Spool** | Handoff between UI, Compiler, and Inference | Bounded ring-buffer IPC or reserved disk spool sectors (LBA 24,576+) |
| **Capability Verification Gate** | Validating that model plans pass the deterministic verifier | Kernel-enforced capability tokens with cryptographic/monotonic nonce |

---

## 3. Kernel Gap Analysis: Current vs. Required

| Subsystem | Current Kernel Status (Gate G12 Baseline) | Target Native Requirement | Gap Severity |
|---|---|---|---|
| **Ring 3 Entry** | Working (TSS, GDT, SYSRET/IRET, CPL=3 verified) | Ring 3 user execution with isolated address spaces | **CLOSED** |
| **Basic Syscalls** | Working (`EXIT`, `GET_VERSION`, `FILE_CREATE`, `FILE_WRITE`, `DEBUG_PRINT`) | Extended syscall table (`MMAP`, `YIELD`, `IPC_RECV`, `IPC_SEND`) | **MODERATE** |
| **FPU / SIMD State** | Basic SSE enabled; context switch does not save 256-bit AVX2 state | `xsave`/`xrstor` in scheduler task switch | **MODERATE** |
| **Heap Allocator** | Freestanding bump allocator in Ring 3 (128 KiB young heap) | Dynamic slab/buddy allocator for multi-megabyte weight storage | **HIGH** |
| **Large File I/O** | Single-sector PIO read/write via IDE/ATA controller | Multi-megabyte streaming or DMA sector reads for model weights | **HIGH** |

---

## 4. Architectural Mitigation: Two-Tier Development Lane

To ensure immediate functional progress while kernel memory subsystems mature:
1. **Hosted Bridge Baseline (Task L402):** A secure, authenticated loopback IPC bridge connecting the native OCaml Ring 3 control plane to the optimized C inference runner (`models/inference/quant_engine`).
2. **Freestanding Native Ring 3 Runner (Tasks L403–L405):** A compact embedded transformer runner compiled with `-nostdlib` running directly in Ring 3 under QEMU, operating on embedded weight buffers within available guest RAM.
