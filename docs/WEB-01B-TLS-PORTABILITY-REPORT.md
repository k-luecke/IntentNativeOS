# INOS WEB-CORE-001: Milestone WEB-01B — TLS 1.3 Portability & Architectural Feasibility Report

**Document ID**: `INOS-DOC-WEB-01B-PORTABILITY`  
**Milestone**: `WEB-01B` (Gate `WEB-01B.2` Architecture Assessment)  
**Author**: Antigravity Core Team  
**Status**: APPROVED & ADOPTED  

---

## 1. Executive Summary & Architectural Scope

Milestone **WEB-01B** establishes the secure HTTPS transport layer for the native INOS Web Subsystem (`WEB-CORE-001`). In accordance with architectural review decisions, handwritten cryptographic state machines, custom AES-GCM/ChaCha20 implementations, and bespoke X.509 parsers are strictly rejected for production TLS. Instead, INOS adopts a strategy of porting maintained, memory-safe cryptographic and TLS implementations, providing clean operating system abstractions for hardware time, physical entropy, and network I/O.

This report documents the architectural assessment of candidate TLS implementations (`rustls` vs. `Mbed TLS`), details the OS dependency integration layer (CMOS RTC provisional wall clock, monotonic timer, vetted ChaCha20 CSPRNG), and establishes the certificate verification policy required for enterprise browsing (Microsoft 365, Google Workspace, banking services).

---

## 2. Hardware Primitives & OS Integration Layer

A secure TLS stack requires two fundamental platform facilities: **Entropy** (for ephemeral key generation, nonces, and session IDs) and **Time** (for certificate validity verification).

### 2.1. Entropy Harvesting & Vetted CSPRNG (`SYS_GET_RANDOM`)
* **Physical Sources**: Hardware entropy is harvested via `RDRAND` (NIST SP 800-90A/B/C compliant digital random number generator) and `RDSEED` (true physical thermal noise entropy source) where supported by the CPU, supplemented by high-resolution timer jitter from `RDTSC`.
* **Entropy Pool & Threshold**: The kernel maintains a 512-bit entropy accumulator (`EntropyPool`). A strict 256-bit physical entropy threshold must be satisfied before any cryptographic operation is authorized. If the threshold is not reached, `SYS_GET_RANDOM` fails closed (`PermissionDenied` / `DeviceBusy`).
* **CSPRNG State Machine**: Once seeded, the entropy pool drives a standard ChaCha20 stream cipher engine with block-counter increments and periodic re-seeding via timer jitter, ensuring forward secrecy and side-channel resistance.

### 2.2. Provisional Wall-Clock & Monotonic Clocks (`SYS_GET_TIME`)
* **Architectural Reality**: PC/AT CMOS RTC (`0x70`/`0x71`) provides initial calendar date and time. However, CMOS RTC is strictly an **untrusted provisional wall-clock source**: it is unauthenticated, subject to clock drift, CMOS battery exhaustion, and local tampering.
* **Separation of Clocks**:
  * **Provisional Wall Clock (`utc_seconds`)**: Used exclusively for coarse certificate validity window checking (`notBefore` / `notAfter`). Applications are informed that this clock is provisional.
  * **Strictly Monotonic Timer (`monotonic_nanos`)**: Derived directly from hardware `RDTSC` cycle counters, independent of RTC updates. Used for TCP Retransmission Timeouts (RTO), TLS handshake round-trip timers, session expiration, and rate limiting.

---

## 3. TLS Implementation Evaluation: `rustls` vs. `Mbed TLS`

| Architectural Dimension | `rustls` (v0.23+ with `alloc`) | `Mbed TLS` (v3.x C library) | INOS Evaluation Decision |
| :--- | :--- | :--- | :--- |
| **Language & Safety** | 100% Rust (`forbid(unsafe_code)` in core logic). Immune to buffer overflows, use-after-free, and double-free. | C99 codebase. Susceptible to memory management bugs; requires ASLR, stack canaries, and strict compiler hardening. | **`rustls` Strongly Preferred** |
| **no_std / OS Decoupling** | First-class `no_std + alloc` support. Zero POSIX socket, file descriptor, or libc dependencies. | Requires POSIX libc wrapper (`malloc`, `free`, `time`, `socket`, `fcntl`, `errno`). | **`rustls` Strongly Preferred** |
| **Cryptographic Provider** | Modular `CryptoProvider` trait (pluggable `ring`, `aws-lc-rs`, or custom backend). | Monolithic C crypto engine tightly integrated with library headers. | **`rustls` Preferred** |
| **Certificate Verification** | `webpki` parser: strict RFC 5280 / CAB Forum compliance, enforced SAN validation. | Custom X.509 engine: configurable, but historically susceptible to parsing vulnerabilities. | **`rustls` Preferred** |
| **Protocol Support** | Modern focus: TLS 1.2 and TLS 1.3 only. Legacy insecure algorithms (SSL 3.0, TLS 1.0, RC4, 3DES, SHA-1) removed. | Wide legacy support (TLS 1.0 - 1.3). Requires compile-time pruning to disable weak ciphers. | **`rustls` Preferred** |
| **Footprint in Ring 3** | ~400 KB - 800 KB binary footprint with dead-code elimination. | ~300 KB - 600 KB compiled C library. | **Acceptable / Comparable** |

### Decision
INOS selects **`rustls`** as the native TLS engine for `WEB-CORE-001`.

---

## 4. `rustls` OS Adapter Architecture for INOS Ring 3

To port `rustls` into INOS Ring 3 applications without host or Linux dependencies, we establish the following zero-overhead adapter architecture:

```
+--------------------------------------------------------------------+
|                  INOS Native Browser (Ring 3)                      |
|       (HTTP/1.1 & HTTP/2 Client, DOM Engine, Cookie Store)         |
+---------------------------------+----------------------------------+
                                  |
                                  v
+--------------------------------------------------------------------+
|                   rustls (no_std + alloc)                          |
|             ClientConnection / Handshake State Machine             |
+---------------------------------+----------------------------------+
                                  |
            +---------------------+---------------------+
            |                                           |
            v                                           v
+-----------------------+                   +-----------------------+
|  InosCryptoProvider   |                   |  InosTimeProvider     |
| (rustls::crypto)      |                   | (pki_types::UnixTime) |
| - AES-128-GCM / 256   |                   +-----------+-----------+
| - ChaCha20-Poly1305   |                               |
| - X25519 Key Exchange |                               v
| - Ed25519 & RSA PKI   |                   +-----------------------+
| - SecureRandom:       |                   | SYS_GET_TIME (23)     |
|   SYS_GET_RANDOM (25) |                   | - UTC Provisional     |
+-----------+-----------+                   | - RDTSC Monotonic     |
            |                               +-----------------------+
            v
+-----------------------+
| SYS_GET_RANDOM (25)   |
| Hardware Entropy Pool |
+-----------------------+
```

### 4.1. SecureRandom Adapter
Implement `rustls::crypto::SecureRandom`:
```rust
struct InosSecureRandom;

impl rustls::crypto::SecureRandom for InosSecureRandom {
    fn fill(&self, buf: &mut [u8]) -> Result<(), rustls::crypto::GetRandomFailed> {
        inos_app_sdk::get_random(buf)
            .map_err(|_| rustls::crypto::GetRandomFailed)
    }
}
```

### 4.2. Time Adapter
Implement `rustls::pki_types::UnixTime`:
```rust
pub fn inos_current_time() -> rustls::pki_types::UnixTime {
    let (utc_sec, _) = inos_app_sdk::get_time();
    rustls::pki_types::UnixTime::since_unix_epoch(core::time::Duration::from_secs(utc_sec))
}
```

---

## 5. Strict Certificate Validation & Revocation Policy

For enterprise banking, email, and productivity suites, security cannot be relaxed. The INOS certificate validation engine enforces the following requirements:

1. **Subject Alternative Name (SAN) Enforcement**:
   - Validation strictly adheres to RFC 6125 and CAB Forum Baseline Requirements.
   - The `subjectAltName` extension (dNSName and iPAddress) is **mandatory** for host matching.
   - Fallback to Subject Common Name (`CN`) is **explicitly disabled** to prevent name-confusion and impersonation attacks.
2. **Key Usage & Extended Key Usage (EKU)**:
   - Leaf certificates must specify `digitalSignature` or `keyEncipherment` in Key Usage.
   - EKU must contain `id-kp-serverAuth` (`1.3.6.1.5.5.7.3.1`).
3. **Chain Constraints & Trust Anchors**:
   - Intermediate certificates must possess `basicConstraints` with `cA=TRUE` and valid `pathLenConstraint`.
   - Root certificates are anchored in a cryptographic store packaged in the browser application image or verified from an authenticated INOS-FS path (`/system/etc/security/cacerts.pem`) accessible via read-only capability tokens.
4. **Revocation Architecture**:
   - Online verification roadmap: Prioritizes **OCSP Stapling** (`status_request` TLS extension) verified during the handshake.
   - CRL Distribution Point parsing will be provided for enterprise-enrolled environments.

---

## 6. Gate Completion & Road to WEB-01B.3

* **Gate WEB-01B.1 (DNS & Reliable TCP)**: **PASSED**. Bounded RFC 1035 parser, transaction matching, pointer-loop mitigation, TCP sequence/window validation, and clean FIN-ACK teardown are verified.
* **Gate WEB-01B.2 (Time, Entropy & TLS Portability)**: **PASSED**. CMOS RTC wall clock, monotonic timer, vetted ChaCha20 CSPRNG (256-bit entropy threshold), and the `rustls` portability architecture are verified.
* **Gate WEB-01B.3 (Authenticated HTTPS)**: Authorized to proceed with integrating `rustls` into the Ring 3 network runtime.
