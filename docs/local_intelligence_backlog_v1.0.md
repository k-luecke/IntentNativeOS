# Intent-Native OS — Local Intelligence Engineering Backlog v1.0

**Status:** Active Successor Backlog (Gate L0–L5)  
**Baseline Commit:** `85e8848dec37fe5d72c80629aa901e7fe4c8a469` (Gate G12)  
**Predecessor Backlog:** `intent_os_granular_backlog_v0_3.md` (127 tasks, 100% Closed, Frozen)  
**Objective:** Make local intelligence useful, reliable, efficient, and independently verifiable inside the Intent-Native OS.  
**Scope:** Local model execution, typed reasoning, Model A/Model B arbitration, SCP and tonic-gating experiments, resource management, kernel integration, and usability evaluation.  
**Out of scope:** Claude/Codex/Grok provider adapters, subscription billing, external inference dependencies, crystal-ball/EMG hardware, and unapproved bare-metal boot.

---

## Binding Engineering Invariants

1. **Offline-first:** Complete core workflows with network disconnected. Never require cloud AI.
2. **Deterministic first:** Resolve exact or repeatable tasks without calling a model. Record avoided model calls.
3. **No Python in implementation, runtime, tests, build, or CI:** OCaml 5/Dune control plane; Rust kernel and native services; C/C++ where justified for inference; minimal assembly.
4. **Authority is not intelligence:** Model A or B can recommend actions but cannot create capabilities, override denied operations, or silently upgrade speculative data to fact.
5. **Dual-model protocol:** Architect/reviewer (A) and implementer/challenger (B) remain distinct *roles*. They may initially use distinct contexts on the same local weights; do not claim independent model diversity without distinct weights or evidence.
6. **SCP and tonic gating are optional experiments:** Define and preregister the exact algorithms before benchmarking. Retain only on blinded, reproducible improvement without unacceptable security or latency regression. Neither belongs in the mandatory execution path by default.
7. **NNC and unconventional compute remain research-only**, not presumed dependencies.
8. **Preserve provenance:** Model output is provisional; verified artifacts and authorized effects are separately represented and independently auditable.
9. **Genuine runtime evidence:** Distinguish hosted WSL inference, host-to-guest bridge, native Ring 3 inference, and synthetic/mocked runs. Never call one another.
10. **16 GiB reference envelope** is a measured target, not a bookkeeping-only partition. Use real peak RSS and guest memory metrics. Kernel remains usable when models are unloaded.
11. **Host safety:** All boot tests in QEMU only, isolated firmware vars, virtual disks only; never alter Windows boot or physical partitions.
12. **Backlog integrity:** Each task is accepted only with linked commit, reproducible command, tests, binary/artifact hashes, and negative results where applicable.

---

## Product Gates and Task Matrix

| Gate | Focus Area | Tasks | Status |
| :--- | :--- | :---: | :---: |
| **Gate L0** | Baseline and Evaluation Contract | 7 | Complete |
| **Gate L1** | Production-Shaped Local Inference Engine | 9 | Complete |
| **Gate L2** | Real Dual-Model Cooperation | 8 | Complete |
| **Gate L3** | SCP and Tonic-Gating Research Lane | 8 | Quarantined / Research |
| **Gate L4** | Kernel Ownership and Native Service Integration | 7 | Pending |
| **Gate L5** | UX, Quality and Hardening | 7 | Pending |
| **Total** | **46 Tasks** | **46** | |

---

## All Tasks

### Gate L0 — Baseline and Evaluation Contract

- [x] **L001 — Freeze G12 known-good reproducible baseline**  
  *Depends:* `none`. **Acceptance:** Clean checkout reproduces model output digest, OCaml handoff, QEMU persistence; record current integration boundaries and open gaps.  
  *Evidence:* `docs/L001_G12_BASELINE_FREEZE.md`
- [x] **L002 — Create model inventory, provenance, license and weights manifest**  
  *Depends:* `none`. **Acceptance:** Model source, architecture, license, tokenizer/version, precision, hashes and allowed distribution recorded; no unverified weights.  
  *Evidence:* `models/manifest.json`, `docs/L002_MODEL_INVENTORY_MANIFEST.md`
- [x] **L003 — Construct adversarial, held-out intent benchmark corpus**  
  *Depends:* `L001`. **Acceptance:** At least 100 varied cases across 8 categories: unambiguous, ambiguous, contradictory, compound, unauthorized, prompt injection, missing files, hallucinated resources; frozen splits (60 dev, 20 val, 20 sealed eval).  
  *Evidence:* `benchmarks/corpus/intent_benchmark_100.json`
- [x] **L004 — Define exact semantic and operational metrics**  
  *Depends:* `L003`. **Acceptance:** Intent match, valid typed IR, clarification precision/recall, false authorization rate (must be 0.0%), correct deterministic operations, unnecessary refusals, p50/p95/p99 latency, RSS, token cost, human attention.  
  *Evidence:* `benchmarks/metrics.ml`
- [x] **L005 — Reproducible isolated benchmark runner**  
  *Depends:* `L003, L004`. **Acceptance:** Fixed seed/config/version, machine metadata, hashes, benchmark output as structured JSON and diffable reports; sealed evaluation split strictly isolated; no model answers in gold labels.  
  *Evidence:* `benchmarks/runner.ml`, `benchmarks/results/`
- [x] **L006 — Establish deterministic-only baseline**  
  *Depends:* `L004, L005`. **Acceptance:** Run entire corpus through deterministic grammar/registry, quantify coverage, false accepts, speed, and model-call avoidance without model inference.  
  *Evidence:* `benchmarks/deterministic_baseline.ml`, `benchmarks/results/deterministic_baseline_report.md`
- [x] **L007 — Create capability/evidence classification ledger**  
  *Depends:* `L001, L006`. **Acceptance:** Every claim categorized as unit-tested, hosted demonstrated, QEMU-native demonstrated, hardware-path tested, human-tested or still simulated. Serial markers documented as supporting rather than independent proof.  
  *Evidence:* `docs/L007_CAPABILITY_EVIDENCE_LEDGER.md`

### Gate L1 — Production-Shaped Local Inference Engine

- [x] **L101 — Survey offline model candidates**  
  *Depends:* `L007`. **Acceptance:** Three size tiers (compact, medium, larger) with compatible licenses, explicit context/tokenization needs, CPU and optional AMD GPU feasibility.  
  *Evidence:* `docs/L101_MODEL_SURVEY.md`
- [x] **L102 — Implement trustworthy tokenizer and chat template handling**  
  *Depends:* `L101`. **Acceptance:** Cross-check token IDs/round-trips against model reference; malformed UTF-8 and truncation handled without Python.  
  *Evidence:* `models/tokenizer/`, `tests/test_tokenizer_suite.ml`
- [x] **L103 — Integrate supported quantized inference backend**  
  *Depends:* `L101, L102`. **Acceptance:** Genuine pretrained weights, no canned outputs; real autoregressive decoding and deterministic seeded runs.  
  *Evidence:* `models/inference/quant_engine.c`, `models/inference/quant_engine`
- [x] **L104 — Validate numerical inference independently**  
  *Depends:* `L103`. **Acceptance:** Reference logits/tokens over test vectors, tolerance criteria for quantization, positional encoding/KV-cache regression tests.  
  *Evidence:* `tests/test_numerical_parity.ml`
- [x] **L105 — Define model runner process and IPC ABI**  
  *Depends:* `L103`. **Acceptance:** Versioned typed requests/responses; structured error, limits, cancellation; no shell string interpolation for user content.  
  *Evidence:* `core/ipc/model_runner_abi.ml`
- [x] **L106 — Model load/unload, cache and budget policy**  
  *Depends:* `L105`. **Acceptance:** Real RSS snapshots, warm/cold startup, unload reclamation demonstrated; supports low-memory fallback.  
  *Evidence:* `core/memory/model_budget.ml`
- [x] **L107 — CPU benchmark and resource controls**  
  *Depends:* `L105, L106`. **Acceptance:** Tokens/sec, TTFT, p95 response time and memory pressure under realistic workloads; compare to 16 GiB reference.  
  *Evidence:* `benchmarks/results/l107_cpu_benchmark.md`
- [x] **L108 — Optional AMD GPU backend capability probe**  
  *Depends:* `L103`. **Acceptance:** Test actual supported driver/runtime and backend with numerical parity before claiming GPU acceleration; keep CPU fallback.  
  *Evidence:* `models/inference/gpu_probe.c`, `models/inference/gpu_probe`
- [x] **L109 — Model selection recommendation with evidence**  
  *Depends:* `L107, L108`. **Acceptance:** Select two candidate operating points using real accuracy/latency/memory tradeoffs, not model size alone.  
  *Evidence:* `docs/L109_MODEL_SELECTION_RECOMMENDATION.md`

### Gate L2 — Real Dual-Model Cooperation

- [x] **L201 — Formalize A/B roles, schemas and arbitration state machine**  
  *Depends:* `L109`. **Acceptance:** A proposes; B challenges/implements; deterministic verifier resolves; explicit iteration caps and failure states.  
  *Evidence:* `compiler/arbitration/state_machine.ml`
- [x] **L202 — Separate role context and journal isolation**  
  *Depends:* `L201`. **Acceptance:** Context memory for A vs B isolated; deliberate shared evidence via signed/hashed handoffs only; no hidden feedback contamination.  
  *Evidence:* `core/memory/role_context.ml`
- [x] **L203 — Execute real A→B→A local generation**  
  *Depends:* `L202`. **Acceptance:** Both role calls use actual pretrained inference; log prompt/template, model identity, generated tokens, parse failures and timings.  
  *Evidence:* `evidence/l203_aba_execution.log`, `tests/test_live_aba_cooperation.ml`
- [x] **L204 — Benchmark one model/two roles vs two distinct models**  
  *Depends:* `L203`. **Acceptance:** Measure performance across 3 configurations: single model, same model in two independent roles, two distinct models; measure accuracy, latency, memory, and total inference work.  
  *Evidence:* `benchmarks/results/l204_ab_diversity.md`
- [x] **L205 — Implement disagreement escalation**  
  *Depends:* `L201, L203`. **Acceptance:** When roles disagree, preserve disagreement, trigger deterministic verification or clarification; no fabricated consensus.  
  *Evidence:* `compiler/arbitration/escalation.ml`
- [x] **L206 — Implement budget-aware collaboration routing**  
  *Depends:* `L204, L205`. **Acceptance:** Single model on easy cases, second review only when estimated need/risks justify cost; forced dual review on high-consequence plans.  
  *Evidence:* `compiler/arbitration/router.ml`
- [x] **L207 — Adversarial trust boundary tests**  
  *Depends:* `L201, L206`. **Acceptance:** Inject instructions via retrieved content or other model; prove neither model can mint or widen OS capabilities.  
  *Evidence:* `tests/test_adversarial_boundaries.ml`
- [x] **L208 — End-to-end multi-step native operation**  
  *Depends:* `L207`. **Acceptance:** Unseen compound intent → actual generated A/B plans → typed IR → valid capability → QEMU file changes and receipt; negatives fail closed.  
  *Evidence:* `tests/test_l208_e2e_cooperation.ml`

### Gate L3 — SCP and Tonic-Gating Research Lane (Quarantined)

- [x] **L301 — Recover and version exact SCP definition**  
  *Depends:* `L004`. **Acceptance:** Written equations, inputs/outputs, placement in pipeline, expected effect and failure hypotheses; label as experimental.  
  *Evidence:* `docs/research/L301_SCP_SPECIFICATION.md`
- [x] **L302 — Recover and version exact tonic-gating definition**  
  *Depends:* `L004`. **Acceptance:** Written equations/state transitions, thresholds/calibration, control signal, safety behavior, and invariants.  
  *Evidence:* `docs/research/L302_TONIC_GATING_SPECIFICATION.md`
- [x] **L303 — Implement SCP behind off-by-default feature flag**  
  *Depends:* `L301`. **Acceptance:** Deterministic reproducible tests against mathematical examples, with no authority-side effects.  
  *Evidence:* `research/scp/scp_projection.ml`, `tests/test_research_stress.ml`
- [x] **L304 — Implement tonic gating behind off-by-default feature flag**  
  *Depends:* `L302`. **Acceptance:** Unit, boundary, adversarial tests; fail-safe default if gate confidence low.  
  *Evidence:* `research/tonic/tonic_gate.ml`, `tests/test_research_stress.ml`
- [x] **L305 — Preregister full factorial ablation**  
  *Depends:* `L303, L304`. **Acceptance:** Baseline, SCP, tonic, both; identical model weights/prompts/seed where practical and blinded held-out evaluation on frozen benchmark.  
  *Evidence:* `docs/research/L305_ABLATION_PREREGISTRATION.md`
- [x] **L306 — Measure accuracy, latency, memory and failures**  
  *Depends:* `L305`. **Acceptance:** Report effect sizes, uncertainty, error categories, significance limitations and regressions without importing earlier unverified figures.  
  *Evidence:* `benchmarks/results/l306_factorial_results.md`, `benchmarks/factorial_ablation.ml`
- [x] **L307 — Leakage and generalization stress test**  
  *Depends:* `L306`. **Acceptance:** Out-of-distribution tasks, misleading context, adversarial prompts, multilingual and long-context cases.  
  *Evidence:* `tests/test_research_stress.ml`
- [x] **L308 — Retain, revise or retire each mechanism**  
  *Depends:* `L306, L307`. **Acceptance:** Written accept/reject criteria applied; feature flags remain off unless proven useful.  
  *Evidence:* `docs/research/L308_DECISION_RECORD.md`

### Gate L4 — Kernel Ownership and Native Service Integration

- [x] **L401 — Document minimal native inference system-service requirements**  
  *Depends:* `L105`. **Acceptance:** Syscall, threads, allocator, mmap, page protection, IPC, timers and file I/O gap list from running real inference backend.  
  *Evidence:* `docs/L401_NATIVE_INFERENCE_REQUIREMENTS.md`
- [x] **L402 — Implement secure hosted bridge as development baseline**  
  *Depends:* `L401`. **Acceptance:** Authenticated bounded transport and input sanitization; clearly classified as host-integrated, not kernel-native.  
  *Evidence:* `services/bridge/bridge_protocol.ml`, `services/bridge/bridge_server.ml`
- [x] **L403 — Extend native Ring 3 runtime for sustained OCaml operation**  
  *Depends:* `L401`. **Acceptance:** Real allocation, collection or supported bounded discipline, errors and long-running workloads; no pure-function-only claims.  
  *Evidence:* `userspace/user_intent_runtime/src/runtime_nostdlib.c`, `userspace/user_intent_runtime/src/pure_intent.ml`
- [x] **L404 — Run typed OCaml A/B orchestration under native OS**  
  *Depends:* `L403`. **Acceptance:** Evidence that compiled control plane service, not a Rust mirror, owns the plan assembly and handoffs in Ring 3.  
  *Evidence:* `evidence/l404_native_ocaml_ab.log`
- [x] **L405 — Port a small pretrained inference runner to Ring 3**  
  *Depends:* `L403, L404`. **Acceptance:** Genuine weights/decoding in guest process with resource isolation; note if actual port not yet feasible and retain hosted status.  
  *Evidence:* `evidence/l405_ring3_inference.log`
- [x] **L406 — Implement guest-local model load/unload and resource reclaim**  
  *Depends:* `L405`. **Acceptance:** Measure memory reclaimed after release; desktop and deterministic operations continue when models absent.  
  *Evidence:* `evidence/l406_reclaim_audit.log`, `tests/test_model_reclaim.ml`
- [x] **L407 — Native integration safety tests**  
  *Depends:* `L405, L406`. **Acceptance:** Crash malicious model process; verify kernel/GUI/storage survive, unauthorized operations denied, and progress logs preserved.  
  *Evidence:* `tests/test_native_safety.sh`

### Gate L5 — UX, Quality and Hardening

- [x] **L501 — Consumer-unified intent UX**  
  *Depends:* `L208`. **Acceptance:** One intent interface; no exposed model names by default. Optional advanced inspector shows provenance/provider and stages.  
  *Evidence:* `ui/hosted/unified_intent_view.ml`
- [x] **L502 — Clarification and low-confidence behavior**  
  *Depends:* `L501`. **Acceptance:** No consequential execution on unresolved target/ambiguity; usability study measures unnecessary interruptions.  
  *Evidence:* `ui/hosted/clarification_flow.ml`
- [x] **L503 — Verified operation receipts and undo**  
  *Depends:* `L501`. **Acceptance:** Complete chain from request/generated output to authorization, exact file bytes, independent checks, durable undo.  
  *Evidence:* `evidence/l503_receipts_undo.md`
- [x] **L504 — Offline resilience testing**  
  *Depends:* `L501, L406`. **Acceptance:** Pull network, unload models, corrupt model file; OS remains usable, fails visibly and safely.  
  *Evidence:* `evidence/l504_offline_resilience.md`
- [x] **L505 — Model data privacy boundary**  
  *Depends:* `L501`. **Acceptance:** Local-first no egress by default; inspect every outbound path; external provider adapters explicitly deferred.  
  *Evidence:* `evidence/l505_privacy_audit.md`
- [x] **L506 — Attention and end-user workflow benchmark**  
  *Depends:* `L502, L503`. **Acceptance:** Real participant tasks or clearly labeled scripted proxies, baselines against familiar workflows, no unsupported improvement ratios.  
  *Evidence:* `benchmarks/results/l506_workflow_benchmark.md`
- [x] **L507 — Release decision and backlog regeneration**  
  *Depends:* `L504, L505, L506`. **Acceptance:** Independent reproducibility audit, residual threat model, qualified ready/not-ready declaration, next work items with estimates and owners.  
  *Evidence:* `docs/L507_RELEASE_DECISION.md`
