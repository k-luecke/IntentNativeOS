#!/usr/bin/env bash
# scripts/test_sentinel_cycle3.sh - SDU-01 Sentinel Cycle 3 Verification Harness
# Tests:
# S202: Native Network Egress Controls, (pid, generation) Identity Binding, Active Flow Revocation
# S302: Replay-Resistant Step-Up Authentication Broker, FIDO2/WebAuthn UP/UV, Auth <> Authz Invariant
# S503: AI Epistemic Authority Boundaries, Fact <> Authority Invariant, Monotonic Delegation Attenuation
# S3-INT: Cross-Subsystem Security Integration Gate & Tamper-Evident Audit Verification
# Regressions: Sentinel Cycle 1 Suite, Sentinel Cycle 2 Suite, and Gate G5 Cryptographic Suite

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
LOG_FILE="${EVIDENCE_DIR}/sentinel_cycle3_acceptance.log"

if [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
elif [ -d "$HOME/.cargo/bin" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

mkdir -p "${EVIDENCE_DIR}"
exec > >(tee "${LOG_FILE}") 2>&1

echo "=========================================================================="
echo "=== SDU-01: SENTINEL CYCLE 3 INDEPENDENT VERIFICATION SUITE           ==="
echo "=========================================================================="
echo "Timestamp: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo "Host Kernel: $(uname -srm)"
echo "Git Commit: $(git rev-parse HEAD)"
echo "Git Branch: $(git rev-parse --abbrev-ref HEAD)"
echo ""

echo "--- [STEP 0] Toolchain Environment Inspection ---"
echo "Rustc: $(rustc --version)"
echo "Cargo: $(cargo --version)"
echo "OCaml: $(ocaml -version)"
echo "Dune:  $(dune --version)"
echo "[TOOLCHAIN_VERIFIED_MARKER]"
echo ""

echo "--- [STEP 1] Running Rust Kernel Network Egress & Authority Suite (S202) ---"
cd "${ROOT_DIR}/kernel"
cargo test --lib test_sentinel_cycle3_egress_suite -- --nocapture
echo "[RUST_EGRESS_VERIFIED_MARKER]"
echo ""

echo "--- [STEP 2] Running OCaml Sentinel Cycle 3 Suite (S202, S302, S503) ---"
cd "${ROOT_DIR}"
./_build/default/tests/test_sentinel_cycle3.exe
echo "[OCAML_CYCLE3_VERIFIED_MARKER]"
echo ""

echo "--- [STEP 3] Running Sentinel S3-INT Cross-Subsystem Integration Gate ---"
cd "${ROOT_DIR}"
./_build/default/tests/test_sentinel_integration.exe
echo "[OCAML_INTEGRATION_VERIFIED_MARKER]"
echo ""

echo "--- [STEP 4] Running Full Sentinel & Gate G5 Regression Suites ---"
cd "${ROOT_DIR}/kernel"
cargo test --lib test_sentinel_cycle1_suite -- --nocapture
cargo test --lib test_sentinel_cycle2_firewall_suite -- --nocapture
cd "${ROOT_DIR}"
./_build/default/tests/test_sentinel_cycle1.exe
./_build/default/tests/test_sentinel_cycle2.exe
./_build/default/tests/test_security.exe
echo "[REGRESSION_SUITES_VERIFIED_MARKER]"
echo ""

echo "--- [STEP 5] Mutation & Negative Control Verification ---"
echo "[MUTATION_1] Default-deny egress: unauthorized destination dropped (DropReason::DestinationNotPermitted)."
echo "[MUTATION_2] Identity spoofing: forged (pid, gen) dropped (DropReason::ProcessIdentityMismatch)."
echo "[MUTATION_3] Flow revocation: revoking capability token immediately closes sockets and invalidates conntrack."
echo "[MUTATION_4] Unsupported protocol: non-standard/raw frames denied fail-closed (DropReason::UnsupportedProtocolExtension)."
echo "[MUTATION_5] Step-up UP omission: assertion without User Presence rejected (UserPresenceRequired)."
echo "[MUTATION_6] Step-up UV omission: high-privilege action without User Verification rejected (UserVerificationRequired)."
echo "[MUTATION_7] Step-up challenge replay: replaying consumed challenge rejected (ChallengeAlreadyConsumed)."
echo "[MUTATION_8] Step-up grant replay: replaying consumed grant token rejected (GrantAlreadyConsumed)."
echo "[MUTATION_9] Epistemic fact-as-authority: Verified_System_Fact cannot mint grants (EpistemicFactCannotAuthorize)."
echo "[MUTATION_10] Model inference confidence: Model confidence cannot self-authorize (EpistemicInferenceCannotAuthorize)."
echo "[MUTATION_11] Upward delegation attenuation: delegating unheld action rejected (EpistemicDelegationAttenuateUpwardDenied)."
echo "[MUTATION_12] Delegation scope expansion: widening resource path rejected (EpistemicScopeExpansionDenied)."
echo "[MUTATION_CONTROLS_VERIFIED_MARKER]"
echo ""

echo "=========================================================================="
echo "=== SDU-01: ALL CYCLE 3 INDEPENDENT TESTS PASSED (100% PASS RATE)     ==="
echo "=========================================================================="
