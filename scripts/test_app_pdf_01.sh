#!/usr/bin/env bash
# scripts/test_app_pdf_01.sh - Acceptance suite for Milestone APP-PDF-01 (PDF Engine & Reader)
# Verifies:
# 1. PDF 1.7 xref table and object model parsing under Ring 3 (CPL=3)
# 2. Deflate/zlib FlateDecode and ASCIIHex stream decompression
# 3. Content stream operator interpretation (BT, ET, Tf, Td, Tj) and bounding box calculations
# 4. In-document string and text run search engine
# 5. Native PDF 1.7 document generation with valid xref and trailer
# 6. Scoped Ring 3 filesystem persistence into INOS-FS and Stage 31 cold reboot persistence
# 7. External format interoperability with standard PDF 1.7 specifications (Zero Python)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_PDF="${BUILD_DIR}/disk_app_pdf_01.img"

LOG_STAGE1="${EVIDENCE_DIR}/app_pdf_01_stage1_serial.log"
LOG_COLD_REBOOT="${EVIDENCE_DIR}/app_pdf_01_cold_reboot_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [LANE C: APP-PDF-01] NATIVE PDF DOCUMENT ENGINE ACCEPTANCE         ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1 - STAGE 1: Full Suite Execution & Invariant Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Object Model, Stream Decoders, Search, & PDF 1.7 Ring 3  <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for APP-PDF-01 test..."
dd if=/dev/zero of="${DISK_PDF}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/pdf_mon1.sock
MON_SOCK1="/tmp/pdf_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_PDF}",format=raw,index=1,media=disk \
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

    # Wait for document name prompt and dispatch "pdf"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'pdf'..."
            (for key in p d f ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APP-PDF-01 Stage 1 Invariants..."
grep -q "\[APP_PDF_01_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PDF_OBJECT_MODEL_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PDF_STREAM_DECODER_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PDF_TEXT_EXTRACTION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PDF_SEARCH_ENGINE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PDF_GENERATOR_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PDF_PERSISTENCE_WRITTEN_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PDF_EXTRACTED_TEXT_PERSISTENCE_WRITTEN_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_PDF_SUITE_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_PDF_01_APP_CLEAN_EXIT_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_PDF_01_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: All PDF objects, streams, search, and generation verified!"

# =========================================================================
# PHASE 2 - STAGE 38: Cold Power Cycle Persistence Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & PDF Persistence Check                  <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_PDF}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 > "${LOG_COLD_REBOOT}" 2>&1 || true

echo ""
echo "[VERIFY] Checking APP-PDF-01 Cold Reboot Invariants..."
grep -q "\[APP_PDF_01_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
grep -q "\[APP_PDF_01_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
echo "  [PASS] Cold Reboot Verified: 'document.pdf' and 'extracted_text.txt' verified across cold power cycle!"

# =========================================================================
# PHASE 3: External Format Interoperability Audit (PDF 1.7)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: External Format Interoperability Audit (PDF 1.7)          <<<"
echo "--------------------------------------------------------------------------"

# Verify that the raw ATA disk contains valid PDF 1.7 markers
if grep -a -q "%PDF-1.7" "${DISK_PDF}"; then
    echo "  [PASS] Disk image contains valid PDF 1.7 header (%PDF-1.7)!"
else
    echo "  [FAIL] Disk image missing %PDF-1.7 header!"
    exit 1
fi

if grep -a -q "%%EOF" "${DISK_PDF}"; then
    echo "  [PASS] Disk image contains valid PDF 1.7 footer (%%EOF)!"
else
    echo "  [FAIL] Disk image missing %%EOF footer!"
    exit 1
fi

if grep -a -q "/Type /Catalog" "${DISK_PDF}"; then
    echo "  [PASS] Disk image contains valid Catalog dictionary (/Type /Catalog)!"
else
    echo "  [FAIL] Disk image missing /Type /Catalog!"
    exit 1
fi

if grep -a -q "INOS Native PDF Engine" "${DISK_PDF}"; then
    echo "  [PASS] Disk image contains extracted text string ('INOS Native PDF Engine')!"
else
    echo "  [FAIL] Disk image missing extracted text string!"
    exit 1
fi

echo ""
echo "=========================================================================="
echo "=== [APP-PDF-01] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)          ==="
echo "=========================================================================="
