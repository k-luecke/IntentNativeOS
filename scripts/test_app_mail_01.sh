#!/usr/bin/env bash
# scripts/test_app_mail_01.sh - Acceptance suite for Milestone APP-MAIL-01 (Mail Client & MIME Engine)
# Verifies:
# 1. RFC 5322 header unfolding, key-value parsing, address extraction under Ring 3 (CPL=3)
# 2. RFC 2045/2046 MIME multipart/mixed boundary scanning and subpart extraction
# 3. Base64 encoding/decoding and Quoted-Printable decoding engines
# 4. Maildir flag serialization and roundtrip parsing (:2,FS)
# 5. Message composer with multipart attachment assembly and roundtrip fidelity
# 6. Scoped Ring 3 filesystem persistence into INOS-FS (inbox.eml, attachment.csv) and Stage 32 cold reboot persistence
# 7. External format interoperability with standard RFC 5322 / MIME specifications (Zero Python)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_MAIL="${BUILD_DIR}/disk_app_mail_01.img"

LOG_STAGE1="${EVIDENCE_DIR}/app_mail_01_stage1_serial.log"
LOG_COLD_REBOOT="${EVIDENCE_DIR}/app_mail_01_cold_reboot_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [LANE C: APP-MAIL-01] NATIVE MAIL & MIME ENGINE ACCEPTANCE        ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1 - STAGE 1: Full Suite Execution & Invariant Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: RFC 5322, MIME Parser, Decoders, Maildir & Composer Ring 3 <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for APP-MAIL-01 test..."
dd if=/dev/zero of="${DISK_MAIL}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/mail_mon1.sock
MON_SOCK1="/tmp/mail_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_MAIL}",format=raw,index=1,media=disk \
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

    # Wait for document name prompt and dispatch "mail"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'mail'..."
            (for key in m a i l ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APP-MAIL-01 Stage 1 Invariants..."
grep -q "\[APP_MAIL_01_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[MAIL_RFC5322_HEADER_PARSER_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[MAIL_MIME_DECODERS_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[MAIL_MULTIPART_EXTRACTION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[MAIL_MAILDIR_FLAGS_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[MAIL_COMPOSER_ROUNDTRIP_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[MAIL_MESSAGE_PERSISTENCE_WRITTEN_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[MAIL_ATTACHMENT_PERSISTENCE_WRITTEN_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_MAIL_SUITE_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_MAIL_01_APP_CLEAN_EXIT_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_MAIL_01_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: All RFC 5322 headers, MIME parts, decoders, Maildir flags, and composer verified!"

# =========================================================================
# PHASE 2 - STAGE 39: Cold Power Cycle Persistence Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & Mail Persistence Check                 <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_MAIL}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 > "${LOG_COLD_REBOOT}" 2>&1 || true

echo ""
echo "[VERIFY] Checking APP-MAIL-01 Cold Reboot Invariants..."
grep -q "\[APP_MAIL_01_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
grep -q "\[APP_MAIL_01_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
echo "  [PASS] Cold Reboot Verified: 'inbox.eml' and 'attachment.csv' verified across cold power cycle!"

# =========================================================================
# PHASE 3: External Format Interoperability Audit (RFC 5322 / MIME)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: External Format Interoperability Audit (RFC 5322 / MIME)  <<<"
echo "--------------------------------------------------------------------------"

# Verify that the raw ATA disk contains valid RFC 5322 headers and MIME structure
if grep -a -q "From: Kyle Luecke <kyle@inos.org>" "${DISK_MAIL}"; then
    echo "  [PASS] Disk image contains valid From header!"
else
    echo "  [FAIL] Disk image missing From header!"
    exit 1
fi

if grep -a -q "MIME-Version: 1.0" "${DISK_MAIL}"; then
    echo "  [PASS] Disk image contains MIME-Version header!"
else
    echo "  [FAIL] Disk image missing MIME-Version header!"
    exit 1
fi

if grep -a -q "boundary=" "${DISK_MAIL}"; then
    echo "  [PASS] Disk image contains MIME boundary parameter!"
else
    echo "  [FAIL] Disk image missing MIME boundary parameter!"
    exit 1
fi

if grep -a -q "Subsystem,Status" "${DISK_MAIL}"; then
    echo "  [PASS] Disk image contains persistent attachment data ('Subsystem,Status')!"
else
    echo "  [FAIL] Disk image missing persistent attachment data!"
    exit 1
fi

echo ""
echo "=========================================================================="
echo "=== [APP-MAIL-01] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)         ==="
echo "=========================================================================="
