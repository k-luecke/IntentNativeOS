# SDU-01: Sentinel Normative Standards Register

**Document ID:** SDU-01-STANDARDS  
**Classification:** Normative Reference Baseline  
**Revision:** v1.0  
**Effective Date:** 2026-10-11  

---

## 1. Overview and Applicability

This Standards Register establishes the authoritative technical references governing all security engineering, architecture reviews, implementation decisions, and verification tests conducted by the Sentinel Development Unit (SDU-01).

Every security mechanism implemented within INOS must map to at least one normative clause defined in this register. Divergences from normative standards must be documented with an explicit Architecture Decision Record (ADR) justifying the adaptation.

---

## 2. Identity and Authentication Baseline

### 2.1 NIST SP 800-63-4: Digital Identity Guidelines
- **Scope:** Identity proofing, enrollment, credential lifecycle, and authentication mechanics.
- **Key Invariants:**
  - **Authenticator Assurance Level 3 (AAL3):** High-privilege operations (capsule execution, capability minting, kernel updates, persistent storage wiping) require multi-factor hardware-bound cryptographic authenticators.
  - **Replay Resistance:** All authentication assertions and step-up authorization grants must use cryptographically bound single-use nonces and monotonic sequence numbers.
  - **Session & Epoch Lifecycles:** Grants must be strictly bounded in time (TTL) and invalidation epochs; bearer tokens without cryptographic binding are prohibited.

### 2.2 FIDO Alliance: FIDO2 / CTAP 2.1 & W3C WebAuthn Level 3
- **Scope:** Hardware-based passkeys, USB/NFC security keys, and platform authenticators.
- **Key Invariants:**
  - Client-to-Authenticator Protocol (CTAP) 2.1 compliant transaction confirmation.
  - User Presence (UP) and User Verification (UV) verification flags enforced in kernel broker.
  - Asymmetric public-key authentication preventing credential sharing and credential reuse across distinct origin domains.

---

## 3. Secure Software Engineering & Supply Chain

### 3.1 NIST SP 800-218: Secure Software Development Framework (SSDF v1.1)
- **Scope:** Development lifecycle practices to mitigate software vulnerabilities.
- **Core Practice Mapping:**
  - **PO.1 - PO.5 (Prepare the Organization):** Role-separated development loop (Architect, Implementer, Independent Verifier).
  - **PS.1 - PS.3 (Protect the Software):** Tamper-evident repository tracking, cryptographic commit signing, hermetic toolchain locking.
  - **PW.1 - PW.9 (Produce Well-Secured Software):** Deny-by-default syscall design, strict type safety (Rust memory safety and OCaml typed contracts), user-pointer page table boundary checks.
  - **RV.1 - RV.3 (Respond to Vulnerabilities):** Rollback-resistant atomic update pipeline and automated anomaly alerts.

### 3.2 Supply Chain Levels for Software Artifacts (SLSA v1.0) & in-toto
- **Scope:** Build integrity and provenance tracking.
- **Key Invariants:**
  - Hermetic build environments reproducing identical binaries from verified source trees.
  - Cryptographically signed in-toto attestations accompanying all kernel, driver, and userland release bundles.
  - Prevention of unsigned binary execution across Ring 3 / Ring 0 execution gates.

---

## 4. Zero Trust & Network Enforcement

### 4.1 NIST SP 800-207: Zero Trust Architecture (ZTA)
- **Scope:** Network and service boundary enforcement.
- **Key Invariants:**
  - **Explicit Verification:** Never assume trust based on network locality, process parentage, or past interaction. Every packet, syscall, and IPC message is evaluated against explicit capability grants.
  - **Least Privilege Access:** Egress and ingress ports are closed by default; processes require an active scoped capability token to open or bind sockets.
  - **Continuous Telemetry & Audit:** Real-time logging of network policy violations, dropped packets, and unexpected socket connection attempts.

### 4.2 IETF RFC 8446 (TLS 1.3) & RFC 9000 (QUIC)
- **Scope:** Encrypted and authenticated transport layers.
- **Key Invariants:**
  - Mandatory forward secrecy with Ephemeral Diffie-Hellman or Post-Quantum KEM key exchange.
  - Strict cipher suite restriction (ChaCha20-Poly1305, AES-256-GCM; legacy 3DES/RC4/CBC ciphers forbidden).
  - Encrypted SNI (ECH) and certificate validation against an isolated, immutable trust store.

---

## 5. Cryptography & Post-Quantum Readiness

### 5.1 FIPS 203: Module-Lattice-Based Key-Encapsulation Mechanism (ML-KEM)
- **Scope:** Quantum-resistant key establishment (formerly Kyber).
- **Key Invariants:**
  - Standardized parameter sets: ML-KEM-768 (Category 3 security, default for INOS session keys) and ML-KEM-1024 (Category 5).
  - Hybrid mode support: Combining ML-KEM with X25519 for dual-layered classical/PQ key exchange during the transition window.

### 5.2 FIPS 204: Module-Lattice-Based Digital Signature Algorithm (ML-DSA)
- **Scope:** Primary post-quantum digital signatures (formerly Dilithium).
- **Key Invariants:**
  - ML-DSA-65 and ML-DSA-87 for signing kernel release packages, capability assertions, and firmware manifests.

### 5.3 FIPS 205: Stateless Hash-Based Digital Signature Algorithm (SLH-DSA)
- **Scope:** Conservative post-quantum backup signature scheme (formerly SPHINCS+).
- **Key Invariants:**
  - Used for long-term root of trust certificates, root anchors, and disaster recovery keys where security relies only on the collision resistance of cryptographic hashes.

---

## 6. Standards Compliance Traceability Matrix

| Standard | Topic | Implementing Module(s) | Verification Suite |
| :--- | :--- | :--- | :--- |
| **NIST SP 800-63-4** | Identity & Step-Up Auth | `security/broker.ml`, `security/pairing.ml` | `tests/test_security.ml` (A002-A007) |
| **NIST SP 800-218** | Secure Development | CI, `scripts/verify_repo.ml` | `tests/test_native_safety.sh` |
| **NIST SP 800-207** | Zero Trust & Capabilities | `kernel/src/capability/mod.rs` | `scripts/test_sentinel_cycle1.sh` |
| **FIPS 203 / 204** | Post-Quantum Algorithms | `security/post_quantum.ml`, `crypto_registry.ml`| `tests/test_security.ml` (A010-A013) |
| **FIDO2 / CTAP2.1** | Hardware Passkeys | `security/assertion.ml`, `security/types.ml` | `tests/test_security.ml` (A005, A006) |
| **RFC 8446 (TLS 1.3)**| Encrypted Transport | `userspace/apps/net_app/`, `kernel/src/net/` | `scripts/test_web_01b.sh` |
| **SLSA v1.0** | Hermetic Build & Provenance| `toolchain.lock.json`, `docs/P010_` | `scripts/build_kernel.sh` |
