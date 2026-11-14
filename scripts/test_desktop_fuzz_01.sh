#!/usr/bin/env bash
# scripts/test_desktop_fuzz_01.sh - Acceptance suite for APF-002A (Desktop Interaction & Reproducible Input Fuzzing)
# Verifies:
# 1. Deterministic pseudo-random seed recording and input trace capture
# 2. Stochastic rapid focus changes between resident windows with key sanitization
# 3. Formula error fuzzing and recovery in Ring 3 Grid (#SYNTAX!, #DIV/0!, #VALUE!)
# 4. Input queue burst saturation (100 events) and non-blocking drop-tail hygiene
# 5. Active window termination while focused with automatic focus fallback
# 6. Deterministic replay mechanism (reproduce_fuzz.sh)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_FUZZ="${BUILD_DIR}/disk_desktop_fuzz_01.img"

LOG_SERIAL="${EVIDENCE_DIR}/desktop_fuzz_01_serial.log"
TRACE_FILE="${BUILD_DIR}/desktop_fuzz_trace.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [APF-002A: DESKTOP INTERACTION & REPRODUCIBLE FUZZING SUITE]       ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools, apps, and kernel are freshly compiled..."
bash "${SCRIPT_DIR}/build_kernel.sh"

echo ""
echo "[STEP 1] Generating deterministic input trace with seed 20261012..."
bash "${SCRIPT_DIR}/fuzz_desktop_input.sh" 20261012 100 "${TRACE_FILE}"

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Desktop Fuzzing, Rapid Focus Transitions & Error Recovery <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for Desktop Fuzzing test..."
dd if=/dev/zero of="${DISK_FUZZ}" bs=1M count=16 status=none

rm -f "${LOG_SERIAL}" /tmp/fuzz_mon1.sock
MON_SOCK1="/tmp/fuzz_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_FUZZ}",format=raw,index=1,media=disk \
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

    # Wait for document name prompt and dispatch "fuzztest"
    for _ in {1..1000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_SERIAL}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'fuzztest'..."
            (for key in f u z z t e s t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # Replay deterministic input trace over hardware monitor
    for _ in {1..1000}; do
        if grep -q "\[DESKTOP_FUZZ_STARTED_MARKER_v1.0\]" "${LOG_SERIAL}" 2>/dev/null; then
            echo "[INPUT HW] Replaying deterministic fuzz trace..."
            bash "${SCRIPT_DIR}/reproduce_fuzz.sh" "${TRACE_FILE}" "${MON_SOCK1}" || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

echo ""
echo "[VERIFY] Checking APF-002A Desktop Fuzzing Invariants..."

# Check 1: Fuzzing started and seed logged
grep -q "\[DESKTOP_FUZZ_STARTED_MARKER_v1.0\]" "${LOG_SERIAL}"
grep -q "\[DESKTOP_FUZZ_SEED: 20261012\]" "${LOG_SERIAL}"
echo "  [PASS] 1. Desktop Fuzzing initialized with deterministic seed 20261012."

# Check 2: Rapid focus transitions
grep -q "\[DESKTOP_FUZZ_RAPID_FOCUS_VERIFIED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 2. Stochastic rapid focus changes verified; all held keys sanitized."

# Check 3: Formula error states in Ring 3 Grid
grep -q "\[GRID_FORMULA_ERROR_RECOVERY_MARKER_v1.0\]" "${LOG_SERIAL}"
grep -q "\[DESKTOP_FUZZ_FORMULA_ERROR_RECOVERY_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 3. Grid formula errors (#SYNTAX!, #DIV/0!) handled cleanly with recovery."

# Check 4: Queue burst overflow hygiene
grep -q "\[DESKTOP_FUZZ_QUEUE_BURST_HYGIENE_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 4. Input queue burst overflow bounded at 16 capacity with drop logging."

# Check 5: Focused window termination fallback
grep -q "\[DESKTOP_FUZZ_FOCUSED_CLOSE_FALLBACK_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 5. Active window closure gracefully transferred focus to sibling surface."

# Check 6: Complete marker
grep -q "\[DESKTOP_FUZZ_PASSED_MARKER_v1.0\]" "${LOG_SERIAL}"
echo "  [PASS] 6. Milestone APF-002A fully verified."

echo ""
echo "=========================================================================="
echo "=== [GATE APF-002A: PASSED 6/6 INVARIANTS VERIFIED]                   ==="
echo "=========================================================================="
