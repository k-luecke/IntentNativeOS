#!/usr/bin/env bash
# scripts/test_web_01b.sh - Acceptance suite for Milestone WEB-01B (Secure HTTPS Transport)
# Verifies:
#  1. Bounded RFC 1035 DNS client & parsing with transaction matching and pointer-loop defenses.
#  2. Negative capability matrix (unauthorized DNS & socket creation fail fail-closed).
#  3. Reliable TCP transport with sequence tracking, window validation, and clean FIN-ACK teardown.
#  4. CMOS RTC provisional wall-clock source and separate monotonic RDTSC timer ticks.
#  5. Vetted ChaCha20 CSPRNG hardware entropy pool with mandatory 256-bit readiness check.
#  6. Ring 3 CPL=3 isolation and capability-brokered syscall operations.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_APP="${BUILD_DIR}/disk_web01b.img"
LOG_STAGE1="${EVIDENCE_DIR}/web01b_stage1_serial.log"

if [ ! -f "${OVMF_CODE}" ]; then
    echo "[ERROR] OVMF_CODE.fd not found at ${OVMF_CODE}" >&2
    exit 1
fi

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE WEB-01B] SECURE HTTPS TRANSPORT ACCEPTANCE SUITE             ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: DNS, Reliable TCP, RTC Time, CSPRNG & Ring 3 Execution   <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
dd if=/dev/zero of="${DISK_APP}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/web01b_mon.sock
MON_SOCK="/tmp/web01b_mon.sock"

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

    # Wait for document name prompt and dispatch "web01b"
    for _ in {1..2000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'web01b'..."
            (for key in w e b 0 1 b ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK}" 2>/dev/null || true
            break
        fi
        sleep 0.01
    done
) &

wait "${QEMU_PID}" || true
rm -f "${MON_SOCK}"

echo ""
echo "[VERIFY] Checking WEB-01B Serial Invariants..."
grep -q "\[NET_INIT\] VirtIO-Net PCI device initialized successfully\." "${LOG_STAGE1}"
grep -q "\[WEB_01B\] Initializing CMOS RTC clock & hardware entropy CSPRNG\.\.\." "${LOG_STAGE1}"
grep -q "\[WEB_01B_WORKFLOW\] Stage 1: Secure HTTPS Transport (WEB-01B)\.\.\." "${LOG_STAGE1}"
grep -q "\[WEB_01B_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[NET_APP\] Native Ring 3 Network Test Application started (Milestone WEB-01B)\." "${LOG_STAGE1}"
grep -q "\[NET_APP\] Hardware CPL=3 isolation confirmed\." "${LOG_STAGE1}"
grep -q "\[NET_APP\] PASS: Negative socket creation rejected fail-closed (PermissionDenied)\." "${LOG_STAGE1}"
grep -q "\[SECURITY_POLICY\] APPROVED: Policy granted scoped capability for 'net:socket' to org\.inos\.net" "${LOG_STAGE1}"
grep -q "\[NET_APP\] Scoped capability token successfully issued by trusted OS broker\." "${LOG_STAGE1}"
grep -q "\[WEB_01A_UDP_EXCHANGE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01A_TCP_STREAM_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B_TCP_RELIABLE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[NET_APP\] PASS: Unauthorized DNS resolution rejected (PermissionDenied)\." "${LOG_STAGE1}"
grep -q "\[KERNEL_SYSCALL\] SYS_NET_RESOLVE: 'host\.test' -> 10\.0\.2\.2" "${LOG_STAGE1}"
grep -q "\[WEB_01B_DNS_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[NET_APP\] RTC provisional wall clock and monotonic time verified\." "${LOG_STAGE1}"
grep -q "\[WEB_01B_RTC_TIME_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[NET_APP\] CSPRNG generated 32 random bytes under hardware entropy readiness\." "${LOG_STAGE1}"
grep -q "\[WEB_01B_ENTROPY_CSPRNG_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[NET_APP\] Milestone WEB-01B ALL ACCEPTANCE GATES PASSED (100% VERIFIED)\." "${LOG_STAGE1}"
grep -q "\[WEB_01B_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"

echo ""
echo "=========================================================================="
echo "=== [WEB-01B] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)              ==="
echo "=========================================================================="
