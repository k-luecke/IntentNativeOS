# SDU-01: Normative Standards & Adversarial Threat Model (Task S002)

**Task ID:** S002  
**Phase:** Phase 0 (Architecture and Standards)  
**Classification:** Threat Model & Normative Specification  
**Status:** Complete  
**Date:** 2026-10-11  
**Author:** SDU-01 Architect  
**Predecessor Document:** `docs/P008_THREAT_MODEL_AND_TRUST_BOUNDARIES.md`  

---

## 1. Trust Hierarchy & Boundary Taxonomy

INOS operates under a strict, non-bypassable five-tier trust hierarchy. Privilege decreases monotonically from T0 down to T4. Higher tiers must never delegate execution authority to lower tiers without formal verification and cryptographic mediation.

```text
+========================================================================+
| T0: Trusted Minimal Kernel Core & Hardware Primitives                  |
| - PML4 page tables, GDT/IDT, TSS, CPU ring transitions (CPL=0 vs 3)    |
| - Physical frame allocation, APIC interrupt routing, virtio drivers    |
| - Invariant: Zero dynamic interpreters or AI models in T0.             |
+========================================================================+
                                   ^
                                   | Syscall ABI Gate (verify_user_buffer)
+------------------------------------------------------------------------+
| T1: Trusted Capability & Cryptographic Broker                          |
| - 128-bit Scoped Capability Registry, downward attenuation engine       |
| - Hardware authenticator assertion validation (FIDO2 / CTAP2.1)        |
| - Post-quantum key encapsulation (ML-KEM) and signature verification   |
| - Invariant: Replay-resistant single-use grants; fail-closed defaults. |
+------------------------------------------------------------------------+
                                   ^
                                   | Authenticated IPC Envelopes
+------------------------------------------------------------------------+
| T2: OS Control Plane & Intent Runtime                                  |
| - Clarification state machine, intent compiler, append-only journal    |
| - Model A / Model B arbitration pipeline, task graph resolver          |
| - Invariant: Speculative reasoning tagged as Hypothesis; cannot mint.  |
+------------------------------------------------------------------------+
                                   ^
                                   | Sandboxed Process Boundary
+------------------------------------------------------------------------+
| T3: Untrusted Executors, Capsules & Foundation Models                  |
| - Model A (Architect) / Model B (Implementer) local weights            |
| - User applications (notes, calculator, web render), guest capsules    |
| - Invariant: Strictly CPL=3, unmapped kernel memory, zero raw sockets. |
+------------------------------------------------------------------------+
                                   ^
                                   | Ingestion Sanitizer Gate
+------------------------------------------------------------------------+
| T4: External Untrusted Inputs & Adversarial Network Traffic            |
| - Web content, external files, user text prompts, raw network packets  |
| - Invariant: Untrusted by definition; never evaluated as code.         |
+========================================================================+
```

---

## 2. Adversary Taxonomy & Threat Scenarios

### 2.1 Threat Scenario 1: Forged & Tampered Capability Credentials
- **Attacker Goal:** A compromised Ring 3 process (T3) fabricates a 128-bit capability token or modifies handle flags to read arbitrary files (`/sys/config/master.key`) or load kernel code.
- **Enforcement Defense:**
  - Token IDs are 128-bit cryptographically pseudorandom values issued by the kernel registry.
  - Verification uses constant-time comparison (`token_id_eq_ct`) and checks process generation (`PROCESS_GENERATIONS`), preventing handle forgery, brute-force enumeration, and PID reuse attacks.
- **Negative Test Obligation:** Unit test must attempt syscall invocations with random 128-bit values and modified bits; 100% must return `CapError::UnownedCapability` or `UnauthorizedCaller`.

### 2.2 Threat Scenario 2: Privilege Escalation via Upward Delegation
- **Attacker Goal:** A process holding a `READ`-only capability attempts to delegate a `READ | WRITE | EXECUTE` capability to a child or helper process.
- **Enforcement Defense:**
  - `CapabilitySystem::delegate()` strictly enforces monotonic attenuation: `(delegated_rights & !parent.rights) != 0` immediately triggers `CapError::CannotAttenuateUpward`.
- **Negative Test Obligation:** Test must invoke delegation with a bitmask containing `(parent.rights | 0x20)`; must fail closed.

### 2.3 Threat Scenario 3: Revocation Bypass & Stale Handle Invocations
- **Attacker Goal:** After a security revocation event, an attacker continues accessing resources using an already-revoked handle or a child handle derived prior to revocation.
- **Enforcement Defense:**
  - Cascading recursive revocation (`revoke()`) traverses the handle dependency tree and marks all derived handles `is_revoked = true`.
  - Generational epoch invalidation (`advance_epoch()`) invalidates all handles issued in earlier epochs.
- **Negative Test Obligation:** Test must delegate parent -> child -> grandchild, revoke parent, and attempt access through all three handles. All three must return `CapError::CapabilityRevoked`.

### 2.4 Threat Scenario 4: User-Pointer Smuggling into Ring 0 (Confused Deputy)
- **Attacker Goal:** Ring 3 process passes a syscall buffer pointer pointing into kernel virtual address space (`0xFFFF_8000_0000_0000`), tricking the kernel into reading or overwriting kernel memory.
- **Enforcement Defense:**
  - `verify_user_buffer()` traverses the active PML4/PDPT/PD/PT tables. It verifies that every page spanning the buffer is present, user-accessible (`PAGE_USER` bit set), and below the canonical kernel base.
- **Negative Test Obligation:** Test must pass kernel addresses, unmapped pages, and cross-boundary pointers to syscalls; all must fail cleanly with memory access violation errors without triggering a kernel panic.

### 2.5 Threat Scenario 5: Epistemic Confusion (Model Speculation Escalation)
- **Attacker Goal:** An untrusted model outputs reasoning containing "I have verified and authorized this disk format", and the intent runtime treats this statement as an authorized capability grant.
- **Enforcement Defense:**
  - Provenance tagging enforces that model output is typed as `Model_Generated` or `Hypothesis`.
  - Type-system invariants in `security/broker.ml` prevent any non-`Host_Administrator` role from minting authorization tokens.
- **Negative Test Obligation:** Test passes model-generated intent with simulated approval markers to the broker; must result in `Broker.Denied`.

---

## 3. Normative Requirements Mapping Matrix

| Threat Category | Normative Reference | Security Invariant | Mandatory Acceptance Criterion |
| :--- | :--- | :--- | :--- |
| **Credential Forgery** | NIST SP 800-63-4 §5.1 | Cryptographically bound tokens | Zero successful invocations with unissued tokens |
| **Privilege Escalation**| NIST SP 800-207 §2.1 | Monotonic attenuation | Upward attenuation mathematically denied |
| **Revocation Bypass** | NIST SP 800-63-4 §5.3 | Cascading revocation & epoch invalidation | Descendant handles invalidated synchronously |
| **Memory Corruption** | NIST SP 800-218 §PW.5 | Page table verified user buffers | Kernel memory boundary inviolable by Ring 3 |
| **Replay Attacks** | NIST SP 800-63-4 §5.2 | Single-use consumption & monotonic sequence | Second presentation of token denied |
| **Model Infiltration** | NIST SP 800-218 §PO.1 | Trust separation & epistemic isolation | Untrusted role cannot mint grants |

---

## 4. Mutation Testing & Negative Control Mandate

To prevent false confidence from vacuous tests, the Sentinel Cycle 1 test harness must execute **mutation tests**:
1. **Mutation 1 (Disable Attenuation Check):** Temporarily weaken monotonic check; verify that the test suite detects this and FAILS.
2. **Mutation 2 (Disable Token Match):** Temporarily accept any non-zero token; verify that forged token test FAILS.
3. **Mutation 3 (Disable Revocation Cascade):** Revoke only the parent handle without traversing children; verify that child handle invocation test FAILS.

No test is certified as passing unless its corresponding negative control has been verified to fail when the security mechanism is removed.
