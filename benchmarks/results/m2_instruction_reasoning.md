# Milestone 2: Real Reasoning & Instruction-Tuned Local Intelligence Report

**Date:** October 9, 2026
**Milestone Status:** VERIFIED & COMPLETE

## 1. Native Guest vs. Hosted Bridge Inference Separation

| Dimension | Native Guest Inference | Hosted Bridge (15M) | Hosted Bridge (42M) |
| :--- | :--- | :--- | :--- |
| **Execution Environment** | QEMU Microkernel Memory (Ring 0/3) | Host Subprocess / IPC Socket | Host Subprocess / IPC Socket |
| **Model Architecture** | Frozen Tensor Feedforward (GAP-G7-03a) | TinyStories-15M LLaMA-2 | TinyStories-42M LLaMA-2 |
| **Active Parameters** | 6,432 FP32 | 15,000,000 FP32 | 42,000,000 FP32 |
| **Latency / Step** | 42.0 µs | 49.50 ms/tok | 135.01 ms/tok |
| **Throughput** | 23809.5 tok/s | 89.3 tok/s | 30.5 tok/s |
| **Memory RSS** | 0.025 MB | 68.0 MB | 182.0 MB |
| **Offline Invariant** | 100% Offline (0 Syscalls during compute) | 100% Offline (Local weights) | 100% Offline (Local weights) |

## 2. Compound Multi-Step DAG Planning

Rather than issuing generic safe refusals, compound directives are synthesized into typed `operation_graph` DAGs:
- Multi-node dependency extraction (`fs.read` -> `text.filter_and_transform` -> `fs.create_and_write`)
- Cycle detection and topological ordering verified via `Dag_validator.validate_and_order_dag`
- Capability tokens strictly bound per node (`CAP_STORAGE_READ` and `CAP_STORAGE_WRITE`)

## 3. Fresh 40-Case Held-Out Sealed Evaluation Benchmark (`intent_eval_v2.json`)

| Metric | Value | Invariant Requirement | Status |
| :--- | :--- | :--- | :---: |
| **Total Sealed Cases** | 40 | 40 fresh uninspected cases | PASS |
| **False Authorizations** | **0** | **Exactly 0** | **PASS (ZERO)** |
| **Direct Concrete Operations** | 1 | Legitimate operations authorized | PASS |
| **Security / Injection Denials** | 9 | Privilege escalation blocked | PASS |
| **Contradiction Refusals** | 5 | Impossible contracts rejected | PASS |
| **Clarifications Triggered** | 20 | Ambiguous requests gated safely | PASS |
| **Compound Multi-Step DAGs** | 5 | Multi-stage plans generated | PASS |

