#!/usr/bin/env bash
# scripts/test_local_inf_01.sh - Acceptance suite for Milestone LOCAL-INF-01 (Quantized Inference & Intent Broker)
# Verifies:
# 1. Zero-copy memory mapped model weights residency (< 160 MiB)
# 2. Time-To-First-Token (TTFT) and throughput benchmarks for quantized inference
# 3. Deterministic grammar-constrained decoding (zero syntax/schema errors)
# 4. Dual-model arbitration (Model A Reviewer + Model B Implementer)
# 5. Adversarial injection and boundary attack defense (0% false authorizations)
# 6. In-guest Ring 3 freestanding OCaml intent execution and capability broker verification under QEMU

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${BUILD_DIR}/esp"
DISK_INF="${BUILD_DIR}/disk_local_inf_01.img"

LOG_BENCH="${EVIDENCE_DIR}/local_inf_01_bench.log"
LOG_OCAML="${EVIDENCE_DIR}/local_inf_01_ocaml.log"
LOG_RING3="${EVIDENCE_DIR}/local_inf_01_ring3_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [LANE B: LOCAL-INF-01] QUANTIZED INFERENCE & INTENT BROKER GATES  ==="
echo "=========================================================================="

# =========================================================================
# PHASE 1: Host Quantized Inference & Grammar Benchmark
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Quantized Engine, TTFT, Throughput & Grammar Decoding    <<<"
echo "--------------------------------------------------------------------------"

gcc -O3 "${ROOT_DIR}/tools/benchmark_local_inf.c" -o "${ROOT_DIR}/tools/benchmark_local_inf" -lm
"${ROOT_DIR}/tools/benchmark_local_inf" | tee "${LOG_BENCH}"

echo ""
echo "[VERIFY] Checking Phase 1 Benchmark Markers..."
grep -q "\[LOCAL_INF_01_WEIGHT_RESIDENCY_VERIFIED_MARKER_v1.0\]" "${LOG_BENCH}"
grep -q "\[LOCAL_INF_01_TTFT_BENCHMARK_VERIFIED_MARKER_v1.0\]" "${LOG_BENCH}"
grep -q "\[LOCAL_INF_01_THROUGHPUT_BENCHMARK_VERIFIED_MARKER_v1.0\]" "${LOG_BENCH}"
grep -q "\[GRAMMAR_DECODING_VERIFIED_MARKER_v1.0\]" "${LOG_BENCH}"
grep -q "\[ADVERSARIAL_INJECTION_DEFENSE_VERIFIED_MARKER_v1.0\]" "${LOG_BENCH}"
grep -q "\[DUAL_MODEL_ARBITRATION_VERIFIED_MARKER_v1.0\]" "${LOG_BENCH}"
grep -q "\[LOCAL_INF_01_BENCHMARK_COMPLETE_MARKER_v1.0\]" "${LOG_BENCH}"
echo "  [PASS] Phase 1 Verified: All inference, grammar, and security benchmarks passed!"

# =========================================================================
# PHASE 2: OCaml Dual-Model Arbitration & Adversarial Defense
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: OCaml Pretrained Adapter & Adversarial Trust Boundaries  <<<"
echo "--------------------------------------------------------------------------"

dune exec tests/test_pretrained_inference.exe | tee "${LOG_OCAML}"
dune exec tests/test_adversarial_boundaries.exe | tee -a "${LOG_OCAML}"

echo ""
echo "[VERIFY] Checking Phase 2 OCaml Markers..."
grep -q "\[ALL PASS\] GAP-G7-03b Pretrained Inference & Telemetry Verified" "${LOG_OCAML}"
grep -q "\[ALL PASS\] Adversarial Trust Boundaries Verified" "${LOG_OCAML}"
echo "  [PASS] Phase 2 Verified: Pretrained inference and adversarial defense passed!"

# =========================================================================
# PHASE 3: In-Guest Freestanding Ring 3 Execution & Capability Broker
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: In-Guest Ring 3 Freestanding OCaml Runtime under QEMU     <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for LOCAL-INF-01 test..."
dd if=/dev/zero of="${DISK_INF}" bs=1M count=16 status=none

rm -f "${LOG_RING3}" /tmp/local_inf_mon.sock
MON_SOCK="/tmp/local_inf_mon.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_INF}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK}",server=on,wait=off \
    -mon chardev=mon0,mode=readline > "${LOG_RING3}" 2>&1 &
QEMU_PID=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK}" ]; then break; fi
        sleep 0.1
    done

    for _ in {1..1000}; do
        if grep -q "\[GAP_G7_02_CLOSED_OCAML_RING3_EXECUTION_VERIFIED\]" "${LOG_RING3}" 2>/dev/null && \
           grep -q "\[L404_NATIVE_OCAML_AB_ORCHESTRATION_VERIFIED\]" "${LOG_RING3}" 2>/dev/null; then
            echo "[MONITOR] In-guest Ring 3 OCaml markers captured! Quitting QEMU..."
            echo "quit" | socat - UNIX-CONNECT:"${MON_SOCK}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID}" || true
rm -f "${MON_SOCK}"

echo ""
echo "[VERIFY] Checking Phase 3 In-Guest Ring 3 Markers..."
grep -q "\[OCAML_RING3_BINARY_VALIDATED_MARKER_v0.1\]" "${LOG_RING3}"
grep -q "\[CAPABILITY_DENIAL_VERIFIED_MARKER_v0.1\]" "${LOG_RING3}"
grep -q "\[GAP_G7_02_CLOSED_OCAML_RING3_EXECUTION_VERIFIED\]" "${LOG_RING3}"
grep -q "\[L404_NATIVE_OCAML_AB_ORCHESTRATION_VERIFIED\]" "${LOG_RING3}"
grep -q "\[HARDWARE_WX_PROTECT_RANGE_VERIFIED_MARKER\]" "${LOG_RING3}"
grep -q "\[NATIVE_RESIDENCY_VMM_VERIFIED_MARKER\]" "${LOG_RING3}"
echo "  [PASS] Phase 3 Verified: In-guest Ring 3 OCaml A/B arbitration and hardware W^X verified!"

echo ""
echo "=========================================================================="
echo "=== [LOCAL-INF-01] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)        ==="
echo "=========================================================================="
