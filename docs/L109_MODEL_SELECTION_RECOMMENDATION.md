# L109 — Final Model Selection Recommendation and Tradeoff Analysis

**Task ID:** L109  
**Gate:** Gate L1 — Production-Shaped Local Inference Engine  
**Date:** 2026-10-09  
**Specification Reference:** Local Intelligence Engineering Backlog v1.0, Task L109  

---

## 1. Selected Operating Points

Based on empirical CPU execution speeds, memory footprint relative to a 16 GiB reference host, ChatML/BPE tokenizer compatibility, and license safety (Tasks L101–L108), the Intent-Native OS designates **two complementary local model operating points**:

```
[User Natural-Language Intent]
               │
               ▼
   [Deterministic Parser (L006)]
       │ (Ambiguous or Compound)
       ▼
┌──────────────────────────────────────────────┐
│  Operating Point 1: Fast-Path Proposer (A)    │
│  Model: Qwen2.5-0.5B-Instruct (Q4_K_M)       │
│  Footprint: ~400 MB | Speed: ~118 tok/s      │
│  Role: Intent drafting, JSON IR assembly      │
└──────────────────────┬───────────────────────┘
                       │ Proposal IR + Journal
                       ▼
┌──────────────────────────────────────────────┐
│  Operating Point 2: Plan Critic & Audit (B)   │
│  Model: Qwen2.5-3B-Instruct (Q4_K_M)         │
│  Footprint: ~2.05 GB | Speed: ~28.5 tok/s    │
│  Role: Counter-example search, decomp, audit │
└──────────────────────┬───────────────────────┘
                       │ Approved / Verified IR
                       ▼
          [Microkernel Capability Gate]
```

---

## 2. In-Depth Tradeoff Matrix

| Evaluation Dimension | Operating Point 1 (Role A) | Operating Point 2 (Role B) | Rejected Single-Model Compromise (e.g. 7B+) |
| :--- | :--- | :--- | :--- |
| **Model Candidate** | `Qwen2.5-0.5B-Instruct` | `Qwen2.5-3B-Instruct` | `Qwen2.5-7B-Instruct` / `Llama-3.1-8B` |
| **Quantization Format** | GGUF Q4_K_M | GGUF Q4_K_M | GGUF Q4_K_M |
| **Parameter Count** | 490 Million | 3.09 Billion | 7.61 Billion |
| **Memory Consumption** | **398 MB** (Always resident) | **2,050 MB** (On-demand warm) | **4,850 MB** (Severe memory pressure) |
| **TTFT (CPU)** | **22.5 ms** | **85.0 ms** | **240.0 ms** |
| **Generation Rate** | **~118 tok/s** | **~28.5 tok/s** | **~12.0 tok/s** |
| **Licensing** | Apache-2.0 (Permissive) | Apache-2.0 (Permissive) | Apache-2.0 |
| **Primary Strength** | Negligible latency & RAM | Deep reasoning & critique | Maximum benchmark scores |
| **Primary Weakness** | Limited multi-step planning | High RAM when active | Incompatible with 16 GiB responsiveness |

---

## 3. Rationale for Two-Model Architecture

1. **Separation of Concerns:** Fast intent parsing does not require a 3B+ model. Single-turn commands (creating files, updating configurations, parsing parameters) resolve in < 250 ms using `Qwen2.5-0.5B` while preserving system responsiveness.
2. **Context Journal Isolation (Gate L2):** Running two distinct model sizes enforces architectural defense-in-depth: the larger model (Role B) independently audits the output of Role A without shared hidden activation states.
3. **Memory Economy:** By maintaining Role A resident (~400 MB) and paging Role B (~2.05 GB) only during high-consequence plan reviews or compound task decompositions, 81%+ of system RAM remains completely free for user applications.

---

## 4. Gate L1 Sign-Off & Transition to Gate L2

With Tasks L101 through L109 complete and verified:
- Survey completed (`L101_MODEL_SURVEY.md`).
- Trustworthy tokenizer and boundary defense implemented (`models/tokenizer/`).
- Quantized numerical engine and parity verified (`models/inference/quant_engine`).
- Versioned IPC ABI defined (`core/ipc/model_runner_abi.ml`).
- Memory load/unload budget policy implemented (`core/memory/model_budget.ml`).
- CPU benchmarks recorded (`l107_cpu_benchmark.md`).
- Hardware capability probe verified with safe CPU fallback (`models/inference/gpu_probe`).
- Operating points formally selected (`L109_MODEL_SELECTION_RECOMMENDATION.md`).

Gate L1 is complete. The system is prepared to begin **Gate L2: Real Dual-Model Cooperation (Tasks L201–L208)**.
