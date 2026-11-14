# SDU-01: Sentinel Engineering Checklist

**Backlog ID:** SDU-01-CHECKLIST  
**Unit:** Sentinel Development Unit (SDU-01)  
**Parent Project:** Intent-Native Operating System (INOS)  
**Status:** Cycle 2 Complete (Development & Testing Verified; Production Release: Not Authorized)  
**Total Engineering Tasks:** 36 Tasks across 9 Phases  

---

## Delivery Sequence Summary

| Phase | Priority | Primary Outcome | Tasks | Gate Status |
| :--- | :--- | :--- | :---: | :---: |
| **0. Architecture and standards** | Critical | Verified requirements and security boundaries | 4 | Complete (Cycle 1) |
| **1. Enforcement foundations** | Critical | Capability isolation, policy protection, auditability | 4 | Complete (Cycle 1 & 2: S101-S104) |
| **2. Firewall and network management** | Critical | Native network protection and controls | 4 | In Progress (Cycle 2 & 3: S201, S202 Complete) |
| **3. Identity and recovery** | Critical | FIDO2 hardware authentication and robust data recovery | 4 | In Progress (Cycle 2 & 3: S301, S302 Complete) |
| **4. Autonomous update system** | Critical | Signed, rollback-resistant security maintenance | 4 | Pending |
| **5. Malware and AI threat defense** | High | Event-driven detection and restricted agents | 4 | In Progress (Cycle 2 & 3: S502, S503 Complete) |
| **6. Post-quantum migration** | High | Versioned cryptography and standards-based PQ support | 4 | Pending (Cycle 1: S003 Registry Verified) |
| **7. Consumer and analyst interfaces** | High | Simple security center plus advanced diagnostics | 4 | Pending |
| **8. Adversarial validation** | Release blocking | Reproducible tests, security review, performance evidence | 4 | Active (Negative & Mutation Harness) |

---

## Detailed Task Breakdown

### Phase 0: Architecture and Standards (Priority: Critical)

- [x] **S001 — Map INOS capability, networking, capsule, and crypto architecture**  
  *Priority:* Critical | *Owner:* Architect | *Depends:* `none`  
  *Deliverable:* `docs/security/sentinel/cycle1_architecture_inventory.md`  
  *Acceptance Test:* Inventory documents actual repository implementations, explicitly distinguishing Existing (verified in tree), Partial (stubbed/incomplete), Missing (planned), and Unverified capabilities across `kernel/src/capability/mod.rs`, `security/`, `kernel/src/net/`, and `capsules/`.

- [x] **S002 — Normative standards mapping and adversarial threat model**  
  *Priority:* Critical | *Owner:* Architect | *Depends:* `S001`  
  *Deliverable:* `docs/security/sentinel/cycle1_standards_threat_model.md`  
  *Acceptance Test:* Threat model maps T0 (Kernel) to T4 (Untrusted Inputs) trust boundaries, catalogues attack vectors (forged tokens, model speculation privilege escalation, replay attacks, memory corruption), and specifies explicit negative test obligations.

- [x] **S003 — Establish Sentinel normative standards register**  
  *Priority:* Critical | *Owner:* Architect | *Depends:* `none`  
  *Deliverable:* `docs/security/sentinel/standards_register.md`  
  *Acceptance Test:* Authoritative register links implementation modules to NIST SP 800-63-4, NIST SP 800-218 (SSDF), NIST SP 800-207 (Zero Trust), FIPS 203/204/205, FIDO2/CTAP2.1, and RFC 8446.

- [x] **S004 — SDU-01 Autonomous supervisor contract and safety governance**  
  *Priority:* Critical | *Owner:* Architect | *Depends:* `none`  
  *Deliverable:* `docs/security/sentinel/SDU_01_AUTONOMOUS_UNIT_CHARTER.md`  
  *Acceptance Test:* Charter defines role boundaries (Architect, Implementer, Independent Verifier, Sandbox) and formalizes the Cardinal Rule: autonomous agents cannot independently approve security changes for production release.

---

### Phase 1: Enforcement Foundations (Priority: Critical)

- [x] **S101 — Capability handle lifecycle, downward attenuation, and recursive revocation cascade**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S001, S004`  
  *Deliverable:* `kernel/src/lib.rs`, `kernel/src/capability/mod.rs`  
  *Acceptance Test:* Invocations verify that handles can be created, delegated with monotonically attenuated rights, and recursively revoked (invalidating all child handles in the tree). Verified by negative test harness.  
  *Evidence:* `evidence/sentinel_cycle1_acceptance.log`, `kernel/src/lib.rs:test_sentinel_cycle1_suite`

- [x] **S102 — Deny-by-default syscall mediation and user-pointer boundary validation**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S101`  
  *Deliverable:* `kernel/src/lib.rs`, `security/broker.ml`, `tests/test_sentinel_cycle1.ml`  
  *Acceptance Test:* Syscall dispatcher denies any unauthenticated invocation; unowned or forged capability tokens fail with error; user pointers are verified via page-table walks before access.  
  *Evidence:* `evidence/sentinel_cycle1_acceptance.log`, `tests/test_sentinel_cycle1.ml`

- [x] **S103 — Tamper-evident, hash-chained append-only security event audit log**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S102`  
  *Deliverable:* `security/audit.ml`, `tests/test_sentinel_cycle2.ml`  
  *Acceptance Test:* Security events (grants, revocations, policy violations) form an immutable hash-chained log with SHA-256 integrity verification. Tampered records are detected instantly; checkpoints anchored with HMAC/digest; crash/truncation recovery verified.  
  *Evidence:* `evidence/sentinel_cycle2_acceptance.log`, `tests/test_sentinel_cycle2.ml:test_s103_audit_subsystem`

- [x] **S104 — Monotonic privilege attenuation and anti-escalation token verification**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S101`  
  *Deliverable:* `kernel/src/capability/mod.rs`, `kernel/src/lib.rs`  
  *Acceptance Test:* Attempts to delegate rights not held in the parent capability return `CapError::CannotAttenuateUpward`. Upward privilege escalation is mathematically impossible.  
  *Evidence:* `evidence/sentinel_cycle1_acceptance.log`, `kernel/src/lib.rs:test_sentinel_cycle1_suite`

---

### Phase 2: Firewall and Network Management (Priority: Critical)

- [x] **S201 — Native stateful packet inspection (SPI) firewall in kernel/net**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S102`  
  *Deliverable:* `kernel/src/net/firewall.rs`, `kernel/src/net/mod.rs`, `kernel/src/lib.rs`  
  *Acceptance Test:* Kernel inspects TCP/UDP state transitions, dropping unsolicited inbound packets, invalid SYN/ACK sequences, and malformed frames; tracks 5-tuple state; enforces process network capability requirements.  
  *Evidence:* `evidence/sentinel_cycle2_acceptance.log`, `kernel/src/lib.rs:test_sentinel_cycle2_firewall_suite`

- [x] **S202 — Per-process and per-capsule network egress policy enforcement**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S201`  
  *Deliverable:* `kernel/src/net/firewall.rs`, `kernel/src/net/socket.rs`, `kernel/src/arch/x86_64/syscall.rs`, `kernel/src/lib.rs`  
  *Acceptance Test:* Outbound network access is strictly default-deny; sockets and outbound flows bind strictly to kernel-derived identity `(pid, generation)`; capability revocation immediately terminates active conntrack flows and invalidates open sockets; fail-closed handling for unsupported protocols, IPv6 extension headers, and unauthorized raw frame attempts.  
  *Evidence:* `evidence/sentinel_cycle3_acceptance.log`, `kernel/src/lib.rs:test_sentinel_cycle3_egress_suite`, `tests/test_sentinel_cycle3.ml:test_s202_network_egress`  
  *Maturity Level:* E2 (Kernel & Integration Tested; QEMU Ring 3 Syscall Harness Verified)

- [ ] **S203 — DNS security enforcement, DNSSEC validation, and encrypted DoH transport**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S202`  
  *Deliverable:* `kernel/src/net/dns.rs`, `userspace/apps/net_app/`  
  *Acceptance Test:* Plaintext unverified DNS responses are blocked when DNSSEC or DoH policy is active.

- [ ] **S204 — Socket capability binding and unmediated raw network access blocking**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S202`  
  *Deliverable:* `kernel/src/net/socket.rs`  
  *Acceptance Test:* Direct raw ethernet socket creation restricted to Ring 0 drivers; Ring 3 processes strictly mediated.

---

### Phase 3: Identity and Recovery (Priority: Critical)

- [x] **S301 — FIDO2 / CTAP 2.1 hardware authenticator binding**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S003, S102`  
  *Deliverable:* `security/assertion.ml`, `tests/test_sentinel_cycle2.ml`  
  *Acceptance Test:* Hardware tokens register via CTAP2.1; User Presence (UP) and User Verification (UV) flags are verified cryptographically; monotonic sign counters prevent cloned key rollback; classical keys rejected under Post_Quantum_Strict policy; revoked keys fail closed.  
  *Evidence:* `evidence/sentinel_cycle2_acceptance.log`, `tests/test_sentinel_cycle2.ml:test_s301_hardware_identity`  
  *Maturity Level:* E1 (Unit & Emulation Verified; E3 deferred to physical USB HID token)

- [x] **S302 — Replay-resistant step-up authentication broker for sensitive operations**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S301`  
  *Deliverable:* `security/broker.ml`, `tests/test_sentinel_cycle3.ml`  
  *Acceptance Test:* WebAuthn/FIDO2 assertion verification bound to canonical operation digest; User Presence (UP) and User Verification (UV) flags enforced; single-use challenge consumption with monotonic sequence; relying-party binding; cardinal invariant enforced: *Authentication $\neq$ Authorization* (valid assertion mints strictly bounded, single-use grant token with zero scope expansion).  
  *Evidence:* `evidence/sentinel_cycle3_acceptance.log`, `tests/test_sentinel_cycle3.ml:test_s302_step_up_broker`  
  *Maturity Level:* E1 (Cryptographically Verified; E3 deferred to physical token)

- [ ] **S303 — Cryptographically authenticated key enrollment and Shamir threshold recovery**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S302`  
  *Deliverable:* `security/recovery.ml`  
  *Acceptance Test:* Offline threshold recovery successfully reconstructs master identity with $K$-of-$N$ valid shares; fails with $K-1$ shares.

- [ ] **S304 — Crash-consistent transaction journal and cold-reboot state recovery**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S103`  
  *Deliverable:* `kernel/src/storage/transaction.rs`  
  *Acceptance Test:* Injected power fault during capability or identity migration cleanly rolls back without data corruption or privilege leakage on reboot.

---

### Phase 4: Autonomous Update System (Priority: Critical)

- [ ] **S401 — Dual-signature release verification and SLSA provenance validation**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S003, S103`  
  *Deliverable:* `security/update.ml`, `core/update/`  
  *Acceptance Test:* Update bundles missing valid cryptographic signatures or in-toto attestations are rejected before staging.

- [ ] **S402 — A/B slot immutable OS staging and atomic bootloader transition**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S401`  
  *Deliverable:* `kernel/src/storage/`, `tools/firmware/`  
  *Acceptance Test:* System boots from active slot; new update writes to passive slot; atomic flip occurs only after self-test verification.

- [ ] **S403 — Anti-rollback monotonic counter and firmware/OS downgrade protection**  
  *Priority:* Critical | *Owner:* Implementer | *Depends:* `S402`  
  *Deliverable:* `kernel/src/security/anti_rollback.rs`, `security/update.ml`  
  *Acceptance Test:* Update payloads with version numbers lower than current monotonic counter are unconditionally rejected.

- [ ] **S404 — Autonomous staging pipeline with independent verifier gate and HITL signoff**  
  *Priority:* Critical | *Owner:* Verifier | *Depends:* `S401, S403`  
  *Deliverable:* `scripts/release_gate.sh`, `docs/security/sentinel/`  
  *Acceptance Test:* Staging completes autonomously; final activation halts at release gate until cryptographic human signoff is supplied.

---

### Phase 5: Malware and AI Threat Defense (Priority: High)

- [ ] **S501 — Behavioral anomaly detection engine for Ring 3 processes and capsules**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S102, S103`  
  *Deliverable:* `kernel/src/process/fuzz.rs`, `services/sentinel_monitor/`  
  *Acceptance Test:* Excessive syscall burst rates, rapid failed capability probes, or unauthorized memory access triggers automatic process suspension and alert.

- [x] **S502 — Prompt injection firewall and untrusted context sanitizer for Model A/B**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S002`  
  *Deliverable:* `security/agent_boundary.ml`, `tests/test_sentinel_cycle2.ml`  
  *Acceptance Test:* Injected instructions within user documents and tool outputs are classified as Tainted; defense-in-depth sanitization strips injection signatures; kernel capability enforcement strictly gates tool execution (prompt cannot bypass capability check).  
  *Evidence:* `evidence/sentinel_cycle2_acceptance.log`, `tests/test_sentinel_cycle2.ml:test_s502_agent_boundaries`

- [x] **S503 — Epistemic status enforcement (speculation cannot upgrade to capability grant)**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S101, S502`  
  *Deliverable:* `security/agent_boundary.ml`, `tests/test_sentinel_cycle3.ml`, `tests/test_sentinel_integration.ml`  
  *Acceptance Test:* Strict type separation between facts, human commands, model inferences, and external ingress; cardinal invariant enforced: `Verified_System_Fact` does NOT grant capability or execution authority; model inferences and confidence scores (even 0.999) cannot mint or synthesize capability grants; monotonic cross-agent delegation strictly attenuates rights downwards; attempts at upward attenuation, scope expansion, or exceeding delegation depth fail closed; immutable hash-chained audit provenance recorded for all transitions.  
  *Evidence:* `evidence/sentinel_cycle3_acceptance.log`, `tests/test_sentinel_cycle3.ml:test_s503_epistemic_boundaries`, `tests/test_sentinel_integration.ml`  
  *Maturity Level:* E1 (Unit & Integration Verified; E4 Adversarially Reviewed)

- [ ] **S504 — Isolated containment sandbox for untrusted scripts and legacy binaries**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S102`  
  *Deliverable:* `capsules/sandbox/`, `kernel/src/arch/x86_64/paging.rs`  
  *Acceptance Test:* Sandboxed code cannot read parent process memory, write outside assigned virtual directory, or emit raw network packets.

---

### Phase 6: Post-Quantum Migration (Priority: High)

- [ ] **S601 — Versioned cryptographic registry supporting hybrid classical/PQ primitives**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S003`  
  *Deliverable:* `security/crypto_registry.ml`, `security/types.ml`  
  *Acceptance Test:* Registry negotiates between `Standard_Modern` (Ed25519) and `Post_Quantum_Strict` (ML-DSA-65, ML-KEM-768); fail-closed enforcement verified.

- [ ] **S602 — ML-KEM (FIPS 203) key encapsulation mechanism integration**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S601`  
  *Deliverable:* `security/post_quantum.ml`, `kernel/src/crypto/`  
  *Acceptance Test:* Known Answer Test (KAT) test vectors for ML-KEM-768 pass with exact match against NIST reference vectors.

- [ ] **S603 — ML-DSA (FIPS 204) and SLH-DSA (FIPS 205) signature validation**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S601`  
  *Deliverable:* `security/post_quantum.ml`, `security/assertion.ml`  
  *Acceptance Test:* ML-DSA-65 signature generation and verification pass KAT validation; corrupted signatures fail closed.

- [ ] **S604 — Post-quantum secure IPC wire format and certificate verification**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S602, S603`  
  *Deliverable:* `core/ipc/envelope.ml`, `security/`  
  *Acceptance Test:* IPC envelopes support post-quantum signed payloads; downgrade attacks to classical algorithms under `Post_Quantum_Strict` are rejected.

---

### Phase 7: Consumer and Analyst Interfaces (Priority: High)

- [ ] **S701 — Unified Security Center GUI widget / surface**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S103, S201`  
  *Deliverable:* `ui/security_center/`, `userspace/apps/`  
  *Acceptance Test:* Surface displays real-time security posture (Green/Yellow/Red), active network connections, active capabilities, and firewall status.

- [ ] **S702 — Forensic event inspector and structured telemetry exporter**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S103, S701`  
  *Deliverable:* `ui/security_center/forensics.ml`, `services/telemetry/`  
  *Acceptance Test:* Security analyst can inspect filtered audit logs, drill down into process capability trees, and export signed JSON reports.

- [ ] **S703 — Interactive capability grant manager and step-up prompt UI**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S302, S701`  
  *Deliverable:* `ui/modals/security_prompt.ml`  
  *Acceptance Test:* Modal cleanly displays target action, parameters, requesting process, and security implications; enforces timeout and cancel by default.

- [ ] **S704 — Plain-language security advice translator for non-technical users**  
  *Priority:* High | *Owner:* Implementer | *Depends:* `S701`  
  *Deliverable:* `ui/security_center/explainer.ml`  
  *Acceptance Test:* Technical alerts (e.g. "Egress blocked on TCP 443 for PID 104") translated into clear, actionable prose without security jargon.

---

### Phase 8: Adversarial Validation (Priority: Release blocking)

- [ ] **S801 — Automated red-team negative control test suite in QEMU**  
  *Priority:* Release-blocking | *Owner:* Verifier | *Depends:* `S102, S202, S504`  
  *Deliverable:* `scripts/test_adversarial_suite.sh`, `evidence/`  
  *Acceptance Test:* Suite executes 25 adversarial scenarios (forged tokens, buffer overruns, unowned handles, replayed grants); 100% must be trapped and logged.

- [ ] **S802 — Capability fuzzing harness (forging tokens, tampering handles, replay attacks)**  
  *Priority:* Release-blocking | *Owner:* Verifier | *Depends:* `S101, S102`  
  *Deliverable:* `security/fuzz.ml`, `kernel/src/process/fuzz.rs`  
  *Acceptance Test:* Fuzz engine executes 10,000 iterations of malformed capability grants with zero kernel crashes, panics, or unauthorized state changes.

- [ ] **S803 — Cold reboot and power-fault resilience verification under attack**  
  *Priority:* Release-blocking | *Owner:* Verifier | *Depends:* `S304, S402`  
  *Deliverable:* `scripts/test_fault_recovery.sh`  
  *Acceptance Test:* System cleanly recovers from mid-transaction power cut; storage journal rolls back uncommitted security states with zero leakage.

- [ ] **S804 — Performance envelope verification (latency overhead, RAM budget, cold-start)**  
  *Priority:* Release-blocking | *Owner:* Verifier | *Depends:* All prior  
  *Deliverable:* `benchmarks/security_perf_report.md`  
  *Acceptance Test:* Capability checking adds < 5% overhead to syscall latency; Sentinel background monitor consumes < 32 MB RAM within 16 GiB envelope.
