# INOS Sentinel — Standards Conformance Register

**Document Version:** 1.0.0  
**Status:** NORMATIVE AUDIT REGISTER  
**Governing Standard:** INOS-SEC-STD-001

---

## 1. Standards Mapping & Conformance Matrix

| Standard / Specification | Clause / Section | Description | INOS Implementation Artifact | Test Harness | Conformance Status & Limitations |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **TUF v1.0.33** | §4.3 | Role Separation & Multi-Role Signing | `security/update.ml` | `tests/test_sentinel_cycle4.ml` | Full conformance for Root, Targets, Snapshot, Timestamp roles. Production keys held offline. |
| **TUF v1.0.33** | §5.1 | Anti-Rollback & Version Monotonicity | `security/update.ml` | `tests/test_sentinel_cycle4.ml` | Strictly enforced via monotonic sequence checking. |
| **RFC 1035** | §4.1.4 | DNS Message Compression & Parsing | `kernel/src/net/dns.rs` | `kernel::test_dns_parsing` | Bounded pointer traversal (max depth 8); loop detection fail-closed. |
| **RFC 4034** | §2–§5 | DNSSEC Resource Records (RRSIG, DNSKEY, DS) | `kernel/src/net/dns.rs` | `tests/test_sentinel_cycle4.ml` | Wire format parsing and type definition for RRSIG, DNSKEY, DS. |
| **RFC 4035** | §5 | DNSSEC Signature Validation | `kernel/src/net/dns.rs` | `tests/test_sentinel_cycle4.ml` | Validates stub signatures against trust anchors; full iterative resolver recursion deferred. |
| **RFC 8484** | §3 | DNS Queries over HTTPS (DoH) | `kernel/src/net/dns.rs` | `tests/test_sentinel_cycle4.ml` | Profile support; requires HTTPS capability token. |
| **BIP-39** | §Wordlist | Mnemonic Generation (24 words) | `security/recovery_v2.ml` | `tests/test_sentinel_cycle4.ml` | Standard English 2048-word dictionary, 256-bit entropy, 8-bit SHA-256 checksum. |
| **RFC 5869** | §2 | HMAC-based Extract-and-Expand KDF (HKDF) | `security/recovery_v2.ml` | `tests/test_sentinel_cycle4.ml` | HKDF-SHA256 derived key wrapping. |
| **RFC 3394** | §2 | AES Key Wrap Algorithm | `security/recovery_v2.ml` | `tests/test_sentinel_cycle4.ml` | Authenticated key wrapping for volume master encryption keys. |
| **RFC 8439** | §2.5 | ChaCha20-Poly1305 AEAD | `security/recovery_v2.ml` | `tests/test_sentinel_cycle4.ml` | Authenticated encryption for identity state wrapping. |
| **FIDO2 / CTAP2** | §5.1 | Authenticator Data & User Presence/Verification | `security/broker.ml` | `tests/test_sentinel_cycle2.ml` | UP and UV flags verified; authenticator counter monotonically enforced. |
| **W3C WebAuthn** | §5.1 | Assertion Verification & RP ID Binding | `security/broker.ml` | `tests/test_sentinel_cycle3.ml` | RP ID and challenge digest validation. |
| **FIPS 203** | §6 | ML-KEM-768 Key Encapsulation Mechanism | `security/post_quantum.ml` | `tests/test_security.ml` | NIST KAT test vectors verified. |
| **FIPS 204** | §6 | ML-DSA-65 Digital Signature Algorithm | `security/post_quantum.ml` | `tests/test_security.ml` | NIST KAT signature vectors verified. |
| **NIST SP 800-53** | AC-3, AC-4 | Access Enforcement & Information Flow | `kernel/src/capability/` | `kernel::test_sentinel_cycle1_suite` | Capability-based access control, zero ambient authority. |
| **NIST SP 800-53** | AU-10 | Non-repudiation / Audit Logging | `security/audit.ml` | `tests/test_sentinel_cycle2.ml` | Append-only hash-chained journal with checkpoint anchoring. |
| **NIST SP 800-53** | SC-7 | Boundary Protection / Firewall | `kernel/src/net/firewall.rs` | `kernel::test_sentinel_cycle2_firewall_suite` | Stateful default-deny inbound and outbound firewall. |
| **NIST SP 800-53** | SI-4 | System Monitoring / Intrusion Detection | `security/agent_boundary.ml` | `tests/test_sentinel_cycle3.ml` | Epistemic authority boundaries and taint tracking. |

---

## 2. Documented Implementation Limitations & Exclusions

1. **DNSSEC Recursion vs Stub Validation:** Sentinel implements local cryptographic validation of RRSIG/DNSKEY records provided in DNS responses. It does not implement a full recursive caching resolver capable of traversing the global DNS root zone hierarchy from scratch; it relies on upstream authoritative/recursive servers for answer records, but cryptographically verifies them locally.
2. **Physical Hardware Key Attestation:** FIDO2 authenticator attestation certificates (AAGUID, batch certificates) are validated in software simulation. Hardware-specific root CA trust chains for Yubico/Google Titan keys are deferred to the physical device pilot in Phase 8.
3. **AI Agent Signing Authority:** In strict adherence to TUF v1.0.33 and INOS-SEC-STD-001, autonomous AI agents possess zero cryptographic signing authority for release bundles or TUF root/targets metadata.
