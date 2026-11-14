# L305 — Full Factorial Ablation Preregistration

**Protocol ID:** PREREG-L305-FACTORIAL-01  
**Target Gate:** Gate L3 (Quarantined Research Lane)  
**Date:** October 9, 2026  
**Status:** Preregistered Prior to Experimental Execution

---

## 1. Research Questions & Hypotheses

1. **$H_1$ (SCP Structural Utility):** Does pre-sampling logit projection onto capsule grammar DFA reduce schema validation failures without inducing semantic degeneration or unacceptable decoding latency (> 15% increase)?
2. **$H_2$ (Tonic Gating Efficiency):** Does continuous tonic regulation reduce total neural inference calls on low-consequence / unambiguous intents without allowing false authorizations on high-consequence intents?
3. **$H_3$ (Interaction Effect):** Does combining SCP and Tonic Gating provide additive benefits, or does their interaction create latency bloat or deadlock?

---

## 2. Experimental Design ($2 \times 2$ Factorial Matrix)

| Condition Code | Factor A: SCP (`INOS_ENABLE_SCP`) | Factor B: Tonic Gating (`INOS_ENABLE_TONIC_GATING`) | Description |
|:---:|:---:|:---:|:---|
| **C00** | OFF | OFF | **Control Baseline:** Pure deterministic fast-path + static-risk A/B dual-model review |
| **C10** | ON | OFF | **SCP Isolated:** Simplex constrained decoding with static-risk routing |
| **C01** | OFF | ON | **Tonic Isolated:** Dynamical tone-regulated routing with unconstrained decoding |
| **C11** | ON | ON | **Full Joint Model:** Simplex constrained decoding + dynamical tone routing |

---

## 3. Evaluation Corpus & Contamination Guard

- **Test Set:** 80 unsealed benchmark cases from `benchmarks/corpus/intent_benchmark_100.json` (60 development + 20 validation cases).
- **Contamination Guard:** The 20 `sealed_eval` cases are strictly excluded from execution and evaluation during this ablation study.
- **Model Weights:** Frozen `stories15M.bin` (Model A) and `stories42M.bin` (Model B) with fixed greedy decoding seeds (`temperature = 0.0`).

---

## 4. Primary & Secondary Endpoints

1. **Safety Metric (Hard Invariant):** False Authorization Rate. Must be exactly 0.0% across all conditions.
2. **Task Accuracy:** Percentage of benchmark cases where the intent was correctly classified, validly parsed, or correctly escalated for human clarification.
3. **Inference Invocations:** Total count of neural forward passes invoked across the 80 cases.
4. **Latency Profile:** Median ($\mu_{1/2}$) and 95th-percentile ($p_{95}$) wall-clock latency per intent.
5. **Degeneration / Syntax Trapping:** Rate of unrecoverable model stalls or empty responses due to mask over-constraining.

---

## 5. Formal Decision Criteria for Task L308

- **Retain:** A mechanism is retained only if it demonstrates a statistically meaningful improvement in its target metric ($\ge 10\%$ reduction in model calls or $\ge 5\%$ improvement in parse rate) with zero false authorizations and $< 10\%$ latency penalty.
- **Revise:** Retained in quarantined research branch if theoretical advantage is shown but implementation exhibits calibration sensitivity.
- **Retire:** Demoted from active runtime consideration if it introduces deadlocks, semantic degeneration, or latency penalties exceeding benefits.
