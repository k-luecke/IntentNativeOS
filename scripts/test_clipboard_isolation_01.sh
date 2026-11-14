#!/usr/bin/env bash
# scripts/test_clipboard_isolation_01.sh - Acceptance suite for APF-002C (Capability & Input Security)
# Verifies:
# 1. Capability-mediated clipboard access with rights::CLIPBOARD_READ and rights::CLIPBOARD_WRITE
# 2. Foreground focus ownership enforcement: background processes denied clipboard access fail-closed
# 3. Fail-closed rejection when no capability token or insufficient rights are presented
# 4. Ephemeral read validity & TOCTOU protection: clipboard epoch advances on write, stale tokens denied
# 5. Untrusted application security policy denial: unauthorized apps denied clipboard tokens fail-closed
# 6. Key-state tracking and release synthesis on focus switch (no stuck keys)
# 7. Non-blocking drop-tail queue overflow hygiene with diagnostic logging

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_CLIP="${BUILD_DIR}/disk_clipboard_isolation_01.img"

LOG_SERIAL="${EVIDENCE_DIR}/clipboard_isolation_01_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [APF-002C: CAPABILITY & INPUT SECURITY ACCEPTANCE SUITE]           ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Clipboard Capability Mediation, TOCTOU & Input Hygiene    <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for clipboard security test..."
dd if=/dev/zero of="${DISK_CLIP}" bs=1M count=16 status=none

rm -f "${LOG_SERIAL}" /tmp/clip_mon1.sock
MON_SOCK1="/tmp/clip_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_CLIP}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK1}",server=on,wait=off \
    -mon chardev=mon0,mode=readline > "${LOG_SERIAL}" 2>&1 &
QEMU_PID1=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK1}" ]; then break; fi
        sleep 0.1
    done

    # Wait for document name prompt and dispatch "cliptest"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_SERIAL}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'cliptest'..."
            (for key in c l i p t e s t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APF-002C Clipboard Isolation & Input Hygiene Invariants..."

# Check 1: Stage started
grep -q "\[CLIPBOARD_ISOLATION_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 1. Clipboard Isolation Stage 1 initialized."

# Check 2: Background process denial
grep -q "\[CLIPBOARD_BACKGROUND_DENIAL_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 2. Background process denied clipboard access fail-closed."

# Check 3: Tokenless request denial
grep -q "\[CLIPBOARD_NO_TOKEN_DENIAL_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 3. Tokenless request rejected fail-closed."

# Check 4: Insufficient rights denial
grep -q "\[CLIPBOARD_INSUFFICIENT_RIGHTS_DENIAL_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 4. Insufficient capability rights rejected fail-closed."

# Check 5: Authorized write
grep -q "\[CLIPBOARD_WRITE_AUTHORIZED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 5. Authorized foreground SetClipboard succeeded."

# Check 6: Ephemeral read validity / TOCTOU protection
grep -q "\[CLIPBOARD_EPHEMERAL_STALE_DENIAL_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 6. Stale capability token rejected fail-closed (TOCTOU protection)."

# Check 7: Authorized fresh read
grep -q "\[CLIPBOARD_READ_AUTHORIZED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 7. Fresh read token authorized and payload verified."

# Check 8: Untrusted policy denial
grep -q "\[CLIPBOARD_POLICY_DENIAL_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 8. Untrusted application policy denial confirmed fail-closed."

# Check 9: Key-state tracking and overflow hygiene
grep -q "\[COMPOSITOR_KEY_STATE_HYGIENE_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 9. Key-state tracking, synthesized releases, and overflow hygiene confirmed."

# Check 10: Complete marker
grep -q "\[CLIPBOARD_ISOLATION_COMPLETE_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 10. Milestone APF-002C fully verified."

echo ""
echo "=========================================================================="
echo "=== [GATE APF-002C: PASSED 10/10 INVARIANTS VERIFIED]                 ==="
echo "=========================================================================="
