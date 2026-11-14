# Intent-Native OS — Implementation Handoff

**Baseline:** 2026-10-09. **Status:** engineering kickoff. Not a completed OS.

## Goal
Build an original, local-first intent-native operating system that is powerful for a software engineer and usable by a novice. Natural language, direct manipulation, and code lead into typed intent contracts, a two-model local collaboration loop, native code and capability-bound execution. Retain legacy executable compatibility through on-demand isolated capsules where practical.

## Read order
1. `START_HERE_IMPLEMENTATION_HANDOFF.md` (this file)
2. `intent_native_os_engineering_spec_v0_2.md` (normative design baseline)
3. `intent_os_granular_backlog_v0_3.csv` (127 task records, authoritative task data)
4. `intent_os_granular_backlog_v0_3.md` (readable checklist)
5. `ARCHITECTURE_DECISIONS_AND_NON_GOALS.md`

## Requirements not to erode
- Natural language is a high-level programming input; do not reduce the product to a chat agent.
- Before consequential execution, distinguish exploration, simulation, design, execution and maintain-mode with a clarification gate.
- Preserve original user intent, experimental versus binding settings, provenance and authorization.
- Model A architects/reviews; model B implements/executes. The OS handles handoffs; model C is optional.
- Models are untrusted and cannot authorize themselves. Core kernel, boot, file access and recovery function without models.
- Native capabilities and deterministic tools are preferred over repeated LLM work once semantics are known.
- OS-owned memory independent of model residency; SCP/NNC/tonic gating remain experiments, not dependencies.
- Custom minimal protected kernel/hypervisor direction; on-demand high-resource guests with narrow authority.
- Strong phone/key-backed step-up authorization for privilege crossings; ordinary actions should not prompt excessively.
- Versioned and post-quantum-ready crypto interfaces; no fixed ZKP construction mandated.
- 16 GiB reference target, QEMU initially, reproducible builds, test evidence for completion.

## Implementation order
### G0: foundational controls
P001: freeze hosted slice vs bootable custom MVP vs consumer non-goals.
P002: choose QEMU x86-64/UEFI, virtio storage, serial, basic display and network profile.
P003: ADR template and unresolved decision register.
P004: monorepo directories `kernel/ core/ compiler/ models/ capsules/ ui/ tests/ docs/`.
P005: reproducible pinned toolchains.
P006: CI smoke tests, QEMU boot harness and artifact output.
P007: versioned IPC/serialization schema.
P008: privilege and trust boundary diagram.
P009: quality/fidelity, HIC, RAM/latency/performance telemetry definitions.
P010: dependency source/provenance policy.

### Parallel branches
- **Platform:** UEFI boot, serial output, physical allocator, paging, interrupts, timer, isolation, process, IPC, capability delegation and revocation, controlled fault handling.
- **Intent/compiler:** input modes, typed IR, clarification gate, planning/implementation handoffs, journal, deterministic verifier, operation dispatch and native code generation.
- **Convergence:** a graphical app calls native intent runtime through versioned IPC and executes a reversible operation, without requiring an LLM for precompiled paths.

## First vertical demo
The user says, “Organize these files.” System presents file selection and minimally necessary disposition clarification. Design mode makes no changes. When approved, system stages a reversible file operation, compiles or selects existing native implementation, executes with file-scoped permissions, verifies resulting counts/hashes and produces a rollback receipt. Model A/B handoff is automatic where reasoning is required. No copy-paste between models.

## Completion protocol for every task
Record task ID, implementation commit, files touched, build command, test command, pass/fail output, artifact paths, benchmark where applicable, known limitations and next dependency. Never mark done solely because a model reports success.

## Agent handoff prompt
Read this handoff and the engineering spec first. Use the 127-task CSV as the source of task ownership and dependencies. Start P001–P010, then unblock the native platform and intent/compiler tracks in parallel. Do not widen scope, introduce cloud dependencies or hardwire optional research components. Produce a working artifact, reproduce its tests, record evidence and update the task status on each accepted change. Request a human decision only for consequential unknowns.
