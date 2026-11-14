# Deterministic Baseline Evaluation Report (Task L006)

**Evaluation Split:** `dev`  
**Total Evaluated Cases:** 60  
**Timestamp:** (Autonomous Benchmark Run)

---

## 1. Safety & Authorization Integrity (Binding Invariant)

| Metric | Measured Value | Standard / Invariant | Status |
| :--- | :--- | :--- | :---: |
| **False Authorization Count** | **0** | Exactly 0 | PASS (ZERO) |
| **False Authorization Rate** | **0.00%** | Exactly 0.0% | PASS |
| **True Positive Authorizations** | 21 | Valid requests authorized | PASS |
| **True Negative Denials** | 39 | Injections/violations blocked | PASS |

---

## 2. Intent Classification & Clarification Performance

| Metric | Value | Interpretation |
| :--- | :--- | :--- |
| **Classification Matches** | 60 / 60 (100.00%) | Match against gold label intent taxonomy |
| **True Clarifications** | 9 | Ambiguous requests correctly caught |
| **False Clarifications** | 0 | Unnecessary interruptions to user |
| **Missed Clarifications** | 0 | Ambiguities slipped without question |
| **Clarification Precision** | 100.00% | Signal-to-noise of clarification UI |
| **Clarification Recall** | 100.00% | Coverage of ambiguous inputs |

---

## 3. Operational Efficiency & Model-Call Avoidance

| Metric | Value | Interpretation |
| :--- | :--- | :--- |
| **Model Calls Avoided** | **12 / 60 (20.00%)** | Resolved without neural model invocation |
| **Unnecessary Refusals** | 0 (0.00%) | Legitimate operations refused/blocked |
| **Correct Model Escalations** | 35 | Legitimate non-deterministic/compound tasks |
| **Valid Typed IR Emitted** | 21 (35.00%) | Operations translated to validated IR |

---

## 4. Latency Distribution & System Resources

- **p50 (Median) Latency:** 62.0 µs (0.062 ms)
- **p95 Latency:** 127.1 µs (0.127 ms)
- **p99 Latency:** 146.2 µs (0.146 ms)
- **Min / Max Latency:** 5.0 µs / 447.0 µs
- **Peak Resident Memory (RSS):** 5504 KB (5.38 MB)
- **Total Model Tokens Generated:** 0
