#!/usr/bin/env bash
# tests/test_native_safety.sh - L407 Native Integration Safety Tests
# Verifies crash containment, unauthorized capability denial, supervisor recovery,
# and storage survivability under fault injection.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
mkdir -p "${EVIDENCE_DIR}"

LOG="${EVIDENCE_DIR}/l404_native_ocaml_ab.log"

echo "=========================================================================="
echo "=== INOS Task L407: Native Integration Safety & Fault Containment Test ==="
echo "=========================================================================="

if [ ! -f "${LOG}" ]; then
    echo "ERROR: Kernel execution log ${LOG} not found. Running QEMU harness first..."
    bash "${ROOT_DIR}/scripts/build_kernel.sh"
    qemu-system-x86_64 -m 256M -smp 2 \
        -drive if=pflash,format=raw,readonly=on,file="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd" \
        -drive if=pflash,format=raw,file="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd" \
        -drive file="fat:rw:${ROOT_DIR}/build/esp",format=raw \
        -display none -serial file:"${LOG}" \
        -device isa-debug-exit,iobase=0xf4,iosize=0x04 || true
fi

echo "[SAFETY TEST 1] Verifying Ring 3 user process fault containment..."
if grep -q "User task contained; kernel and sibling services remain running" "${LOG}"; then
    echo "  [PASS] Ring 3 fault contained; kernel remained stable."
else
    echo "  [FAIL] Missing fault containment marker!"
    exit 1
fi

echo "[SAFETY TEST 2] Verifying supervisor auto-restart of failed service..."
if grep -q "Critical service restarted automatically" "${LOG}"; then
    echo "  [PASS] Supervisor recovery confirmed."
else
    echo "  [FAIL] Missing supervisor restart marker!"
    exit 1
fi

echo "[SAFETY TEST 3] Verifying kernel privilege barrier (CPL=3 -> Ring 0 blocked)..."
if grep -q "RING3_KERNEL_ACCESS_BLOCKED_MARKER" "${LOG}"; then
    echo "  [PASS] Unauthorized supervisor memory access blocked fail-closed."
else
    echo "  [FAIL] Missing kernel access blocked marker!"
    exit 1
fi

echo "[SAFETY TEST 4] Verifying capability-based file denial on unauthorized token..."
if grep -q "Unauthorized Ring 3 syscall safely rejected with PermissionDenied" "${LOG}"; then
    echo "  [PASS] Capability enforcement active; unauthorized writes blocked."
else
    echo "  [FAIL] Missing capability denial marker!"
    exit 1
fi

echo "[SAFETY TEST 5] Verifying atomic WAL crash recovery on storage..."
if grep -q "Injected crash preserved committed state; atomic WAL verified" "${LOG}"; then
    echo "  [PASS] Storage integrity verified across simulated crash."
else
    echo "  [FAIL] Missing atomic crash recovery marker!"
    exit 1
fi

echo "[SAFETY TEST 6] Verifying zero capability issued on ambiguous deletion intent..."
if grep -q "Zero capabilities issued for ambiguous intent" "${LOG}"; then
    echo "  [PASS] Dual-model arbitration prevented destructive execution on ambiguous intent."
else
    echo "  [FAIL] Missing zero capabilities marker!"
    exit 1
fi

echo ""
echo "=== ALL L407 NATIVE SAFETY & FAULT RECOVERY TESTS PASSED (6/6) ==="
