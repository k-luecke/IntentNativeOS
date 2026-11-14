#!/usr/bin/env bash
# scripts/run_qemu_test.sh - Automated QEMU boot test for Task K002
# Verifies that the genuinely built x86-64 kernel image boots under QEMU and emits
# the known serial marker.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== [K002] Automated QEMU Kernel Boot Test ==="

bash "${SCRIPT_DIR}/build_kernel.sh"

LOG_FILE="${ROOT_DIR}/evidence/qemu_boot_serial.log"
mkdir -p "${ROOT_DIR}/evidence"

echo "[TEST] Running QEMU with timeout 20s..."
set +e
timeout 20s bash "${SCRIPT_DIR}/launch_qemu.sh" > "${LOG_FILE}" 2>&1
QEMU_EXIT_CODE=$?
set -e

echo "[TEST] QEMU finished with exit code ${QEMU_EXIT_CODE}."
cat "${LOG_FILE}"

# QEMU isa-debug-exit with value 0x10 returns exit code (0x10 << 1) | 1 = 33
if [ "${QEMU_EXIT_CODE}" -ne 33 ] && [ "${QEMU_EXIT_CODE}" -ne 0 ]; then
    echo "ERROR: QEMU did not exit cleanly via isa-debug-exit (expected 33, got ${QEMU_EXIT_CODE})"
    exit 1
fi

EXPECTED_MARKER="[INOS_BOOT_OK_MARKER_v0.1]"
if grep -q "${EXPECTED_MARKER}" "${LOG_FILE}"; then
    echo ""
    echo "========================================================="
    echo "[PASS] K002 VERIFIED: QEMU successfully booted kernel!"
    echo "Confirmed serial marker: ${EXPECTED_MARKER}"
    echo "Serial log saved to: evidence/qemu_boot_serial.log"
    echo "========================================================="
    exit 0
else
    echo "ERROR: Expected marker '${EXPECTED_MARKER}' not found in serial log!"
    exit 1
fi
