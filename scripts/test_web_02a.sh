#!/usr/bin/env bash
# scripts/test_web_02a.sh - Acceptance suite for Milestone WEB-02A
# Verifies:
#  1. Hardware CPL=3 isolated execution of native Ring 3 Document Renderer (PID 310).
#  2. Page table isolation & disjoint physical memory layout (PML4_RENDER at 0x00A0_0000).
#  3. Formal W^X verification (zero pages simultaneously W and X) and stack guard pages.
#  4. Zero ambient authority fail-closed rejection:
#     - Network socket capability and direct socket syscall denied
#     - Filesystem write capability and direct file create denied
#     - Direct hardware/RTC access denied
#  5. Versioned, bounded IPC (inos.web.ipc.v1):
#     - Authorized document feed reception
#     - Authorized display command submission
#     - Forged sender identity rejection fail-closed
#     - Stale generation endpoint handle rejection fail-closed
#     - Oversized message payload rejection fail-closed
#  6. Trusted Compositor security boundaries:
#     - Browser Chrome / Surface 1 unauthorized drawing rejection fail-closed
#     - Content surface boundary & integer overflow rejection fail-closed
#     - Invalid UTF-8 text rejection fail-closed
#  7. Post-termination lifecycle & fault containment:
#     - Generation incremented and handles invalidated
#     - Flushed endpoint queues
#     - Compositor and kernel operational post-exit

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_APP="${BUILD_DIR}/disk_web02a.img"
LOG_STAGE1="${EVIDENCE_DIR}/web02a_stage1_serial.log"

if [ ! -f "${OVMF_CODE}" ]; then
    echo "[ERROR] OVMF_CODE.fd not found at ${OVMF_CODE}" >&2
    exit 1
fi

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE WEB-02A] PROCESS ISOLATION & BOUNDED IPC ACCEPTANCE SUITE    ==="
echo "=========================================================================="

echo ""
echo "[STEP 1] Compiling web_render binary and kernel..."
if [ "${SKIP_BUILD:-0}" != "1" ]; then
    bash "${SCRIPT_DIR}/build_web_render.sh"
    bash "${SCRIPT_DIR}/build_kernel.sh"
else
    echo "[STEP 1] Skipping build (SKIP_BUILD=1)."
fi

cleanup() {
    rm -f /tmp/web02a_mon.sock
}
trap cleanup EXIT INT TERM

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Renderer Process Isolation, Bounded IPC & Compositor Boundary<<<"
echo "--------------------------------------------------------------------------"

dd if=/dev/zero of="${DISK_APP}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/web02a_mon.sock
MON_SOCK="/tmp/web02a_mon.sock"

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
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE1}" &
QEMU_PID=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK}" ]; then break; fi
        sleep 0.1
    done

    # Wait for document name prompt and dispatch "web02a"
    for _ in {1..2000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'web02a'..."
            (for key in w e b 0 2 a ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK}" 2>/dev/null || true
            break
        fi
        sleep 0.01
    done
) &

wait "${QEMU_PID}" || true
rm -f "${MON_SOCK}"

echo ""
echo "[VERIFY] Checking WEB-02A Serial Invariants..."
grep -q "\[PAGE_TABLES\] PML4_RENDER initialized" "${LOG_STAGE1}"
grep -q "\[PAGE_TABLES\] Physical memory disjointness verification: PASSED\." "${LOG_STAGE1}"
grep -q "\[PAGE_TABLES\] W\^X permission verification: PASSED\." "${LOG_STAGE1}"
grep -q "\[PAGE_TABLES\] Stack guard page verification: PASSED\." "${LOG_STAGE1}"
grep -q "\[WEB_02A_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"

# Userspace Renderer Invariants
grep -q "\[WEB_RENDER\] Hardware CPL=3 isolation confirmed\." "${LOG_STAGE1}"
grep -q "\[WEB_RENDER\] Registered Process ID 310 confirmed" "${LOG_STAGE1}"
grep -q "\[WEB_02A_AMBIENT_NET_DENIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_AMBIENT_FS_DENIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_AMBIENT_AUTHORITY_DENIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_IPC_DOC_RECV_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_IPC_DISPLAY_SUBMIT_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_FORGED_IPC_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_STALE_HANDLE_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_OVERSIZED_IPC_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_CHROME_PROTECTION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_COMPOSITOR_BOUNDS_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_UTF8_VALIDATION_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_ALL_ACCEPTANCE_GATES_PASSED_MARKER_v1.0\]" "${LOG_STAGE1}"

# Post-termination & fault containment invariants
grep -q "\[WEB_02A_POST_TERMINATION_LIFECYCLE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_FAULT_CONTAINMENT_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_02A_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"

echo ""
echo "=========================================================================="
echo "=== [WEB-02A] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)               ==="
echo "=========================================================================="
