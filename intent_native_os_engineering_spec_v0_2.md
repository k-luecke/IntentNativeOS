# Intent-Native OS — Engineering Specification v0.2

**Status:** Design draft, not a security certification or implementation claim  
**Scope:** Interfaces, state machines, trust boundaries, invariants, and first executable vertical slice  
**Baseline:** x86-64 development target; 16 GiB RAM reference system; offline-first; two local models; OCaml-first semantic control plane  
**Last revised:** 2026-10-09

## 0. Scope and normative language

**MUST** indicates a baseline system requirement; **SHOULD** a strong design preference; **MAY** an optional extension. This specification defines observable contracts, not the final choice of libraries, proof systems, foundation models, or kernel implementation language.

A complete OS is the long-term target. V0.2's first executable prototype MAY run on an existing host kernel as long as its kernel boundary is an explicit adapter rather than an architectural dependency.

## 1. System invariants

- **I-01 Intent integrity:** An executable action MUST reference a versioned, approved or policy-permitted Intent Contract. Unresolved material ambiguities prohibit consequential execution.
- **I-02 Intent mode separation:** EXPLORE, DESIGN, and SIMULATE MUST NOT silently transition to EXECUTE or MAINTAIN.
- **I-03 Capability confinement:** No model output, compiled program, guest, or service may grant itself new authority. Grants are issued only by a trusted authorization service.
- **I-04 Model independence:** Boot, core UI, file access, recovery, permission checks, and existing native programs MUST remain usable with both models unloaded.
- **I-05 Reproducible accountability:** Execution MUST record the intent version, plan version, executable or operation digest, authorization evidence, relevant tool results, and outcome.
- **I-06 No silent exfiltration:** Local-only is the default inference and data-movement policy. Remote services require explicit policy authorization.
- **I-07 Bounded work:** Reasoning iterations, subprocesses, resource budgets, and retry behavior MUST be bounded and cancellable.
- **I-08 Preservation:** Original user files MUST NOT be overwritten by generated changes without an explicit permitted operation; reversible changes SHOULD use snapshots, journaling, or transactional staging.
- **I-09 Proof semantics:** A successful cryptographic proof establishes only its specified statement under declared assumptions, not that a generated plan was wise or that a kernel is uncompromised.
- **I-10 PQ migration:** Cryptographic protocol and credential formats MUST be versioned and algorithm-agile; candidate ZK backends remain open.
- **I-11 Consumer parity:** Novice and engineering surfaces MUST operate on the same authoritative state, not separate incompatible task implementations.
- **I-12 Compatibility transparency:** The execution resolver MUST disclose irreducible requirements, including guest licensing and unsupported hardware, rather than imply that any binary is universally runnable.

## 2. Architecture and boundaries

```text
[Human: text / voice / pointer / drag-and-drop / code]
                         |
             [Input & Context Frontend]
                         |
           [Clarification + Intent Compiler]
                         |
                  [Typed Intent IR] <----> [OS-owned Semantic Store]
                         |
                [Plan/Task Supervisor]
                  /                \
       [Local Model A]  <---->  [Local Model B]    [Optional review model]
                  \                /
                 [Typed Task Graph]
                         |
          [Validation & Policy Compiler]
                         |
      [Capability Catalog / Code Generation / Native Compiler]
                         |
           [Execution Resolver & Capsule Manager]
                         |
          [Trusted Capability and Crypto Broker]
                         |
            [Kernel/Hypervisor Boundary]
                         |
             [CPU / GPU / Devices / Storage]

Execution receipts, measurements, failures and user feedback return through the
semantic store to the task supervisor and Model A.
```

**Trust levels:**

- **T0** Trusted kernel/hypervisor primitives: isolation, memory protection, authorization enforcement, IPC and device assignment. Small attack surface; no model inference inside T0.
- **T1** Trusted policy/crypto verifier and capability broker: may be isolated privileged services; must be protected from model-produced instructions and untrusted data.
- **T2** OS service control plane: input compiler, semantic store, model router, compiler frontend, execution resolver. Bugs here must not bypass T0/T1.
- **T3** Untrusted executors: generated binaries, model runtimes, user applications, compatibility layers and guests. All are capability-limited.
- **T4** External/untrusted inputs: documents, downloaded programs, network content, model completions, unverified memory records.

Trust is about authority, not model intelligence. T2/T3 compromise MUST NOT imply possession of T1 administrator secrets.

## 3. Typed Intent IR

### 3.1 Principal data types (schematic OCaml)

```ocaml
type intent_id = string
type revision = int
type artifact_hash = string

type mode = Explore | Design | Simulate | Execute | Maintain

type value_origin =
  | User_explicit
  | User_selected_example
  | Workspace_observed
  | Model_inferred
  | Policy_default

type binding = Example | Preference | Constraint | Authorized_action

type 'a qualified = {
  value : 'a;
  origin : value_origin;
  binding : binding;
  evidence : artifact_hash list;
}

type material_ambiguity = {
  field_path : string;
  alternatives : string list;
  consequence : string;
}

type intent_contract = {
  id : intent_id;
  revision : revision;
  mode : mode;
  objective : string qualified;
  targets : string qualified list;
  constraints : string qualified list;
  ambiguities : material_ambiguity list;
  verification_criteria : string list;
  authority_scope : string list;
  parent_revision_hash : artifact_hash option;
  content_hash : artifact_hash;
}
```

This is a wire-contract sketch, not compilable production OCaml. Real representations MUST avoid unstructured strings for capabilities, target references, predicates and verification criteria; these require distinct typed AST nodes. The wire schema MUST have a version identifier and canonical serialization for stable hashes.

### 3.2 Semantic preservation

Input: **“Optimize my solver”**, with latency selected solely as an exploration experiment.

```yaml
schema: inos.intent.v0.2
mode: EXPLORE
objective:
  kind: DESIGN_COMPILER_EXAMPLE
  origin: USER_EXPLICIT
  binding: CONSTRAINT
example_workload:
  kind: OPTIMIZE_SOLVER
  target: UNRESOLVED
  metric:
    value: MIN_LATENCY
    origin: USER_SELECTED_EXAMPLE
    binding: EXAMPLE
allowed_effects: [READ_CONTEXT, GENERATE_DESIGN_ARTIFACT]
forbidden_effects: [MODIFY_SOLVER, LAUNCH_OPTIMIZATION]
status: READY_FOR_EXPLORATION
```

**Rule:** A selected example must not be upgraded to a binding optimization objective without an explicit state transition and an authorized execution contract.

## 4. Clarification gate state machine

```text
RECEIVED -> CONTEXT_RESOLVED -> INTENT_DRAFTED -> [READY | CLARIFICATION_REQUIRED]
CLARIFICATION_REQUIRED -> HUMAN_RESPONSE -> INTENT_DRAFTED
READY -> [EXPLORATION | DESIGN | SIMULATION | AWAITING_AUTHORIZATION]
AWAITING_AUTHORIZATION -> [AUTHORIZED | REJECTED | EXPIRED]
AUTHORIZED -> PLANNING -> EXECUTING -> VERIFYING -> [COMPLETED | REVISION_REQUIRED | FAILED]
```

**Transitions:**

- `GO` with no uniquely approved pending target -> `CLARIFICATION_REQUIRED`.
- `GO` with one identifiable, approved, still-valid pending target -> dispatch that task only, subject to policy.
- MODE escalation requires an explicit human action or previously delegated, scope-limited standing policy.
- Material changes to objective, affected objects, risks or capability scope invalidate prior approval.
- A clarification question MUST identify the smallest unresolved decision blocking the next step; don't ask for incidental optimizations in EXPLORE mode.
- The gate SHOULD prefer direct-manipulation choices when they reduce language ambiguity.

**Deterministic checks:** target resolution cardinality; schema validity; forbidden effect set; stale revision; capability existence; requested mode; approval expiry; known unsupported platform constraints. Model assistance may classify ambiguous language but never authoritatively override these checks.

## 5. Model collaboration protocol

### 5.1 Roles

- **Model A, architect/reviewer:** maps intent to plan, reviews evidence, revises or requests clarification.
- **Model B, implementer/challenger:** reviews feasibility, uses permitted tools to implement or select capabilities, returns measured artifacts.
- **Model C:** optional and off by default. Independent evaluation may be requested based on risk or benchmark evidence.
- Both A and B SHOULD use independent small local foundation models with individually trained or configured LoRA adapters, but adapters/model identity MUST remain replaceable.

### 5.2 Required handoff envelope

```yaml
schema: inos.handoff.v0.2
handoff_id: uuid
intent_ref: {id: uuid, revision: 3, hash: sha256:...}
plan_ref: {hash: sha256:...}
from_role: ARCHITECT
to_role: IMPLEMENTER
iteration: 2
operation: IMPLEMENT
inputs: [{artifact: sha256:..., read_capability: cap-ref}]
output_contract:
  required: [patch, test_report, manifest, observations]
  schema: inos.execution_receipt.v0.2
limits: {max_wall_seconds: 600, max_iterations: 3, max_memory_bytes: 2147483648}
capability_scope: [read:project, write:staging, execute:test-runner]
return_to: ARCHITECT
```

Handoff artifacts MUST be immutable/content-addressed once committed. Tool logs are evidence, not assertions of correctness. Separate context projections SHOULD be constructed for A and B from the same global state.

### 5.3 Task loop invariants

1. A's plan MUST reference the current Intent Contract.
2. B MUST NOT interpret delegated implementation as permission to enlarge the objective.
3. Compiler/tool outputs MUST be checked deterministically when checks exist.
4. Model A may `ACCEPT`, `REVISE`, `REQUEST_CLARIFICATION`, `ESCALATE_REVIEW`, or `FAIL`.
5. Execution stops on exhausted budget, revocation, stale intent revision, policy violation, or unreachable verification criterion.
6. No model may authorize its own protected action.

## 6. Compiler architecture

### 6.1 IR stages

- **Intent IR:** typed, versioned semantic objective and constraints.
- **Plan IR:** partial-order task/dependency graph; explicit nondeterministic reasoning nodes.
- **Operation IR:** deterministic primitives with typed inputs, effects, capabilities, postconditions and provenance links.
- **Backend IR:** OCaml native, LLVM/MLIR, native libraries, GPU target, VM/API execution descriptor as suitable.
- **Executable artifact:** binary/library/service or configuration, plus executable manifest and provenance.

**Not every intent compiles into machine code.** A native filesystem operation may become a direct system call; a research task may use evidence retrieval; a legacy `.exe` may launch a translated or guest environment. Compilation is partly *selection and composition of preverified capabilities*, not always code generation.

### 6.2 Minimal effect model

```ocaml
type effect =
  | Read_resource of resource_ref
  | Write_resource of resource_ref
  | Execute_native of executable_ref
  | Invoke_local_model of model_ref
  | Network_connect of endpoint_ref
  | Launch_guest of guest_profile_ref
  | Change_system_policy of policy_ref
```

Each operation MUST declare an effect set. Effects must be checked against authorization grants and actual runtime mediation. Type-level checks alone do not enforce runtime containment.

### 6.3 Compiler invariants

- Preserve Intent Contract references through every lowering pass.
- Preserve effects/capability ceilings; transformations cannot silently add authority.
- Validate transformations via known compiler correctness constraints, static analysis, tests and differential checks where possible; do not claim formal equivalence without proof.
- Cache compiled capabilities by source/IR digest, target ABI, compiler version, relevant flags and policy profile.
- Generated code executes in staging with constrained capabilities until verification and promotion.

## 7. Semantic memory and provenance

An append-only event journal MUST record intent revisions, approvals, plans, handoffs, generated artifacts, test results, capability grants, revocations and execution outcomes. Materialized indexes MAY provide fast typed graph views.

Memory records MUST include: `id`, `schema_version`, `producer`, `trust_class` (AUTHORITATIVE | VERIFIED | PROVISIONAL | SUPERSEDED), `source_refs`, `created_at`, `validity`, `content_hash` and `parent_refs`.

- SCP, tonic gating and NNC are **experimental adapters**, not default trust-boundary components.
- A model-specific projection MUST cite its source records and MUST NOT promote provisional statements to verified facts merely through summarization.
- Stale hypotheses must be retractable; original receipts retained for audit.
- Persistent execution state survives model unloading, VM exit and reboot.

## 8. Execution resolver and capsule contract

### 8.1 Resolver decision states

`INSPECT -> VERIFY_FORMAT -> DISCOVER_BACKENDS -> EVALUATE_COMPATIBILITY -> CHECK_LICENSE_AND_POLICY -> PREPARE -> LAUNCH -> OBSERVE -> STOP -> RECLAIM`

For each candidate backend, evaluate: ABI/ISA, expected compatibility, graphics constraints, virtualization capability, required guest installation/media, licensing, isolation policy, RAM/CPU/GPU overhead, recovery and user data persistence.

**Backend preference:** native verified operation; supported compatibility execution; guest VM; explain unsupported condition. This is preference, not a mandatory ranking when security or correctness reverses it.

### 8.2 Capsule specification

```yaml
schema: inos.capsule.v0.2
artifact: sha256:...
backend: NATIVE | TRANSLATED | VIRTUALIZED | COMPILE_RUN
lifecycle: ON_DEMAND
resources:
  cpu_priority: FOREGROUND
  memory_policy: ELASTIC_WITH_HOST_RESERVE
  gpu_policy: SHARED_OR_PASSTHROUGH_IF_SUPPORTED
security:
  filesystem: [{scope: user-selected:/Photos/Trip, access: READ_WRITE}]
  network: DENY_BY_DEFAULT_UNTIL_APPROVED_PROFILE
  devices: [DISPLAY, INPUT]
  host_admin: DENIED
persistence:
  runtime: EPHEMERAL
  user_data: PERSIST_ON_AUTHORIZED_PATHS
shutdown:
  on_application_exit: REQUEST_GUEST_SHUTDOWN
  reclaim_after_verified_stop: true
```

Guest Windows binaries are **not** generally compilable into OCaml unikernels. Unikernel design may apply to native services and supervisors. Guest OS images require appropriate rights and legitimate licensing. The host retains memory, devices and control-plane services even under `EXCLUSIVE_PREFERRED` resource scheduling; 100% of RAM cannot be lent to a guest.

### 8.3 Failure behaviors

- VM crashes: revoke guest device mappings and network grants, preserve confirmed user data, produce crash receipt.
- Host low memory: reclaim optional local models first; protect host reserve; terminate or suspend guest only according to declared policy.
- Missing license/media: prompt user; do not acquire or bypass licensing automatically.
- Unsupported GPU acceleration: fall back if viable and disclose performance limitations.
- Revocation: block new effects; stop affected processes as needed; flush/rollback staged changes.

## 9. Security, cryptographic assurance, and post-quantum policy

### 9.1 Trusted authorization flow

`UNTRUSTED_REQUEST -> CANONICAL_OPERATION_DIGEST -> POLICY_CHECK -> FRESH_CHALLENGE -> INDEPENDENT_APPROVAL -> PROOF_VERIFY -> SCOPED_CAPABILITY -> ONE_OPERATION -> REVOKE/EXPIRE`

Privilege approval MUST bind the authenticated user/device to the canonical operation, target, constraints, nonce and freshness period. A phone may act as an independent authenticator with protected non-exportable credentials and a trustworthy transaction display. Offline recovery MUST be provided without a mandatory phone vendor/cloud account.

### 9.2 Cryptographic assurance abstraction

```ocaml
type assurance_goal =
  | Content_integrity
  | Origin_authenticity
  | User_authorization
  | Secret_possession
  | Privacy_preserving_membership
  | Private_policy_satisfaction
  | Computation_integrity

type crypto_profile = {
  version : string;
  assurance_goal : assurance_goal;
  security_level : string;
  pq_requirement : string;
  algorithm_suite_id : string;
}
```

Mechanisms are selected by required assurance and security profile: hashes/MACs, signatures, AKE, specialized proofs, or general-purpose ZKPs when their privacy properties are actually required. **No production proof backend is selected in V0.2.** RoKoko, PLONK-family schemes, STARK variants, and future PQ proof systems are research candidates only. Some classical ZK systems are not quantum resistant.

Post-quantum readiness includes algorithm-agile profiles, versioned keys, supported ML-KEM/ML-DSA/SLH-DSA pathways where appropriate, hybrid transition strategies, crypto inventory and migration procedures. Selected schemes MUST be independently reviewed and measured before trust-boundary deployment.

### 9.3 Threat examples and guarantees

- Compromised model B: may propose malicious instructions but cannot gain host-admin capability without authorization.
- Compromised guest: should be contained by isolation and brokered interfaces; hypervisor escape remains a threat requiring testing.
- Stolen/replayed approval: reject mismatched or stale challenge/operation pair.
- Compromised host kernel: cryptographic keys stored on a separate phone may remain undisclosed, but a compromised kernel may still misuse host-controlled resources. Cryptography alone does not solve this threat.
- Malicious retrieved document: data must not become instructions or grants because it appears in model context.

## 10. Native kernel ABI boundary

The custom kernel design is **independent** of the v0.2 user-space prototype. Minimum abstract interfaces:

```text
cap_create(parent, resource, rights, expiry) -> capability | error
cap_revoke(capability) -> result
ipc_send(endpoint_cap, typed_message) -> result
ipc_recv(endpoint_cap) -> typed_message | error
mem_map(address_space_cap, page_cap, rights) -> result
spawn(image_cap, address_space_cap, initial_caps) -> process_handle
proc_wait(handle) -> exit_receipt
vm_create(profile_cap, resource_budget) -> guest_handle
vm_assign_device(guest_cap, device_cap, policy) -> result
vm_stop(guest_handle, mode) -> result
```

These are architectural ABI concepts, not real syscalls. Kernel implementation, interrupt handling, page tables, graphics/display architecture, IOMMU requirements, and driver model remain open. Candidate microkernel/hypervisor techniques MUST be benchmarked and threat-modeled rather than adopted by assumption.

## 11. Acceptance tests and metrics

### Functional

- **F01** “Go” without unique pending target requests clarification, not execution.
- **F02** Example “minimum latency” remains nonbinding in EXPLORE mode.
- **F03** Approved file rename compiles to deterministic operation and runs without model invocation on reuse.
- **F04** Two models exchange task/receipt artifacts automatically; no copy-paste.
- **F05** Privileged operation fails without valid scoped capability, regardless of model output.
- **F06** All core functionality required for recovery works with local model runtimes stopped.
- **F07** Legacy executable resolver identifies format and returns supported backend or transparent failure.
- **F08** Guest termination releases guest allocations and preserves explicitly approved persistent data.
- **F09** Invalid signature/proof, replayed nonce, stale contract or broadened operation digest is rejected.
- **F10** Software/data never transmitted to external inference provider under LOCAL_ONLY.

### Measured metrics (targets not yet selected)

- Task intent-fidelity rate on user-validated evaluation set.
- Total human intervention time versus direct manual completion, including review and recovery.
- First-pass task completion rate and deterministic verification pass rate.
- System idle resident memory; active A/B peak resident memory; model swapping overhead.
- Local model turn latency and complete A→B→A cycle latency.
- Compilation cache reuse rate and speedups on repeated workflows.
- Kernel/OS boot, crash-recovery, compatibility launch and shutdown latency.
- Guest isolation tests, privilege escalation denial, fuzzing coverage and recovery success.

All benchmarks MUST disclose hardware, test data, versions, failure cases and measurement method. No security certification is implied by benchmark success.

## 12. Vertical-slice implementation plan

**VS0 — Contracts, schemas, tests:** implement canonical serialization, typed Intent IR, state machine, effect checking, durable receipts and fake backends.

**VS1 — Human input / clarification:** parse natural language plus visual selections, retain mode and binding provenance; test ambiguous “go” and experimental-metric cases.

**VS2 — Local A/B loop:** integrate two replaceable local model adapters, independent task projections, bounded planning/implementation, structured handoffs and deterministic checks.

**VS3 — Native operations:** compile a small operation IR into OCaml native binaries or trusted library calls; add content-addressed cache, staging, rollback and verification.

**VS4 — Capability broker:** isolated policy service with deny-by-default privilege escalation, scoped grants, revocation and audit tests; phone authentication comes after offline recovery design.

**VS5 — Capsule resolver:** inspect a binary and launch a minimal native capsule; later add one genuine guest VM, properly constrained file sharing and resource reclaim.

**VS6 — Independent custom-kernel track:** boot in QEMU, memory isolation, IPC, process scheduling and capability checks behind matching interface; integrate when these meet the VS0–VS5 contract tests.

**VS7 — Consumer validation:** test representative tasks with novice and engineering users, with identical system state and clear intervention-time measurements.

A kernel-first approach is not a prerequisite for testing the novel Intent IR. The vertical slice and custom-kernel track should progress independently until interfaces and verification are stable.

## 13. Decisions and open issues

### Agreed direction

- Original intent-native general-purpose OS, not merely an AI chat wrapper.
- Two local independent model roles by default; third evaluator optional.
- Typed intent and clarification before consequential actions.
- OCaml-first semantic core; modular native compiler backends.
- OS-owned semantic memory with experimental SCP/tonic/NNC adapters.
- Custom capability-oriented kernel/hypervisor as eventual base.
- Application-scoped execution capsules, ephemeral VM runtime with persistent authorized user data.
- Strong phone/security-key-based scoped privilege escalation.
- Cryptographic agility and post-quantum readiness; no committed ZKP backend.
- Cloud inference optional; no ongoing subscription needed for core OS use.

### Open technical decisions

1. Exact microkernel versus hybrid hypervisor boundary, bootstrap language and boot chain.
2. Graphics/compositor, GPU isolation and device-driver strategy.
3. Canonical typed AST, effects language and serialization/protocol versioning.
4. Persistent semantic store implementation, consistency and garbage collection.
5. Specific local model pairing, LoRA evaluation method, memory scheduling.
6. Code-generation paths and proofs/tests needed before capability promotion.
7. Hostless Linux/Windows compatibility strategy and guest desktop integration.
8. Offline phone pairing, trust-anchor isolation and secure recovery process.
9. Production cryptographic suites, attestation approach and PQ transition.
10. Practical 16 GiB hardware budgets and target service-level objectives.

## 14. Immediate next engineering output

Prepare separate machine-readable schemas for Intent, Handoff, Capsule, Capability Grant, Receipt and Crypto Profile, along with transition-table tests and a stub OCaml reference runtime. The first executable acceptance test should enforce: *an experimental user choice must never become an authorized system modification without a new, explicit action and verified contract revision*.
