#!/usr/bin/env bash
# scripts/test_web_01a.sh - Acceptance suite for Milestone WEB-01A (Native Networking Foundation)
# Verifies:
#  1. VirtIO-Net PCI device initialization and MAC configuration.
#  2. Deterministic network stack (Ethernet, ARP, IPv4, UDP, TCP).
#  3. Ring 3 CPL=3 isolation and capability-brokered socket creation.
#  4. Negative security matrix (unauthorized socket creation fails fail-closed).
#  5. Real UDP and TCP packet transmission via VirtIO-Net DMA in QEMU user-mode networking.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_APP="${BUILD_DIR}/disk_web01a.img"
LOG_STAGE1="${EVIDENCE_DIR}/web01a_stage1_serial.log"

if [ ! -f "${OVMF_CODE}" ]; then
    echo "[ERROR] OVMF_CODE.fd not found at ${OVMF_CODE}" >&2
    exit 1
fi

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE WEB-01A] NATIVE NETWORKING FOUNDATION ACCEPTANCE SUITE       ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: VirtIO-Net Initialization, Sockets & Ring 3 Network Test  <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
dd if=/dev/zero of="${DISK_APP}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/web01a_mon.sock
MON_SOCK="/tmp/web01a_mon.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_APP}",format=raw,index=1,media=disk \
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

    # Wait for document name prompt and dispatch "net"
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'net'..."
            (for key in n e t ret; do echo "sendkey ${key}"; sleep 0.02; done) | socat - UNIX-CONNECT:"${MON_SOCK}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID}" || true
rm -f "${MON_SOCK}"

echo ""
echo "[VERIFY] Checking WEB-01A Serial Invariants..."
grep -q "\[NET_INIT\] VirtIO-Net PCI device initialized successfully\." "${LOG_STAGE1}"
grep -q "\[WEB_01A_WORKFLOW\] Stage 1: Native Networking Foundation (WEB-01A)\.\.\." "${LOG_STAGE1}"
grep -q "\[WEB_01A_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[NET_APP\] Native Ring 3 Network Test Application started (Milestone WEB-01A)\." "${LOG_STAGE1}"
grep -q "\[NET_APP\] Hardware CPL=3 isolation confirmed\." "${LOG_STAGE1}"
grep -q "\[NET_APP\] PASS: Negative socket creation rejected fail-closed (PermissionDenied)\." "${LOG_STAGE1}"
grep -q "\[SECURITY_POLICY\] APPROVED: Policy granted scoped capability for 'net:socket' to org\.inos\.net" "${LOG_STAGE1}"
grep -q "\[NET_APP\] Scoped capability token successfully issued by trusted OS broker\." "${LOG_STAGE1}"
grep -q "\[KERNEL_SYSCALL\] SYS_NET_SOCKET allocated fd=1 for PID=300" "${LOG_STAGE1}"
grep -q "\[NET_APP\] UDP datagram successfully transmitted via VirtIO-Net DMA\." "${LOG_STAGE1}"
grep -q "\[WEB_01A_UDP_EXCHANGE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01A_TCP_STREAM_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01A_CAPABILITY_ISOLATION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[NET_APP\] Milestone WEB-01A ALL ACCEPTANCE GATES PASSED (100% VERIFIED)\." "${LOG_STAGE1}"
grep -q "\[WEB_01A_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"

echo ""
echo "=========================================================================="
echo "=== [WEB-01A] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)              ==="
echo "=========================================================================="
