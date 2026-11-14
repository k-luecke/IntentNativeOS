#!/usr/bin/env bash
# scripts/test_app_plan_01.sh - Acceptance suite for Milestone APP-PLAN-01 (Calendar, Task Engine & Temporal Scheduler)
# Verifies:
# 1. Gregorian calendar math, leap years, Sakamoto day of week, timestamp bidirectional conversion
# 2. RFC 5545 RRULE expansion (weekday bitmasks, count limits, until limits, exception dates EXDATE)
# 3. Pairwise event conflict detection and free slot interval discovery
# 4. Hierarchical task engine with parent/child cascades and percentage completion
# 5. RFC 5545 iCalendar (.ics) generation, VEVENT, VTODO, and roundtrip import
# 6. Scoped Ring 3 filesystem persistence into INOS-FS and Stage 30 cold reboot persistence
# 7. External format interoperability with RFC 5545 calendar specifications (Zero Python)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_PLAN="${BUILD_DIR}/disk_app_plan_01.img"

LOG_STAGE1="${EVIDENCE_DIR}/app_plan_01_stage1_serial.log"
LOG_COLD_REBOOT="${EVIDENCE_DIR}/app_plan_01_cold_reboot_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [LANE C: APP-PLAN-01] NATIVE CALENDAR & TASK ENGINE ACCEPTANCE     ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# =========================================================================
# PHASE 1 - STAGE 1: Full Suite Execution & Invariant Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Temporal Math, Recurrence, Conflict Detection & Ring 3   <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for APP-PLAN-01 test..."
dd if=/dev/zero of="${DISK_PLAN}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/plan_mon1.sock
MON_SOCK1="/tmp/plan_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_PLAN}",format=raw,index=1,media=disk \
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

    # Wait for document name prompt and dispatch "plan"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'plan'..."
            (for key in p l a n ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APP-PLAN-01 Stage 1 Invariants..."
grep -q "\[APP_PLAN_01_STAGE1_STARTED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PLAN_TEMPORAL_MATH_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PLAN_RRULE_EXPANSION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PLAN_CONFLICT_DETECTION_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PLAN_TASK_HIERARCHY_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PLAN_ICS_ROUNDTRIP_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PLAN_ICS_PERSISTENCE_WRITTEN_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[PLAN_TASKS_PERSISTENCE_WRITTEN_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_PLAN_SUITE_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_PLAN_01_APP_CLEAN_EXIT_MARKER_v1.0\]" "${LOG_STAGE1}"
grep -q "\[APP_PLAN_01_STAGE1_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: All temporal math, RRULE recurrence, conflict search, task tree, and ICS verified!"

# =========================================================================
# PHASE 2 - COLD REBOOT: Cold QEMU Power Cycle & Persistence Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Cold QEMU Reboot & Calendar Persistence Check             <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_COLD_REBOOT}"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_PLAN}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 > "${LOG_COLD_REBOOT}" 2>&1 || true

echo ""
echo "[VERIFY] Checking APP-PLAN-01 Cold Reboot Invariants..."
grep -q "\[APP_PLAN_01_COLD_REBOOT_MOUNTED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
grep -q "\[APP_PLAN_01_COLD_REBOOT_VERIFIED_MARKER_v1.0\]" "${LOG_COLD_REBOOT}"
echo "  [PASS] Cold Reboot Verified: 'calendar.ics' and 'tasks.txt' verified across cold power cycle!"

# =========================================================================
# PHASE 3 - INTEROPERABILITY: External Inspection of RFC 5545 Calendar
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: External Format Interoperability Audit (RFC 5545 ICS)     <<<"
echo "--------------------------------------------------------------------------"

if grep -q "BEGIN:VCALENDAR" "${DISK_PLAN}"; then
    echo "  [PASS] Disk image contains valid RFC 5545 BEGIN:VCALENDAR marker!"
else
    echo "  [FAIL] BEGIN:VCALENDAR not found on raw ATA disk!"
    exit 1
fi

if grep -q "BEGIN:VEVENT" "${DISK_PLAN}"; then
    echo "  [PASS] Disk image contains valid RFC 5545 BEGIN:VEVENT marker!"
else
    echo "  [FAIL] BEGIN:VEVENT not found on raw ATA disk!"
    exit 1
fi

if grep -q "BEGIN:VTODO" "${DISK_PLAN}"; then
    echo "  [PASS] Disk image contains valid RFC 5545 BEGIN:VTODO marker!"
else
    echo "  [FAIL] BEGIN:VTODO not found on raw ATA disk!"
    exit 1
fi

echo ""
echo "=========================================================================="
echo "=== [APP-PLAN-01] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)         ==="
echo "=========================================================================="
