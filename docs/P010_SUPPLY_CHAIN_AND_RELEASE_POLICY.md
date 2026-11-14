# P010: Supply-Chain Integrity and Release Policy

**Task ID:** P010  
**Gate:** G0 (Foundational Controls)  
**Status:** Approved Standard  

---

## 1. Supply-Chain Security Principles

The Intent-Native Operating System executes within zero-trust, high-assurance environments. To prevent supply-chain compromises, dependency hijacking, or untracked binary modifications, all build pipelines must comply with this policy.

---

## 2. Policy Mandates

### 2.1 Complete Pinning of External Dependencies
- All compiler toolchains (OCaml, Dune, Rustc, Cargo, QEMU, OVMF) MUST be pinned with exact versions in `toolchain.lock.json`.
- Arbitrary unpinned package manager commands (`opam install <pkg>`, `cargo install <pkg>`, `apt-get install <pkg>`) without version pinning are forbidden in CI and release scripts.
- Python is banned as a build, test, or runtime dependency.

### 2.2 Content-Addressed Provenance & Checksums
- Every release artifact and intermediate build output MUST produce an immutable SHA-256 manifest.
- Checksums MUST match `intent_os_source_manifest.json` before any promotion across gates.

### 2.3 Hermetic & Reproducible Builds
- Builds must be bit-for-bit reproducible across independent machines using identical toolchain lockfiles or container images (`Dockerfile`).
- Embedded timestamps, non-deterministic filesystem traversal orders, and randomized hash seeds must be normalized during compilation.

### 2.4 Cryptographic Release Signing
- Kernel binaries (`.efi`), microkernel images, and OCaml control plane packages must be cryptographically signed by the release authority key.
- Algorithm agility: Ed25519 for current releases, with planned migration path to post-quantum signatures (ML-DSA / Dilithium) per invariant **I-10**.

### 2.5 License & Compliance Auditing
- Open-source licenses for all incorporated components must be cataloged in a machine-readable notice.
- Kernel code: MIT / Apache 2.0.
- OCaml Control Plane: MIT / LGPL with linking exception.
- No GPLv3 code may link into T0 or T1 without explicit legal review and architectural isolation.
