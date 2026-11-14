# Intent-Native OS — Granular Engineering Backlog v0.3

> Status: proposed tasks, **not completed work**. IDs and dependencies are stable starting points; owners/evidence/status are tracked in CSV.

## Product gates and scope

| Gate | Outcome | Tasks |
|---|---|---:|
| G0 | Build/reproducibility foundation | 10 |
| G1 | Hosted end-to-end intent/compiler A↔B prototype | 34 |
| G2 | Bootable original kernel with isolated user processes | 23 |
| G3 | **Usable original-OS MVP**: native graphical flow with verified, reversible operations | 18 |
| G4 | Legacy Windows/guest demonstration | 17 |
| G5 | Security/recovery candidate | 15 |
| G6 | Measured usability and performance | 10 |

**MVP commitment:** The custom-OS MVP is G3, not merely a Linux application. G1 is an intentionally faster proving ground. G4–G6 are subsequent increments, not gates to G3. A consumer release requires much more validation and hardware coverage.

**Constraints:** local-first; two independent local models by default; third reviewer optional; inference services unprivileged; no user-modifying operation without approved authority; semantic memory retained independently of model RAM; proof backends remain algorithm-agile; 16 GB reference hardware; QEMU target first.

**Do not conflate:** A host demonstration is not a custom OS; Windows compatibility is not native ABI support; model agreement is not verification; a ZKP is not inherently post-quantum; a microkernel prototype is not a consumer release.

## Execution discipline

1. Work from dependency-ready items, not broad epics. Any task estimated to take multiple sessions should be split further before starting.
2. Every item closes only with an observable artifact and a passing acceptance check; logs/hashes/test references go in the Evidence column.
3. Keep core security code deterministic and small. No LLM decides whether its own elevated permission is valid.
4. Preserve a failing test before repairing a regression; avoid re-reviewing decisions already captured in ADRs.
5. Track blocked tasks distinctly from failed tests. Re-scope when results require it, not to chase a calendar date.

## All tasks

### G0 — Build system

- [ ] **P001 — Freeze MVP definition and non-goals**  
  Depends: `none`. **Done when:** One-page acceptance statement distinguishes hosted vertical slice, custom-OS MVP, and future consumer release.
- [ ] **P002 — Choose x86-64 QEMU virt target and supported virtual devices**  
  Depends: `P001`. **Done when:** Target specification lists firmware, CPU, PCI, storage, network, display and memory assumptions.
- [ ] **P003 — Record architecture decision log template**  
  Depends: `P001`. **Done when:** ADR template captures alternatives, evidence, rollback and decision owner.
- [ ] **P004 — Create monorepo with kernel, core, compiler, runtime, UI, tests directories**  
  Depends: `P001`. **Done when:** Fresh clone passes structural checks.
- [ ] **P005 — Create reproducible toolchain lockfile and build container**  
  Depends: `P002,P004`. **Done when:** Fresh environment builds hello-world artifact without manual package guessing.
- [ ] **P006 — Add CI lint, formatting, unit test and artifact retention**  
  Depends: `P004,P005`. **Done when:** Pull request reports failures and stores build output.
- [ ] **P007 — Define interface versioning and serialized message envelope**  
  Depends: `P003,P004`. **Done when:** Schema-roundtrip compatibility tests pass.
- [ ] **P008 — Define threat model and trust boundary inventory**  
  Depends: `P001`. **Done when:** Trust boundary diagram and adversary capabilities recorded.
- [ ] **P009 — Define metrics: human effort, task success, peak RSS, cold start, crashes**  
  Depends: `P001`. **Done when:** Automated report template with measurement method for each metric.
- [ ] **P010 — Set release and supply-chain policy**  
  Depends: `P004`. **Done when:** Build provenance, dependency pinning and license notices documented.

### G1 — Hosted vertical slice

- [ ] **I001 — Define mode enum explore/design/simulate/execute/maintain**  
  Depends: `P007`. **Done when:** Property tests reject invalid modes.
- [ ] **I002 — Define typed objective, target and references**  
  Depends: `I001`. **Done when:** Valid and malformed sample intents parse predictably.
- [ ] **I003 — Define confidence and provenance types without conflating with permission**  
  Depends: `I002`. **Done when:** Hypothesis cannot deserialize as approved fact.
- [ ] **I004 — Define authority tokens and pending approval fields in IR**  
  Depends: `I002,P008`. **Done when:** Simulated request cannot transition to authorized without grant.
- [ ] **I005 — Define experimental versus binding parameters**  
  Depends: `I002`. **Done when:** Latency-as-example test never authorizes solver edits.
- [ ] **I006 — Implement contextual reference resolver for selected file and active project**  
  Depends: `I002`. **Done when:** 'this file' resolves only with unambiguous selection.
- [ ] **I007 — Implement deterministic request classifier for direct action versus clarification**  
  Depends: `I001,I006`. **Done when:** 'go' with no selected target yields clarification.
- [ ] **I008 — Implement clarification question selection by consequence**  
  Depends: `I007,I004`. **Done when:** Low-risk known move asks zero questions, ambiguous delete asks specific question.
- [ ] **I009 — Implement intent state machine with legal transitions**  
  Depends: `I001,I004,I008`. **Done when:** Invalid transitions fail closed and are audited.
- [ ] **I010 — Serialize IR as versioned JSON / CBOR envelope**  
  Depends: `I002,P007`. **Done when:** Save/load roundtrip, version mismatch diagnosed.
- [ ] **M001 — Create append-only event journal with checksums**  
  Depends: `P007`. **Done when:** Restart reconstructs state and detects tampered entry.
- [ ] **M002 — Build materialized current-state view from journal**  
  Depends: `M001`. **Done when:** Replay produces identical state hash twice.
- [ ] **M003 — Store authoritative/verified/provisional/superseded classifications**  
  Depends: `M002,I003`. **Done when:** Provisional model assertion cannot overwrite authoritative constraint.
- [ ] **M004 — Implement task-scoped model context projection**  
  Depends: `M003`. **Done when:** A and B projections contain required references and exclude unrelated test secrets.
- [ ] **M005 — Enforce maximum context budgets with retrieval receipts**  
  Depends: `M004`. **Done when:** Truncation has explicit omission record and never drops binding constraints.
- [ ] **R001 — Implement local model adapter protocol with health and timeout**  
  Depends: `P007`. **Done when:** Mock model passes streaming and error tests.
- [ ] **R002 — Connect existing Qwen local runtime as model A**  
  Depends: `R001`. **Done when:** Model A returns a schema-valid test plan with no cloud dependency.
- [ ] **R003 — Connect second independently trained local model as B**  
  Depends: `R001`. **Done when:** Model B returns schema-valid implementation proposal offline.
- [ ] **R004 — Add strict structured A-plan / B-result envelopes**  
  Depends: `R002,R003,I010`. **Done when:** Invalid field and unknown effect rejected.
- [ ] **R005 — Implement A to B to A handoff state machine**  
  Depends: `R004,M001`. **Done when:** Interrupted cycle resumes without duplicate side effects.
- [ ] **R006 — Implement iteration budget, deadlock and escalation rules**  
  Depends: `R005`. **Done when:** Infinite review loop stops at configured limit and returns useful status.
- [ ] **R007 — Separate proposed code from executable operations**  
  Depends: `R004,I004`. **Done when:** Generated shell string cannot execute absent validated operation.
- [ ] **C001 — Define operation IR with typed inputs, effects and outputs**  
  Depends: `I010`. **Done when:** Illegal input/output edges rejected.
- [ ] **C002 — Implement pure operation registry and dependency resolver**  
  Depends: `C001`. **Done when:** Known transformation resolved without LLM call.
- [ ] **C003 — Implement dependency DAG validator and cycle rejection**  
  Depends: `C001`. **Done when:** Cyclic plan rejected with actionable diagnostic.
- [ ] **C004 — Build deterministic executor for read/hash/copy-in-sandbox operations**  
  Depends: `C002,C003`. **Done when:** Fixture tests pass and cannot escape sandbox.
- [ ] **C005 — Implement execution receipts: artifact digests, result, timings, rollback**  
  Depends: `C004,M001`. **Done when:** Every state-changing run emits a complete receipt.
- [ ] **C006 — Implement differential verifier for planned vs actual output**  
  Depends: `C005`. **Done when:** Intent-mismatched but technically valid output flagged.
- [ ] **C007 — Implement native-code backend prototype for one pure function**  
  Depends: `C001`. **Done when:** Compiled binary passes same vectors as reference interpreter.
- [ ] **C008 — Add reproducible benchmark harness and baseline**  
  Depends: `C007,P009`. **Done when:** Source, binary hash, inputs and distributions in report.
- [ ] **U001 — Build minimal file-picker and intent text field hosted UI**  
  Depends: `I006`. **Done when:** User selects a file and gives instruction without terminal.
- [ ] **U002 — Display intent clarification as contextual choices**  
  Depends: `U001,I008`. **Done when:** User can approve choices or return to exploration.
- [ ] **U003 — Display plan, execution and reversible result preview**  
  Depends: `U002,C005`. **Done when:** Human can inspect before commit and undo test copy.
- [ ] **G1T — End-to-end hosted prototype gate**  
  Depends: `U003,R005,C006,C007`. **Done when:** Ambiguous request clarified then A/B loop compiles/verifies native transform; no manual copy-paste.

### G2 — Custom kernel

- [ ] **K001 — Select boot protocol and documented UEFI loader strategy**  
  Depends: `P002,P005`. **Done when:** ADR details handoff, firmware exit and memory map.
- [ ] **K002 — Build kernel image and automated QEMU launch script**  
  Depends: `K001`. **Done when:** CI boots image to known serial marker.
- [ ] **K003 — Capture UEFI/boot memory map and normalize regions**  
  Depends: `K002`. **Done when:** Memory regions validate against fixture and QEMU trace.
- [ ] **K004 — Initialize x86-64 paging and kernel address space**  
  Depends: `K003`. **Done when:** Identity/mapped translations pass debug tests.
- [ ] **K005 — Initialize exceptions and panic serial diagnostics**  
  Depends: `K004`. **Done when:** Intentional page fault generates correct diagnostic.
- [ ] **K006 — Initialize interrupt table and timer source**  
  Depends: `K005`. **Done when:** Monotonic timer interrupt count verified.
- [ ] **K007 — Build physical page allocator**  
  Depends: `K003,K004`. **Done when:** Allocate/free stress and reserved region tests pass.
- [ ] **K008 — Build virtual memory mapping/unmapping API**  
  Depends: `K007`. **Done when:** Unmapped user page access faults as designed.
- [ ] **K009 — Boot secondary CPUs using supported firmware interface**  
  Depends: `K006,K008`. **Done when:** Test multicore run reports all expected cores.
- [ ] **K010 — Implement per-core scheduler with runnable queues**  
  Depends: `K006,K009`. **Done when:** Two synthetic workers progress and yield.
- [ ] **K011 — Add user-mode transition and isolated address spaces**  
  Depends: `K008,K010`. **Done when:** User process cannot access kernel address.
- [ ] **K012 — Build syscall gate and versioned ABI dispatcher**  
  Depends: `K011,P007`. **Done when:** Unknown syscall rejected, known syscall roundtrips.
- [ ] **K013 — Create typed IPC messages and bounded queues**  
  Depends: `K012,P007`. **Done when:** Bounded send/receive and cancellation tests pass.
- [ ] **K014 — Implement capability handles with rights and target binding**  
  Depends: `K013,P008`. **Done when:** Unowned capability access denied.
- [ ] **K015 — Implement delegation and revocation**  
  Depends: `K014`. **Done when:** Revoked child capability loses access immediately or per documented epoch.
- [ ] **K016 — Add kernel object lifecycle reference counting**  
  Depends: `K013,K015`. **Done when:** No leaks or dangling references in repeated synthetic lifecycle test.
- [ ] **K017 — Integrate minimal user-mode ELF loader**  
  Depends: `K011,K012`. **Done when:** Signed or locally approved sample ELF loads and exits.
- [ ] **K018 — Add user process start/exit/wait primitives**  
  Depends: `K017,K010`. **Done when:** Parent receives exit code and orphan cleanup succeeds.
- [ ] **K019 — Implement kernel fault containment and user crash recovery**  
  Depends: `K018,K005`. **Done when:** User fault leaves kernel and sibling task running.
- [ ] **K020 — Build initial boot/init supervisor**  
  Depends: `K018,K013`. **Done when:** Boot launches init and restarts designated critical user service.
- [ ] **K021 — Add deterministic kernel event logging**  
  Depends: `K020,M001`. **Done when:** Fault/restart/access decision has traceable event.
- [ ] **K022 — Fuzz syscall and IPC parsers**  
  Depends: `K012,K013`. **Done when:** Seed corpus runs without kernel crash in CI.
- [ ] **G2T — Bootable isolation gate**  
  Depends: `K014,K018,K019,K020,K022`. **Done when:** Original kernel boots QEMU, runs two isolated userspace tasks, denies unauthorized access.

### G3 — Native OS MVP

- [x] **S001 — Implement block-device interface and virtio-block test driver**  
  Depends: `G2T`. **Done when:** QEMU disk reads known test blocks.
- [x] **S002 — Implement simple persistent filesystem driver or adapted library**  
  Depends: `S001`. **Done when:** Create/read/rename/delete survive reboot.
- [x] **S003 — Add atomic write and crash recovery tests**  
  Depends: `S002`. **Done when:** Injected power-loss scenarios preserve committed state.
- [x] **S004 — Implement basic network stack adapter / isolated networking service**  
  Depends: `K013`. **Done when:** QEMU loopback or virtual NIC ping and TCP test pass.
- [x] **S005 — Implement keyboard/pointer input event path**  
  Depends: `K013`. **Done when:** Key and pointer events reach isolated UI service.
- [x] **S006 — Implement framebuffer/display output**  
  Depends: `K020`. **Done when:** Graphical framebuffer shows known test image.
- [x] **S007 — Implement simple compositor with window surfaces**  
  Depends: `S006,K013`. **Done when:** Two window surfaces render and clip correctly.
- [x] **S008 — Build font/text rendering and input widgets**  
  Depends: `S007,S005`. **Done when:** Keyboard-only navigation and clear text render work.
- [x] **S009 — Build native file-picker and execution-result viewer**  
  Depends: `S008,S002`. **Done when:** User picks file and observes result without shell.
- [x] **S010 — Port OCaml runtime into protected userspace or build stable IPC bridge**  
  Depends: `K017,K013`. **Done when:** OCaml service can start, allocate and exchange typed message.
- [x] **S011 — Integrate hosted intent compiler services via native OS IPC**  
  Depends: `S010,I010`. **Done when:** Native UI submits typed intent and receives clarification.
- [x] **S012 — Integrate A/B model services through supported device / bridge path**  
  Depends: `S011,R005`. **Done when:** Local-only A/B cycle works end-to-end; resource accounting visible.
- [x] **S013 — Port sandboxed deterministic executor and native capability registry**  
  Depends: `S011,C004`. **Done when:** User file operation obeys capability scope.
- [x] **S014 — Implement snapshot/undo for initial file transformations**  
  Depends: `S003,S013`. **Done when:** Undo restores original bytes and metadata on fixture.
- [x] **S015 — Integrate verifier and activity receipts into UI**  
  Depends: `S014,C006`. **Done when:** User sees verified result, error and undo option.
- [x] **S016 — Add system resource monitor and model unload path**  
  Depends: `S012,K010`. **Done when:** Model stops and kernel reclaims owned pages without reboot.
- [x] **S017 — Test cold boot, repeated workload, restart, no-model fallback**  
  Depends: `S015,S016`. **Done when:** 50-run QEMU test harness reports pass/fail and RAM peaks.
- [x] **G3T — Native custom-OS MVP gate**  
  Depends: `S009,S015,S017`. **Done when:** Boot to graphics, select file, clarify, A/B plan, compile/run transform, verify/undo fully on original kernel.

### G4 — Compatibility

- [x] **V001 — Record host virtualization CPU and IOMMU feature requirements**  
  Depends: `P002,G2T`. **Done when:** Compatible and incompatible CPU profiles distinguished.
- [x] **V002 — Implement CPU virtualization probe and safe fallback**  
  Depends: `V001`. **Done when:** Reports hardware support without crashing unsupported host.
- [x] **V003 — Define guest capsule specification and authority scopes**  
  Depends: `K014,P007`. **Done when:** Invalid guest device request rejected.
- [x] **V004 — Create guest resource budgeting and reclaim contract**  
  Depends: `V003,S016`. **Done when:** Host reserves minimum memory; guest allocation measured.
- [x] **V005 — Implement guest launch/stop API backed by test hypervisor**  
  Depends: `V002,V003`. **Done when:** Minimal test guest boots then stops and frees pages.
- [x] **V006 — Implement virtio console/storage/network abstractions**  
  Depends: `V005,S001,S004`. **Done when:** Guest test exercises device interfaces.
- [x] **V007 — Create disposable guest disk overlays and persistent-data mount policy**  
  Depends: `V006,S003`. **Done when:** Guest runtime discarded while permitted data retained.
- [x] **V008 — Implement shared display buffer and native compositor integration**  
  Depends: `V005,S007`. **Done when:** Guest test framebuffer appears as normal native window.
- [x] **V009 — Implement keyboard/pointer forwarding and clipboard isolation**  
  Depends: `V008,S005`. **Done when:** Input reaches guest; forbidden clipboard transfer denied.
- [x] **V010 — Add file broker with user-selected document grants**  
  Depends: `V007,K015`. **Done when:** Guest cannot enumerate host filesystem beyond grant.
- [x] **V011 — Implement capsule stop/reclaim policy on app exit**  
  Depends: `V004,V009`. **Done when:** No guest pages / virtual devices remain after clean exit.
- [x] **V012 — Implement compatibility resolver PE/ELF architecture and dependency scanner**  
  Depends: `C002,S002`. **Done when:** Sample Windows/Linux/native artifacts classified deterministically.
- [x] **V013 — Prototype lightweight Windows API compatibility in isolated environment**  
  Depends: `V012`. **Done when:** Named test executable either runs or returns precise unsupported diagnostic.
- [x] **V014 — Define licensed Windows guest-image provisioning workflow**  
  Depends: `V007,V012`. **Done when:** Test workflow requests user-supplied lawful media/entitlement and makes no unauthorized download.
- [x] **V015 — Run one compatible Windows app in ephemeral guest**  
  Depends: `V011,V014,V008`. **Done when:** App opens in native compositor, saves file through broker, guest stops afterward.
- [x] **V016 — Benchmark CPU, RAM, graphics and model unloading under high guest priority**  
  Depends: `V015,S016`. **Done when:** Resource-pressure runs collect wall time, host responsiveness and headroom.
- [x] **G4T — Legacy application demonstration gate**  
  Depends: `V015,V016`. **Done when:** At least one real legacy app works end-to-end, host isolation and teardown proven.

### G5 — Security

- [x] **A001 — Define privileged operation canonical serialization and digest binding**  
  Depends: `I004,K014`. **Done when:** Same action hashes identically; changed target changes digest.
- [x] **A002 — Define authorization broker service and verifier trust placement**  
  Depends: `A001,P008`. **Done when:** Threat analysis shows model or guest cannot mint grant.
- [x] **A003 — Implement replay-resistant short-lived single-action grants**  
  Depends: `A002,K015`. **Done when:** Expired, altered and repeated requests rejected.
- [x] **A004 — Prototype offline phone pairing with authenticated key agreement**  
  Depends: `A002`. **Done when:** Pairing rejects active MITM in test harness.
- [x] **A005 — Implement phone-side trusted approval display and authenticator assertion**  
  Depends: `A004,A001`. **Done when:** Phone displays target/action digest independently of host prose.
- [x] **A006 — Integrate hardware-backed approval verifier**  
  Depends: `A003,A005`. **Done when:** Tampered approval cannot authorize privileged test operation.
- [x] **A007 — Define offline recovery, replacement phone and revocation process**  
  Depends: `A006`. **Done when:** Lost-phone simulation restores access without bypassing documented controls.
- [x] **A008 — Define algorithm-agile crypto registry and security profiles**  
  Depends: `P008`. **Done when:** Unsupported or downgraded suite fails closed.
- [x] **A009 — Add standardized post-quantum KEM/signature interoperability harness**  
  Depends: `A008`. **Done when:** KATs and version migration tests run using vetted libraries.
- [x] **A010 — Add optional proof backend interface, without mandatory ZKP selection**  
  Depends: `A008`. **Done when:** Mock and signature-backed test implementations obey interface and policy.
- [x] **A011 — Add measured boot/update trust-root design and rollback prevention plan**  
  Depends: `A008,K001`. **Done when:** Boot-chain threat review, key custody and recovery documented.
- [x] **A012 — Implement signed versioned system update and rollback path**  
  Depends: `A011,S003`. **Done when:** Interrupted update returns bootable known-good version.
- [x] **A013 — Fuzz capability broker, image loader and guest file broker**  
  Depends: `A003,K017,V010`. **Done when:** Coverage corpus runs with no privilege boundary violation.
- [x] **A014 — Independent code audit checklist and security test evidence registry**  
  Depends: `A013`. **Done when:** Finding severity, reproduction, mitigation, regression retained.
- [x] **G5T — Security candidate gate**  
  Depends: `A007,A009,A012,A014`. **Done when:** Privileged changes require bound approval; offline recovery and update rollback demonstrated.

### G6 — Usability

- [x] **Q001 — Design novice test scripts and observational scorecards**  
  Depends: `P009,G3T`. **Done when:** Three task scripts reproducible by evaluator without coaching.
- [x] **Q002 — Design engineer test scripts for IR and source inspection**  
  Depends: `Q001`. **Done when:** Expert can trace intent -> generated operations -> execution receipts.
- [x] **Q003 — Measure manual vs automatic task total human attention**  
  Depends: `Q001`. **Done when:** Prompting, supervision, review and correction all recorded.
- [x] **Q004 — Test model A/B consistency on ambiguous and malicious instructions**  
  Depends: `R005,I009`. **Done when:** Misinterpreted intent count and reject behavior recorded.
- [x] **Q005 — Measure 16 GB peak RAM under native intent workload**  
  Depends: `G3T`. **Done when:** Memory report includes model weights, cache, OS and app.
- [x] **Q006 — Evaluate optional SCP / tonic gating / NNC only against baselines**  
  Depends: `Q004,Q005`. **Done when:** Per-tech ablation proves benefit or records abandonment.
- [x] **Q007 — Establish boot, GUI responsiveness and crash-free repetition targets**  
  Depends: `S017`. **Done when:** Chosen thresholds documented then tracked in CI.
- [x] **Q008 — Establish application-compatibility manifest and failure explanations**  
  Depends: `G4T`. **Done when:** Every supported app has exact versions, results and known limitations.
- [x] **Q009 — Write first-run setup, safe-mode boot and recovery interface**  
  Depends: `G3T,A012`. **Done when:** Nontechnical tester completes documented recovery path.
- [x] **Q010 — Publish developer image, checksums, build instructions, known risks**  
  Depends: `Q007,Q009`. **Done when:** New machine / clean VM reproduces tested build.

## First dependency-ready sprint

Start with `P001`, then `P002`/`P003`/`P004`/`P008`/`P009` as permitted. Avoid blocking G1 on a working native kernel. Immediately follow with `P005`, `P007`, `I001`, `I002`, `M001`, and `R001` as dependencies clear.

## Explicit future work outside G3 MVP

Secure boot key management, extensive physical GPU/display drivers, USB/Bluetooth breadth, suspend/resume, native browser port, extensive Windows application compatibility, attestation, formal verification, commercial provisioning/support, broad accessibility certification, and consumer-scale update infrastructure are not implied to be solved at G3. They require separate epics and risk assessments.
