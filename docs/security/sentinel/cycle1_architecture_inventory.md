# SDU-01: Architecture Inventory & Security Boundary Audit (Task S001)

**Task ID:** S001  
**Phase:** Phase 0 (Architecture and Standards)  
**Classification:** Technical Architecture Audit  
**Status:** Complete  
**Date:** 2026-10-11  
**Author:** SDU-01 Architect  

---

## 1. Executive Summary

This architecture inventory maps the security controls, capabilities, networking infrastructure, browser/capsule isolation mechanisms, and cryptographic dependencies within the Intent-Native Operating System (`IntentNativeOS`).

In accordance with SDU-01 Execution Requirements, all capabilities are evaluated against actual repository source code and categorized into four strict integrity states:
1. **Existing:** Fully implemented and validated by unit or integration tests in tree.
2. **Partial:** Partially implemented, stubbed, or restricted in scope.
3. **Missing:** Designed in specifications but lacking codebase implementation.
4. **Unverified:** Implemented in code but lacking automated, reproducible test evidence.

---

## 2. Capability System & Privilege Management

### 2.1 Kernel Capability System (`kernel/src/capability/mod.rs`)
- **Object Handle Table & Rights (`Existing`):**
  - Bitmask rights: `READ (0x1)`, `WRITE (0x2)`, `EXECUTE (0x4)`, `DELEGATE (0x8)`, `REVOKE (0x10)`, `CREATE (0x20)`, `IPC (0x40)`, `SCHEDULE (0x80)`, `MIGRATE_ADMIN (0x100)`.
  - Object lifecycle tracking with reference counting (`KernelObject`, `ref_count`, `is_alive`).
  - Strict downward attenuation: `delegate()` rejects any child request containing rights not held by parent (`CapError::CannotAttenuateUpward`).
- **Recursive Revocation Cascade (`Existing`):**
  - `revoke(handle_id, caller_pid)` traverses descendant capability handle tree, invalidating child handles and decrementing object reference counts.
  - Generational epoch invalidation (`advance_epoch()`) invalidates all previous epoch grants.
- **Scoped 128-bit Capability Tokens (`Existing`):**
  - `ScopedCapabilityToken` binds `token_id (u128)`, `owner_pid`, `owner_generation`, `path [u8; 32]`, `rights`, `epoch`, and `transaction_bound`.
  - Constant-time verification (`token_id_eq_ct`) mitigates side-channel timing attacks.
  - Process generation counters (`PROCESS_GENERATIONS`) prevent PID-reuse privilege confusion.
- **Transactional Capability Migration (`Existing`):**
  - Atomic migration between package identities (e.g. `org.inos.script` -> `org.inos.intscript`) with rollback capability on interruption (`simulate_interrupted_migration_recovery`).
- **Multicore Concurrency Stress (`Unverified`):**
  - Capability table locks under SMP 4+ concurrent revocation and delegation threads lack automated high-stress verification.

### 2.2 Security Broker & Authorization Plane (`security/broker.ml`, `security/types.ml`)
- **Trust Placement Enforcement (`Existing`):**
  - `can_mint_grants()` in `security/broker.ml` strictly restricts grant minting to `Host_Administrator`.
  - Calls originating from `Untrusted_Model`, `Untrusted_Guest_VM`, or `System_Service` are denied immediately (`A002`).
- **Replay Resistance (`Existing`):**
  - Monotonic grant sequences (`next_grant_seq`), single-use consumption tracking (`consumed_grants`), TTL expiry, and canonical digest verification (`A003`).

---

## 3. Memory & Process Isolation (Capsules & Browser)

### 3.1 Ring Isolation & Paging (`kernel/src/arch/x86_64/paging.rs`, `cpu.rs`)
- **Address Space Separation (`Existing`):**
  - Dual Ring 3 processes execute at CPL=3 with distinct PML4 page tables (`CR3` switching on context switch).
  - Identical virtual user ranges (e.g., `0x0040_0000`) map to disjoint physical frames.
- **Syscall User-Pointer Verification (`Existing`):**
  - `verify_user_buffer()` in `kernel/src/arch/x86_64/syscall.rs` walks the 4-level x86-64 page table ensuring every user-provided pointer is mapped with the `USER` bit set and not pointing into Ring 0 kernel virtual memory (`>= 0xFFFF_8000_0000_0000`).
- **Fault Containment (`Existing`):**
  - User-space Page Faults (`#PF`) and General Protection Faults (`#GP`) are trapped cleanly; the offending Ring 3 process is terminated (exit code 139) while the kernel and peer processes remain operational.
- **Capsule Sandbox Boundary (`Partial`):**
  - Memory bounds and file access are mediated, but hardware-accelerated virtualization extensions (Intel VT-x / AMD-V) for guest VMs remain stubbed or reliant on hosted emulation.

---

## 4. Network Security & Firewall

### 4.1 Kernel Networking (`kernel/src/net/`)
- **Socket Abstraction & TCP/IP (`Existing`):**
  - Native TCP/UDP stack with virtio-net device driver (`kernel/src/net/socket.rs`, `tcp.rs`).
- **Stateful Packet Inspection (SPI) Firewall (`Partial`):**
  - Basic frame filtering exists, but a fully dynamic conntrack table and programmable stateful rule evaluation engine are not yet unified in the default packet path.
- **Per-Process Egress Policy (`Partial`):**
  - Syscall gate checks general socket creation capability, but granular per-IP/per-port egress policy enforcement is incomplete.
- **DNS Security (`Partial`):**
  - Native DNS resolver exists (`kernel/src/net/dns.rs`), but DNSSEC signature verification and DNS-over-HTTPS (DoH) encrypted transport are stubbed.

---

## 5. Cryptography & Key Management

### 5.1 Kernel & Control Plane Crypto (`kernel/src/crypto/`, `security/`)
- **Entropy & Randomness (`Existing`):**
  - Hardware RDRAND / RDSEED extraction with software entropy pool fallback (`kernel/src/crypto/entropy.rs`).
- **Pure OCaml SHA-256 (`Existing`):**
  - Clean implementation without foreign C/Python dependencies (`security/sha256.ml`).
- **Post-Quantum Registry (`Existing`):**
  - `security/post_quantum.ml` and `security/crypto_registry.ml` implement KAT verification for ML-KEM-768 (FIPS 203) and ML-DSA-65 (FIPS 204).
  - Strict policy gate: `Post_Quantum_Strict` fails closed against classical algorithms.
- **Kernel-Space Post-Quantum Verification (`Missing`):**
  - Post-quantum signature verification is currently hosted in the OCaml control plane; the Ring 0 kernel uses classical Ed25519 and SHA-256.
- **Hardware Root of Trust (TPM 2.0) (`Missing`):**
  - Measured boot PCR extension is emulated in `security/update.ml` (`A011`), but physical TPM 2.0 driver integration in `kernel/src/drivers/tpm.rs` is not yet present.

---

## 6. Architecture Status Summary Table

| Subsystem Component | Implementation File(s) | Status | Test Harness / Evidence |
| :--- | :--- | :---: | :--- |
| **Capability Lifecycle** | `kernel/src/capability/mod.rs` | **Existing** | `scripts/test_app_cap_01.sh` |
| **Token Constant-Time Eq** | `kernel/src/capability/mod.rs:301` | **Existing** | `kernel/src/capability/mod.rs` |
| **Syscall Deny-by-Default** | `kernel/src/arch/x86_64/syscall.rs` | **Existing** | `tests/test_adversarial_boundaries.ml` |
| **Page Table User Walk** | `kernel/src/arch/x86_64/syscall.rs:188` | **Existing** | `scripts/test_app_cap_01.sh` |
| **Broker Trust Placement** | `security/broker.ml` | **Existing** | `tests/test_security.ml` (A002) |
| **Replay Resistant Grants** | `security/broker.ml` | **Existing** | `tests/test_security.ml` (A003) |
| **Step-Up Authentication** | `security/assertion.ml` | **Existing** | `tests/test_security.ml` (A005, A006) |
| **Post-Quantum Registry** | `security/post_quantum.ml` | **Existing** | `tests/test_security.ml` (A008, A009) |
| **Append-Only Audit Log** | `security/audit.ml` | **Existing** | `tests/test_security.ml` (A008) |
| **Stateful Packet Firewall** | `kernel/src/net/mod.rs` | **Partial** | Basic socket filter tests |
| **Per-Process Egress Policy**| `kernel/src/net/policy.rs` | **Partial** | Syscall gate checks only |
| **DNSSEC / DoH Validation** | `kernel/src/net/dns.rs` | **Partial** | Plain UDP DNS tests |
| **Kernel Ring 0 PQ Engine** | `kernel/src/crypto/` | **Missing** | Control plane only |
| **Hardware TPM 2.0 Driver** | `kernel/src/drivers/tpm.rs` | **Missing** | Software measured boot only |
| **Multicore Lock Stress** | `kernel/src/capability/mod.rs` | **Unverified**| Needs SMP 4+ harness |
