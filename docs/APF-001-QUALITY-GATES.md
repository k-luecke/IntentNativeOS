# Autonomous Product Foundry: Quality Gates & Verification Standards (APF-001)

**Document Identifier:** `APF-001-QUALITY-GATES`  
**Standard of Proof:** Independent automated test harnesses, reproducible QEMU execution logs, byte-for-byte cold-reboot persistence audits, and external reference round-trips. Self-certified claims without test evidence are rejected.

---

## 1. Quality Gates (QG-1 through QG-6)

### Quality Gate 1: Foundation & Architectural Integrity (QG-1)
- [ ] **QG-1.1 (Zero Python):** Full automated audit confirming zero `.py` scripts, build dependencies, or runtime invocations across the monorepo.
- [ ] **QG-1.2 (Reproducible Build):** Clean checkout in isolated worktree builds all Ring 3 applications, SDK, tools, and UEFI kernel image with bit-level reproducibility.
- [ ] **QG-1.3 (Host Virtualization Safety):** QEMU tests run exclusively against isolated synthetic disk images (`disk_*.img`) and firmware variables (`OVMF_VARS.fd`). No host disk modification.

### Quality Gate 2: Functional Correctness & Persistence (QG-2)
- [ ] **QG-2.1 (Files + Archive):** Deflate RFC 1951 decompression/compression matches standard reference bytes; ZIP central directory and local headers pass `unzip -t` and 7-Zip validation.
- [ ] **QG-2.2 (Grid Calculation):** 2D cell DAG resolves cascading dependencies topologically; `#CIRC!` cycle prevention halts infinite loops in \(\le 1\) cycle iteration; CSV and XLSX match RFC 4180 and ECMA-376 standards.
- [ ] **QG-2.3 (Cold Reboot Durability):** All application-modified files written to INOS-FS survive QEMU cold shutdown and reboot, verified by SHA-256 checksums matching pre-shutdown values.

### Quality Gate 3: Negative Security & Boundary Confinement (QG-3)
- [ ] **QG-3.1 (Negative Authorization Matrix):** All syscalls presenting missing tokens, forged signatures, wrong caller PIDs, expired epochs, or out-of-scope paths fail fail-closed (`PermissionDenied`).
- [ ] **QG-3.2 (Zip-Slip & Path Traversal):** Archive extraction attempts containing `../`, leading `/`, or Windows drive letters are intercepted and aborted before disk writes.
- [ ] **QG-3.3 (Decompression Bomb Protection):** Archives with compression ratios exceeding 100:1 or total uncompressed size exceeding memory quotas abort cleanly.
- [ ] **QG-3.4 (Fault Containment):** Faulting Ring 3 applications (#PF, #GP, #UD, infinite loop) are terminated and reclaimed without crashing sibling apps or the kernel supervisor.

### Quality Gate 4: Performance & Resource Footprint (QG-4)
- [ ] **QG-4.1 (16 GiB Hardware Reference):** Total system memory footprint (kernel + all active apps + local intelligence runtime) does not exceed 16 GiB peak RSS.
- [ ] **QG-4.2 (Interactive Latency):** Ring 3 UI event loop processes keyboard and mouse inputs with \(< 16 \text{ ms}\) frame delivery (60 FPS).
- [ ] **QG-4.3 (Model TTFT & Throughput):** Local model inference on x86-64 CPU achieves Time-To-First-Token \(\le 500 \text{ ms}\) and generation speed \(\ge 15 \text{ tok/s}\) for 1.5B/3B tier.

### Quality Gate 5: Cross-Application Interoperability & Rollback (QG-5)
- [ ] **QG-5.1 (Typed Intent Protocol):** Applications communicate exclusively via typed `INTP` capability messages through the OS broker.
- [ ] **QG-5.2 (Reversible Action Ledger):** Multi-application agent operations produce a rollback receipt, allowing atomic undo of file and state changes.
- [ ] **QG-5.3 (External Format Fidelity):** Exported `.xlsx`, `.docx`, and `.pdf` files render cleanly in Microsoft Office, LibreOffice, and Adobe Acrobat without corruption errors.

### Quality Gate 6: Feature-Frozen Release Candidate (QG-6)
- [ ] **QG-6.1 (Zero Blocker Defects):** Zero open P0/P1 defects across functional, security, and stability backlogs.
- [ ] **QG-6.2 (Automated Regression Suite 100% Pass):** All regression suites (`test_app_cap_*.sh`, `test_app_archive_*.sh`, `test_app_grid_*.sh`, `test_everyday_ui_*.sh`) pass in automated sequence.
- [ ] **QG-6.3 (End-to-End Integrated Demonstration):** Successful execution of an unscripted multi-app workflow using realistic enterprise data.
