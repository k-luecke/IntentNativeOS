#!/usr/bin/env bash
# scripts/test_desktop_multi_reboot_01.sh - Acceptance suite for APF-002E (Multi-Cycle Cold Reboot Resilience)
# Verifies:
# 1. Cycle 1: Fresh boot, native desktop workflow execution, document persistence, clean power cut
# 2. Cycle 2: Cold Reboot #1, INOS-FS remount, WAL journal replay, document persistence verification
# 3. Cycle 3: Cold Reboot #2, repeated cold power cycle, recovery idempotency, multi-cycle file integrity
# 4. External format validation of post-reboot disk artifacts using pure Rust tools/format_validator

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_MULTI="${BUILD_DIR}/disk_desktop_multi_reboot_01.img"

LOG_CYCLE1="${EVIDENCE_DIR}/desktop_multi_cycle1_serial.log"
LOG_CYCLE2="${EVIDENCE_DIR}/desktop_multi_cycle2_serial.log"
LOG_CYCLE3="${EVIDENCE_DIR}/desktop_multi_cycle3_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [APF-002E: MULTI-CYCLE COLD REBOOT RESILIENCE SUITE]              ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> CYCLE 1: Cold Boot, Native Desktop Workflow & Initial Persistence  <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for Multi-Cycle test..."
dd if=/dev/zero of="${DISK_MULTI}" bs=1M count=16 status=none

rm -f "${LOG_CYCLE1}" /tmp/multi_mon1.sock
MON_SOCK1="/tmp/multi_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_MULTI}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK1}",server=on,wait=off \
    -mon chardev=mon0,mode=readline > "${LOG_CYCLE1}" 2>&1 &
QEMU_PID1=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK1}" ]; then break; fi
        sleep 0.1
    done

    # Wait for document prompt and dispatch "desktop"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_CYCLE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'desktop'..."
            (for key in d e s k t o p ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # Wait for intent modal option prompt and dispatch "1"
    for _ in {1..1000}; do
        if grep -q "\[F106_AWAITING_OPTION_INPUT\]" "${LOG_CYCLE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware confirmation keypress '1' for Layered Modal..."
            (for key in 1 ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking Cycle 1 Invariants..."
grep -q "\[DESKTOP_WORKFLOW_STARTED_MARKER_v1.0\]" "${LOG_CYCLE1}"
grep -q "\[DESKTOP_WORKFLOW_COMPLETE_MARKER_v1.0\]" "${LOG_CYCLE1}"
echo "  [PASS] Cycle 1: Desktop workflow executed and all document artifacts committed to ATA disk."

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> CYCLE 2: Cold Power Cycle #1 & First Recovery Verification        <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_CYCLE2}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_MULTI}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 > "${LOG_CYCLE2}" 2>&1 || true

echo ""
echo "[VERIFY] Checking Cycle 2 (Cold Reboot #1) Invariants..."
grep -q "\[DESKTOP_WORKFLOW_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_CYCLE2}"
grep -q "\[DESKTOP_WORKFLOW_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_CYCLE2}"
echo "  [PASS] Cycle 2: First cold reboot cleanly mounted INOS-FS and verified all 4 document formats."

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> CYCLE 3: Cold Power Cycle #2 & Multi-Cycle Idempotency Audit       <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_CYCLE3}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_MULTI}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 > "${LOG_CYCLE3}" 2>&1 || true

echo ""
echo "[VERIFY] Checking Cycle 3 (Cold Reboot #2) Invariants..."
grep -q "\[DESKTOP_WORKFLOW_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_CYCLE3}"
grep -q "\[DESKTOP_WORKFLOW_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_CYCLE3}"
echo "  [PASS] Cycle 3: Second cold reboot confirmed repeated power cycle durability and recovery idempotency."

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 4: Format Validator Conformance Audit on Multi-Cycle Disk   <<<"
echo "--------------------------------------------------------------------------"

MULTI_OUT_DIR="${BUILD_DIR}/multi_reboot_conformance"
rm -rf "${MULTI_OUT_DIR}"
"${ROOT_DIR}/tools/format_validator/target/release/format_validator" --disk "${DISK_MULTI}" --out-dir "${MULTI_OUT_DIR}"

echo ""
echo "=========================================================================="
echo "=== [GATE APF-002E (PART 2): MULTI-CYCLE COLD REBOOT 3/3 CYCLES PASS] ==="
echo "=========================================================================="
