#!/usr/bin/env bash
# scripts/test_workflow_rollback_01.sh - Acceptance suite for APF-002E (Action Ledger Rollback on Injected Crash)
# Verifies:
# 1. Action Ledger transaction recording and compensatory rollback pairing
# 2. Injected crash / fault detection during in-flight pipeline execution
# 3. Deterministic reverse-order compensatory rollback execution
# 4. Complete storage state restoration with zero orphaned partial locks
# 5. Non-regression of cross-application workflow pipelines

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_ROLLBACK="${BUILD_DIR}/disk_workflow_rollback_01.img"

LOG_SERIAL="${EVIDENCE_DIR}/workflow_rollback_01_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [APF-002E: WORKFLOW ACTION LEDGER ROLLBACK & CRASH RESILIENCE]    ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, native Ring 3 apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Executing Cross-App Flow with Injected Crash Rollback     <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for Rollback test..."
dd if=/dev/zero of="${DISK_ROLLBACK}" bs=1M count=16 status=none

rm -f "${LOG_SERIAL}" /tmp/rollback_mon1.sock
MON_SOCK1="/tmp/rollback_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_ROLLBACK}",format=raw,index=1,media=disk \
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

    # Wait for document prompt and dispatch "flowapp"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_SERIAL}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'flowapp'..."
            (for key in f l o w a p p ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APF-002E Action Ledger & Rollback Invariants..."

# Check 1: Action Ledger initialized
grep -q "\[AGENT_REVERSIBLE_ACTION_LEDGER_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 1. Reversible Action Ledger verified with 1:1 action-to-compensation parity."

# Check 2: Injected crash detected
grep -q "\[ACTION_LEDGER_INJECTED_CRASH_DETECTED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 2. Injected crash / simulated fault safely detected during transaction."

# Check 3: Compensatory rollback verified
grep -q "\[ACTION_LEDGER_COMPENSATORY_ROLLBACK_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 3. Reverse-order compensatory rollback executed; pre-transaction state restored."

# Check 4: Dynamic cross-app execution complete
grep -q "\[DYNAMIC_CROSS_APP_EXECUTION_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
grep -q "\[AGENT_FLOW_SUITE_COMPLETE_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 4. Dynamic cross-app execution completed cleanly post-recovery."

echo ""
echo "=========================================================================="
echo "=== [GATE APF-002E (PART 1): ACTION LEDGER ROLLBACK 4/4 VERIFIED]     ==="
echo "=========================================================================="
