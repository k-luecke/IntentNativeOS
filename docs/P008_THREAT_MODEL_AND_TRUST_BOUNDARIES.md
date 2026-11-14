# P008: Threat Model and Trust Boundary Inventory

**Task ID:** P008  
**Gate:** G0 (Foundational Controls)  
**Status:** Approved Specification  

---

## 1. Trust Hierarchy & Boundary Taxonomy

The Intent-Native Operating System establishes an explicit, asymmetric trust hierarchy. Security guarantees are enforced through architectural containment, address-space isolation, and capability verification, rather than model self-policing.

```text
+-------------------------------------------------------------------+
| T0: Trusted Minimal Kernel / Hypervisor Primitives                |
| - Physical memory allocation, page table enforcement, GDT/IDT     |
| - Hardware device assignment, APIC interrupts, CPU ring isolation |
| - Highest privilege. NO models or dynamic compilers in T0.        |
+-------------------------------------------------------------------+
                                  ^
                                  | Capability invocations (Syscalls)
+-------------------------------------------------------------------+
| T1: Trusted Capability & Cryptographic Broker                     |
| - Cryptographic signature verification, key management            |
| - Step-up authorization validation (hardware token / phone / PIN) |
| - Capability grant minting and revocation enforcement             |
+-------------------------------------------------------------------+
                                  ^
                                  | Authenticated IPC Envelopes
+-------------------------------------------------------------------+
| T2: OS Control Plane Services                                     |
| - Input & Context Frontend, Clarification State Machine           |
| - Intent Compiler, Task Scheduler, Append-Only Semantic Journal   |
| - Execution Resolver, Capsule Lifecycle Manager                   |
+-------------------------------------------------------------------+
                                  ^
                                  | Mediated Task Graphs
+-------------------------------------------------------------------+
| T3: Untrusted Executors & Foundation Models                       |
| - Local Model A (Architect / Reviewer)                            |
| - Local Model B (Implementer / Challenger)                        |
| - Generated native binaries, guest capsules, user applications    |
+-------------------------------------------------------------------+
                                  ^
                                  | Ingestion / Processing
+-------------------------------------------------------------------+
| T4: External Untrusted Inputs                                     |
| - User documents, raw files, network traffic, prompt injections   |
+-------------------------------------------------------------------+
```

---

## 2. Adversary Model & Attack Vectors

### 2.1 Adversary Capabilities
We consider adversaries with the following potential footholds:
1. **Adversarial Human Input (T4):** Malicious prompts, prompt injections, indirect jailbreaks embedded within user documents or web content.
2. **Compromised Foundation Models (T3):** Models emitting hallucinated, exfiltrative, or destructive plans, attempting privilege escalation or bypassing constraints.
3. **Compromised Guest Capsules / User Binaries (T3):** Generated native code or third-party legacy binaries executing arbitrary shellcode, attempting kernel escape, or accessing unauthorized file paths.

### 2.2 Mitigation Guarantees

| Attack Vector | Vulnerable Component | Architecture Invariant / Defense |
| :--- | :--- | :--- |
| **Prompt Injection / Jailbreak** | Model A/B (T3) | Models are strictly untrusted workers. Model outputs cannot grant capabilities, bypass the clarification gate, or execute commands directly. |
| **Unauthorized File Deletion / Overwrite** | Intent Compiler / Capsule | **I-08 (Preservation):** All destructive operations require explicit user approval at the clarification gate and execute within transactionally staged CoW overlays with rollback receipts. |
| **Model Self-Authorization** | Model A/B (T3) | **I-03 (Capability Confinement):** Capability grants are minted solely by the T1 Broker following human authorization. |
| **Silent Exfiltration / Network Phoning** | Capsule / Model Runtime | **I-06 (No Silent Exfiltration):** Network is default-deny. VirtIO-Net drops packets without an active approved network capability profile. |
| **Buffer Overflow in Untrusted Code** | Kernel Boundary (T0) | Ring 0 / Ring 3 memory isolation, W^X (Write XOR Execute), SMEP, SMAP, and memory-safe Rust kernel primitives. |
| **Supply-Chain Dependency Poisoning** | Build System | Pinned toolchains (`toolchain.lock.json`), offline-first builds, and content-addressed source verification. |

---

## 3. Privilege Crossing Rules & Step-Up Authorization

1. **Read Operations within Project Scope:** Handled automatically under standard workspace grant.
2. **Non-Destructive Simulation:** Executed in ephemeral sandbox with zero external side-effects.
3. **Material State Modification (File edits, code commits):** Requires explicit human confirmation at the clarification gate.
4. **Destructive Operations (Deletions, format, hardware reconfiguration, policy updates):** Requires cryptographic step-up approval (hardware security key, local PIN, or paired device).
