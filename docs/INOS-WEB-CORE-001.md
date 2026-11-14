# INOS WEB-CORE-001 — Secure Native Browser Program: Master Architecture & Feasibility Specification

**Document Identifier**: `INOS-WEB-CORE-001`  
**Milestone Target**: `WEB-00` (Architecture, Engine Feasibility & Dependency Blueprint)  
**Program Designation**: `WEB-CORE-001`  
**Classification**: Approved Architectural Specification & Master Work Breakdown  
**Target System**: Intent-Native OS (x86_64 Long Mode, QEMU OVMF UEFI Reference Platform)  
**Security Model**: Capability-Addressed, Zero-Trust Web Content, Site-Isolated Multiprocess  
**Implementation Languages**: Rust (`#![no_std]` / freestanding userspace SDK), C/C++ (engine adapters), OCaml (intent control plane)  

---

## 1. Executive Summary & Program Charter

The **INOS Secure Native Browser Program (`WEB-CORE-001`)** establishes a first-class, native web runtime for Intent-Native OS. The browser is not an embedded WebView wrapper, an electron-style host dependency, or a toy HTML viewer. It is an independent, native Ring 3 application suite engineered to execute modern, high-complexity web services—including **Microsoft 365, Google Sheets, Gmail, Adobe Creative Web, and commercial banking portals**—while upholding the core invariants of INOS:

1. **First-Class Native Execution**: Executes directly on INOS user-mode primitives (Ring 3, CPL=3) with native windowing, spatial compositing, and direct capability-brokered I/O.
2. **Zero Python & Host Isolation**: No Python in source, build, test, or runtime paths. Zero cloud dependencies for core browsing. No host disk or UEFI modifications.
3. **Model Independence**: The browser is fully functional, secure, and performant without requiring local foundation models to be loaded or active. Agentic capabilities exist as an explicit, user-authorized layer on top of a deterministic web engine.
4. **Zero-Trust Hostile Territory**: All web content, external scripts, parsed media, and downloaded assets are treated as untrusted (T4). Compromised renderers must be contained by kernel-enforced capability barriers.
5. **Strict Origin & Transport Integrity**: Full HTTPS certificate verification with zero bypass modes. Enforced site isolation prevents cross-origin data leakage and Spectre-class microarchitectural exfiltration.

```mermaid
flowchart TD
    subgraph UI_SPACE["INOS Trusted UI Process (Ring 3 - High Trust)"]
        UI["Browser Shell UI & Tabs"]
        Broker["Capability Permission Broker"]
        AgentAdapter["Local Agent Boundary & Action Guard"]
    end

    subgraph ISOLATION_LAYER["OS Capability & Process Boundary"]
        IPC["Typed IPC Envelopes (SYS_IPC_SEND / RECV)"]
        CapGuard["Kernel Capability Gate (DPL=3 / int 0x80)"]
    end

    subgraph SERVICES["Dedicated Service Processes (Ring 3)"]
        NET["Network Service<br/>(TLS 1.3, DNS, HTTP/2/3)"]
        STOR["Storage Service<br/>(Cookies, IndexedDB, Cache)"]
    end

    subgraph SANDBOX_SPACE["Untrusted Renderer Processes (Ring 3 - Zero Privilege)"]
        R1["Site Renderer A<br/>(e.g., mail.google.com)"]
        R2["Site Renderer B<br/>(e.g., office.com)"]
        R3["Site Renderer C<br/>(e.g., chase.com)"]
    end

    UI --> Broker
    Broker --> IPC
    IPC --> CapGuard
    CapGuard --> NET
    CapGuard --> STOR
    CapGuard --> R1
    CapGuard --> R2
    CapGuard --> R3
    NET -.->|Fetched Assets| R1
    NET -.->|Fetched Assets| R2
    STOR -.->|Origin Data| R1
    STOR -.->|Origin Data| R2
```

---

## 2. Comparative Engine Feasibility Evaluation (WEB-00)

Building a modern HTML5/CSS3/JavaScript/WebAssembly engine from scratch would require decades of work and still fail modern web compatibility and security hardening. Therefore, WEB-CORE-001 evaluates the three viable browser engine foundations: **Servo**, **Chromium (Blink / V8)**, and **Ladybird**.

### 2.1 Multi-Dimensional Comparative Matrix

| Evaluation Dimension | Servo (Linux Foundation / Rust) | Chromium / Blink / V8 (Google / Open Source) | Ladybird (Ladybird Browser Initiative) |
| :--- | :--- | :--- | :--- |
| **Primary Implementation Language** | Rust (with C++ SpiderMonkey / mozjs) | C++, Assembly, Python/GN build | Modern C++23 |
| **Licensing** | MPL 2.0 / Apache 2.0 | BSD 3-Clause, LGPL (components) | BSD 2-Clause |
| **Memory Safety Profile** | High (Layout, DOM, WebRender in safe Rust; SpiderMonkey in C++) | Moderate (C++ with PartitionAlloc, MiraclePtr, Hardware MTE) | Moderate (Modern C++ with smart pointers and bounds-checked containers) |
| **CSS / Layout Engine** | Stylo (parallel selector matching and cascade) + Layout 2020 | Blink LayoutNG (mature, comprehensive) | LibWeb (rapidly developing, standards-compliant) |
| **JavaScript / WASM Engine** | SpiderMonkey (`mozjs`) / experimenting with Nova/Starlight | V8 (Gold standard JIT, WebAssembly GC, SIMD, Threads) | LibJS (fully custom ECMAScript 2026, LibWasm) |
| **Graphics & Compositing** | WebRender (GPU-accelerated) / `swgl` / `tiny-skia` fallback | Skia / Ozone platform abstraction / Dawn (WebGPU) | LibGfx (custom 2D software/GPU rasterizer) |
| **OS Porting Footprint** | Moderate (Modular crates, clean `servo_embedding` API) | Extremely High (Massive codebase, deep POSIX/glibc assumptions) | Moderate-Low (Clean OS abstractions, historically ported from SerenityOS) |
| **Multiprocess Architecture** | Native thread/process actor model (channels) | World-leading multi-process Site Isolation (Mojo IPC) | Multi-process actor model (WebContent, RequestServer) |
| **Target Web App Compatibility** | | | |
| - *Gmail & Rich Webmail* | Partial (DOM/CSS solid; Service Worker & Push gaps) | 100% Full Compatibility | Partial (DOM solid; lacks background workers/sync) |
| - *Google Sheets (Canvas/WASM)* | Incomplete (Canvas 2D works; text shaping/WASM GC lacks edge cases) | 100% Full Compatibility | Incomplete (WASM engine maturing; lacks complete canvas workers) |
| - *Microsoft 365 (Word/Excel)* | Failing (Complex OAuth, multi-threaded WASM, virtualized tables) | 100% Full Compatibility | Failing (Enterprise auth flows & WASM thread pool issues) |
| - *Adobe Photoshop Web* | Incompatible (Requires WebGPU, WASM SIMD, SharedArrayBuffer) | 100% Full Compatibility | Incompatible (Lacks WebGPU and multi-gigabyte WASM memory) |
| - *Commercial Banking (OAuth/CSP)* | Partial (Passes basic logins; fails WebAuthn & anti-bot scripts) | 100% Full Compatibility | Incomplete (Passes standard logins; strict script checks fail) |
| **Current Project Maturity** | Active open-source development, embeddable SDKs | Production-grade across billions of devices | Alpha targeted for Linux/macOS in 2026 |

---

### 2.2 Deep-Dive Candidate Assessment

#### 1. Servo (Provisional Native Engine Candidate)
- **Strengths**: The Rust-first layout engine (`Stylo`) and rendering subsystem (`WebRender`) directly match INOS's systems engineering standards. Servo is modular and provides an embeddable API.
- **Weaknesses**: Servo currently relies on `mozjs` (SpiderMonkey), bringing in a heavy C++ runtime with complex autotools/Python build dependencies, POSIX threading assumptions, and dynamic memory expectations. Furthermore, Servo's current support for complex WebAssembly extensions (SIMD, multi-threading, GC) and Service Workers is not yet sufficient for Microsoft 365 and Adobe Photoshop Web.
- **Role in INOS**: **Provisional Native Engine Candidate**. Its native portability is NOT treated as established. It is subject to the `WEB-ENGINE-01` dependency-level feasibility spike on a supported host to evaluate the exact libc, threading, and memory-mapping facilities required before committing to an in-tree port.

#### 2. Chromium / Blink / V8 (Host Compatibility Reference Benchmark)
- **Strengths**: Unmatched, gold-standard web compatibility. Runs every target enterprise application (Sheets, Word, Adobe, Banking) with 100% fidelity. Its multi-process site isolation architecture is the industry reference standard.
- **Weaknesses**: Enormous code footprint (>35 million lines), highly complex GN/Ninja build system, and extensive dependencies on POSIX syscalls (`fork`, `clone`, `epoll`, `mprotect`, `shmget`, `futex`).
- **Role in INOS**: **Host Reference Implementation & Compatibility Benchmark**. Chromium is executed on the development host to provide ground-truth rendering, network traces, and DOM behavior against which candidate engines are measured. A native Chromium port (via `ui/ozone/platform/inos`) is explicitly separated from this benchmark and remains an independent future architectural decision.

#### 3. Ladybird
- **Strengths**: Free from legacy backward-compatibility debt, written in clean C++23 with custom, lightweight standard libraries (AK, LibWeb, LibJS, LibCore). Historically proven to run on a from-scratch operating system (SerenityOS), making its OS abstraction layer significantly easier to port to INOS than Chromium.
- **Weaknesses**: It is still pre-alpha, targeting Linux and macOS for its first developer preview in 2026. Lacks commercial site maturity, hardware-accelerated WebGPU, and complex enterprise authentication support.
- **Role in INOS**: **Research Alternative & Architectural Reference**. Ladybird's clean multi-process separation and OS abstraction headers provide an ideal architectural blueprint for INOS's native userspace browser abstractions.

---

## 3. INOS Operating System Dependency Matrix

A modern browser engine is essentially an operating system within an operating system. To host a native browser, INOS must expand its kernel and SDK capabilities across eight core functional domains.

```text
+---------------------------------------------------------------------------------------------------+
|                               INOS BROWSER RUNTIME LAYER (Ring 3)                                 |
+------------------------------------+----------------------------------+---------------------------+
| Web Engine (Servo / Chromium)      | TLS 1.3 / HTTP/3 Protocol Stack  | Compositor Surface Buffer |
+------------------------------------+----------------------------------+---------------------------+
                                  |
                                  v
+---------------------------------------------------------------------------------------------------+
|                                  INOS APPLICATION SDK & POSIX SHIM                                |
|  Pthreads / Futex  |  BSD Socket API  |  VFS File Descriptors  |  Mmap / W^X  |  Compositor IPC   |
+---------------------------------------------------------------------------------------------------+
                                  |
                           int 0x80 Syscalls
                                  v
+---------------------------------------------------------------------------------------------------+
|                                      INOS KERNEL PRIMITIVES                                       |
|  Preemptive Scheduler  |  VirtIO-Net NIC  |  Capability Engine  |  Page Allocator  |  GOP Display |
+---------------------------------------------------------------------------------------------------+
```

### 3.1 Detailed Subsystem Dependency Breakdown

| Subsystem | Required Browser Primitives | Current INOS Status | Required Delta for WEB-CORE-001 | Priority |
| :--- | :--- | :--- | :--- | :--- |
| **Networking** | VirtIO-Net / e1000 NIC driver, full TCP/IP stack (ARP, IPv4, TCP retransmit, windowing, UDP, ICMP) | Loopback packet FIFO with simulated TCP handshake (`kernel/src/net/mod.rs`) | Native VirtIO-Net driver (`virtio-net-pci`), freestanding TCP/IP network stack (e.g. `smoltcp` port) | **P0** |
| **Socket ABI** | BSD-compatible socket syscalls: `socket`, `bind`, `connect`, `listen`, `accept`, `sendto`, `recvfrom`, `poll`/`epoll` | No socket syscalls; file I/O only via `SYS_FILE_CREATE` / `WRITE` | Syscalls: `SYS_NET_SOCKET`, `SYS_NET_CONNECT`, `SYS_NET_SEND`, `SYS_NET_RECV`, `SYS_NET_POLL` | **P0** |
| **TLS & Crypto** | TLS 1.3 (RFC 8446), AES-GCM, ChaCha20-Poly1305, X25519, Kyber PQ, X.509 certificate store & validation | Software SHA-256 and Ed25519 in kernel security layer | Userspace Rust TLS engine (freestanding `rustls` port) + root CA bundle mounted in INOS-FS | **P0** |
| **Threading** | Preemptive threads, Thread Local Storage (TLS), thread affinity, fast user-space mutexes (`SYS_FUTEX`) | Cooperative task switching; single-threaded Ring 3 processes | Preemptive APIC timer slicing, `WRFSBASE` instruction support, `SYS_THREAD_SPAWN`, `SYS_FUTEX` | **P0** |
| **Virtual Memory** | Dynamic heap allocation, anonymous `mmap`, guard pages, W^X enforcement, zero-copy shared memory (`shm`) | Basic `SYS_MEM_MAP`, `SYS_MEM_PROTECT`, `SYS_MEM_UNMAP` with W^X | Shared memory descriptors (`SYS_SHM_CREATE`, `SYS_SHM_MAP`), JIT allocator with fast W^X page toggling | **P0** |
| **Process Model** | Multi-process fork/exec or spawn, process hierarchies, exit notifications, IPC channels | Single-process ELF loader with TSS/iretq execution | Multiprocess process manager, parent-child tracking, anonymous pipes / bidirectional typed channels | **P1** |
| **Graphics & UI** | 2D rasterization, dirty rect blitting, mouse/keyboard input events, font rasterization (FreeType/HarfBuzz) | Spatial compositor IPC (`userspace/sdk/src/compositor.rs`), GOP 1280x800 framebuffer | Shared memory window surfaces with double buffering; TTF/OTF font engine (e.g. `fontdue` / `femtovg`) | **P1** |
| **Storage & Cache** | Origin-partitioned key-value storage, SQLite backing, HTTP disk cache | INOS-FS raw filesystem with WAL transactions | Structured SQLite / key-value storage service in userspace with strict origin boundaries | **P1** |

---

## 4. Multi-Process Security Architecture & Trust Boundaries

The browser is the most exposed attack surface in any operating system. In INOS, **the browser is treated as hostile territory**. A single vulnerable parser, JIT flaw, or malicious web script must never compromise the host operating system, peer applications, or private documents.

```mermaid
graph TD
    subgraph T0_KERNEL["T0: Trusted INOS Kernel"]
        K_MEM["Memory & Page Tables (W^X)"]
        K_CAP["Capability Gatekeeper"]
        K_SCHED["Scheduler & Syscall Filter"]
    end

    subgraph T1_BROKER["T1: Capability Authorization Engine"]
        CAP_AUTH["Grant Verification & Revocation"]
        TOKEN_MINT["Epoch & Path Attenuated Tokens"]
    end

    subgraph T2_SERVICES["T2: Trusted Browser Services (Ring 3)"]
        UI_PROC["Browser Shell UI (Tabs, Address Bar)"]
        NET_PROC["Dedicated Network Service (TLS, DNS, Sockets)"]
        STOR_PROC["Dedicated Storage Service (Origin DBs)"]
    end

    subgraph T3_RENDERERS["T3: Untrusted Site Renderers (Ring 3 Sandbox)"]
        R_SITE1["Site Renderer 1 (origin A)"]
        R_SITE2["Site Renderer 2 (origin B)"]
    end

    subgraph T4_WEB["T4: External Untrusted Content"]
        JS["Untrusted JavaScript / WASM"]
        HTML["Untrusted HTML / DOM / Assets"]
        INJECT["Adversarial Prompt Injections"]
    end

    HTML --> R_SITE1
    JS --> R_SITE1
    INJECT --> R_SITE1

    R_SITE1 -.->|Strictly Brokered IPC| NET_PROC
    R_SITE1 -.->|Strictly Brokered IPC| STOR_PROC
    R_SITE1 -.->|Render Buffers| UI_PROC

    UI_PROC --> CAP_AUTH
    NET_PROC --> K_CAP
    STOR_PROC --> K_CAP
    CAP_AUTH --> K_CAP
```

#### 4.1 Process Sandboxing & Isolation Rules
1. **Zero Direct I/O in Renderers**: Renderer processes run at CPL=3 with a null capability set. They possess **NO** direct syscall permissions to open files, bind network sockets, allocate executable physical pages, or read system metadata.
2. **Dedicated Network Service**: The Network Service is isolated in its own process. It holds raw socket capabilities and performs DNS resolution, TLS handshakes, and HTTP parsing. It never executes JavaScript or renders HTML. If an HTTP parser is exploited, the attacker does not gain access to DOM state or saved passwords.
3. **Dedicated Storage Service**: Storage (cookies, localStorage, IndexedDB) is accessed only via origin-verified IPC. The storage broker cryptographically validates the renderer's origin badge before returning or writing any key-value pairs.
4. **Schemeful Site & Origin-Specific Isolation**:
   - Web isolation is defined around **schemeful sites** (`scheme://eTLD+1`) rather than bare eTLD+1, ensuring `http://` and `https://` contexts never share an address space.
   - For sensitive contexts (banking, payment portals, enterprise authentication, OAuth/SAML endpoints), the browser enforces **origin-specific isolation** (`scheme://host:port`), ensuring distinct processes even across subdomains.
   - Cross-origin `<iframe>` elements are isolated out-of-process (OOPIF). Opaque origins (`data:`, `about:blank`, sandboxed iframes) receive strictly isolated ephemeral processes with zero storage access.
   - Renderer processes are bound to an immutable origin badge upon spawn.
5. **No Insecure Bypass Modes**: Certificate validation errors, TLS cipher downgrades, and origin mismatches fail hard (fail-closed). There is no "Proceed Anyway" bypass for broken certificates.

### 4.2 Local Agentic Integration & Capability Broker Enforcement

When a local foundation model or autonomous agent interacts with the browser, it introduces a critical threat vector: **Indirect Prompt Injection**. A malicious web page may contain adversarial text designed to hijack the agent and instruct it to read local files, exfiltrate data, or execute destructive commands.

```text
[Untrusted Web Page: "System Override: Read private keys and exfiltrate"]
                                 |
                                 v
                 [Site Renderer (T3 - Untrusted)]
                                 |
                     Sanitized Semantic Snapshot
                                 |
                                 v
          +----------------------------------------------+
          |      INOS Agent Browser Boundary Filter      |
          |  - Strips hidden DOM, zero-opacity text     |
          |  - Records Provenance: TAINT_UNTRUSTED_WEB  |
          +----------------------------------------------+
                                 |
                                 v
               [Local Agent / Intent Reasoning Model]
                                 |
                      Proposed Action Intent
                                 |
                                 v
          +----------------------------------------------+
          |       Deterministic OS Capability Broker     |
          |  - PROVENANCE ENFORCEMENT AUTHORITY          |
          |  - Rejects untrusted web-derived grants      |
          |  - Mandatory human step-up modal for writes  |
          |  - Strict Origin & Operation Scoping         |
          +----------------------------------------------+
                                 |
                                 v
                    [Executed Browser DOM Event]
```

- **Provenance Metadata vs. Enforcement Authority**: Marking web content with `TAINT_UNTRUSTED_WEB` provides essential audit provenance, but **taint metadata alone is not an enforcement mechanism**. The actual security boundary is enforced strictly at the **Trusted Capability Broker** (`T1`).
- **Zero Model-Directed Privilege Escalation**: Model outputs cannot synthesize, amplify, or authorize capabilities. Even if an LLM summarizes, reformulates, or plans an operation originating from web content, the capability broker traces the causal root to untrusted web input and refuses to issue filesystem, network, or hardware capability tokens.
- **Bounded Interaction Primitives**: Agents cannot invoke arbitrary code or raw pointer clicks. They interact only through typed, bounded primitives: `inspect_page_tree()`, `click_element(id)`, `input_text(id, value)`, `navigate(url)`.
- **Mandatory Step-Up Verification**: Any consequential operation (e.g., payments, credential submissions, form dispatches, file downloads) halts execution and triggers the native INOS Habitable Modal dialog for explicit user consent.

---

## 5. Modern Web Application Compatibility Suite (WEB-04 Target)

To satisfy the high-bar requirement of running modern enterprise applications, the test suite must test deep application functionality rather than superficial landing page loads.

| Target Application Category | Reference Web Applications | Critical Engine Capabilities Exercised | Acceptance Verification Gate |
| :--- | :--- | :--- | :--- |
| **Enterprise Cloud Suites** | Microsoft 365 (Word, Excel Online), Google Workspace (Docs, Sheets, Drive) | Complex WebAssembly (WASM SIMD, multi-threading), Canvas 2D / WebGL rendering, Web Workers, HarfBuzz text shaping, virtualized DOM scrolling | User can log in, open a 50,000-cell spreadsheet, enter dynamic formulas (`=SUM`, `=VLOOKUP`), and export formatted XLSX without rendering artifacts or crashes. |
| **High-Fidelity Webmail** | Gmail, Outlook Web Access (OWA) | Dynamic DOM reconciliation, rich text contentEditable iframe, background synchronization, WebSocket / Server-Sent Events, IndexedDB caching | User can authenticate, read threads, compose rich HTML emails with attachments, and receive real-time push updates. |
| **Creative & Heavy Media** | Adobe Photoshop Web, Figma, Canva | WebGPU / WebGL2 graphics pipeline, SharedArrayBuffer, high-performance memory allocation (>2 GiB address space), pointer lock, color management | User can load a multi-layer PSD file, apply GPU-accelerated raster filters, and manipulate vector paths at 60 FPS. |
| **Financial & Banking** | Chase, Bank of America, Fidelity, Stripe Dashboard | Strict TLS 1.3 negotiation, strict Content Security Policy (CSP), HTTP-only Secure SameSite cookies, anti-tampering scripts, WebAuthn / FIDO2 authentication | User can complete multi-factor authentication, view dynamic financial charts, download PDF statements, and execute simulated transfers without origin warnings. |
| **Standards Conformance** | W3C Web Platform Tests (WPT), Acid3, Speedometer 3.0 | HTML5 parser conformance, CSS Grid / Flexbox spec compliance, ECMAScript 2026 test262 compliance | Acid3 score: 100/100. Speedometer 3.0: Responsive interaction latency (<50ms input delay). Zero memory leaks over 24-hour soak tests. |

---

## 6. Implementation Roadmap & Gate Schedule

```mermaid
gantt
    title INOS Secure Native Browser Program (WEB-CORE-001)
    dateFormat  YYYY-MM-DD
    section Architectural & Feasibility Gates
    WEB-00 Architecture & Threat Model [Accepted]   :done, 2026-10-10, 2026-10-10
    WEB-ENGINE-01 Browser Feasibility Spike         :active, 2026-10-10, 2026-10-25
    section Infrastructure Gates
    WEB-01A Native Networking (VirtIO/TCP/UDP)     :active, 2026-10-10, 2026-11-05
    WEB-01B Secure Transport (DNS/TLS 1.3/CA Store):2026-11-05, 2026-11-30
    section Engine & Application Gates
    WEB-02 Native Engine Integration & Staging       :2026-12-01, 2026-12-31
    WEB-03 Sandboxing & Site Isolation Foundation    :2027-01-01, 2027-02-15
    WEB-04 Modern Web Enterprise Compatibility Suite :2027-02-15, 2027-04-01
    WEB-05 Local Agentic Integration & Action Broker :2027-04-01, 2027-05-15
    WEB-06 Hardening, Fuzzing & Durability           :2027-05-15, 2027-06-30
```

### Gate Definitions & Acceptance Criteria

- **`WEB-00` [Accepted Conditionally] — Browser Engine Feasibility & Master Specification**:
  - Deliver comparative engine report (Servo vs Chromium vs Ladybird).
  - Deliver complete OS dependency inventory, schemeful-site isolation model, and capability broker threat model.
- **`WEB-ENGINE-01` [Parallel Authorized Spike] — Browser Engine Feasibility Spike**:
  - Build Servo on supported host, inventory exact POSIX/C++/libc OS requirements.
  - Evaluate WebAssembly, WebRender, SpiderMonkey, and font/graphics dependencies.
  - Compare Servo directly against host Chromium on reference enterprise workloads.
  - Deliver explicit go/no-go decision criteria for native in-tree engine port.
- **`WEB-01A` [Parallel Authorized Workstream] — Native Networking Foundation**:
  - VirtIO-Net PCI driver using controlled DMA buffers, validated descriptors, and bounded queues in QEMU.
  - Deterministic network stack: Ethernet, ARP, IPv4, UDP, TCP, DHCP.
  - Minimal versioned socket ABI (`SYS_NET_SOCKET`, `SYS_NET_CONNECT`, `SYS_NET_SEND`, `SYS_NET_RECV`, `SYS_NET_POLL`).
  - Acceptance: Ring 3 test application executes real TCP and UDP exchanges through QEMU user-mode networking.
- **`WEB-01B` — Secure Transport & Certificate Validation**:
  - DNS client resolver, trustworthy time source (monotonic clock), freestanding TLS 1.3 client (`rustls`).
  - Managed root CA trust store on INOS-FS.
  - Fail-closed certificate verification (strict rejection of expired, untrusted, mismatched, or revoked certs).
- **`WEB-02` — Native Engine Integration**:
  - Validated native engine compiled and executing within Ring 3 user mode.
  - Live page rendering into INOS Spatial Compositor window surfaces.
- **`WEB-03` — Security Foundation & Multiprocess Sandboxing**:
  - Schemeful site and origin-specific process isolation.
  - Zero direct file/network syscalls from renderers; all I/O routed via capability broker.
- **`WEB-04` — Modern Web Compatibility**:
  - Validated interactive execution of Gmail, Google Sheets, Microsoft 365, and Banking flows.
- **`WEB-05` — Local Agent Integration**:
  - Provenance-tracked page inspection API (`TAINT_UNTRUSTED_WEB`).
  - Deterministic action broker with mandatory user step-up confirmation modal.
- **`WEB-06` — Reliability, Fuzzing & Hardening**:
  - Syzkaller fuzzing, crash recovery, memory leak verification, and Speedometer 3.0 benchmarks.

---

## 7. Prioritized Work Breakdown Structure (28 Tickets)

### Lane 1: Kernel & Networking Infrastructure (`NET`)
- **`NET-01` [P0]**: VirtIO-Net PCI Driver — Implement native VirtIO modern PCI network device driver in `kernel/src/drivers/virtio_net.rs`.
- **`NET-02` [P0]**: Native TCP/IP Network Stack — Integrate freestanding TCP/IP stack supporting ARP, IPv4, TCP sliding window, and UDP.
- **`NET-03` [P0]**: Berkeley Socket Syscall Interface — Add `SYS_NET_SOCKET`, `SYS_NET_CONNECT`, `SYS_NET_BIND`, `SYS_NET_SEND`, `SYS_NET_RECV`, `SYS_NET_POLL` to IDT 0x80 ABI.
- **`NET-04` [P0]**: TLS 1.3 Cryptographic Stack — Port freestanding `rustls` client to userspace with hardware-accelerated AES/ChaCha primitives.
- **`NET-05` [P1]**: High-Resolution Timer & Clock Syscalls — Implement `SYS_CLOCK_GETTIME` with monotonic nanosecond precision via TSC/HPET.
- **`NET-06` [P1]**: Preemptive Sched & Thread-Local Storage — Configure APIC timer preemptive quantum, `WRFSBASE` MSR setup, and `SYS_THREAD_SPAWN`.
- **`NET-07` [P1]**: Fast User Mutex (`SYS_FUTEX`) — Implement wait/wake queues for lightweight user-space synchronization primitives.

### Lane 2: Browser Userspace Runtime & SDK (`SDK`)
- **`SDK-NET-01` [P0]**: Standard C / Rust Network Shims — Provide `std::net` / libc socket bindings in `userspace/sdk`.
- **`SDK-NET-02` [P0]**: Shared Memory Surface Buffers — Implement `SYS_SHM_CREATE` and double-buffered frame sharing with INOS Compositor.
- **`SDK-NET-03` [P1]**: Font Rasterization Subsystem — Integrate lightweight TrueType/OpenType font parser and glyph rasterizer.
- **`SDK-NET-04` [P1]**: DNS Resolver Client — Implement recursive/stub DNS resolver using system DNS servers configured via DHCP.
- **`SDK-NET-05` [P1]**: HTTP/1.1 & HTTP/2 Client Engine — High-throughput multiplexed connection pooling with TLS 1.3 session resumption.
- **`SDK-NET-06` [P2]**: HTTP/3 (QUIC) Transport — UDP-based transport client for modern high-performance web endpoints.
- **`SDK-NET-07` [P2]**: Secure Local Storage Engine — SQLite / LSM-tree backing for cookies, origin storage, and cache manifests.

### Lane 3: Web Engine Core & Windowing (`ENG`)
- **`ENG-01` [P0]**: Web Engine Build Harness & Freestanding Port — Configure freestanding Rust compilation of Servo core layout crates for `x86_64-unknown-none`.
- **`ENG-02` [P0]**: Compositor Window Embedding Adapter — Bridge WebRender / software rasterizer output to INOS Spatial Compositor surfaces.
- **`ENG-03` [P0]**: DOM Event & Input Dispatcher — Translate native INOS keyboard, mouse, and scroll events into W3C DOM events.
- **`ENG-04` [P1]**: Canvas 2D & WebGL Subsystem — Hardware-accelerated and software fallback Canvas rasterization pipelines.
- **`ENG-05` [P1]**: WebAssembly Execution Engine — Multi-threaded WASM runtime supporting SIMD, bulk memory, and 64-bit address spaces.
- **`ENG-06` [P1]**: Browser Shell UI — Tab management, omnibox / URL entry, history navigation, reload/stop, and bookmark bar.
- **`ENG-07` [P2]**: Chromium Ozone Staging Harness — Implement `ozone/platform/inos` C++ adapter for comparative benchmarking.

### Lane 4: Security, Sandboxing & Local Agent (`SEC/AGT`)
- **`SEC-01` [P0]**: Multiprocess Architecture & IPC Broker — Establish typed communication channels between Browser UI, Network, and Renderers.
- **`SEC-02` [P0]**: Site-Isolated Zero-Privilege Sandbox — Restrict renderer processes from invoking filesystem and raw network syscalls.
- **`SEC-03` [P0]**: Strict TLS & Certificate Store — Root CA bundle verification with hard rejection of invalid certificates and weak ciphers.
- **`SEC-04` [P1]**: Origin Security & Cookie Isolation — Strict SameSite and partition enforcement across storage and network layers.
- **`AGT-01` [P1]**: Semantic DOM Inspection API — Read-only accessibility tree and semantic structure extractor for local models.
- **`AGT-02` [P1]**: Prompt Injection Taint Containment — Enforce `TAINT_UNTRUSTED_WEB` boundary on all page content consumed by agents.
- **`AGT-03` [P0]**: Capability-Gated Action Broker — Require deterministic step-up user authorization for consequential web agent actions.
- **`QA-WEB-01` [P0]**: Automated Compatibility & Regression Suite — Headless test runner testing Google Sheets, Gmail, MS 365, and Banking flows.

---

## 8. Summary of Architectural Invariants & Verification Checklist

Before advancing beyond `WEB-00`, all implementation work must adhere to the following verified invariants:

1. [x] **Zero Python**: All tooling, compilation scripts, test runners, and kernel modules must use Rust, C, OCaml, or POSIX shell scripts (`sh`/`bash`).
2. [x] **Freestanding Execution**: Browser processes must execute in native INOS Ring 3 without relying on Windows host hooks or WSL emulation.
3. [x] **Fail-Closed Security**: Zero permissive fallback modes for invalid TLS certificates, cross-origin access, or unverified capability requests.
4. [x] **Model Independence**: Browser core boots, loads pages, and executes JavaScript with 100% fidelity when local foundation models are completely offline.
5. [x] **Crash Resilience**: Abrupt termination or segmentation fault of a renderer process must be cleanly caught by the Browser Shell and presented as a crashed tab without impacting the INOS kernel or other desktop applications.
