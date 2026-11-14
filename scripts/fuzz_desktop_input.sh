#!/usr/bin/env bash
# scripts/fuzz_desktop_input.sh - Deterministic Input Fuzzer & Trace Generator for INOS Desktop
# Generates reproducible sequences of keyboard navigation, modifiers, formula inputs, and focus transitions.

set -euo pipefail

SEED="${1:-20261012}"
ITERATIONS="${2:-100}"
TRACE_FILE="${3:-build/fuzz_input_trace.log}"

mkdir -p "$(dirname "${TRACE_FILE}")"

echo "=== [INOS FUZZER] Generating deterministic fuzz sequence (Seed: ${SEED}, Iterations: ${ITERATIONS}) ==="

# Deterministic pseudo-random number generator (xorshift64)
state="${SEED}"
next_rand() {
    state=$(( (state ^ (state << 13)) & 0xFFFFFFFFFFFFFFFF ))
    state=$(( (state ^ (state >> 7))  & 0xFFFFFFFFFFFFFFFF ))
    state=$(( (state ^ (state << 17)) & 0xFFFFFFFFFFFFFFFF ))
    echo "${state}"
}

KEY_POOL=(
    "up" "down" "left" "right" "home" "end" "pgup" "pgdn"
    "backspace" "delete" "ret" "tab" "spc"
    "a" "b" "c" "d" "e" "1" "2" "3" "4" "5"
    "equal" "plus" "minus" "slash" "asterisk"
    "shift" "ctrl" "alt"
)

rm -f "${TRACE_FILE}"
echo "# FUZZ_TRACE_SEED: ${SEED}" > "${TRACE_FILE}"
echo "# TOTAL_STEPS: ${ITERATIONS}" >> "${TRACE_FILE}"

for ((i=1; i<=ITERATIONS; i++)); do
    r=$(next_rand)
    idx=$(( (r & 0x7FFFFFFF) % ${#KEY_POOL[@]} ))
    key="${KEY_POOL[idx]}"
    echo "sendkey ${key}" >> "${TRACE_FILE}"
done

echo "[INOS FUZZER] Trace successfully written to ${TRACE_FILE} (${ITERATIONS} commands)."
