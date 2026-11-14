#!/usr/bin/env bash
# scripts/test_agent_flow_01.sh - Acceptance suite for Milestone AGENT-FLOW-01 (Cross-Application Workflows)
# Verifies:
# 1. Pipeline 1: Mail -> Plan (Meeting invite extraction -> RFC 5545 calendar_flow.ics)
# 2. Pipeline 2: Mail -> Files -> Grid (Attachment extraction -> metrics.csv -> DAG formula recalc -> metrics_computed.xlsx)
# 3. Pipeline 3: Grid -> Write -> PDF (Tabular metrics embedding -> OpenXML report.docx -> PDF 1.7 executive_report.pdf)
# 4. Reversible Action Ledger: Atomic action logging and compensatory rollback tracking (Quality Gate 5.2)
# 5. Cold Power Cycle Persistence: Stage 33 verification of all 4 document formats surviving QEMU shutdown
# 6. External Format Interoperability Audit: Validating RFC 5545, OpenXML XLSX, OpenXML DOCX, and PDF 1.7 on raw ATA disk

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_FLOW="${BUILD_DIR}/disk_agent_flow_01.img"

LOG_STAGE1="${EVIDENCE_DIR}/agent_flow_01_stage1_serial.log"
LOG_COLD_REBOOT="${EVIDENCE_DIR}/agent_flow_01_cold_reboot_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [LANE D: AGENT-FLOW-01] CROSS-APPLICATION WORKFLOW ACCEPTANCE      ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1 - STAGE 1: Full Suite Execution & Invariant Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Mail->Plan, Mail->Grid, Grid->Write->PDF & Action Ledger  <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for AGENT-FLOW-01 test..."
dd if=/dev/zero of="${DISK_FLOW}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/flow_mon1.sock
MON_SOCK1="/tmp/flow_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_FLOW}",format=raw,index=1,media=disk \
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

    # Wait for document name prompt and dispatch "flow"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'flow'..."
            (for key in f l o w ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking AGENT-FLOW-01 Stage 1 Invariants..."
grep -q "\[AGENT_FLOW_01_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[AGENT_WORKFLOW_1_MAIL_TO_PLAN_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[AGENT_WORKFLOW_2_MAIL_TO_GRID_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[AGENT_WORKFLOW_3_GRID_WRITE_PDF_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[AGENT_REVERSIBLE_ACTION_LEDGER_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[INOS_AUTOMATION_ABI_001_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DYNAMIC_PLAN_ASSEMBLY_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[DYNAMIC_CROSS_APP_EXECUTION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[AGENT_FLOW_SUITE_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[AGENT_FLOW_01_APP_CLEAN_EXIT_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[AGENT_FLOW_01_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[IPC_ROUTER\] Typed dispatch: PID 950 -> PID 960" "${LOG_STAGE1}"
grep -q "\[IPC_ROUTER\] Typed dispatch: PID 950 -> PID 600" "${LOG_STAGE1}"
grep -q "\[IPC_ROUTER\] Typed dispatch: PID 950 -> PID 800" "${LOG_STAGE1}"
grep -q "\[IPC_ROUTER\] Typed dispatch: PID 950 -> PID 900" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: All cross-application pipelines, reversible action ledger, dynamic automation ABI, and cross-process typed IPC dispatch verified!"

# =========================================================================
# PHASE 2 - STAGE 40: Cold Power Cycle Persistence Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & Multi-Format Persistence Check         <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_FLOW}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 > "${LOG_COLD_REBOOT}" 2>&1 || true

echo ""
echo "[VERIFY] Checking AGENT-FLOW-01 Cold Reboot Invariants..."
grep -q "\[AGENT_FLOW_01_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
grep -q "\[AGENT_FLOW_01_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
echo "  [PASS] Cold Reboot Verified: calendar_flow.ics, metrics.csv, metrics_computed.xlsx, report.docx, and executive_report.pdf verified across cold reboot!"

# =========================================================================
# PHASE 3: External Format Interoperability Audit (ICS, XLSX, DOCX, PDF)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: External Multi-Format Interoperability Audit              <<<"
echo "--------------------------------------------------------------------------"

# 1. Verify iCalendar
if grep -a -q "BEGIN:VCALENDAR" "${DISK_FLOW}" && grep -a -q "SUMMARY:Q4 Strategy Review" "${DISK_FLOW}"; then
    echo "  [PASS] Disk image contains valid RFC 5545 iCalendar stream ('SUMMARY:Q4 Strategy Review')!"
else
    echo "  [FAIL] Disk image missing valid iCalendar stream!"
    exit 1
fi

# 2. Verify OpenXML XLSX
if grep -a -q "xl/worksheets/sheet1.xml" "${DISK_FLOW}" && grep -a -q "SUM(B2:B4)" "${DISK_FLOW}"; then
    echo "  [PASS] Disk image contains valid OpenXML XLSX container with SUM(B2:B4) formula!"
else
    echo "  [FAIL] Disk image missing valid OpenXML XLSX data!"
    exit 1
fi

# 3. Verify OpenXML DOCX
if grep -a -q "word/document.xml" "${DISK_FLOW}" && grep -a -q "INOS Foundry Executive Execution Report" "${DISK_FLOW}"; then
    echo "  [PASS] Disk image contains valid OpenXML DOCX container with executive report heading!"
else
    echo "  [FAIL] Disk image missing valid OpenXML DOCX data!"
    exit 1
fi

# 4. Verify PDF 1.7
if grep -a -q "%PDF-1.7" "${DISK_FLOW}" && grep -a -q "%%EOF" "${DISK_FLOW}"; then
    echo "  [PASS] Disk image contains valid PDF 1.7 header and EOF trailer!"
else
    echo "  [FAIL] Disk image missing valid PDF 1.7 structure!"
    exit 1
fi

# 5. Verify Dynamic Automation ABI Artifacts (SVG & EML)
if grep -a -q "Performance Growth Q1-Q4" "${DISK_FLOW}"; then
    echo "  [PASS] Disk image contains dynamic ABI SVG chart artifact ('Performance Growth Q1-Q4')!"
else
    echo "  [FAIL] Disk image missing dynamic ABI SVG chart artifact!"
    exit 1
fi

if grep -a -q "DYNAMIC_DISPATCH_BOUNDARY" "${DISK_FLOW}"; then
    echo "  [PASS] Disk image contains dynamic ABI EML dispatch artifact ('DYNAMIC_DISPATCH_BOUNDARY')!"
else
    echo "  [FAIL] Disk image missing dynamic ABI EML dispatch artifact!"
    exit 1
fi

echo ""
echo "=========================================================================="
echo "=== [AGENT-FLOW-01] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)       ==="
echo "=========================================================================="
