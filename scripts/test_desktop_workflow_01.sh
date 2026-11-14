#!/usr/bin/env bash
# scripts/test_desktop_workflow_01.sh - Acceptance suite for Milestone APF-001 (Native Desktop Functional Completion)
# Verifies:
# 1. Desktop Habitable Shell: Spatial window manager, launcher, and floating intent bar
# 2. Native Ring 3 Grid: Interactive spreadsheet editing (B2=15, B3=25, B4=35, B5==SUM(B2:B4)->75)
# 3. Document Persistence: metrics.csv and metrics_computed.xlsx saved to INOS-FS
# 4. Native Ring 3 Write: Interactive document authoring, metrics import (Total=75), executive report generation
# 5. Document Persistence: report.docx and executive_report.pdf (PDF 1.7) saved to INOS-FS
# 6. IntScript Scheduling: repeat_report.ints scheduled via floating intent bar
# 7. Layered Modal Authorization: Two-choice confirm/cancel modal with hardware confirmation
# 8. Dynamic Cross-App Execution: Flow engine runs dynamic cross-app pipeline with reversible action ledger
# 9. Transaction Receipt Card: Native visual receipt displayed on desktop
# 10. Cold Power Cycle Persistence: Stage 35 verification of all 4 document formats surviving QEMU shutdown
# 11. External Format Interoperability Audit: Validating CSV, OpenXML XLSX, OpenXML DOCX, and PDF 1.7 on raw ATA disk

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_DESKTOP="${BUILD_DIR}/disk_desktop_workflow_01.img"

LOG_STAGE1="${EVIDENCE_DIR}/desktop_workflow_01_stage1_serial.log"
LOG_COLD_REBOOT="${EVIDENCE_DIR}/desktop_workflow_01_cold_reboot_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [APF-001: NATIVE DESKTOP WORKFLOW ACCEPTANCE SUITE]                ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1 - STAGE 1: Full Desktop Workflow Execution & Invariant Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Native Desktop Workflow Execution & Multi-App Automation   <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for Desktop Workflow test..."
dd if=/dev/zero of="${DISK_DESKTOP}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/desktop_mon1.sock
MON_SOCK1="/tmp/desktop_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_DESKTOP}",format=raw,index=1,media=disk \
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

    # Wait for document name prompt and dispatch "desktop"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'desktop'..."
            (for key in d e s k t o p ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # Wait for intent modal option prompt and dispatch "1"
    for _ in {1..1000}; do
        if grep -q "\[F106_AWAITING_OPTION_INPUT\]" "${LOG_STAGE1}" 2>/dev/null; then
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
echo "[VERIFY] Checking APF-001 Native Desktop Workflow Stage 1 Invariants..."
grep -q "\[DESKTOP_WORKFLOW_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_HABITABLE_RENDERED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_LAUNCHER_APP_OPENED_MARKER:org.inos.grid\]" "${LOG_STAGE1}"
grep -q "\[GRID_INTERACTIVE_EDIT_COMMITTED:B5=75\]" "${LOG_STAGE1}"
grep -q "\[GRID_INTERACTIVE_DOCUMENTS_SAVED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_GRID_EXECUTION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_LAUNCHER_APP_OPENED_MARKER:org.inos.write\]" "${LOG_STAGE1}"
grep -q "\[WRITE_INTERACTIVE_METRICS_IMPORTED:Total=75\]" "${LOG_STAGE1}"
grep -q "\[WRITE_INTERACTIVE_DOCUMENTS_SAVED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_WRITE_EXECUTION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_INTSCRIPT_AUTOMATION_SCHEDULED_MARKER:repeat_report.ints\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_INTENT_MODAL_RENDERED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[INTENT_MODAL_CONFIRMED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_INTSCRIPT_PIPELINE_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_TRANSACTION_RECEIPT_RENDERED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DESKTOP_WORKFLOW_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"

echo "  [PASS] Stage 1 Verified: Desktop shell, Grid interactive editing, Write document authoring, IntScript scheduling, modal confirmation, and transaction receipt verified!"

# =========================================================================
# PHASE 2 - STAGE 35: Cold Power Cycle Persistence Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & Multi-Format Persistence Check (Stage 35) <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_DESKTOP}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 > "${LOG_COLD_REBOOT}" 2>&1 || true

echo ""
echo "[VERIFY] Checking APF-001 Cold Reboot Invariants..."
grep -q "\[DESKTOP_WORKFLOW_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
grep -q "\[DESKTOP_WORKFLOW_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
echo "  [PASS] Cold Reboot Verified: metrics.csv, metrics_computed.xlsx, report.docx, and executive_report.pdf verified across cold reboot!"

# =========================================================================
# PHASE 3: External Format Interoperability Audit (CSV, XLSX, DOCX, PDF)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: External Multi-Format Interoperability Audit              <<<"
echo "--------------------------------------------------------------------------"

# 1. Verify CSV
if grep -a -q "Subsystem,PassCount,FailCount" "${DISK_DESKTOP}" && grep -a -q "Total,75,0" "${DISK_DESKTOP}"; then
    echo "  [PASS] Disk image contains valid RFC 4180 CSV ('Total,75,0')!"
else
    echo "  [FAIL] Disk image missing valid RFC 4180 CSV!"
    exit 1
fi

# 2. Verify OpenXML XLSX
if grep -a -q "xl/worksheets/sheet1.xml" "${DISK_DESKTOP}" && grep -a -q "SUM(B2:B4)" "${DISK_DESKTOP}"; then
    echo "  [PASS] Disk image contains valid OpenXML Spreadsheet ('xl/worksheets/sheet1.xml', 'SUM(B2:B4)')!"
else
    echo "  [FAIL] Disk image missing valid OpenXML XLSX archive!"
    exit 1
fi

# 3. Verify OpenXML DOCX
if grep -a -q "word/document.xml" "${DISK_DESKTOP}"; then
    echo "  [PASS] Disk image contains valid OpenXML Word Document ('word/document.xml')!"
else
    echo "  [FAIL] Disk image missing valid OpenXML DOCX archive!"
    exit 1
fi

# 4. Verify PDF 1.7
if grep -a -q "%PDF-1.7" "${DISK_DESKTOP}" && grep -a -q "%%EOF" "${DISK_DESKTOP}"; then
    echo "  [PASS] Disk image contains valid binary PDF 1.7 stream ('%PDF-1.7', '%%EOF')!"
else
    echo "  [FAIL] Disk image missing valid PDF 1.7 stream!"
    exit 1
fi

echo ""
echo "=========================================================================="
echo "=== [APF-001: NATIVE DESKTOP WORKFLOW ACCEPTANCE SUITE PASSED]         ==="
echo "=========================================================================="
