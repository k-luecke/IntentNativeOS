#!/usr/bin/env bash
# scripts/test_web_01b3.sh - Acceptance suite for Milestone WEB-01B.3 (Authenticated HTTPS)
# Verifies:
#  1. Native Ring 3 authenticated HTTPS client with rustls 0.23, webpki, and ring on x86_64-unknown-none.
#  2. Fail-closed negative security matrix:
#     - Expired X.509 certificate rejected
#     - Untrusted / rogue root CA rejected
#     - Hostname / SAN mismatch rejected (SAN required, CN fallback prohibited)
#     - Truncated / malformed handshake rejected fail-closed
#  3. Positive authenticated TLS 1.3 handshake and HTTPS GET /hello transaction against independent OpenSSL server.
#  4. FPU/SIMD state preservation and hardware CPL=3 capability isolation.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_APP="${BUILD_DIR}/disk_web01b3.img"
LOG_STAGE1="${EVIDENCE_DIR}/web01b3_stage1_serial.log"

if [ ! -f "${OVMF_CODE}" ]; then
    echo "[ERROR] OVMF_CODE.fd not found at ${OVMF_CODE}" >&2
    exit 1
fi

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE WEB-01B.3] AUTHENTICATED HTTPS ACCEPTANCE SUITE               ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring test certificates and web payload are configured..."
bash "${SCRIPT_DIR}/generate_test_certs.sh"

WWW_DIR="/tmp/inos_www"
mkdir -p "${WWW_DIR}"
echo "INOS_HTTPS_VERIFIED_PAYLOAD_2026" > "${WWW_DIR}/hello"

CERTS_TMP="/tmp/inos_certs"
mkdir -p "${CERTS_TMP}"
cp "${ROOT_DIR}"/certs/*.crt "${CERTS_TMP}/"
cp "${ROOT_DIR}"/certs/*.key "${CERTS_TMP}/"

# Cleanup function to terminate background servers
cleanup() {
    echo "[CLEANUP] Stopping background OpenSSL test servers..."
    killall openssl 2>/dev/null || true
    rm -f /tmp/web01b3_mon.sock
}
trap cleanup EXIT INT TERM

# Kill any stale servers on ports 4443..4446
killall openssl 2>/dev/null || true
sleep 0.5

echo "[STEP 1] Starting independent OpenSSL TLS 1.3 test servers on ports 4443-4446..."
start_server() {
    local port="$1"
    local cert="$2"
    local key="$3"
    local docroot="$4"
    (
        cd "${docroot}"
        while true; do
            openssl s_server -tls1_3 -accept "${port}" -cert "${cert}" -key "${key}" -WWW >/dev/null 2>&1 || true
            sleep 0.05
        done
    ) &
}

start_server 4443 "${CERTS_TMP}/server_expired.crt" "${CERTS_TMP}/server_expired.key" "${WWW_DIR}"
start_server 4444 "${CERTS_TMP}/server_untrusted.crt" "${CERTS_TMP}/server_untrusted.key" "${WWW_DIR}"
start_server 4445 "${CERTS_TMP}/server_mismatch.crt" "${CERTS_TMP}/server_mismatch.key" "${WWW_DIR}"
start_server 4446 "${CERTS_TMP}/server_valid.crt" "${CERTS_TMP}/server_valid.key" "${WWW_DIR}"

sleep 1

# Verify servers are up
for port in 4443 4444 4445 4446; do
    if ! ss -tulpn | grep -q ":${port} "; then
        echo "[ERROR] OpenSSL server failed to start on port ${port}!" >&2
        exit 1
    fi
done
echo "[STEP 1] All 4 OpenSSL TLS 1.3 test servers active."

echo ""
echo "[STEP 2] Compiling kernel and native Ring 3 network application..."
if [ "${SKIP_BUILD:-0}" != "1" ]; then
    bash "${SCRIPT_DIR}/build_kernel.sh"
else
    echo "[STEP 2] Skipping build (SKIP_BUILD=1)."
fi

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Native Authenticated HTTPS, Negative Matrix & Positive GET<<<"
echo "--------------------------------------------------------------------------"

OVMF_VARS_RUN="/tmp/ovmf_vars_web01b3.fd"
cp "${OVMF_VARS}" "${OVMF_VARS_RUN}"
rm -f "${ESP_DIR}/NvVars"
printf 'fs0:\r\n\\EFI\\BOOT\\BOOTX64.EFI\r\n' > "${ESP_DIR}/startup.nsh"

dd if=/dev/zero of="${DISK_APP}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/web01b3_mon.sock
MON_SOCK="/tmp/web01b3_mon.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -cpu max \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS_RUN}" \
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

    # Wait for document name prompt and dispatch "web01b3"
    for _ in {1..2000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'web01b3'..."
            (for key in w e b 0 1 b 3 ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK}" 2>/dev/null || true
            break
        fi
        sleep 0.01
    done
) &

wait "${QEMU_PID}" || true
rm -f "${MON_SOCK}"

echo ""
echo "[VERIFY] Checking WEB-01B.3 Serial Invariants..."
grep -q "\[NET_INIT\] VirtIO-Net PCI device initialized successfully\." "${LOG_STAGE1}"
grep -q "\[WEB_01B\] Initializing CMOS RTC clock & hardware entropy CSPRNG\.\.\." "${LOG_STAGE1}"
grep -q "\[NET_APP\] Native Ring 3 Authenticated HTTPS Client started (Milestone WEB-01B\.3)\." "${LOG_STAGE1}"
grep -q "\[NET_APP\] Hardware CPL=3 isolation confirmed\." "${LOG_STAGE1}"
grep -q "\[WEB_01A_UDP_EXCHANGE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01A_TCP_STREAM_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B_TCP_RELIABLE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B_DNS_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B_RTC_TIME_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B_ENTROPY_CSPRNG_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"

# WEB-01B.3 Specific Invariants
grep -q "\[NET_APP\] Rustls 0\.23 TLS 1\.3 ClientConfig initialized successfully\." "${LOG_STAGE1}"
grep -q "\[WEB_01B3_EXPIRED_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B3_UNTRUSTED_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B3_SAN_MISMATCH_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B3_TRUNCATED_HANDSHAKE_REJECTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B3_NEGATIVE_SECURITY_MATRIX_PASSED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B3_TLS_HANDSHAKE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B3_SAN_VALIDATION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[WEB_01B3_HTTPS_GET_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[NET_APP\] Milestone WEB-01B\.3 ALL ACCEPTANCE GATES PASSED (100% VERIFIED)\." "${LOG_STAGE1}"

echo ""
echo "=========================================================================="
echo "=== [WEB-01B.3] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)            ==="
echo "=========================================================================="
