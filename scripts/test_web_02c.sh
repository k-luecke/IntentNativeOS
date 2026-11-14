#!/usr/bin/env bash
# scripts/test_web_02c.sh - Acceptance suite for Milestone WEB-02C
# Verifies:
#  1. WHATWG Conformance Fixtures & explicit XFAIL behavior matrix.
#  2. Deterministic Seeded Fuzzing with boundary cuts (UTF-8, entity, attribute, comment, raw text).
#  3. Independent Structural Oracle asserting ground-truth DOM tree properties.
#  4. Combinatorial Resource Stress & Peak Telemetry Measurement (heap bytes & operations).
#  5. Authoritative Sentinel S103 Kernel Telemetry & Nonblocking Audit Logging.
#  6. Post-termination lifecycle & fault containment.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_APP="${BUILD_DIR}/disk_web02c.img"
LOG_STAGE3="/tmp/web02c_stage3_serial.log"
FINAL_LOG="${EVIDENCE_DIR}/web02c_stage3_serial.log"

if [ ! -f "${OVMF_CODE}" ]; then
    echo "[ERROR] OVMF_CODE.fd not found at ${OVMF_CODE}" >&2
    exit 1
fi

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE WEB-02C] PARSER CONFORMANCE, FUZZING & SECURITY TELEMETRY    ==="
echo "=========================================================================="

echo ""
echo "[STEP 1] Running freestanding unit and fuzzer tests (Layer 1)..."
cargo test --manifest-path "${ROOT_DIR}/userspace/apps/web_render/Cargo.toml"

echo ""
echo "[STEP 2] Compiling web_render binary and kernel for UEFI (Layer 2)..."
if [ "${SKIP_BUILD:-0}" != "1" ]; then
    bash "${SCRIPT_DIR}/build_web_render.sh"
    bash "${SCRIPT_DIR}/build_kernel.sh"
else
    echo "[STEP 2] Skipping build (SKIP_BUILD=1)."
fi

cleanup() {
    rm -f /tmp/web02c_mon.sock
}
trap cleanup EXIT INT TERM

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: Ring 3 Conformance, Fuzzing & Sentinel Telemetry (WEB-02C) <<<"
echo "--------------------------------------------------------------------------"

dd if=/dev/zero of="${DISK_APP}" bs=1M count=16 status=none

rm -f "${LOG_STAGE3}" /tmp/web02c_mon.sock
MON_SOCK="/tmp/web02c_mon.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -cpu max \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_APP}",format=raw,index=1,media=disk,file.locking=off \
    -netdev user,id=net0 \
    -device virtio-net-pci,disable-legacy=off,disable-modern=on,romfile="",netdev=net0,mac=52:54:00:12:34:56 \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE3}" &
QEMU_PID=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK}" ]; then break; fi
        sleep 0.1
    done

    # Wait for document name prompt and dispatch "web02c"
    for _ in {1..4000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE3}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'web02c'..."
            (for key in w e b 0 2 c ret; do echo "sendkey ${key}"; sleep 0.005; done) | socat - UNIX-CONNECT:"${MON_SOCK}" 2>/dev/null || true
            break
        fi
        sleep 0.005
    done
) &

wait "${QEMU_PID}" || true
rm -f "${MON_SOCK}"
cp "${LOG_STAGE3}" "${FINAL_LOG}"

echo ""
echo "[VERIFY] Checking WEB-02C Serial Invariants..."
grep -q "\[PAGE_TABLES\] PML4_RENDER initialized" "${LOG_STAGE3}"
grep -q "\[PAGE_TABLES\] Physical memory disjointness verification: PASSED\." "${LOG_STAGE3}"
grep -q "\[PAGE_TABLES\] W\^X permission verification: PASSED\." "${LOG_STAGE3}"
grep -q "\[PAGE_TABLES\] Stack guard page verification: PASSED\." "${LOG_STAGE3}"
grep -q "\[WEB_02C_STAGE3_STARTED_MARKER_v1.0\]" "${LOG_STAGE3}"

# Userspace Parser & Conformance Acceptance Invariants
grep -q "\[WEB_RENDER\] Hardware CPL=3 isolation confirmed\." "${LOG_STAGE3}"
grep -q "\[WEB_RENDER\] Registered Process ID 310 confirmed" "${LOG_STAGE3}"
grep -q "\[WEB_02A_ALL_ACCEPTANCE_GATES_PASSED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02B_ALL_ACCEPTANCE_GATES_PASSED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_WHATWG_CONFORMANCE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_XFAIL_MATRIX_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_SEEDED_FUZZING_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_STRUCTURAL_ORACLE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_RESOURCE_TELEMETRY_MEASURED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_SENTINEL_S103_TELEMETRY_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_ALL_ACCEPTANCE_GATES_PASSED_MARKER_v1.0\]" "${LOG_STAGE3}"

# Kernel Sentinel S103 Telemetry Invariants
grep -q "\[SENTINEL_S103_AUDIT\]" "${LOG_STAGE3}"
grep -q "\[SENTINEL_S103_TELEMETRY_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_KERNEL_S103_TELEMETRY_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE3}"

# Post-termination & fault containment invariants
grep -q "\[WEB_02C_POST_TERMINATION_LIFECYCLE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_FAULT_CONTAINMENT_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE3}"
grep -q "\[WEB_02C_STAGE3_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE3}"

echo ""
echo "=========================================================================="
echo "=== [WEB-02C] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)               ==="
echo "=========================================================================="
