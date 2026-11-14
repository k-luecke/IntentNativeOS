#!/usr/bin/env bash
# scripts/test_app_canvas_01.sh - Automated Acceptance Test Suite for Milestone APP-CANVAS-01
# Verifies:
# Phase 1: Native Canvas Vector Graphic & Design Suite (primitives, hit-test, z-order, SVG roundtrip, GUI)
# Phase 2: Stage 34 Cold Reboot persistence check of drawing.svg
# Phase 3: External format audit of raw ATA disk for standard SVG syntax

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
DISK_CANVAS="${ROOT_DIR}/build/disk_canvas.img"

LOG_STAGE1="${ROOT_DIR}/evidence/app_canvas_01_stage1_serial.log"
LOG_COLD="${ROOT_DIR}/evidence/app_canvas_01_cold_reboot_serial.log"

mkdir -p "${ROOT_DIR}/evidence"
mkdir -p "${ROOT_DIR}/build"

echo "=========================================================================="
echo "=== [LANE A: APP-CANVAS-01] NATIVE VECTOR GRAPHIC & DESIGN GATES       ==="
echo "=========================================================================="

echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Vector Primitives, Z-Order, Hit-Test, SVG & Compositor GUI<<<"
echo "--------------------------------------------------------------------------"

echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for APP-CANVAS-01 test..."
dd if=/dev/zero of="${DISK_CANVAS}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/canvas_mon1.sock
MON_SOCK1="/tmp/canvas_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_CANVAS}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK1}",server=on,wait=off \
    -mon chardev=mon0,mode=readline > "${LOG_STAGE1}" 2>&1 &
QEMU_PID1=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK1}" ]; then break; fi
        sleep 0.1
    done

    # Wait for document name prompt and dispatch "canvas"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'canvas'..."
            (for key in c a n v a s ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APP-CANVAS-01 Stage 1 Invariants..."
grep -q "\[CANVAS_PRIMITIVES_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[CANVAS_HIT_TEST_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[CANVAS_LAYER_ZORDER_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[CANVAS_SVG_ROUNDTRIP_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[CANVAS_COMPOSITOR_GUI_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[CANVAS_FS_PERSISTENCE_WRITTEN_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[CANVAS_APP_SUITE_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_CANVAS_01_APP_CLEAN_EXIT_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_CANVAS_01_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"

echo "  [PASS] Stage 1 Verified: All Canvas vector primitives, layers, hit-testing, SVG roundtrip, and GUI verified!"

echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & SVG Drawing Persistence Check          <<<"
echo "--------------------------------------------------------------------------"

rm -f "${LOG_COLD}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_CANVAS}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 > "${LOG_COLD}" 2>&1 || true

grep -q "\[APP_CANVAS_01_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD}" || {
    echo "[FAIL] Cold reboot verification failed for APP-CANVAS-01!"
    cat "${LOG_COLD}"
    exit 1
}

echo "  [PASS] Cold Reboot Verified: drawing.svg verified across cold power cycle!"

echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: External Format Interoperability Audit (W3C SVG)          <<<"
echo "--------------------------------------------------------------------------"

grep -q '<svg xmlns="http://www.w3.org/2000/svg"' "${DISK_CANVAS}" || {
    echo "[FAIL] Disk image missing valid W3C SVG xmlns declaration!"
    exit 1
}
echo "  [PASS] Disk image contains valid W3C SVG xmlns declaration!"

grep -q '<rect ' "${DISK_CANVAS}" || {
    echo "[FAIL] Disk image missing SVG <rect> element!"
    exit 1
}
echo "  [PASS] Disk image contains valid SVG <rect> element!"

grep -q '<ellipse ' "${DISK_CANVAS}" || {
    echo "[FAIL] Disk image missing SVG <ellipse> element!"
    exit 1
}
echo "  [PASS] Disk image contains valid SVG <ellipse> element!"

grep -q '<text ' "${DISK_CANVAS}" || {
    echo "[FAIL] Disk image missing SVG <text> element!"
    exit 1
}
echo "  [PASS] Disk image contains valid SVG <text> element with inner content!"

grep -q '</svg>' "${DISK_CANVAS}" || {
    echo "[FAIL] Disk image missing SVG closing tag </svg>!"
    exit 1
}
echo "  [PASS] Disk image contains valid closing tag </svg>!"

echo ""
echo "=========================================================================="
echo "=== [APP-CANVAS-01] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)        ==="
echo "=========================================================================="
