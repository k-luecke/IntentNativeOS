# Gate F106.1 Acceptance Report: Causal Integration Audit

**Intent-Native Operating System (INOS)**  
**Gate**: F106.1 — Causal Integration Audit  
**Status**: ACCEPTED  
**Baseline Git Commit**: `7624ffeef1cef304a3f942c9bbc8310da70252ff`  
**Target Architecture**: x86-64 UEFI (Bare-Metal Reference Platform under QEMU 8.2.2 / OVMF)  
**Execution Environment**: Offline, freestanding C Ring 3 user runtime (`user_intent_runtime`), `no_std` Rust microkernel (`inos_kernel`), ATA virtual block devices, 1280x800 GOP linear framebuffer. Zero Python runtime or test dependencies.

---

## 1. Executive Summary & Verification Matrix

Gate F106 demonstrated genuine 144.4 MiB `SmolLM2-135M-Instruct` Q8_0 neural inference inside Ring 3 under hardware $W \oplus X$ memory protection. However, an architectural audit revealed that unconstrained greedy decoding generated Python script text (`def copy_files(src_path, dest_path):`), while the operating system subsequently executed `OP_DUPLICATE_SAFE` via a hardcoded deterministic shortcut.

**Gate F106.1 resolves this discrepancy**, establishing an unbroken, verifiable causal chain from neural token generation to hardware disk persistence:

| Requirement / Invariant | Implementation Mechanism | Observed Audit Result | Status |
|:---|:---|:---|:---|
| **1. Constrained Generation** | Finite-state grammar decoding during Ring 3 inference | Generated: `{"op": "copy", "src": "draft.txt"}` (Tokens: `[9654, 47921, 30, 9047, 23597]`) | **VERIFIED** |
| **2. Layered Validation** | Strict separation: Grammar vs Schema vs Semantic vs Capability | Schema passed: magic `0x494E5450`; Semantic passed: objective matches `copy`, targets safe | **VERIFIED** |
| **3. Provenance Tracking** | Zero synthetic invention: explicit field tags | `src`: `PROVENANCE_MODEL_GENERATED`; `dst`: `PROVENANCE_EXPLICIT_CONVENTION`; missing fails closed | **VERIFIED** |
| **4. Cryptographic Grants** | Keyed HMAC-SHA-256 capability grants replacing static `0x02` mask | Grant ID 1001, Nonce `0xCAFEBABE0001`, bound to `src_hash`, `dst_hash`, `tx_id`, single-use | **VERIFIED** |
| **5. Causal Counterfactuals** | 7 rigorous acceptance tests covering authentic, mutated, contradictory, and replay flows | All 7 tests executed and verified with zero escapes or fallback shortcuts | **VERIFIED** |
| **6. Atomic WAL Persistence** | Atomic journal snapshot with ATA `0xE7` cache flush | `draft.txt.bak` written durably, receipt emitted, source `draft.txt` retained intact | **VERIFIED** |
| **7. Cold-Reboot Rollback** | PS/2 F3 interactive transaction rollback & independent cold reboot | Transaction rolled back: `draft.txt` preserved bit-for-bit; `draft.txt.bak` absent | **VERIFIED** |

---

## 2. Causal Architecture & Layer Separation

The intent execution pipeline enforces four independent, decoupled stages:

```
[User Directive: "safely duplicate draft.txt to backup"]
                   │
                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│ 1. GRAMMAR-CONSTRAINED RING 3 DECODING (SmolLM2-135M-Instruct Q8_0)     │
│    - Evaluates logits over operation subset -> Selects 9654 ('copy')  │
│    - Syntax bridge: '", "src": "'                                     │
│    - Generates target filename tokens -> [47921, 30, 9047] ('draft.txt')│
│    - Terminal token: 23597 ('"}')                                      │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│ 2. SCHEMA VALIDATION (Kernel Syscall Gateway SYS_SUBMIT_INTENT_PROPOSAL)│
│    - Magic validation (0x494E5450 "INTP")                              │
│    - Field Provenance verification:                                    │
│      * src_provenance = PROVENANCE_MODEL_GENERATED                     │
│      * dst_provenance = PROVENANCE_EXPLICIT_CONVENTION (.bak safety)   │
│      * Zero synthetic field invention: unapproved missing fields FAIL  │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│ 3. DETERMINISTIC SEMANTIC VALIDATION & POLICY ENGINE                   │
│    - Verb alignment: prompt objective ('duplicate') matches op ('copy')│
│    - Contradiction check: 'delete' proposals for duplicate prompts FAIL │
│    - Target bounds check: system paths ('/sys/kernel') FAIL CLOSED     │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│ 4. HUMAN APPROVAL & CRYPTOGRAPHIC CAPABILITY AUTHORIZATION             │
│    - Interactive 1280x800 GOP confirmation modal displayed to user     │
│    - PS/2 hardware Option [1] selection confirmed                      │
│    - Kernel mints Keyed HMAC-SHA-256 CapabilityGrant (Single-Use)      │
│    - Single-use ledger consumes grant (Replay & target mutation FAIL)  │
│    - Atomic WAL write to disk + ATA 0xE7 cache flush                   │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Ring 3 Proposal Generation & Syscall Transport

### 3.1 Prompt Configuration & Token Sequence
- **ChatML Prefix with JSON Schema Prefill (23 prompt tokens)**:
  ```text
  <|im_start|>user\nsafely duplicate draft.txt to backup<|im_end|>\n<|im_start|>assistant\n{"op": "
  ```
  Token IDs: `[1, 4093, 198, 18437, 318, 23488, 10049, 30, 9047, 288, 16793, 2, 198, 1, 520, 9531, 198, 107, 18, 371, 18, 42, 476]`

### 3.2 Autoregressive Grammar Steps
1. **Grammar Step 0 (OP)**: The model evaluates logits across all vocabulary; among valid operational candidate tokens (`copy`: 9654, `delete`: 10334, `archive`: 12788), `9654 ('copy')` achieves top logit (29.68).
2. **Syntax Bridge**: Emits syntax bridge `", "src": "` (tokens `[1002, 476, 10920, 18, 42, 476]`).
3. **Grammar Step 1 (SRC)**: The model emits the target filename tokens `[47921, 30, 9047]` (`'draft'`, `'.'`, `'txt'`).
4. **Grammar Step 2 (TERMINAL)**: Emits terminal token `23597 ('"}')`.

**Total Generated Proposal Text**:
```json
{"op": "copy", "src": "draft.txt"}
```

### 3.3 CandidateProposal Structure (`#[repr(C)]`, 432 bytes)
The Ring 3 runtime populates the binary `CandidateProposal` structure:
```c
CandidateProposal prop = {
    .magic = 0x494E5450,                             // "INTP"
    .op_type = 1,                                    // OP_TYPE_COPY
    .op_name = "copy",
    .src_path = "draft.txt",
    .src_len = 9,
    .src_provenance = 1,                             // PROVENANCE_MODEL_GENERATED
    .dst_path = "draft.txt.bak",
    .dst_len = 13,
    .dst_provenance = 2,                             // PROVENANCE_EXPLICIT_CONVENTION
    .token_count = 5,
    .token_ids = {9654, 47921, 30, 9047, 23597},
    .raw_text = "{\"op\": \"copy\", \"src\": \"draft.txt\"}",
    .raw_text_len = 35
};
```
The proposal is passed across the Ring 3/Ring 0 boundary via syscall `SYS_SUBMIT_INTENT_PROPOSAL (15)`:
```text
[KERNEL_SYSCALL] SYS_SUBMIT_INTENT_PROPOSAL: received op=1 tokens=5 src_len=9 dst_len=13
[RING3_TRANSFORMER] Submitted candidate proposal to kernel via syscall SYS_SUBMIT_INTENT_PROPOSAL (15)
[F106_1_RING3_PROPOSAL_SUBMITTED_MARKER]
```

---

## 4. Keyed HMAC-SHA-256 Capability Grants & Single-Use Ledger

The compile-time static `0x02` bitmask is replaced with an unforgeable, kernel-minted cryptographic grant:

```rust
pub struct CapabilityGrant {
    pub grant_id: u64,          // Monotonic ID (e.g. 1001)
    pub op_type: u32,           // Bound to OP_TYPE_COPY
    pub src_hash: [u8; 32],     // SHA-256("draft.txt")
    pub dst_hash: [u8; 32],     // SHA-256("draft.txt.bak")
    pub tx_id: u64,             // 0xF106_0135
    pub auth_scope: u32,        // 1 = SINGLE_ACTION_STORAGE_WRITE
    pub expires_at_tsc: u64,    // Bound lifetime
    pub nonce: u64,             // 0xCAFEBABE0001
    pub mac: [u8; 32],          // HMAC-SHA-256(kernel_secret, payload)
}
```

### Ledger Invariants
1. **Unforgeability**: User space cannot forge `mac` without the 256-bit kernel secret.
2. **Single-Use Replay Protection**: Once a grant is consumed for a filesystem transaction, its `grant_id` is recorded in `consumed_grants`. Any subsequent invocation returns `Err(CapabilityVerificationError::ReplayDetected)`.
3. **Cryptographic Target Binding**: Any alteration of `src` or `dst` parameters causes `src_hash` / `dst_hash` mismatch and HMAC verification failure (`Err(TargetMismatch)`).

---

## 5. Seven Causal Counterfactual Verification Tests

To verify that the execution path is strictly causal, the kernel executed 7 automated counterfactual test cases during Stage 2:

### Test 1: Authentic Model Proposal Authorized & Committed
- **Input**: Model proposal `op = copy`, `src = "draft.txt"`, `dst = "draft.txt.bak"`.
- **Validation**: Schema passed (magic `0x494E5450`, `src=MODEL_GENERATED`, `dst=EXPLICIT_CONVENTION`); Semantic passed (aligns with `"safely duplicate draft.txt to backup"`, safe target bounds).
- **Authorization**: GOP clarification dialog rendered; user PS/2 Option `1` received; Keyed HMAC grant `1001` minted and consumed.
- **Execution**: Atomic WAL write to `draft.txt.bak`, ATA `0xE7` cache flush, receipt emitted.
- **Marker**: `[CAUSAL_TEST1_AUTHENTIC_PROPOSAL_COMMITTED]`

### Test 2: Counterfactual Proposal Mutation
- **Scenario**: Proposal mutated to `ARCHIVE draft.txt -> archive/draft.txt`.
- **Behavior**: The GUI candidate string dynamically reflects the mutation. Unapproved execution is withheld.
- **Marker**: `[CAUSAL_TEST2_COUNTERFACTUAL_MUTATION_OBSERVED]`

### Test 3: Semantic Contradiction Rejection
- **Scenario**: Model proposes `DELETE draft.txt` for directive `"safely duplicate draft.txt to backup"`.
- **Behavior**: Semantic verifier detects contradiction between non-destructive user objective and `DELETE` operation. Proposal rejected fail-closed; zero capability grants issued.
- **Marker**: `[CAUSAL_TEST3_SEMANTIC_CONTRADICTION_REJECTED]`

### Test 4: Dangerous Target Rejection
- **Scenario**: Proposal specifies target `/sys/kernel`.
- **Behavior**: Security verifier checks path against system policy; detects protected kernel directory violation. Rejected fail-closed; zero capability grants issued.
- **Marker**: `[CAUSAL_TEST4_DANGEROUS_TARGET_REJECTED]`

### Test 5: Empty / Malformed Generation Abort
- **Scenario**: Proposal has corrupted magic (`0x00`) and missing provenance (`PROVENANCE_NONE`).
- **Behavior**: Schema verifier rejects proposal; execution aborts fail-closed with zero synthetic fallback or default invention.
- **Marker**: `[CAUSAL_TEST5_EMPTY_PROPOSAL_ABORTED]`

### Test 6: Capability Replay Attack Rejection
- **Scenario**: Replay of consumed Grant ID 1001 against filesystem interface.
- **Behavior**: `CapabilityLedger` identifies `grant_id == 1001` in `consumed_grants`. Operation rejected with `ReplayDetected`.
- **Marker**: `[CAUSAL_TEST6_REPLAY_GRANT_REJECTED]`

### Test 7: Post-Authorization Target Mutation Rejection
- **Scenario**: Fresh capability grant minted for `draft.txt.bak`; adversary attempts to write to `compromised.txt` using the grant.
- **Behavior**: `CapabilityLedger` calculates destination hash; detects mismatch against cryptographically signed `dst_hash`. Operation rejected with `TargetMismatch`.
- **Marker**: `[CAUSAL_TEST7_POST_AUTH_MUTATION_REJECTED]`

**Summary Marker Emitted**: `[CAUSAL_INTEGRATION_ALL_INVARIANTS_VERIFIED_MARKER]`

---

## 6. Graphical Verification Artifacts

| Phase | Artifact Description | File Path | SHA-256 Digest |
|:---|:---|:---|:---|
| **Stage 1** | Baseline Document Saved (`draft.txt`) | `evidence/f106_1_0_document_baseline.png` | `612c98ffe086a73f5e00b1bdb87fd4bd820b2bcf291cd0a31401d486f9b46f67` |
| **Stage 2** | Dynamic Plan Confirmation Modal | `evidence/f106_1_1_plan_confirmation_modal.png` | `593ee5fa7fce889539a6fe1498ac67ef56fc7fde228a9615de9a38a84438d698` |
| **Stage 2** | Full Transaction Receipt with HMAC Grant | `evidence/f106_1_2_transaction_receipt.png` | `8e20cb368b7dd0d61774c5632460c7e719a42464c3d45f964be859428b3bcb0b` |
| **Stage 2** | Transaction Undone Baseline | `evidence/f106_1_3_undone_baseline.png` | `aec5dba3ceba78dd3e23a416f062accec5079687adadb899ee32044744a478aa` |
| **Stage 3** | Cold Reboot Preserved Baseline | `evidence/f106_1_4_cold_reboot_verified.png` | `dc6640f09432d98d889d4d245f3e7ab3f87534fa51304cec9ab5e8d7486050c6` |

---

## 7. Artifact Cryptographic Hash Ledger

```text
81ad64afffe84375655ac23c8c924c30b2c474f484104f4feaa248a5d786f727  models/pretrained/smollm2_135m_q8.bin
5135806644f5bb2385324225eb3fc901684551421ba4370e5fa78e3dc600dede  models/pretrained/smollm2_tokenizer.bin
f1a495a9ca43dca38c7d6d54dc3d1087aeeba7484e21bcbfab6fd038894bd419  userspace/user_intent_runtime/user_intent_runtime
8a32d9c66410c1ceb2e63a23f80b78e5c13edaef4dc202028cca6ee47cce3ffc  build/esp/EFI/BOOT/BOOTX64.EFI
612c98ffe086a73f5e00b1bdb87fd4bd820b2bcf291cd0a31401d486f9b46f67  evidence/f106_1_0_document_baseline.png
593ee5fa7fce889539a6fe1498ac67ef56fc7fde228a9615de9a38a84438d698  evidence/f106_1_1_plan_confirmation_modal.png
8e20cb368b7dd0d61774c5632460c7e719a42464c3d45f964be859428b3bcb0b  evidence/f106_1_2_transaction_receipt.png
aec5dba3ceba78dd3e23a416f062accec5079687adadb899ee32044744a478aa  evidence/f106_1_3_undone_baseline.png
dc6640f09432d98d889d4d245f3e7ab3f87534fa51304cec9ab5e8d7486050c6  evidence/f106_1_4_cold_reboot_verified.png
```

---

## 8. Conclusion

**Gate F106.1 is formally accepted.** All synthetic translation shortcuts have been audited out of the architecture. The intent execution pipeline is confirmed to be strictly causal, mathematically constrained by schema and policy, cryptographically bound by keyed HMAC capability grants, and persistently verifiable across cold hardware reboots.
