# Specification: Phase C — Checkpointed Semantic Event Journal & Suffix Replay

## 1. Overview & Problem Definition

In INOS, the semantic event journal (`core/memory/journal.ml`) and materialized current-state view (`core/memory/state_view.ml`) currently reconstruct system state through an uncheckpointed linear scan:
- `open_journal`: Sequentially reads, parses, and validates every single line from sequence `1` (`genesis_digest = "0000...0000"`) through the log head.
- `replay_journal`: Iterates across all historical entries from an empty state (`create_empty ()`), applying event mutations one-by-one.

As milestone events accumulate across project history, opening and replaying the journal scales as $O(N)$ in time and CPU cycles.

This specification designs a **validated checkpointing and suffix replay protocol** for the OCaml semantic journal that:
1. Accurately distinguishes between an **unkeyed integrity checksum** and an **authenticated cryptographic MAC**.
2. Establishes a **trusted checkpoint anchor** requiring cryptographic chain continuity.
3. Implements **fail-closed fallback rules** so an invalid checkpoint never silently reconstructs an incomplete state.

---

## 2. Cryptographic Taxonomy & Authentication

### 2.1 Unkeyed Checksum vs. Authenticated MAC
A critical architectural boundary must be maintained between integrity detection and authorization:

| Primitive | Formula | Security Property | Threat Mitigated |
| :--- | :--- | :--- | :--- |
| **Integrity Checksum** | $\text{SHA256}(\text{seq} \parallel \text{chain\_digest} \parallel \text{state\_digest})$ | Detects accidental bitflips, disk corruption, torn writes, or truncation. | Media corruption, partial writes. |
| **Authenticated MAC** | $\text{HMAC-SHA256}(K_{\text{journal}}, \text{seq} \parallel \text{chain\_digest} \parallel \text{state\_digest})$ | Proves authenticity using a secret or protected key ($K_{\text{journal}}$) held in the supervisor trust store. | Malicious modification or injection by untrusted processes. |

**Policy**: In the freestanding local configuration where a secret key is not provisioned, the field is explicitly identified and treated as an `integrity_checksum`. When a trust store key is present, an authenticated `hmac_signature` is enforced.

---

## 3. Checkpoint Record Format & Schema

Checkpoints are serialized to `<journal_path>.checkpoint` using atomic write-to-temp and `rename()`:

```json
{
  "version": 1,
  "checkpoint_seq": 1050,
  "checkpoint_chain_digest": "3a03db4d8ba7b3868e3a9f5fcefa02899585a3987e196469fdb90c4697ca152f",
  "materialized_state_digest": "df82847f790b93f1829efc14cfba06b6d6939a4435b4cc0be9f3ea137dfaada1",
  "integrity_checksum": "af059e2edcb155c2839d3a272ee936c0ce51e70e6026e1615101653e3a6bd047",
  "auth_mac": null,
  "timestamp": 1791650926.125,
  "state": {
    "intents": { ... },
    "capabilities": { ... },
    "facts": { ... },
    "metadata": { ... },
    "event_count": 1050
  }
}
```

### Invariants:
1. `checkpoint_seq`: Must match the sequence number of the last event applied to `state`.
2. `checkpoint_chain_digest`: Must bit-for-bit equal the `digest` field of event `checkpoint_seq` in the journal file.
3. `materialized_state_digest`: The deterministic canonical digest of the materialized state table (`compute_state_hash (state)`).
4. `integrity_checksum`: The SHA-256 digest over the canonical string:
   $$\text{"CHECKPOINT:v1|" } \parallel \text{seq} \parallel \text{"|" } \parallel \text{chain\_digest} \parallel \text{"|" } \parallel \text{state\_digest}$$

---

## 4. Suffix Replay Protocol & Continuity Verification

When `open_journal` or `replay_journal` is invoked:

```
[Start Replay]
      │
      ▼
Check if `<journal>.checkpoint` exists?
      ├─────► NO ───► [Full Genesis Replay (seq 1..N)]
      │
     YES
      │
      ▼
Validate Checkpoint Integrity:
1. Verify `integrity_checksum` == SHA256(canonical checkpoint header).
2. Verify `materialized_state_digest` == compute_state_hash(state).
3. If MAC key present, verify `auth_mac` == HMAC-SHA256(K, header).
      │
      ├─────► VALIDATION FAILED ───► [Trigger Fail-Safe Fallback Protocol (§5)]
      │
   PASSED
      │
      ▼
Locate Suffix in Journal:
Seek to entry where `seq = checkpoint_seq + 1`.
      │
      ▼
Verify Chain Continuity:
Entry `(checkpoint_seq + 1).prev_digest` == `checkpoint_chain_digest`?
      │
      ├─────► MISMATCH (Broken Link) ───► [Trigger Fail-Safe Fallback Protocol (§5)]
      │
   MATCHED
      │
      ▼
Fast Suffix Replay:
Initialize `current_state` with validated checkpoint state.
Replay only suffix events (seq = checkpoint_seq + 1 .. head).
Return materialized state in O(K) time where K = N - checkpoint_seq.
```

---

## 5. Fail-Safe Fallback & Fail-Closed Rules

If a checkpoint fails verification (corrupt checksum, mismatched state digest, or broken chain continuity):

> [!CAUTION]
> **Anti-Tampering Rule**: The system MUST NEVER blindly trust an unanchored checkpoint or silently reconstruct partial state.

1. **Genesis Verification Check**:
   The loader checks whether:
   - The primary journal file contains an unbroken sequence starting at `seq = 1` with `prev_digest == genesis_digest`.
   - All events from `seq = 1` through current head have valid checksums and unbroken hash linkage.
2. **Deterministic Fallback (If Genesis Intact)**:
   - Log explicit diagnostic warning:
     `"[JOURNAL_RECOVERY_WARNING] Checkpoint corrupt or broken at seq %d; falling back to full genesis replay."`
   - Reconstruct state by replaying from `seq = 1`.
   - Regenerate and atomically publish a new valid checkpoint.
3. **Fail-Closed Abort (If Genesis Incomplete or Broken)**:
   - If the journal file has been truncated, corrupted, or has sequence gaps before the checkpoint:
     - Return `Error (JournalIncompleteOrCorrupt { reason = "Checkpoint invalid and genesis log incomplete" })`.
     - **Fail closed immediately**. Do NOT proceed with partial state.

---

## 6. Implementation Checklist (For Future Execution)

- [ ] Add `save_checkpoint` with canonical state serialization and checksum calculation.
- [ ] Add `load_checkpoint` with strict schema validation and `compute_state_hash` match.
- [ ] Implement suffix scanning in `open_journal` with chain continuity check (`entry.prev_digest == checkpoint.chain_digest`).
- [ ] Implement dual-track fallback: genesis replay if unbroken log exists; fail-closed abort if log is damaged.
- [ ] Test suite covering:
  - Nominal checkpoint fast-path.
  - Checkpoint corruption detection.
  - Suffix chain continuity mismatch.
  - Interrupted write recovery (`.tmp` atomic rename).
  - Truncated journal fail-closed behavior.
