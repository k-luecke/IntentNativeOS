# SDU-01: Sentinel Autonomous Security Development Unit Charter

**Document ID:** SDU-01-CHARTER  
**Classification:** Core Governance & Safety Boundary  
**Baseline Date:** 2026-10-11  
**Status:** Operational (Cycle 1 Active)  
**Parent System:** Intent-Native Operating System (INOS)  

---

## 1. Mission and Mandate

The **Sentinel Development Unit (SDU-01)** is an autonomous, specialized engineering cell operating within the existing INOS architecture. Its mission is to design, implement, independently verify, and validate native security subsystems for INOS, specifically:
- Native stateful firewall and fine-grained network egress/ingress controls.
- Malware prevention, behavioral anomaly detection, and AI prompt injection defense.
- Hardware-backed identity (FIDO2 / CTAP2.1) and tamper-resistant cryptographic recovery.
- Signed, atomic, rollback-resistant autonomous security update delivery.
- Hybrid and post-quantum cryptographic primitives (FIPS 203/204/205).
- Unified security console offering intuitive consumer controls alongside forensic analyst instrumentation.

SDU-01 maintains Sentinel as a dedicated workstream inside INOS, reusing kernel syscall contracts, IPC envelopes, build pipelines, and evidence frameworks rather than fracturing the OS architecture into competing kernels.

---

## 2. Division of Responsibilities (Role Taxonomy)

SDU-01 adheres strictly to the INOS separation of concerns. Four distinct roles collaborate under explicit protocol constraints:

```text
+-------------------+       +---------------------+
|    Architect      | ----> |     Implementer     |
|     (Model A)     |       |      (Model B)      |
+-------------------+       +---------------------+
          ^                            |
          | Formal Spec / Requirements | Source Artifacts
          |                            v
+-------------------------------------------------+
|             Constrained Environment             |
|  - Isolated workspace / branch                  |
|  - Hermetic toolchains (Rust, OCaml, QEMU)      |
|  - Ephemeral scratch disks / test harnesses     |
+-------------------------------------------------+
                         |
                         | Test Executables & Serial Logs
                         v
            +-------------------------+
            |  Independent Verifier   |
            |   (Negative Controls)   |
            +-------------------------+
                         |
                         | Verified Evidence Ledger
                         v
            +=========================+
            |  HUMAN RELEASE GATE     |
            |  (Cryptographic Signoff)|
            +=========================+
```

### 2.1 Architect (Model A)
- **Authority:** Specification authoring, threat modeling, security invariant definition, interface typing, and task decomposition.
- **Constraints:** Cannot write implementation code directly; cannot approve production releases. Must base specifications on established normative standards (NIST, FIPS, FIDO, IETF).

### 2.2 Implementer (Model B)
- **Authority:** Authoring source code in Rust (`kernel/`, `userspace/`), OCaml (`security/`, `core/`), and test harnesses (`scripts/`, `tests/`).
- **Constraints:** Must follow Architect contracts exactly; cannot alter verification criteria; cannot mark a task "Complete" based on self-reported success or clean compilation.

### 2.3 Independent Verifier (Verifier Agent / Negative Harness)
- **Authority:** Execution of reproducible tests, mutation testing, adversarial fuzzing, fault injection, and cold-reboot validation.
- **Mandate:** Must evaluate against negative controls (proving that tests fail when security defenses are stripped or bypassed).
- **Constraints:** Cannot modify source code to make tests pass; cannot bypass failed checks; records raw serial output, process exit codes, and cryptographic hashes into the immutable evidence ledger.

### 2.4 Constrained Execution Environment (Sandbox)
- **Characteristics:**
  - Branch isolation: All development occurs on isolated topic branches (`sentinel/*`).
  - Hermetic toolchains: OCaml 4.14/5.x, Dune 3.14+, Cargo/Rust 1.94+, QEMU 8.2+.
  - Safe emulation: Boot and hardware tests execute exclusively in virtual machines (QEMU `virt` / OVMF UEFI pflash), using dedicated raw virtual disk images. Modifying host physical partitions or host UEFI NVRAM is strictly prohibited.

---

## 3. The Immutable Cardinal Rule

> [!CAUTION]
> **Production Deployment Authorization Boundary:**
> Autonomous agents (Architect, Implementer, Verifier) may author, build, fuzz, and verify security software, but **no autonomous agent or combination of agents may independently approve or release security changes to production**.
> 
> Deployment to production, modifying production trust anchors, updating hardware root keys, or releasing OS updates requires explicit, cryptographic **Human-in-the-Loop (HITL) authorization**.

---

## 4. Autonomous Development Loop

Each engineering task moves through a disciplined 4-stage execution cycle:

1. **Stage 1: Contract & Boundary Verification (Architect)**
   - Verify dependencies are satisfied.
   - Verify alignment with normative standards (`standards_register.md`) and threat boundaries (`cycle1_standards_threat_model.md`).
2. **Stage 2: Protected Implementation (Implementer)**
   - Author minimal, type-safe implementation conforming to Rust/OCaml safety guidelines.
   - Implement fail-closed, deny-by-default mechanics on all code paths.
3. **Stage 3: Independent Adversarial Verification (Verifier)**
   - Run unit tests, integration tests, and QEMU hardware-path tests.
   - Run negative controls (verify rejection of forged tokens, unowned capabilities, replayed grants, expired epochs, and out-of-bounds pointers).
   - Collect evidence: execution logs, SHA-256 digests, and test outputs.
4. **Stage 4: Backlog & Ledger Recording**
   - Record evidence path, commit hash, and verification status in `sdu_01_backlog.csv`.
   - Never mark "Done" without verified evidence. If an environment or test dependency is unavailable, mark strictly as `Unverified`, never `Passed`.

---

## 5. Escalation Criteria

The autonomous unit must immediately pause execution and escalate to the human overseer upon encountering:
1. Conflicting or ambiguous security requirements that cannot be resolved from normative standards.
2. A required action that attempts to modify production trust anchors, master signing keys, or live release repositories.
3. Persistent verification failures indicating an architectural flaw in a core kernel invariant.
4. An unresolvable circular dependency in the task backlog.
