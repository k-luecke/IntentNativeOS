# ADR 0001: Strict No-Python Architecture: OCaml Control Plane and Rust Kernel

**Status:** Accepted  
**Date:** 2026-10-09  
**Decision Owner:** Lead Architect  
**Technical Stakeholders:** Intent Compiler, Kernel Platform, CI  
**Supersedes:** None  

---

## 1. Context and Problem Statement
Early scaffolding considerations suggested using Python for build tooling, test harnesses, and host glue code. However, the system architecture mandates an OCaml-first semantic control plane (Typed Intent IR, clarification state machine, model coordination) and a memory-safe custom kernel. Introducing Python introduces a second high-level runtime, a vast uncontrolled dependency graph, non-reproducible interpreter packaging, and high memory overhead that conflicts with the 16 GiB reference target.

---

## 2. Alternatives Considered

### Option A: Python Scaffolding + OCaml + Rust + C
- **Pros:** Fast initial throwaway scripts.
- **Cons:** Two high-level languages to maintain; packaging headaches; Python interpreter cannot be easily self-hosted in the custom microkernel; violates model independence and small footprint invariants.

### Option B: OCaml Control Plane + Rust Kernel (Selected)
- **Pros:**
  - Single high-level typed language (OCaml 5.x) for the entire user space and semantic control plane.
  - Native code compilation via `ocamlopt` produces fast, standalone binaries with compact memory footprints.
  - Pure typed functional invariants with algebraic data types for Intent IR and verification state machines.
  - Rust provides modern memory-safe kernel primitives without a garbage collector.
  - Clean path to self-hosting: OCaml native services can run directly on the custom kernel.
- **Cons:** Requires OCaml/Dune environment setup.

---

## 3. Decision Outcome & Rationale
**Selected: Option B.**
Python is completely excluded from the Intent-Native OS codebase, build systems, test suites, and IPC tooling.
- **Control Plane & Compiler:** OCaml with Dune build system and Alcotest / QCheck.
- **Kernel & Boot:** Rust with minimal assembly.
- **Host Automation:** Dune and PowerShell (for Windows platform tasks).
- **CI:** GitHub Actions with Dune.

---

## 4. Empirical Evidence & Verification Method
- OCaml native binaries compile directly to ELF/Mach-O/PE without external runtime dependencies.
- Resident memory footprint of OCaml runtime is under 15 MiB compared to 60+ MiB for Python runtimes.
- Verification achieved through Dune test runner (`dune runtest`) and repository structure validator.

---

## 5. Rollback Strategy
If a specific host platform requires low-level scripting not supported by Dune/PowerShell, audited POSIX shell or standalone compiled Rust utilities may be used. Under no circumstance will Python be introduced as an OS dependency.
