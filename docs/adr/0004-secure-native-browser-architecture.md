# ADR 0004: Secure Native Browser Architecture & Engine Selection (WEB-CORE-001)

**Status:** Accepted  
**Date:** 2026-10-10  
**Decision Owner:** Lead Architect  
**Technical Stakeholders:** Kernel Platform, Userspace SDK, Security & Capabilities  
**Supersedes:** None  

---

## 1. Context and Problem Statement
INOS requires a first-class native web browser capable of executing modern, high-complexity enterprise applications (Microsoft 365, Google Sheets, Gmail, Adobe Creative Web, banking portals) with strict security, process isolation, local agentic integration, and zero Python/host dependencies. Building an HTML5, CSS3, JavaScript, and WebAssembly engine from scratch is non-viable. We must select a browser engine strategy and define the operating system integration and security boundaries.

---

## 2. Alternatives Considered

### Option A: Servo (Preferred Native-First Engine Candidate)
- **Pros:** Rust-first modular architecture (Stylo CSS, WebRender graphics); memory safety in layout and DOM; clean embeddable WebView API; aligned with INOS freestanding Rust userspace.
- **Cons:** SpiderMonkey (C++) dependency for JS; still developing complete WebAssembly multi-threading, GC, and Service Worker support required for complex cloud suites (Microsoft 365, Photoshop Web).

### Option B: Chromium / Blink / V8 (Gold-Standard Compatibility Benchmark)
- **Pros:** 100% real-world web compatibility; gold standard site isolation and sandbox; flawlessly executes Microsoft 365, Sheets, and Adobe Web. Ozone platform layer provides display/input abstraction.
- **Cons:** Massive footprint (>35M LOC); extensive POSIX syscall dependencies; heavy porting burden to a nascent kernel.

### Option C: Ladybird (Research Alternative)
- **Pros:** Modern C++23, independent architecture, no legacy POSIX baggage, clean OS abstractions from SerenityOS.
- **Cons:** Pre-alpha status with 2026 target for desktop Linux/macOS; lacks enterprise PWA and WebGPU maturity.

---

## 3. Decision Outcome & Rationale
**Selected: Evidence-Based Dual-Track Strategy (WEB-01A & WEB-ENGINE-01).**
1. **Provisional Native Engine Candidate:** Investigate **Servo** via an independent feasibility spike (`WEB-ENGINE-01`) on a supported host to inventory exact OS requirements (SpiderMonkey C++, POSIX/libc, WebAssembly, fonts/graphics). Native portability is provisional, not assumed.
2. **Host Compatibility Benchmark:** Maintain **Chromium on the development host** as the behavioral and rendering reference implementation. An in-tree Chromium port (`ui/ozone/platform/inos`) is explicitly separated and will be evaluated only if Servo proves non-viable.
3. **Rigorous Site Isolation:** Enforce **schemeful sites** (`scheme://eTLD+1`) and **origin-specific isolation** (`scheme://host:port` for authentication/banking) with out-of-process iframes (OOPIF) and ephemeral sandboxes for opaque origins.
4. **Capability Broker Enforcement Authority:** Provenance metadata (`TAINT_UNTRUSTED_WEB`) is tracked for audits, but the **Trusted Capability Broker** is the sole enforcement authority. Web-derived instructions cannot authorize operations or escalate privileges regardless of model interpretation.
5. **Fail-Closed Secure Transport:** Zero bypass modes for invalid TLS certificates or cross-origin access.
6. **No Python Invariant:** All tooling, tests, and build scripts strictly exclude Python.

---

## 4. Verification & Milestones
Governed by program `WEB-CORE-001` milestones `WEB-00` (accepted conditionally), `WEB-ENGINE-01` (engine feasibility spike), `WEB-01A` (native networking), `WEB-01B` (secure transport), and `WEB-02` through `WEB-06` detailed in `docs/INOS-WEB-CORE-001.md`.
