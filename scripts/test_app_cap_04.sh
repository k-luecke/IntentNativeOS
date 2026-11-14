#!/usr/bin/env bash
# scripts/test_app_cap_04.sh - Acceptance suite for Milestone APP-CAP-04 (Package Security & Process Reliability)
# Verifies:
# 1. Unauthorized package installation & replacement rejection
# 2. Identity spoofing & capability escalation fail-closed defense
# 3. Corrupted / damaged package registry recovery
# 4. Interrupted installation/update rollback to last-known-good package
# 5. Complete uninstall cleanup (deleting package, revoking tokens, closing surfaces)
# 6. Concurrent multi-process execution (dual resident processes)
# 7. Hardware fault containment under concurrent loads (#PF trapping without disturbing siblings)
# 8. 100-cycle mixed lifecycle resource reclamation with zero net leakage
# 9. Cold reboot persistence verification of package database across power cycle

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_APPCAP04="${BUILD_DIR}/disk_app_cap_04.img"

LOG_STAGE1="${EVIDENCE_DIR}/app_cap_04_stage1_serial.log"
LOG_COLD_REBOOT="${EVIDENCE_DIR}/app_cap_04_cold_reboot_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [LANE C: APP-CAP-04] PACKAGE SECURITY & PROCESS RELIABILITY        ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1: Adversarial Package Security, Rollback, & 100-Cycle Reclamation
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Adversarial Package Security, Rollback & Fault Containment <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for APP-CAP-04 test..."
dd if=/dev/zero of="${DISK_APPCAP04}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/cap04_mon1.sock
MON_SOCK1="/tmp/cap04_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -cpu max \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_APPCAP04}",format=raw,index=1,media=disk \
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

    # Wait for document name prompt and dispatch "appcap04"
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'appcap04'..."
            (for key in a p p c a p 0 4 ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APP-CAP-04 Stage 1 Invariants..."
grep -a -q "\[APP_CAP_04_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_UNAUTHORIZED_INSTALL_REJECTED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_IDENTITY_SPOOF_REJECTED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_CAP_ESCALATION_REJECTED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_CORRUPT_REGISTRY_RECOVERED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_INTERRUPTED_UPDATE_ROLLBACK_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_UNINSTALL_CLEANUP_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_CONCURRENT_APPS_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_FAULT_CONTAINMENT_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_ZERO_LEAK_RECLAMATION_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_SUITE_COMPLETE_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_CAP_04_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: All adversarial tests, rollbacks, fault containment, and zero-leak cycles passed!"

# =========================================================================
# PHASE 2: Cold QEMU Reboot & Package Registry Persistence Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & Package Registry Persistence Check     <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT}"

stdbuf -oL -eL qemu-system-x86_64 \
    -cpu max \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_APPCAP04}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 2>&1 | tee "${LOG_COLD_REBOOT}" || true

echo ""
echo "[VERIFY] Checking APP-CAP-04 Cold Reboot Invariants..."
grep -a -q "\[APP_CAP_04_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
grep -a -q "\[APP_CAP_04_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
echo "  [PASS] Cold Reboot Verified: Hardened package registry and installed package verified across cold power cycle!"

echo ""
echo "=========================================================================="
echo "=== [APP-CAP-04] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)           ==="
echo "=========================================================================="
