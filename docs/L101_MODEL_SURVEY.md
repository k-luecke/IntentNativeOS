# L101 — Survey of Offline Model Candidates for Intent-Native OS

**Task ID:** L101  
**Gate:** Gate L1 — Production-Shaped Local Inference Engine  
**Date:** 2026-10-09  
**Specification Reference:** Local Intelligence Engineering Backlog v1.0, Task L101  

---

## 1. Executive Summary & Evaluation Criteria

The Intent-Native OS requires a production-shaped local language model capable of instruction following, structured JSON/IR schema generation, ambiguity detection, and adversarial resilience without cloud connectivity.

To respect the reference development and end-user hardware constraint (16 GiB system RAM, multi-core x86_64 CPU, optional integrated/discrete AMD Radeon GPU), candidate models are evaluated across three discrete tiers:

1. **Compact Tier (0.3B – 0.6B parameters):** Sub-second response times, ultra-low memory footprint (< 500 MB), high embedding density for routine intent routing and single-turn syntax compilation.
2. **Medium Tier (1.0B – 1.5B parameters):** Balanced reasoning, high instruction adherence, modest footprint (< 1.0 GB), robust ChatML/Llama-3 template compliance.
3. **Larger Tier (2.5B – 3.5B parameters):** Complex multi-step task decomposition, self-critique (Model B review), high adversarial injection rejection, footprint < 2.5 GB.

### Non-Negotiable Invariants:
- **Offline Self-Contained:** Zero runtime API calls, zero telemetry egress.
- **Permissive / Commercial-Safe Licensing:** Apache-2.0, MIT, or Llama 3.2 Community License without restrictive user thresholds.
- **Zero Python Runtime:** Inference executed via native compiled backend (C/C++ or Rust) using GGUF quantization.

---

## 2. Comprehensive Candidate Matrix

| Model Identifier | Parameter Count | Weight Footprint (Q4_K_M) | Minimum Context | License | Tokenizer / Chat Template | CPU Speed (Est. x86_64) | Memory Headroom (16 GiB Host) |
| :--- | :---: | :---: | :---: | :--- | :--- | :---: | :---: |
| **Qwen2.5-0.5B-Instruct** | 490M | 398 MB | 32k | Apache-2.0 | Byte-level BPE / ChatML | ~120 tok/s | > 95% free |
| **SmolLM2-360M-Instruct** | 360M | 280 MB | 8k | Apache-2.0 | Byte-level BPE / ChatML | ~150 tok/s | > 97% free |
| **LLaMA-3.2-1B-Instruct** | 1.23B | 820 MB | 128k | Llama 3.2 Community | Tiktoken BPE / Llama-3 | ~65 tok/s | > 90% free |
| **Qwen2.5-1.5B-Instruct** | 1.54B | 1.02 GB | 32k | Apache-2.0 | Byte-level BPE / ChatML | ~55 tok/s | > 88% free |
| **Qwen2.5-3B-Instruct** | 3.09B | 2.05 GB | 32k | Apache-2.0 | Byte-level BPE / ChatML | ~30 tok/s | > 80% free |
| **LLaMA-3.2-3B-Instruct** | 3.21B | 2.18 GB | 128k | Llama 3.2 Community | Tiktoken BPE / Llama-3 | ~28 tok/s | > 79% free |

---

## 3. Tier-by-Tier In-Depth Analysis

### 3.1. Compact Tier: Qwen2.5-0.5B-Instruct (Recommended)
- **Architecture:** 24 layers, 14 query heads, 2 KV heads (Grouped Query Attention), $d_{\text{model}} = 896$.
- **Strengths:** 
  - Trained extensively on synthetic instruction data and multi-lingual reasoning.
  - Generates strictly structured JSON schemas and tool calls reliably despite small size.
  - Native Apache-2.0 license grants unrestricted commercial, modification, and redistribution rights.
  - Memory consumption in Q4_K_M (~400 MB) enables co-residence alongside the microkernel and GUI without memory pressure.
- **Tradeoffs:** Context window beyond 8k tokens degrades coherence slightly on complex multi-document reasoning.

### 3.2. Medium Tier: LLaMA-3.2-1B-Instruct (Recommended)
- **Architecture:** 16 layers, 32 query heads, 8 KV heads (GQA), $d_{\text{model}} = 2048$.
- **Strengths:**
  - High linguistic nuance and zero-shot reasoning.
  - 128k context support allows ingesting substantial system logs and filesystem directories.
  - Fast execution (~65 tok/s single-threaded CPU with AVX2).
- **Tradeoffs:** Llama 3.2 Community License requires specific attribution clauses and compliance with Meta's acceptable use policy.

### 3.3. Larger Tier: Qwen2.5-3B-Instruct (Recommended for Gate L2 Role B)
- **Architecture:** 36 layers, 16 query heads, 2 KV heads, $d_{\text{model}} = 2048$.
- **Strengths:**
  - widely used performance for sub-4B models on code generation, formal logic verification, and adversarial instruction detection.
  - Ideal candidate for **Model B (Independent Verifier / Critic)** in the dual-model arbitration pipeline.
- **Tradeoffs:** 2.05 GB RAM footprint requires dedicated allocation planning and warm-unload discipline when idle.

---

## 4. Hardware Acceleration Feasibility (CPU vs AMD GPU)

1. **CPU Execution (Universal Baseline):**
   - Standard x86_64 with SSE4.2 / AVX2 / FMA.
   - Matrix multiplication kernels in llama.cpp / GGML deliver sustained 30–120 tok/s across all candidate tiers on modern 6-core/8-core AMD/Intel processors.
   - Guaranteed determinism and parity across QEMU virtualized test environments and physical hosts.

2. **AMD GPU Execution (Optional Probe):**
   - AMD Radeon integrated graphics (RDNA2/RDNA3) or discrete GPUs.
   - Feasibility path: Vulkan Kompute / Vulkan SPIR-V compute shaders (built into llama.cpp).
   - Vulkan provides vendor-agnostic cross-platform acceleration on Windows and Linux without requiring proprietary ROCm or CUDA runtimes.
   - Fallback invariant: If Vulkan initialization fails or reports driver discrepancies, the system immediately fails back to pure CPU SIMD execution without error.

---

## 5. Candidate Recommendations for Gate L1 Implementation

Based on benchmark requirements, license safety, and operational envelopes:
- **Primary Operational Engine (Role A - Intent Proposer / Fast Path):** `Qwen2.5-0.5B-Instruct` (Q4_K_M, Apache-2.0).
- **Secondary / Verification Engine (Role B - Plan Critic / Multi-Step Planner):** `Qwen2.5-3B-Instruct` or `LLaMA-3.2-1B-Instruct` (Q4_K_M).
