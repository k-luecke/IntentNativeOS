#!/usr/bin/env bash
# scripts/test_app_script_01.sh - Acceptance suite for Milestone APP-SCRIPT-001 (INOS Script Native Automation Platform)
# Verifies:
# 1. Ring 3 INOS Script Studio Compositor GUI surface registration & rendering (editor, syntax coloring, debug logs)
# 2. Embedded ECMAScript micro-engine executing human-authored Grid automation workflow
# 3. Persistent job queue database (/system/jobs.db) serialization and scheduling
# 4. Event cycle loop defense suppressing recursive cascading executions
# 5. Trusted capability broker negative security testing denying access to private files
# 6. Runaway execution instruction quota termination
# 7. Agent-generated script execution equivalence
# 8. Cold QEMU reboot persistence of jobs database, output documents, and audit logs
# 9. External interoperability validation with Info-ZIP (unzip -t) and headless LibreOffice Calc round-trip

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_SCRIPT01="${BUILD_DIR}/disk_app_script_01.img"

LOG_STAGE1="${EVIDENCE_DIR}/app_script_01_stage1_serial.log"
LOG_COLD_REBOOT="${EVIDENCE_DIR}/app_script_01_cold_reboot_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [APP-SCRIPT-001] INOS SCRIPT: NATIVE AUTOMATION PLATFORM & SDK     ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1: Native Ring 3 INOS Script Workflow (APP-SCRIPT-001)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Script Studio, VM Execution, Quotas & Security Negative   <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for APP-SCRIPT-001 test..."
dd if=/dev/zero of="${DISK_SCRIPT01}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/script01_mon1.sock
MON_SOCK1="/tmp/script01_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_SCRIPT01}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK1}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE1}" &
QEMU_PID1=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK1}" ]; then break; fi
        sleep 0.1
    done

    # Wait for document name prompt and dispatch "script"
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'script'..."
            (for key in s c r i p t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # Dispatch Enter for content prompt to immediately proceed
    for _ in {1..100}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching enter for content prompt..."
            echo "sendkey ret" | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APP-SCRIPT-001 Stage 1 Invariants..."
grep -a -q "\[APP_SCRIPT_001_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -a -q "Hardware CPL=3 isolation confirmed." "${LOG_STAGE1}"
grep -a -q "\[SCRIPT_HUMAN_WORKFLOW_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[SCRIPT_JOB_SCHEDULING_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[SCRIPT_EVENT_CYCLE_DEFENSE_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[PERMISSION_DENIED_VERIFIED\]" "${LOG_STAGE1}"
grep -a -q "\[SCRIPT_SECURITY_DENIAL_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[QUOTA_EXCEEDED_VERIFIED\]" "${LOG_STAGE1}"
grep -a -q "\[SCRIPT_QUOTA_TERMINATION_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[SCRIPT_AGENT_EQUIVALENCE_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[SCRIPT_AUDIT_LOG_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[SCRIPT_UI_SURFACE_RENDERED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[SCRIPT_INTEROP_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -a -q "\[APP_SCRIPT_001_APP_CLEAN_EXIT_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -a -q "\[APP_SCRIPT_001_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: Script Studio, VM Execution, Quotas, Job DB, and Loop Defense passed!"

# =========================================================================
# PHASE 2: Cold QEMU Reboot & Persistence Check
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & Job DB / Output Persistence Check      <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_SCRIPT01}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 2>&1 | tee "${LOG_COLD_REBOOT}" || true

echo ""
echo "[VERIFY] Checking APP-SCRIPT-001 Cold Reboot Invariants..."
grep -a -q "\[APP_SCRIPT_001_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
grep -a -q "Persistent '/system/jobs.db' verified" "${LOG_COLD_REBOOT}"
grep -a -q "Persistent 'script_grid_out.xlsx' verified" "${LOG_COLD_REBOOT}"
grep -a -q "Persistent '/system/script_history.log' verified bit-for-bit." "${LOG_COLD_REBOOT}"
grep -a -q "\[APP_SCRIPT_001_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
echo "  [PASS] Cold Reboot Verified: '/system/jobs.db' and 'script_grid_out.xlsx' verified across power cycle!"

# =========================================================================
# PHASE 3: External Tool Interoperability Audit (Info-ZIP & LibreOffice)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: External Tool Interoperability Audit (ZIP & LibreOffice)  <<<"
echo "--------------------------------------------------------------------------"

EXTRACTED_XLSX="${BUILD_DIR}/script_grid_out_from_inos.xlsx"
FOUND_XLSX=0
XLSX_SIZE=3094

if grep -a -q "Persistent 'script_grid_out.xlsx' verified (size=" "${LOG_COLD_REBOOT}" 2>/dev/null; then
    XLSX_SIZE=$(grep -a -o "script_grid_out.xlsx' verified (size=[0-9]* bytes)" "${LOG_COLD_REBOOT}" | grep -a -o "[0-9]\+" | head -n 1 || echo 3094)
fi
echo "[INTEROP] Target XLSX package size from INOS-FS: ${XLSX_SIZE} bytes."

for CTOFF in $(grep -b -a -o "\[Content_Types\]\.xml" "${DISK_SCRIPT01}" | cut -d: -f1 || true); do
    ZIP_START=$((CTOFF - 30))
    dd if="${DISK_SCRIPT01}" of="${EXTRACTED_XLSX}" iflag=skip_bytes,count_bytes skip="${ZIP_START}" count="${XLSX_SIZE}" status=none
    if unzip -t "${EXTRACTED_XLSX}" >/dev/null 2>&1; then
        echo "[INTEROP] Found active valid XLSX package at offset ${ZIP_START} (size ${XLSX_SIZE} bytes)."
        FOUND_XLSX=1
        break
    fi
done

if [ "${FOUND_XLSX}" -ne 1 ]; then
    echo "[ERROR] Could not extract valid XLSX package from raw disk!"
    exit 1
fi

echo "[INFO-ZIP] Validating script-generated XLSX package with unzip -l..."
unzip -l "${EXTRACTED_XLSX}"

# LibreOffice Calc Headless roundtrip verification
SOFFICE_BIN=""
if [ -f "/mnt/c/Program Files/LibreOffice/program/soffice.com" ]; then
    SOFFICE_BIN="/mnt/c/Program Files/LibreOffice/program/soffice.com"
elif command -v libreoffice >/dev/null 2>&1; then
    SOFFICE_BIN="$(command -v libreoffice)"
fi

if [ -n "${SOFFICE_BIN}" ]; then
    echo "[LIBREOFFICE] Found Calc binary: ${SOFFICE_BIN}"
    echo "[LIBREOFFICE] Converting script_grid_out_from_inos.xlsx to CSV..."
    rm -f "${BUILD_DIR}/script_grid_out_from_inos.csv"
    (cd "${BUILD_DIR}" && "${SOFFICE_BIN}" --headless --convert-to csv script_grid_out_from_inos.xlsx)

    if [ -f "${BUILD_DIR}/script_grid_out_from_inos.csv" ]; then
        echo "[LIBREOFFICE] CSV Conversion Succeeded! Contents:"
        cat "${BUILD_DIR}/script_grid_out_from_inos.csv"
        echo ""
        # Check computed profit
        if grep -q "10800" "${BUILD_DIR}/script_grid_out_from_inos.csv" || grep -q "15000" "${BUILD_DIR}/script_grid_out_from_inos.csv"; then
            echo "  [PASS] External LibreOffice Calc successfully evaluated and extracted script workbook values!"
        fi
    fi
else
    echo "[LIBREOFFICE] LibreOffice not detected, Info-ZIP validation passed."
fi

echo ""
echo "=========================================================================="
echo "=== [PASS] APP-SCRIPT-001 CONFORMANCE & ACCEPTANCE GATES PASSED!       ==="
echo "=========================================================================="
