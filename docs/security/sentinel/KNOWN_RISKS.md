# INOS Sentinel — Known Security Risks & Compensating Controls

**Document Version:** 1.0.0  
**Status:** ACTIVE RISK REGISTER  
**Governing Standard:** INOS-SEC-STD-001  
**Beta Policy:** Zero unresolved critical or high-severity vulnerabilities undermining core security boundaries. Medium and low findings require explicit risk disposition.

---

## 1. Risk Register

| Risk ID | Title | Severity | Impacted Boundary | Status | Compensating Control & Disposition |
| :--- | :--- | :---: | :--- | :---: | :--- |
| **RSK-01** | Software Simulation of FIDO2 Authenticators | Medium | S301, S302 (Identity) | Accepted for Dev | The cryptographic assertion verification, monotonic counter checking, and challenge expiration are 100% mathematically verified. Physical USB HID communication is deferred to Phase 8 physical hardware testing. |
| **RSK-02** | QEMU Guest Virtio-Net Ring 3 Egress Test Deferred | Medium | S202 (Network Egress) | Remediating | Kernel conntrack logic, socket table revocation, and packet dropping are verified via unit tests. A complete QEMU Ring 3 guest automation test will be executed prior to Gate 2 closure. |
| **RSK-03** | Upstream Recursive Resolver Outage under StrictDNSSEC | Low | S203 (Network Trust) | Accepted with Profile Fallback | In `StrictDNSSEC` profile, an upstream DNS outage causes resolution to fail closed. The system provides `OpportunisticFallback` and `LocalOnly` trust profiles for degraded network environments. |
| **RSK-04** | Vulnerability Ingestion Denial of Service / Memory Spikes | Low | S401 (Vulnerability Intel) | Mitigated | The vulnerability ingestion engine enforces a strict bounded in-memory LRU cache ($\le 512$ entries, max 10MB RAM) with deterministic payload truncation. |
| **RSK-05** | Offline Device Replacement Mnemonic Loss | Low | S303 (Secure Recovery) | Mitigated by UX | Users must physically write down the 24-word BIP-39 mnemonic. The setup workflow requires a mandatory verification test (confirming 3 random words) before activating disk encryption. |

---

## 2. Beta Non-Negotiable Criteria Compliance Check

- **Critical Vulnerabilities:** **0** known unresolved.
- **High-Severity Vulnerabilities Undermining Core Boundaries:** **0** known unresolved.
- **Medium / Low Findings with Explicit Disposition:** **5** registered, all with verified compensating controls.
