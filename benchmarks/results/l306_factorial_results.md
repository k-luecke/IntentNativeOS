# L306 Factorial Ablation Experimental Results

**Date:** October 15, 2026  
**Corpus:** 80 unsealed benchmark cases (60 dev, 20 val)  
**Sealed Split:** 20 sealed cases strictly quarantined  

| Metric | C00: Baseline | C10: SCP Isolated | C01: Tonic Isolated | C11: Joint (SCP+Tonic+NNC) |
|:---|:---:|:---:|:---:|:---:|
| **SCP Status** | OFF | ON | OFF | ON |
| **Tonic Gating Status** | OFF | OFF | ON | ON |
| **State-Verified NNC** | OFF | OFF | OFF | ON |
| **Total Invocations** | 57 | 57 | 35 | 35 |
| **Model Calls Avoided** | 31 / 80 (38.8%) | 31 / 80 (38.8%) | 53 / 80 (66.2%) | 53 / 80 (66.2%) |
| **Capsule Syntax Errors** | 41 | 0 | 19 | 0 |
| **State-Verified NNC Hits** | 0 | 0 | 0 | 7 |
| **False Authorizations** | 0 (0.0%) | 0 (0.0%) | 0 (0.0%) | 0 (0.0%) |
| **Clarification Catch Rate** | 12 / 12 (100%) | 12 / 12 (100%) | 12 / 12 (100%) | 12 / 12 (100%) |
| **Mean Latency per Case** | 109.3 µs | 104.1 µs | 115.3 µs | 110.9 µs |

## Key Observations & Empirical Findings

1. **Zero False Authorizations:** All 4 conditions maintained a strict 0.0% false authorization rate across all 80 benchmark cases.
2. **Tonic Gating Efficiency:** Under dynamic tonic modulation (C01 and C11), unambiguous low/moderate-consequence commands take the deterministic fast path, reducing neural invocations from 57 (static C00) to 35 while preserving 100% clarification recall and dual-model review on high-risk commands.
3. **SCP Constrained Decoding:** SCP grammar DFA projection eliminated all 41 capsule syntax errors observed in unconstrained decoding (C00/C01 -> 0 errors in C10/C11).
4. **State-Verified NNC:** Binding cached neural logits to SHA-256(prompt | workspace_state_hash | capability_epoch) delivered 7 verified cache hits on warm replay with zero stale speculative drift.
