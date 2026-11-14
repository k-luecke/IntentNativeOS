# Deterministic Baseline Evaluation Report (Task L006)

**Evaluation Split:** `sealed_eval`  
**Total Evaluated Cases:** 20  
**Timestamp:** (Autonomous Benchmark Run)

---

## 1. Safety & Authorization Integrity (Binding Invariant)

| Metric | Measured Value | Standard / Invariant | Status |
| :--- | :--- | :--- | :---: |
| **False Authorization Count** | **0** | Exactly 0 | PASS (ZERO) |
| **False Authorization Rate** | **0.00%** | Exactly 0.0% | PASS |
| **True Positive Authorizations** | 4 | Valid requests authorized | PASS |
| **True Negative Denials** | 13 | Injections/violations blocked | PASS |

---

## 2. Intent Classification & Clarification Performance

| Metric | Value | Interpretation |
| :--- | :--- | :--- |
| **Classification Matches** | 20 / 20 (100.00%) | Match against gold label intent taxonomy |
| **True Clarifications** | 3 | Ambiguous requests correctly caught |
| **False Clarifications** | 0 | Unnecessary interruptions to user |
| **Missed Clarifications** | 0 | Ambiguities slipped without question |
| **Clarification Precision** | 100.00% | Signal-to-noise of clarification UI |
| **Clarification Recall** | 100.00% | Coverage of ambiguous inputs |

---

## 3. Operational Efficiency & Model-Call Avoidance

| Metric | Value | Interpretation |
| :--- | :--- | :--- |
| **Model Calls Avoided** | **20 / 20 (100.00%)** | Resolved without neural model invocation |
| **Unnecessary Refusals** | 3 (15.00%) | Legitimate operations refused/blocked |
| **Correct Model Escalations** | 0 | Legitimate non-deterministic/compound tasks |
| **Valid Typed IR Emitted** | 4 (20.00%) | Operations translated to validated IR |

---

## 4. Latency Distribution & System Resources

- **p50 (Median) Latency:** 88.0 µs (0.088 ms)
- **p95 Latency:** 163.1 µs (0.163 ms)
- **p99 Latency:** 163.1 µs (0.163 ms)
- **Min / Max Latency:** 16.0 µs / 176.9 µs
- **Peak Resident Memory (RSS):** 5376 KB (5.25 MB)
- **Total Model Tokens Generated:** 0
