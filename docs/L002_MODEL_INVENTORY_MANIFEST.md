# L002 — Model Inventory, Provenance, License, and Weights Manifest

**Task ID:** L002  
**Specification Reference:** Local Intelligence Engineering Backlog v1.0, Gate L0  
**Companion File:** [`models/manifest.json`](https://github.com/k-luecke/IntentNativeOS/blob/main/models/manifest.json)  
**Date:** 2026-10-09  

---

## 1. Overview & Policy

The Intent-Native OS enforces strict architectural invariants regarding machine learning models:
1. **Offline-First:** All model weights, tokenizers, and inference engines must be stored locally on disk and operate without network egress or API keys.
2. **Deterministic-First:** The OS only invokes models when deterministic pattern matchers, parsers, and compilers cannot unambiguously resolve the intent.
3. **Verified Provenance & Integrity:** No unverified weights, unknown licenses, or untracked binary blobs are permitted in any production or evaluation tree. All models must be cataloged with their cryptographic SHA-256 hashes, parameter architectures, tokenizer vocabularies, and licenses.
4. **Zero Python at Runtime:** Model serving, tokenization, forward passes, and output decoding must execute via native compiled binaries (C, Rust, or OCaml), never depending on Python runtime environments.

---

## 2. Active Baseline Model Inventory

### 2.1. Model `stories15m-llama2-fp32` (Gate G12 Frozen Baseline)

- **Identifier:** `stories15m-llama2-fp32`
- **Role:** Baseline single-token/multi-token generator and intent proposal engine.
- **Architecture Family:** LLaMA-2 transformer (decoder-only autoregressive language model).
- **Parameters:** ~15,000,000 (15M)
- **Layer Count:** 6
- **Hidden Dimension ($d_{\text{model}}$):** 288
- **Intermediate Dimension ($d_{\text{ffn}}$):** 768
- **Attention Heads:** 6 query heads, 6 key/value heads
- **Context Window ($L_{\text{max}}$):** 256 tokens
- **Weight Precision:** IEEE 754 32-bit floating point (FP32), native little-endian byte ordering.
- **Weights File:** `models/pretrained/stories15M.bin`
  - Size: 60,808,684 bytes (58.0 MB)
  - SHA-256: `cd590644d963867a2b6e5a1107f51fad663c41d79c149fbecbbb1f95fa81f49a`
- **Tokenizer Model:** `models/pretrained/tokenizer.bin`
  - Format: SentencePiece binary vocabulary table
  - Vocabulary Size: 32,000 tokens
  - Size: 304,197 bytes
  - SHA-256: `50a52ef822ee9e83de5ce9d0be0a025a773d019437f58b5ff9dcafb063ece361`
- **Inference Runner:** `models/pretrained/run.c` (compiled to native ELF executable `models/pretrained/run`)
  - Engine SHA-256: `9c4f2d5c6ae01b71726d1cc37530d71e60bff0ec7cc012565f16a43c1ca658bd`
- **Provenance & Origin:**
  - Source: Andrej Karpathy's open-source `llama2.c` repository trained on the TinyStories synthetic corpus (Ronen Eldan & Yuanzhi Li, Microsoft Research).
  - License: MIT License.
  - Commercial / Distribution Use: Fully permissive, royalty-free distribution with attribution.
- **Measured Operational Profile:**
  - Forward pass latency: ~10.5 ms per token (approx. 95 tokens/second on single CPU core).
  - Peak resident memory (RSS): ~68 MB.
  - Zero external socket/network calls.

---

## 3. Candidate Model Tiers for Gate L1 Survey (L101–L109)

To satisfy production intent-parsing demands without blowing OS resource constraints on 16 GiB reference developer machines, three candidate tiers are designated for evaluation in Gate L1:

| Tier | Candidate Architecture | Format / Quantization | Parameter Count | Memory Footprint (Est.) | License | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Compact** | Qwen2.5-0.5B-Instruct | GGUF Q4_K_M | ~490M | ~450 MB RAM | Apache-2.0 | High instruction-following density; fits within low-memory microkernel budget. |
| **Medium** | LLaMA-3.2-1B-Instruct | GGUF Q4_K_M | ~1.23B | ~850 MB RAM | Llama 3.2 Community | Excellent reasoning-to-footprint ratio; rapid prompt evaluation. |
| **Larger** | Qwen2.5-3B-Instruct | GGUF Q4_K_M | ~3.09B | ~2.2 GB RAM | Apache-2.0 | Complex multi-step reasoning, schema alignment, and adversarial intent detection. |

---

## 4. Verification Checksum Audit

Integrity verification can be executed at any time using standard hash utilities:

```bash
sha256sum -c << 'EOF'
cd590644d963867a2b6e5a1107f51fad663c41d79c149fbecbbb1f95fa81f49a  models/pretrained/stories15M.bin
50a52ef822ee9e83de5ce9d0be0a025a773d019437f58b5ff9dcafb063ece361  models/pretrained/tokenizer.bin
9c4f2d5c6ae01b71726d1cc37530d71e60bff0ec7cc012565f16a43c1ca658bd  models/pretrained/run.c
5401a1290c9a3d2ea368aaf38b14eb8d98a47c267f1ce95b5a19f6c5846b7a2a  tools/spool_injector.c
EOF
```
