# INOS Sentinel — MVP Beta Requirements Specification

**Document Version:** 1.0.0  
**Status:** ACTIVE SPECIFICATION  
**Governing Directive:** SDU-01 Continuous Autonomous Engineering Directive  
**Objective:** Non-negotiable technical, functional, security, and standards requirements required to declare the INOS Sentinel Minimum Viable Product (MVP) ready for limited external beta release.

---

## 1. Beta Readiness Stopping Condition

The autonomous engineering unit executes continuously until **all 9 Beta Readiness Gates** achieve verified passing status, with:
1. **Zero Known Unresolved Critical Vulnerabilities** within the supported INOS threat model.
2. **Zero Known Unresolved High-Severity Vulnerabilities** undermining a core kernel, cryptographic, identity, or network security boundary.
3. Explicit risk disposition, formal justification, and compensating controls for any medium- or low-severity findings.
4. Reproducible cryptographic and integration evidence recorded in [`evidence/`](https://github.com/k-luecke/IntentNativeOS/blob/main/evidence/).

Checking a box in any tracking list does NOT constitute readiness. A requirement is satisfied only when supported by reproducible test logs, negative controls, and recorded maturity levels.

---

## 2. The 9 MVP Beta Readiness Gates

```mermaid
flowchart LR
    G1["1. Kernel & Capability<br/>Isolation"] --> G2["2. Firewall & Network<br/>Security"]
    G2 --> G3["3. Identity & Secure<br/>Recovery"]
    G3 --> G4["4. Autonomous Security<br/>Maintenance"]
    G4 --> G5["5. Malware & AI-Agent<br/>Protection"]
    G5 --> G6["6. Native Consumer<br/>Interface"]
    G6 --> G7["7. Standards<br/>Conformance"]
    G7 --> G8["8. Performance &<br/>Reliability"]
    G8 --> G9["9. Independent Security<br/>Assessment"]
    G9 --> Beta["Human-in-the-Loop<br/>Beta Distribution Signoff"]
```

### Gate 1: Kernel & Capability Isolation
- **REQ-G1-1:** Strict Ring 0 / Ring 3 privilege boundary enforcement via hardware page tables (`USER_ACCESSIBLE` bit cleared on all kernel pages).
- **REQ-G1-2:** Monotonic capability delegation and attenuation: child process capability tokens cannot exceed parent permissions.
- **REQ-G1-3:** Zero ambient authority: all privileged syscalls (`SYS_STORAGE_*`, `SYS_NET_*`, `SYS_MODULE_LOAD`) strictly require valid, unrevoked capability tokens.
- **REQ-G1-4:** User pointer validation: all syscall buffer pointers must be validated against `validate_user_access` before kernel dereference.

### Gate 2: Firewall & Network Security
- **REQ-G2-1:** Stateful default-deny inbound firewall with 5-tuple conntrack table (`kernel/src/net/firewall.rs`).
- **REQ-G2-2:** Default-deny outbound egress filtering with kernel-derived `(pid, generation)` identity binding.
- **REQ-G2-3:** Active flow revocation: capability revocation (`SYS_CAP_REVOKE`) immediately terminates active conntrack flows and invalidates open sockets (`is_revoked = true`).
- **REQ-G2-4:** Fail-closed protocol handling: non-standard IP protocols, IPv6 extension headers, and unauthorized raw frame attempts are dropped.
- **REQ-G2-5 (S203):** Standards-based DNS security with DNSSEC validation (RFC 4033–4035), DoH/DoT authenticated transport profiles (RFC 8484 / RFC 7858), and downgrade resistance. Real distinction maintained between upstream forwarding and local signature validation.

### Gate 3: Identity & Secure Recovery
- **REQ-G3-1 (S301):** WebAuthn / FIDO2 / CTAP2 assertion verification with User Presence (UP) and User Verification (UV) flags enforced.
- **REQ-G3-2 (S302):** Replay-resistant step-up authentication broker. Cardinal Invariant: **Authentication $\neq$ Authorization**. Valid FIDO2 assertion mints strictly bounded, single-use grant token bound to canonical operation digest.
- **REQ-G3-3 (S303):** 256-bit machine-generated recovery secret with standardized 24-word BIP-39 mnemonic encoding and 8-bit SHA-256 checksum verification.
- **REQ-G3-4 (S303):** Authenticated key wrapping using RFC 3394 AES-KW or ChaCha20-Poly1305 with HKDF-SHA256 key derivation.
- **REQ-G3-5 (S303):** Strict architectural separation: Identity Recovery (re-registering hardware keys in broker) vs Data Recovery (unwrapping disk volume master encryption keys).
- **REQ-G3-6 (S303):** Post-quantum hybrid migration support (ML-KEM-768 encapsulation alongside classical symmetric wrapping).

### Gate 4: Autonomous Security Maintenance
- **REQ-G4-1 (S401):** Automated vulnerability intelligence engine ingesting NIST NVD (CVE API 2.0), CISA KEV, and OSV feeds with deterministic normalization and bounded local cache ($\le 512$ entries).
- **REQ-G4-2 (S401):** Component mapping against INOS software/firmware inventory with explicit 5-tier uncertainty (`Definite`, `Probable`, `Potential`, `Unknown`, `NotApplicable`).
- **REQ-G4-3 (S401):** Feed freshness monitoring, stale-data alerting, and safe offline execution. Cardinal Invariant: AI analysis of advisories **cannot** mint grants or apply changes autonomously.
- **REQ-G4-4 (S402):** Standards-compliant TUF v1.0.33 secure update engine with 4 cryptographic roles (Root, Targets, Snapshot, Timestamp). AI development agents have zero release-signing keys.
- **REQ-G4-5 (S402):** Monotonic anti-rollback protection and expiration timestamp verification.
- **REQ-G4-6 (S402):** Transactional A/B slot staging (`Slot_A`, `Slot_B`), health check watchdog, and automated fallback on trial boot failure.

### Gate 5: Malware & AI-Agent Protection
- **REQ-G5-1 (S501):** Behavioral anomaly detection and untrusted binary sandboxing with strictly capability-gated filesystem/network access.
- **REQ-G5-2 (S502):** Defense-in-depth prompt injection mitigation: strict provenance taint tracking (`Clean`, `Tainted`, `Redacted`) on external documents and tool inputs.
- **REQ-G5-3 (S503):** Epistemic authority boundaries: formal type segregation (`Verified_System_Fact`, `Interactive_Human_Command`, `Model_Inference`, `Untrusted_External_Ingress`).
- **REQ-G5-4 (S503):** Cardinal Invariant: **Fact $\neq$ Authority**. System measurements cannot mint execution authority; model inferences/confidence scores cannot grant capabilities.
- **REQ-G5-5 (S503):** Monotonic downward cross-agent delegation; upward attenuation and scope expansion fail closed.

### Gate 6: Native Consumer Interface
- **REQ-G6-1:** Native, responsive, accessible security console UI (IntScript/Canvas/WebRender).
- **REQ-G6-2:** Clear, un-obscured visual threat states: Safe (Green), Action Required (Yellow), Critical Threat / Compromised (Red).
- **REQ-G6-3:** Plain-language permission prompts displaying canonical target, action, caller identity, and cryptographic digest preview.
- **REQ-G6-4:** Consumer-accessible network controls: one-click network isolation, active flow monitor, per-application egress toggles.
- **REQ-G6-5:** Guided 24-word recovery enrollment and test-restoration workflow with error-locating checksum feedback.

### Gate 7: Standards Conformance
- **REQ-G7-1:** Normative mapping of every security capability to established standards (NIST SP 800-53, FIPS 203, FIPS 204, RFC 4033-4035, RFC 8484, TUF v1.0.33, BIP-39, RFC 3394, RFC 8439).
- **REQ-G7-2:** Explicit disclosure of limitations and delta between specification and implementation in [`STANDARDS_CONFORMANCE.md`](https://github.com/k-luecke/IntentNativeOS/blob/main/docs/security/sentinel/STANDARDS_CONFORMANCE.md).

### Gate 8: Performance & Reliability
- **REQ-G8-1:** Firewall packet evaluation latency $\le 50\,\mu\text{s}$ per packet on reference hardware.
- **REQ-G8-2:** Kernel memory overhead for firewall and conntrack tables bounded to $\le 16\,\text{MB}$.
- **REQ-G8-3:** Step-up broker authentication verification latency $\le 100\,\text{ms}$.
- **REQ-G8-4:** Vulnerability cache bounded to $\le 10\,\text{MB}$ RAM with deterministic LRU eviction.
- **REQ-G8-5:** A/B update atomic commit and rollback recovery window $\le 5$ seconds.

### Gate 9: Independent Security Assessment
- **REQ-G9-1:** Complete adversarial mutation test suite with 100% fail-closed detection across all attack surfaces.
- **REQ-G9-2:** Independent verification by agent role V and Red Team role R challenging all Implementer B assertions.
- **REQ-G9-3:** Documented risk acceptance register for all known low/medium findings in [`KNOWN_RISKS.md`](https://github.com/k-luecke/IntentNativeOS/blob/main/docs/security/sentinel/KNOWN_RISKS.md).
- **REQ-G9-4:** Comprehensive, reproducible beta readiness dossier in [`BETA_READINESS_REPORT.md`](https://github.com/k-luecke/IntentNativeOS/blob/main/docs/security/sentinel/BETA_READINESS_REPORT.md).
