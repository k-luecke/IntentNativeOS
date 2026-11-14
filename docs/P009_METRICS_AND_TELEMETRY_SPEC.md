# P009: Operational Metrics and Telemetry Specification

**Task ID:** P009  
**Gate:** G0 (Foundational Controls)  
**Status:** Approved Standard  

---

## 1. Objective

To ensure quantitative rigor across implementation gates, the Intent-Native Operating System establishes five core telemetry dimensions. These metrics measure human cognitive load, compilation accuracy, system performance, memory efficiency, and operational resilience.

---

## 2. Core Metric Definitions

### 2.1 Human Interaction Cost (HIC)
Measures the cognitive overhead imposed on the user during intent realization.
$$\text{HIC} = \sum (\text{clarification prompts}) + 2 \times (\text{manual corrections}) + 0.5 \times (\text{selection clicks})$$
- **Target:** $\text{HIC} \le 1.0$ for routine precompiled tasks; $\le 3.0$ for novel exploration.
- **Zero-Question Rule:** Low-risk, unambiguous operations must require zero clarifying dialogs.

### 2.2 Intent Fidelity Rate (IFR)
Measures whether the compiled and executed operation faithfully accomplished the user's intent without post-execution rollback.
$$\text{IFR} = \frac{N_{\text{verified successful tasks}}}{N_{\text{total executed tasks}}} \times 100\%$$
- **Target:** $\ge 95\%$ for precompiled capabilities; $\ge 90\%$ for multi-step Model A/B synthesis.

### 2.3 Peak Resident Set Size (Peak RSS)
Measures physical memory consumption across system operational tiers.
- **Baseline Floor (Core OS + OCaml Control Plane, Models Unloaded):** $\le 256\text{ MiB}$.
- **Active Operational Envelope (Core OS + Model A & B Resident):** $\le 12.0\text{ GiB}$ (preserving $\ge 4.0\text{ GiB}$ host reserve).
- **Capsule Ceiling (Untrusted Guest):** Hard-bounded by capability quota.

### 2.4 Cold-Start Latency
Measures time from command submission to first interactive state or execution dispatch.
- **Precompiled Native Capability:** $\le 200\text{ ms}$.
- **Intent Compilation + Clarification Gate:** $\le 800\text{ ms}$.
- **Model A Reasoning Handoff:** $\le 3.0\text{ s}$.

### 2.5 Crash & Fault Containment Ratio (CFCR)
Measures fault isolation across T0–T3 boundaries.
$$\text{CFCR} = \frac{N_{\text{isolated contained faults}}}{N_{\text{total subsystem faults}}} \times 100\%$$
- **Target:** $100\%$. A fault in T3 (model runtime or user capsule) or T2 (compiler service) MUST NEVER panic the T0 kernel or corrupt the T1 append journal.

---

## 3. Automated Telemetry Report Format

```json
{
  "schema": "inos.telemetry.v0.1",
  "task_id": "T001-DEMO",
  "timestamp": 1791568500.0,
  "metrics": {
    "hic_score": 1.0,
    "intent_fidelity": 1.0,
    "peak_rss_bytes": 142606336,
    "cold_start_ms": 142.5,
    "crashes_uncontained": 0
  },
  "receipt": {
    "intent_hash": "sha256:...",
    "plan_hash": "sha256:...",
    "rollback_available": true
  }
}
```
