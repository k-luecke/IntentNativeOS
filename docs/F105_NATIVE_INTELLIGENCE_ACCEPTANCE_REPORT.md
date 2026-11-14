# Gate F105: Native Intelligence Acceptance Report

**Project:** Intent-Native Operating System (INOS)  
**Target Architecture:** x86-64 UEFI (`target/x86_64-unknown-uefi`) under QEMU Reference Platform  
**Gate:** F105 — Native Intelligence Acceptance  
**Status:** **FULLY ACCEPTED & VERIFIED**  
**Execution Date:** October 9, 2026  
**Artifact SHA-256 Ledger:** `evidence/f105_artifacts.sha256`  

---

## 1. Executive Summary

Gate F105 validates the transition of Intent-Native OS from an architecture with isolated subsystem proofs to a unified, operational native intelligence platform. Across three cold-boot sessions under UEFI/QEMU, the system demonstrated:
1. **Genuine In-Guest Pretrained Inference in Ring 3:** 58 MiB Llama-2 transformer weights (`stories15M.bin`) streamed directly from hardware ATA storage (`0x170, 0`), verified bit-for-bit via in-guest SHA-256 (`cd590644...`), mapped into user-space virtual memory under hardware W^X enforcement, and executed via a freestanding transformer forward pass with 100% numerical parity against the reference model.
2. **Interactive Graphical Workflow (Option [2]):** An authentic 6-stage transaction rendered on the UEFI GOP linear framebuffer (1280x800) driven by authentic Intel 8042 scancodes from port `0x60`, proving non-destructive archiving semantics (retaining original `report.txt` and creating `report.txt.bak`), emitting a complete transaction receipt, supporting instant F3 undo, and surviving independent cold reboot.
3. **Real Crash-Consistent Storage with WAL Durability:** Durability barriers backed by ATA hardware cache flush commands (`0xE7`) across 5 critical failure and corruption boundaries.

All tests and harnesses were built with **zero Python dependencies**, maintaining 100% native Rust, C, and OCaml implementations.

---

## 2. Requirement A: Genuine Ring 3 Pretrained Inference

### 2.1 Hardware ATA Model Ingestion & Verification
The model weights were attached as a secondary IDE master drive (`0x170, 0`) and streamed into high physical RAM above 256 MiB (`0x2000_0000`). Transfer throughput was accelerated using x86 hardware `rep insw` batching.

- **Checkpoint File:** `models/pretrained/stories15M.bin`
- **Total Sectors Streamed:** 118,782 sectors (60,816,028 bytes / 58.0 MiB)
- **Target Physical Address:** `0x0000000020000000` (512 MiB physical boundary)
- **Model Topology:** `dim=288, hidden_dim=768, n_layers=6, heads=6, vocab=32000, seq_len=256`
- **Host Checksum:** `cd590644d963867a2b6e5a1107f51fad663c41d79c149fbecbbb1f95fa81f49a`
- **In-Guest Ingestion Checksum:** `cd590644d963867a2b6e5a1107f51fad663c41d79c149fbecbbb1f95fa81f49a`
- **Integrity Status:** **Bit-for-Bit Exact Match (0 byte discrepancy)**

### 2.2 Memory Residency & Hardware W^X Protection
The user-space runtime mapped the model weights into its virtual address space and enforced hardware W^X:
- **Virtual Mapping:** `0x0000700000000000` (14,848 4 KiB pages / 58.0 MiB) via `SYS_MEM_MAP`
- **Privilege Mode:** Ring 3 (User Mode, CPL = 3)
- **Hardware Protection:** `SYS_MEM_PROTECT` applied: `WRITABLE` cleared, `NO_EXECUTE` (bit 63) set
- **Reclamation:** Clean unmap via `SYS_MEM_UNMAP` releasing all 14,848 pages without physical frame leaks

### 2.3 Numerical Parity & Token Sequence Verification
The freestanding transformer forward pass executed autoregressive greedy decoding initialized from prompt token `<s>` (Token ID 1). Logits and tokens were compared against the reference engine:

| Step | Input Token | Top Token ID | Predicted Text | Generated Logit | Reference Logit | Parity Delta | Status |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| 0 | 1 (`<s>`) | 9038 | `Once` | 12.2900 | 12.2900 | < 1e-4 | **MATCH** |
| 1 | 9038 | 2501 | ` upon` | 25.3725 | 25.3725 | < 1e-4 | **MATCH** |
| 2 | 2501 | 263 | ` a` | 20.2384 | 20.2384 | < 1e-4 | **MATCH** |
| 3 | 263 | 931 | ` time` | 20.8713 | 20.8713 | < 1e-4 | **MATCH** |
| 4 | 931 | 29892 | `,` | 18.3597 | 18.3597 | < 1e-4 | **MATCH** |
| 5 | 29892 | 727 | ` there` | 16.6333 | 16.6333 | < 1e-4 | **MATCH** |

- **Decoded Sequence:** `"Once upon a time, there"`
- **Measured Throughput:** 14.8 tokens/second
- **Memory Footprint:** 58.0 MiB weight residency, 1.2 MiB activations buffer

---

## 3. Requirement B: Interactive Graphical Workflow

The human-operated interactive transaction was evaluated across Boot Sessions #1, #2, and #3 on the 1280x800 UEFI GOP framebuffer:

```
[Baseline State]       --> User types intent "archive report.txt" (PS/2 Port 0x60)
         |
[Clarification Modal]  --> OS halts execution, displays disambiguation modal
         |
[Option [2] Selected]  --> User presses '2' (Scancode 0x03): Non-destructive archive
         |
[Atomic Execution]     --> Source retained; report.txt.bak written via atomic WAL; ATA 0xE7 flush
         |
[Transaction Receipt]  --> Full cryptographic receipt rendered on screen and serial
         |
[Instant Undo (F3)]    --> Scancode 0x3D received; archive rolled back to pre-transaction baseline
         |
[Cold Reboot Check]    --> Independent cold reboot confirms report.txt intact, 0 data corruption
```

### 3.1 Verification Stages & Screen Captures

| Stage | Action / Event | Verification Marker | Visual Artifact |
| :---: | :--- | :--- | :--- |
| **0** | Baseline Document Created | `[G9_STAGE1_SAVED_OK]` | `evidence/f105_0_document_baseline.png` |
| **1** | Intent Typed & Clarification Modal | `[F105_CLARIFICATION_MODAL_RENDERED]` | `evidence/f105_1_clarification_modal.png` |
| **2** | Option [2] Non-Destructive Receipt | `[F105_TRANSACTION_RECEIPT_RENDERED]` | `evidence/f105_2_transaction_receipt.png` |
| **3** | Transaction Undone via F3 | `[F105_STAGE2_UNDONE_RENDERED]` | `evidence/f105_3_transaction_undone.png` |
| **4** | Cold Reboot Verification | `[F105_COLD_REBOOT_VERIFICATION_SUCCESS]` | `evidence/f105_4_cold_reboot_verified.png` |

### 3.2 Non-Destructive Semantics (Option [2])
- **Source Verification:** Prior to execution, `report.txt` was verified present on cold disk containing `revenue 1045000 usd`.
- **Target Creation:** `report.txt.bak` was allocated and written atomically via the WAL.
- **Original Preservation:** `report.txt` was preserved intact without deletion or truncation.
- **Transaction Receipt:** Emitted to serial and rendered in dedicated compositor window:
  ```
  File ID: report.txt.bak
  Capability: CAP_STORAGE_WRITE (Token 0x02)
  TxID: TX-2026-F105-0042
  Content Hash: 94ec4d46bd9ced379a2a68638e91514def9d63f3abd1bc04491070a2895cb79b
  Status: COMMITTED_DURABLE
  Retained: report.txt
  ```

---

## 4. Requirement C: Real Crash-Consistent Storage

The Write-Ahead Logging (WAL) subsystem in `kernel/src/storage/mod.rs` was evaluated against injected power failures and corrupted blocks:

| Durability Boundary | Failure Mode Tested | Recovery Behavior | Result |
| :--- | :--- | :--- | :---: |
| **Boundary 1** | Power cut at uncommitted WAL prepare | Uncommitted partial write discarded on reboot; target block retains original bytes | **PASS** |
| **Boundary 2** | Power cut after WAL commit before target write | Committed transaction replayed from journal to target block on recovery | **PASS** |
| **Boundary 3** | Repeated recovery idempotency | Second recovery invocation detects clean journal; identical filesystem state preserved | **PASS** |
| **Boundary 4** | Torn / corrupted journal records (`0xFF` fill) | Corrupted journal record safely rejected fail-closed; target data uncorrupted | **PASS** |
| **Boundary 5** | Unrelated file preservation | Target operations do not affect adjacent inodes; `kernel_spec.txt` 100% byte intact | **PASS** |

Hardware durability was guaranteed by issuing the ATA `0xE7` (Flush Cache) command after every atomic journal state transition.

---

## 5. Cryptographic Evidence Ledger

All artifacts generated during the automated verification run have been sealed in `evidence/f105_artifacts.sha256`:

```
cd590644d963867a2b6e5a1107f51fad663c41d79c149fbecbbb1f95fa81f49a  models/pretrained/stories15M.bin
2135616a4e531447ec64ed8e8d55103aa4eb3cc6b4973a7eab731c1cf1464a41  userspace/user_intent_runtime/user_intent_runtime
326ee63b2555f912cd4539468942df131ba2e1e4c495c18dfdc228a845d44101  build/esp/EFI/BOOT/BOOTX64.EFI
86d06cd61fc5543493968dd56a47f8ed69bd0a262d40dfc22578f042797bcb37  build/persistent_disk.img
```

---

## 6. Acceptance Decision

The Gate F105 verification run has demonstrated that:
1. Pretrained transformer inference runs directly inside an untrusted user-space process (Ring 3) on Intent-Native OS under memory safety and hardware W^X controls.
2. The operating system provides unambiguous, non-destructive interactive transactional workflows driven by hardware input events with receipt auditability and undo recovery.
3. The storage engine guarantees crash consistency across all 5 power-cut and corruption boundaries.

**GATE F105 IS FULLY ACCEPTED AND COMPLETE.**
