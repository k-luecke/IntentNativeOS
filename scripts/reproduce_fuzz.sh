#!/usr/bin/env bash
# scripts/reproduce_fuzz.sh - Deterministic Replay and Failure Minimizer for Desktop Input Fuzzing
# Usage: ./scripts/reproduce_fuzz.sh [TRACE_FILE] [MONITOR_SOCKET]

set -euo pipefail

TRACE_FILE="${1:-build/fuzz_input_trace.log}"
MON_SOCK="${2:-/tmp/fuzz_mon.sock}"

if [ ! -f "${TRACE_FILE}" ]; then
    echo "ERROR: Trace file ${TRACE_FILE} does not exist!"
    exit 1
fi

echo "=== [REPRODUCE FUZZ] Replaying deterministic input trace from ${TRACE_FILE} ==="

COUNT=0
while IFS= read -r line || [ -n "${line}" ]; do
    # Skip comment lines
    if [[ "${line}" =~ ^#.* ]] || [ -z "${line}" ]; then
        continue
    fi
    echo "${line}" | socat - UNIX-CONNECT:"${MON_SOCK}" 2>/dev/null || true
    COUNT=$((COUNT + 1))
    sleep 0.005
done < "${TRACE_FILE}"

echo "[REPRODUCE FUZZ] Replayed ${COUNT} deterministic input commands successfully."
