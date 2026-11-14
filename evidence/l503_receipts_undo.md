# L503 — Verified Operation Receipts and Durable Undo Evidence

**Document ID:** EV-L503-RECEIPTS-UNDO-01  
**Target Gate:** Gate L5 (UX, Quality and Hardening)  
**Date:** October 9, 2026  
**Status:** Cryptographically Verified Operation Chain

---

## 1. Verified Execution Chain Architecture

In the Intent-Native OS, every state-modifying action must produce an immutable provenance receipt linking the natural language user request to the exact disk sectors modified.

```
+-----------------------------------------------------------------------------------+
| 1. User Intent Request                                                            |
|    "create os_changes.txt"                                                        |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
| 2. Dual-Model Arbitration & Deterministic Verification                            |
|    - Model A Plan: CreateFile("os_changes.txt", payload, CapStorageWrite)         |
|    - Model B Review: Approved (no ambiguity, target file explicit)                |
|    - Deterministic Verifier: Path bounded within INOS-FS namespace                |
|    - Capability Token Minted: Nonce 0x0002 (CAP_STORAGE_WRITE)                    |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
| 3. Ring 3 Syscall Boundary Crossing                                               |
|    - Syscall 10: SYS_FILE_CREATE("os_changes.txt", 14, cap_token=2)               |
|    - Syscall 11: SYS_FILE_WRITE("os_changes.txt", payload, 65, cap_token=2)       |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
| 4. Kernel Storage & Persistent WAL Snapshot                                       |
|    - File LBA: 16 (INOS-FS Data Block Region)                                     |
|    - Pre-Mutation Snapshot: Inode #1 version snapshot saved to LBA 32             |
|    - Atomic WAL Commit: Committed 65 bytes with persistent transaction receipt    |
|    - Spool Sector: LBA 24,576+ (INT_ magic header + SHA-256 provenance hash)     |
+-----------------------------------------------------------------------------------+
```

---

## 2. Byte-for-Byte Durable Undo Verification

1. **Pre-Mutation State Capture:**  
   Before any destructive or modifying syscall commits to disk, the kernel snapshot engine allocates an uncommitted shadow block and creates a snapshot record in the filesystem superblock.
2. **Deterministic Rollback Syscall:**  
   When the user or application triggers undo:
   $$\text{sys\_rollback}(\text{target\_inode}, \text{snapshot\_id})$$
   The kernel swaps the root block pointer back to the snapshot sector, restoring the original file contents byte-for-byte.
3. **Cold Boot Recovery:**  
   Tested and proven across repeated cold boots in QEMU (Gates G9 and G12):
   - Power loss simulated immediately following undo.
   - Upon remount, the WAL journal replays and confirms the restored bytes match the original pre-mutation SHA-256 hash exactly.
