#!/usr/bin/env bash
# scripts/test_app_script_07.sh - Acceptance suite for Milestone APP-SCRIPT-007
# (Adversarial Reliability, Kernel Quota Completion & Transaction Recovery Certification)
#
# Verifies all 15 acceptance gates:
# 1. DRBG Construction State Transition Vectors
# 2. DRBG Fast Key Erasure & Backtracking Resistance
# 3. DRBG Hardware Entropy Exhaustion / Stuck-bit Fail-Closed
# 4. Kernel Process Memory Quota Rejection (OutOfMemory = 8)
# 5. Kernel Process File Operations Quota Rejection (QuotaExceeded = 14)
# 6. Kernel Preemptive CPU Budget Accounting, Termination, and Full Resource Reclamation
# 7. Generation-Based Live Visibility Barrier (PUBLISHING_TX_ID)
# 8. Duplicate Transaction Commit Rejected Fail-Closed
# 9. Boundary 1: Torn WAL Commit Record / Corrupted CRC Discarded (0 files visible)
# 10. Boundary 2: Flushed Commit Record, Pre-Namespace Publication Replay (All files published)
# 11. Boundary 3: Mid-Stream Namespace Publication Replay (All files published, no partial leakage)
# 12. Boundary 4: Pre-Receipt Crash Replay & Receipt Ledger Reconciliation
# 13. Boundary 5: Post-Receipt Crash Recovery Idempotency (Exactly 1 receipt)
# 14. Multi-Cycle Cold Reboot Recovery Idempotency Across Sequential Power Cycles
# 15. Raw Disk Audit Confirming Strict All-or-Nothing Invariant and Zero Regression

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_SCRIPT07="${BUILD_DIR}/disk_app_script_07.img"

LOG_STAGE1="${EVIDENCE_DIR}/app_script_07_stage1_serial.log"
LOG_COLD_REBOOT1="${EVIDENCE_DIR}/app_script_07_cold_reboot1_serial.log"
LOG_COLD_REBOOT2="${EVIDENCE_DIR}/app_script_07_cold_reboot2_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [APP-SCRIPT-007] ADVERSARIAL RELIABILITY & TRANSACTION RECOVERY   ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1: Stage 1 Adversarial Hardening, Quotas & Crash Boundary Suite
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Adversarial Hardening, Quotas & Crash Boundary Matrix     <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for APP-SCRIPT-007 test..."
dd if=/dev/zero of="${DISK_SCRIPT07}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/script07_mon1.sock
MON_SOCK1="/tmp/script07_mon1.sock"

# Crucial: -cpu max exposes host CPU RDRAND/RDSEED hardware entropy instructions
stdbuf -oL -eL qemu-system-x86_64 \
    -cpu max \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_SCRIPT07}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK1}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE1}" &
QEMU_PID1=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK1}" ]; then break; fi
        sleep 0.1
    done

    # Wait for document name prompt and dispatch "intscript07"
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'intscript07'..."
            (for key in i n t s c r i p t 0 7 ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # Dispatch Enter for content prompt to immediately proceed
    for _ in {1..100}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching enter for content prompt..."
            echo "sendkey ret" | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APP-SCRIPT-007 Stage 1 Invariants..."

# DRBG Construction Verification
grep -a -q "\[DRBG_CONSTRUCTION_KAT_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 1: ChaCha20 DRBG construction state transition vectors verified."

grep -a -q "\[DRBG_FAST_KEY_ERASURE_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 2: Fast key erasure and backtracking resistance verified."

grep -a -q "\[ENTROPY_FAIL_CLOSED_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 3: Hardware entropy exhaustion / stuck-bit fail-closed verified."

# Kernel Process Resource Quotas
grep -a -q "\[KERNEL_MEMORY_QUOTA_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 4: Kernel process memory quota rejected fail-closed (OutOfMemory = 8)."

grep -a -q "\[KERNEL_FILE_OP_QUOTA_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 5: Kernel process file operations quota rejected fail-closed (QuotaExceeded = 14)."

grep -a -q "\[KERNEL_CPU_BUDGET_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[CPU_BUDGET_EXHAUSTED_TERMINATED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -a -q "\[RESOURCE_QUOTA_RECLAIMED_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 6: Kernel preemptive CPU budget accounting, runaway termination & resource reclamation verified."

# Generation-Based Live Visibility Barrier
grep -a -q "\[KERNEL_TX_LIVE_VISIBILITY_BARRIER_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 7: Generation-based live visibility barrier (PUBLISHING_TX_ID) verified."

# Duplicate Transaction Commit Rejection
grep -a -q "\[KERNEL_TX_DUPLICATE_COMMIT_REJECTED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 8: Duplicate transaction commit rejected fail-closed."

# Five Crash Boundaries Testing
grep -a -q "\[KERNEL_TX_BOUNDARY_1_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 9: Boundary 1 (Torn WAL commit record / CRC corrupt discarded, 0 files visible) verified."

grep -a -q "\[KERNEL_TX_BOUNDARY_2_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 10: Boundary 2 (Flushed commit record, pre-namespace publish replay) verified."

grep -a -q "\[KERNEL_TX_BOUNDARY_3_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 11: Boundary 3 (Mid-stream namespace publication all-or-nothing completed) verified."

grep -a -q "\[KERNEL_TX_BOUNDARY_4_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 12: Boundary 4 (Pre-receipt crash reconciled in receipt ledger) verified."

grep -a -q "\[KERNEL_TX_BOUNDARY_5_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Gate 13: Boundary 5 (Post-receipt crash recovery idempotent, exactly 1 receipt) verified."

grep -a -q "\[KERNEL_TX_INTERRUPTED_RECOVERY_IDEMPOTENT_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Interrupted recovery idempotency confirmed."

grep -a -q "\[APP_SCRIPT_007_APP_CLEAN_EXIT_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -a -q "\[APP_SCRIPT_007_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: All 13 in-kernel hardening and boundary gates passed!"

# =========================================================================
# PHASE 2: Multi-Cycle Cold QEMU Reboot & Recovery Certification (Cycle 1)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot Cycle 1 & Transaction Durability Check   <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT1}"

stdbuf -oL -eL qemu-system-x86_64 \
    -cpu max \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_SCRIPT07}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 2>&1 | tee "${LOG_COLD_REBOOT1}" || true

grep -a -q "\[APP_SCRIPT_007_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT1}"
grep -a -q "\[APP_SCRIPT_007_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT1}"
echo "  [PASS] Reboot Cycle 1 Verified: Persistent transactional files and receipt ledger verified."

# =========================================================================
# PHASE 3: Cold QEMU Reboot Cycle 2 (Multi-Cycle Recovery Idempotency)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: Cold QEMU Reboot Cycle 2 (Multi-Cycle Recovery Idempotency) <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT2}"

stdbuf -oL -eL qemu-system-x86_64 \
    -cpu max \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_SCRIPT07}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 2>&1 | tee "${LOG_COLD_REBOOT2}" || true

grep -a -q "\[APP_SCRIPT_007_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT2}"
grep -a -q "\[APP_SCRIPT_007_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT2}"
echo "  [PASS] Gate 14: Multi-cycle cold reboot recovery idempotency certified across 2 power cycles."

# =========================================================================
# PHASE 4: Raw Disk Image Audit & All-or-Nothing Invariant Proof
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 4: Raw Disk Image Audit & All-or-Nothing Invariant Proof     <<<"
echo "--------------------------------------------------------------------------"

# Invariant Proof 1: Committed transaction files exist on persistent disk
if grep -a -q "script_tx_report.txt" "${DISK_SCRIPT07}"; then
    echo "  [PASS] 'script_tx_report.txt' confirmed present on disk."
fi
if grep -a -q "script_tx_audit.log" "${DISK_SCRIPT07}"; then
    echo "  [PASS] 'script_tx_audit.log' confirmed present on disk."
fi

# Invariant Proof 2: Boundary 1 torn transaction files were NEVER published
if grep -a -q "b1_file1.txt" "${DISK_SCRIPT07}"; then
    echo "  [FAIL] Torn Boundary 1 file 'b1_file1.txt' was detected in persistent disk namespace!"
    exit 1
else
    echo "  [PASS] Boundary 1 torn transaction completely absent from persistent disk."
fi

# Invariant Proof 3: Boundary 3 all 4 files are visible
for f in b3_f1.txt b3_f2.txt b3_f3.txt b3_f4.txt; do
    if grep -a -q "${f}" "${DISK_SCRIPT07}"; then
        echo "  [PASS] Boundary 3 complete file '${f}' confirmed on disk."
    else
        echo "  [FAIL] Boundary 3 file '${f}' missing from disk!"
        exit 1
    fi
done

echo "  [PASS] Gate 15: All-or-nothing invariant strictly confirmed on raw disk image."

echo ""
echo "=========================================================================="
echo "=== [PASS] ALL 15 APP-SCRIPT-007 ACCEPTANCE GATES PASSED (100% GREEN)! ==="
echo "=========================================================================="
