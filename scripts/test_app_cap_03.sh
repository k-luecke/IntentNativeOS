#!/usr/bin/env bash
# scripts/test_app_cap_03.sh - Acceptance suite for Milestone APP-CAP-03 (Lifecycle & Package Management)
# Verifies:
# 1. Persistent package registry (/system/packages.db) without kernel recompilation
# 2. Package install, update, rollback, and uninstall lifecycle
# 3. Resource ownership and zero net frame/token/surface leakage across 100 cycles
# 4. Multi-process timer preemption stress test (>5,000 ticks)
# 5. Cold QEMU reboot persistence of packages.db

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_APP03="${BUILD_DIR}/disk_app_cap_03.img"
LOG_STAGE1="${EVIDENCE_DIR}/app_cap_03_stage1_serial.log"
LOG_COLD_REBOOT="${EVIDENCE_DIR}/app_cap_03_cold_reboot_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE APP-CAP-03] APPLICATION LIFECYCLE & PACKAGE MANAGEMENT       ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, packages, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1: Package Lifecycle, 100-Cycle Zero Leak, & Preemption Stress
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Package DB, 100-Cycle Zero Leak, & Scheduler Stress       <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for APP-CAP-03 test..."
dd if=/dev/zero of="${DISK_APP03}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/app03_mon.sock
MON_SOCK="/tmp/app03_mon.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_APP03}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE1}" &
QEMU_PID1=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK}" ]; then break; fi
        sleep 0.1
    done

    # Wait for document name prompt and dispatch 'appcap03'
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'appcap03'..."
            (for key in a p p c a p 0 3 ret; do echo "sendkey ${key}"; sleep 0.02; done) | socat - UNIX-CONNECT:"${MON_SOCK}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK}"

echo ""
echo "[VERIFY] Checking APP-CAP-03 Stage 1 Invariants..."
grep -q "\[APP_CAP_03_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PKG_INSTALL_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[PKG_UPDATE_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[PKG_ROLLBACK_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[PKG_UNINSTALL_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[APP_CAP_03_PACKAGE_DB_LIFECYCLE_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[APP_CAP_03_ZERO_RESOURCE_LEAK_100_CYCLES_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[APP_CAP_03_SCHEDULER_STRESS_COMPLETE_MARKER\]" "${LOG_STAGE1}"
grep -q "\[APP_CAP_03_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: Persistent packages.db lifecycle, 100-cycle zero net leak, and scheduler stress."

# =========================================================================
# PHASE 2: Cold QEMU Reboot & Package DB Persistence Check
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & Package DB Persistence Check           <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_APP03}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 2>&1 | tee "${LOG_COLD_REBOOT}" &
QEMU_PID2=$!

wait "${QEMU_PID2}" || true

echo ""
echo "[VERIFY] Checking APP-CAP-03 Cold Reboot Invariants..."
grep -q "\[APP_CAP_03_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
grep -q "\[APP_CAP_03_COLD_REBOOT_PERSISTENCE_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
echo "  [PASS] Cold Reboot Verified: packages.db and installed packages verified across cold power cycle!"

echo ""
echo "=========================================================================="
echo "=== [APP-CAP-03] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)           ==="
echo "=========================================================================="
