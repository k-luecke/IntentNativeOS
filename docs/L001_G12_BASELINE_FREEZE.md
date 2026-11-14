# L001 — Gate G12 Known-Good Reproducible Baseline Freeze

**Milestone:** Gate G12 Live Intelligence Integration  
**Commit Hash:** `85e8848dec37fe5d72c80629aa901e7fe4c8a469`  
**Date Frozen:** 2026-10-09  
**Specification Reference:** Local Intelligence Engineering Backlog v1.0, Task L001  

---

## 1. Executive Summary

This document establishes the frozen, known-good reference baseline for the Intent-Native OS following the successful completion and verification of Gate G12.

Gate G12 proved the complete vertical path from a genuine, local pretrained transformer generating tokens without cloud APIs, through typed OCaml intent parsing and capability validation, across a dedicated disk transport spool (LBA 24,576+), into the custom Rust microkernel, terminating in bit-for-bit persistent file modification surviving cold reboot in QEMU.

All subsequent local intelligence development in Gates L0 through L5 builds upon the guarantees, interfaces, and boundary classifications frozen in this baseline.

---

## 2. Artifact Digests & File Manifest

| Artifact | Relative Path | Size (Bytes) | SHA-256 Digest |
| :--- | :--- | :--- | :--- |
| **Model Weights** | `models/pretrained/stories15M.bin` | 60,808,684 | `cd590644d963867a2b6e5a1107f51fad663c41d79c149fbecbbb1f95fa81f49a` |
| **Tokenizer Model** | `models/pretrained/tokenizer.bin` | 304,197 | `50a52ef822ee9e83de5ce9d0be0a025a773d019437f58b5ff9dcafb063ece361` |
| **Inference Engine** | `models/pretrained/run.c` | 20,494 | `9c4f2d5c6ae01b71726d1cc37530d71e60bff0ec7cc012565f16a43c1ca658bd` |
| **Spool Injector** | `tools/spool_injector.c` | 6,367 | `5401a1290c9a3d2ea368aaf38b14eb8d98a47c267f1ce95b5a19f6c5846b7a2a` |
| **Microkernel Source** | `kernel/src/main.rs` | 42,912 | `6e828e679b3ee581ea19e1fa0a59929288e28080f08969f64bf5052309ec98f1` |
| **Disk Transport Spec** | `docs/G12_LIVE_INTELLIGENCE_INTEGRATION.md` | 13,858 | `b7be16fe597371f8eec4a29a0076a0815132338ecb73eb92e59df6231be4d852` |

---

## 3. Reproducible Verification Commands

All baseline tests run entirely offline with zero external network access and zero Python dependencies.

### 3.1. OCaml Test Suite & Live Intelligence Pipeline
```bash
# Environment: Linux / WSL2 Ubuntu (OCaml 4.14.1+, Dune 3.14.0+)
cd .
dune runtest
```
*Expected Output:*
- `test_live_intelligence_pipeline.ml` executes 4 end-to-end integration phases:
  1. Concrete prompt 1 (`system_status.log`) -> 40 tokens -> valid typed IR (provenance hash `c60ebfc...`).
  2. Concrete prompt 2 (`notice_board.txt`) -> 40 tokens -> distinct typed IR.
  3. Deliberately ambiguous prompt -> safely halts without operation emission (`No target filename...`).
  4. Spool serialization -> 2 sectors at LBA 24,576+ with `INT_` magic.
- All 15+ sub-suites pass (`=== [ALL PASS] Gate G12 Live Intelligence Pipeline Verified ===`).

### 3.2. C Inference Engine Execution
```bash
# Build standalone C inference engine
gcc -O3 -o models/pretrained/run models/pretrained/run.c -lm

# Execute deterministic greedy forward pass (t=0.0)
./models/pretrained/run models/pretrained/stories15M.bin -z models/pretrained/tokenizer.bin -t 0.0 -n 40 \
  -i "Instruction: Write system status log to system_status.log status=healthy"
```
*Expected Output:*
- Deterministic token generation: `Instruction: Write system status log to system_status.log st...`
- Token rate: ~80 to 120 tok/s on CPU single thread.

### 3.3. QEMU Cold-Reboot Persistence & Capability Enforcement
```bash
# Run automated headless integration script in WSL
bash scripts/run_g12_integration.sh
```
*Verifiable Outcomes:*
- QEMU boots UEFI via OVMF (`build/OVMF_VARS.fd`).
- Spool parsed from LBA 24,576 (above filesystem data boundary `FS_DATA_END_LBA = 24576`).
- File `system_status.log` created with 131 bytes.
- Second cold reboot verifies `system_status.log` exists bit-for-bit on disk.
- Capability rejection: malformed or unauthenticated packet (`cap_token = 0`) denied with `PermissionDenied`.

---

## 4. Current Integration Boundaries

| Boundary | Status in G12 Baseline | Architecture Description |
| :--- | :--- | :--- |
| **Model Execution** | Hosted Process | Pretrained Llama-2-15M runs via compiled C binary (`run`) in host environment under OCaml supervisory harness. |
| **Intent Compilation** | Hosted OCaml Service | OCaml control plane parses model output, binds typed IR, enforces schemas, and generates cryptographic provenance. |
| **Spool Transport** | ATA Disk Transport Region | Dedicated LBA 24,576–32,767 (top 4 MB of 16 MB disk), isolated from FS cluster allocator. |
| **Capability Gate** | Kernel Ring 0 / Ring 3 | Microkernel validates `cap_token == 0xCAFE_BABE_0001_0001` and sector checksums before authorizing ATA write. |
| **Filesystem Persistence** | Native INOS-FS | Custom persistent filesystem allocates inode, writes blocks, updates superblock, survives cold reboots. |

---

## 5. Identified Gaps & Open Technical Debt

1. **Host-to-Guest Bridging (GAP-L0-01):** Model execution runs on the host and communicates via disk spool injection rather than executing as a native guest user-space service in QEMU Ring 3.
2. **Deterministic Baseline Coverage (GAP-L0-02):** Lack of a formalized metric distinguishing true operations, true clarifications, and false authorizations across a standardized corpus.
3. **Model Selection & Quantization (GAP-L0-03):** Current model is FP32 15M story generator; needs upgrade to instruction-tuned, quantized 0.5B–3B parameter models for rich intent understanding.
4. **Dual-Model Arbitration Isolation (GAP-L0-04):** A/B coordination logic in OCaml previously used stub/reference implementations rather than live concurrent models with isolated context journals.
5. **Research Feature Quarantine (GAP-L0-05):** Experimental concepts (SCP, tonic gating) require formal preregistration and controlled ablation against the baseline.
