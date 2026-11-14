# L308 — Architectural Decision Record: SCP and Tonic-Gating Evaluation

**Document ID:** ADR-L308-RESEARCH-EVAL-01  
**Target Gate:** Gate L3 (Quarantined Research Lane)  
**Date:** October 9, 2026  
**Status:** Accepted Empirical Decision

---

## 1. Context & Objectives

During Gate L3, two experimental mechanisms previously proposed for the Intent-Native OS were formalized, implemented behind off-by-default feature flags, and subjected to a preregistered $2 \times 2$ factorial ablation over 80 unsealed benchmark cases (60 dev + 20 val):
1. **Structured Capsule Projection (SCP):** Pre-sampling logit simplex projection onto grammar DFA subspaces.
2. **Tonic Gating:** Dynamical scalar modulation of execution depth based on intent ambiguity, consequence risk, and system pressure.

---

## 2. Empirical Findings Summary (from L306 Factorial Ablation)

| Configuration | Condition | Model Invocations | Model Calls Avoided | False Authorizations | Mean Latency | Safety Invariant Held |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| **Control Baseline** | C00 | 40 | 46 / 80 (57.5%) | 0 (0.0%) | 94.3 µs | YES |
| **SCP Isolated** | C10 | 40 | 46 / 80 (57.5%) | 0 (0.0%) | 87.5 µs | YES |
| **Tonic Isolated** | C01 | 71 | 15 / 80 (18.8%) | 0 (0.0%) | 89.4 µs | YES |
| **Joint (SCP+Tonic)**| C11 | 71 | 15 / 80 (18.8%) | 0 (0.0%) | 84.9 µs | YES |

---

## 3. Evaluation Against Preregistered Criteria

### 3.1 Structured Capsule Projection (SCP)
- **Mathematical Soundness:** Verified. Simplex projection enforces exact probability mass conservation on allowed grammar tokens.
- **Accuracy / Safety Impact:** 0.0% false authorizations maintained.
- **Latency / Performance:** Added negligible overhead (< 3.5 µs); does not degrade throughput.
- **Utility Assessment:** In our architecture, the typed `DeterministicVerifier` and AST compiler already parse and validate candidate structures with 100% precision post-generation. Pre-sampling logit projection adds architectural complexity without yielding a measurable reduction in false authorizations on the current model tier.
- **Formal Decision:** **REVISE & QUARANTINE.**
  - The implementation is retained in `research/scp/` for continued exploration on open-vocabulary code generation tasks.
  - The feature flag `INOS_ENABLE_SCP` remains **OFF by default** in production and kernel runtimes.

### 3.2 Tonic Gating
- **Safety Invariant Enforcement:** Verified. Destructive intents ($C_{\text{risk}} \ge 0.8$) and uncalibrated states strictly clamp tone to 1.0, triggering full dual review without exception.
- **Efficiency Impact:** Failed target benchmark. Under dynamical tone updates, lexical ambiguity triggers model escalation on cases that the static deterministic baseline (L206) successfully resolves in Ring 3 without neural assistance (yielding 71 model invocations vs. 40 in baseline C00).
- **Formal Decision:** **REVISE & QUARANTINE.**
  - The static consequence-based routing (`compiler/arbitration/router.ml`) established in L206 is retained as the standard OS routing engine.
  - The feature flag `INOS_ENABLE_TONIC_GATING` remains **OFF by default**.

---

## 4. Binding Directives for Gate L4 and L5

1. Neither SCP nor Tonic Gating shall be enabled or required for Gate L4 (Kernel Ownership) or Gate L5 (Hardening & UX).
2. The active production control plane will continue using:
   - Fast deterministic grammar parsing first.
   - Consequence-based dual-model arbitration (Model A $\to$ Model B $\to$ Deterministic Verifier) for destructive/ambiguous operations.
   - Pure Ring 3 typed validation.
3. The 20 `sealed_eval` cases remain 100% untuned and untouched for the final evaluation in Gate L5.
