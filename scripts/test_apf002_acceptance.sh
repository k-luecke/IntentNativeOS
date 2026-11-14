#!/usr/bin/env bash
# scripts/test_apf002_acceptance.sh - Master Acceptance & Reliability Battery for Milestone APF-002
# (Generalized Native Desktop Usability & Reliability)
#
# Validates all 5 quality domains:
# - APF-002A: Desktop Interaction & Reproducible Input Fuzzing (Seed 20261012, focus changes, queue burst, window close)
# - APF-002B: Document Interoperability & Independent Format Validation (pure Rust format_validator, XLSX, DOCX, PDF 1.7, unzip, zipinfo)
# - APF-002C: Capability & Input Security (clipboard rights, foreground focus TOCTOU protection, key-state release tracking)
# - APF-002D: Native Model Inference Parity (autoregressive decoding over full 49,152 vocabulary logits without pre-filtered distractors)
# - APF-002E: Integrated Reliability Gate (Action Ledger rollback on injected crash, 3-cycle cold reboot resilience)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

echo "=========================================================================="
echo "=== [AUTONOMOUS PRODUCT FOUNDRY: MILESTONE APF-002 ACCEPTANCE SUITE]   ==="
echo "=== Generalized Native Desktop Usability & Reliability                 ==="
echo "=========================================================================="

echo ""
echo "=========================================================================="
echo ">>> [1/5] GATE APF-002C: CAPABILITY & INPUT SECURITY (FAIL-CLOSED)      <<<"
echo "=========================================================================="
bash "${SCRIPT_DIR}/test_clipboard_isolation_01.sh"

echo ""
echo "=========================================================================="
echo ">>> [2/5] GATE APF-002A: DESKTOP INTERACTION & REPRODUCIBLE FUZZING     <<<"
echo "=========================================================================="
bash "${SCRIPT_DIR}/test_desktop_fuzz_01.sh"

echo ""
echo "=========================================================================="
echo ">>> [3/5] GATE APF-002B: DOCUMENT INTEROPERABILITY & FORMAT CONFORMANCE <<<"
echo "=========================================================================="
bash "${SCRIPT_DIR}/test_format_conformance_01.sh"

echo ""
echo "=========================================================================="
echo ">>> [4/5] GATE APF-002D: NATIVE MODEL INFERENCE VOCABULARY PARITY (49K) <<<"
echo "=========================================================================="
cd "${ROOT_DIR}"
gcc -O3 -std=c11 tools/benchmark_real_inference.c -o tools/benchmark_real_inference -lm
./tools/benchmark_real_inference

echo ""
echo "=========================================================================="
echo ">>> [5/5] GATE APF-002E: INTEGRATED RELIABILITY GATE                   <<<"
echo "=========================================================================="
echo "--- Part 1: Action Ledger Rollback on Injected Crash ---"
bash "${SCRIPT_DIR}/test_workflow_rollback_01.sh"

echo ""
echo "--- Part 2: Multi-Cycle Cold Reboot Resilience (3 Cycles) ---"
bash "${SCRIPT_DIR}/test_desktop_multi_reboot_01.sh"

echo ""
echo "=========================================================================="
echo "=== [MILESTONE APF-002 ACCEPTANCE PASSED: ALL 5 GATES 100% VERIFIED]   ==="
echo "=========================================================================="
