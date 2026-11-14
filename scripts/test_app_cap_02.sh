#!/usr/bin/env bash
# scripts/test_app_cap_02.sh - Acceptance suite for Milestone APP-CAP-02 (Application Packaging, Scheduling & Lifecycle)
# Verifies:
# 1. HW-00: Read-only hardware inventory (CPUID + PCI scanning, 0 config writes, 0 driver binding)
# 2. APP-CAP-02A: .inapp package verification, SHA-256 integrity, decoupled registry authentication, W^X enforcement
# 3. FS-CAPACITY-01: Storage extension for files > 4096B up to 64 KiB via single-indirect blocks & WAL
# 4. Parallel lanes: RFC 1951 Deflate, ZIP CRC-32, Zip-Slip rejection, Grid spreadsheet engine with formula evaluation & circular ref detection
# 5. APP-CAP-02B: Hardware timer preemption (IRQ0 Vector 32) of non-yielding spin_app with complete 15-GPR state preservation
# 6. APP-CAP-02C: Lifecycle and systematic resource reclamation of capability tokens and compositor surfaces
# 7. Stage 22: Cold QEMU reboot verification of extended files and persistent outputs

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_APP02="${BUILD_DIR}/disk_app_cap_02.img"

LOG_STAGE1="${EVIDENCE_DIR}/app_cap_02_stage1_serial.log"
LOG_COLD_REBOOT="${EVIDENCE_DIR}/app_cap_02_cold_reboot_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE APP-CAP-02] PACKAGING, SCHEDULING & LIFECYCLE ACCEPTANCE     ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1 - STAGE 1: Full Suite Execution & Invariant Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: HW-00, Package Trust, FS-Capacity, Preemption & Lifecycle <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for APP-CAP-02 test..."
dd if=/dev/zero of="${DISK_APP02}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/app02_mon1.sock
MON_SOCK1="/tmp/app02_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_APP02}",format=raw,index=1,media=disk \
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

    # Wait for document name prompt and dispatch "packaging"
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'packaging'..."
            (for key in p a c k a g i n g ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APP-CAP-02 Stage 1 Serial Invariants..."
grep -q "\[APP_CAP_02_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[HW_00_READ_ONLY_DISCOVERY_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PACKAGE_INTEGRITY_TAMPER_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PACKAGE_UNTRUSTED_SANDBOX_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PACKAGE_WNX_ENFORCEMENT_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PACKAGE_VERIFICATION_TRUST_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[FS_CAPACITY_01_EXTENDED_FILE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[ARCHIVE_DEFLATE_ROUNDTRIP_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[ARCHIVE_ZIP_CONTAINER_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[ARCHIVE_ZIP_SLIP_DEFENSE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[GRID_FORMULA_EVAL_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[GRID_CIRCULAR_DEP_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[TIMER_PREEMPTION_OCCURRED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[SPIN_APP_COMPLETED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[LIFECYCLE_RESOURCE_RECLAMATION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_CAP_02_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: HW-00 read-only discovery, package verifier, extended files, parallel apps, preemption anti-starvation, and lifecycle reclamation."

# =========================================================================
# PHASE 2 - STAGE 2: Cold QEMU Reboot & Extended File Persistence Check
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & Extended File Integrity Check          <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_APP02}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 2>&1 | tee "${LOG_COLD_REBOOT}" &
QEMU_PID2=$!

wait "${QEMU_PID2}" || true

echo ""
echo "[VERIFY] Checking APP-CAP-02 Cold Reboot Invariants..."
grep -q "\[APP_CAP_02_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
grep -q "\[APP_CAP_02\] File 'calc.inapp' verified bit-for-bit across cold reboot\." "${LOG_COLD_REBOOT}"
grep -q "\[APP_CAP_02\] File 'calc_output.txt' verified: 89 bytes match committed payload\." "${LOG_COLD_REBOOT}"
grep -q "\[APP_CAP_02_COLD_REBOOT_PERSISTENCE_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
echo "  [PASS] Cold Reboot Verified: calc.inapp (9,908B extended file) and calc_output.txt (89B) preserved with 100% bit-for-bit integrity!"

echo ""
echo "=========================================================================="
echo "=== [APP-CAP-02] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)           ==="
echo "=========================================================================="
