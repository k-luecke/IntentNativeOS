#!/usr/bin/env bash
# tests/test_incremental_hasher.sh - Comprehensive Verification Suite for Incremental Artifact Hasher
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

HASHER_SRC="${ROOT_DIR}/tools/incremental_hasher.c"
HASHER_BIN="${ROOT_DIR}/tools/incremental_hasher"
TEST_DIR="/tmp/test_incremental_hasher_$$"
CACHE_FILE="${TEST_DIR}/working_cache.tsv"
MANIFEST_FILE="${TEST_DIR}/acceptance_evidence.sha256"

mkdir -p "${TEST_DIR}"
trap 'rm -rf "${TEST_DIR}"' EXIT

echo "=========================================================================="
echo "=== [TEST] INCREMENTAL ARTIFACT HASHER & FULL AUDIT VERIFICATION       ==="
echo "=========================================================================="

echo "[1/7] Compiling tools/incremental_hasher.c..."
gcc -O2 -Wall -Wextra "${HASHER_SRC}" -o "${HASHER_BIN}"
echo "  [PASS] Hasher compiled cleanly."

echo "[2/7] Generating synthetic test artifacts..."
echo "Genesis kernel payload" > "${TEST_DIR}/file_a.txt"
echo "Spatial desktop layout definition" > "${TEST_DIR}/file_b.txt"
dd if=/dev/urandom of="${TEST_DIR}/binary_large.dat" bs=1M count=8 status=none
dd if=/dev/urandom of="${TEST_DIR}/image_mock.bmp" bs=1M count=3 status=none
echo "  [PASS] Created 4 test files (total ~11 MB)."

echo "[3/7] Test 1: Cold Run (Initial Cache Generation)..."
"${HASHER_BIN}" \
    --cache "${CACHE_FILE}" \
    --manifest "${MANIFEST_FILE}" \
    --timing \
    "${TEST_DIR}/file_a.txt" \
    "${TEST_DIR}/file_b.txt" \
    "${TEST_DIR}/binary_large.dat" \
    "${TEST_DIR}/image_mock.bmp" 2> "${TEST_DIR}/timing_cold.txt"

# Verify hashes match system sha256sum exactly
sha256sum \
    "${TEST_DIR}/file_a.txt" \
    "${TEST_DIR}/file_b.txt" \
    "${TEST_DIR}/binary_large.dat" \
    "${TEST_DIR}/image_mock.bmp" > "${TEST_DIR}/ref_hashes.txt"

diff -u "${MANIFEST_FILE}" "${TEST_DIR}/ref_hashes.txt"
echo "  [PASS] Cold run produced bit-for-bit exact SHA-256 manifest."
cat "${TEST_DIR}/timing_cold.txt"

echo "[4/7] Test 2: Warm Run (O(1) Metadata Cache Hits)..."
"${HASHER_BIN}" \
    --cache "${CACHE_FILE}" \
    --manifest "${TEST_DIR}/manifest_warm.sha256" \
    --timing \
    "${TEST_DIR}/file_a.txt" \
    "${TEST_DIR}/file_b.txt" \
    "${TEST_DIR}/binary_large.dat" \
    "${TEST_DIR}/image_mock.bmp" 2> "${TEST_DIR}/timing_warm.txt"

grep -q "Cache Hits (O(1)):    4" "${TEST_DIR}/timing_warm.txt"
grep -q "Cache Misses:        0" "${TEST_DIR}/timing_warm.txt"
grep -q "Bytes Read & Hashed: 0" "${TEST_DIR}/timing_warm.txt"
echo "  [PASS] Warm run achieved 100% cache hits (0 bytes read from disk)."
cat "${TEST_DIR}/timing_warm.txt"

echo "[5/7] Test 3: Partial Modification / Cache Invalidation..."
sleep 0.05
echo "Modified content inside file_b" > "${TEST_DIR}/file_b.txt"
"${HASHER_BIN}" \
    --cache "${CACHE_FILE}" \
    --manifest "${TEST_DIR}/manifest_partial.sha256" \
    --timing \
    "${TEST_DIR}/file_a.txt" \
    "${TEST_DIR}/file_b.txt" \
    "${TEST_DIR}/binary_large.dat" \
    "${TEST_DIR}/image_mock.bmp" 2> "${TEST_DIR}/timing_partial.txt"

grep -q "Cache Hits (O(1)):    3" "${TEST_DIR}/timing_partial.txt"
grep -q "Cache Misses:        1" "${TEST_DIR}/timing_partial.txt"
echo "  [PASS] Selectively invalidated exactly 1 modified file; reused 3 cached files."

echo "[6/7] Test 4: Atomic Cache Updates & Interrupted Writes..."
# Check that cache file is updated atomically via temp file
TMP_LEFTOVER="${CACHE_FILE}.tmp.99999"
touch "${TMP_LEFTOVER}"
"${HASHER_BIN}" \
    --cache "${CACHE_FILE}" \
    --timing \
    "${TEST_DIR}/file_a.txt" 2>/dev/null
[ -f "${CACHE_FILE}" ]
# Stale PID-specific tmp file does not corrupt valid cache
grep -q "file_a.txt" "${CACHE_FILE}"
rm -f "${TMP_LEFTOVER}"
echo "  [PASS] Working cache verified robust against interrupted temp files."

echo "[7/7] Test 5: Timestamp-Preserving Tampering vs. Full Audit Mode..."
# Record initial state
REF_FILE="${TEST_DIR}/tamper_target.bin"
dd if=/dev/urandom of="${REF_FILE}" bs=1M count=1 status=none
"${HASHER_BIN}" --cache "${CACHE_FILE}" --manifest "${TEST_DIR}/tamper_manifest.sha256" "${REF_FILE}" 2>/dev/null

# Capture exact mtime
ORIG_MTIME=$(stat -c "%y" "${REF_FILE}")

# TAMPER: Invert byte in middle of file, but keep exact same file size
dd if=/dev/zero of="${REF_FILE}" bs=1 count=1 seek=512 conv=notrunc status=none
# RESTORE original timestamp to fool metadata cache
touch -d "${ORIG_MTIME}" "${REF_FILE}"

echo "  -> Verifying development cache sees unchanged mtime/size..."
"${HASHER_BIN}" --cache "${CACHE_FILE}" --manifest "${TEST_DIR}/tamper_dev_check.sha256" --timing "${REF_FILE}" 2> "${TEST_DIR}/tamper_timing.txt"
grep -q "Cache Hits (O(1)):    1" "${TEST_DIR}/tamper_timing.txt"
echo "     Confirmed: Routine development cache reused digest based on (mtime, size)."

echo "  -> Running full cryptographic audit (--audit) against sealed manifest..."
if "${HASHER_BIN}" --audit "${TEST_DIR}/tamper_manifest.sha256" > "${TEST_DIR}/audit_result.txt" 2>&1; then
    echo "ERROR: Audit unexpectedly passed on tampered file!"
    exit 1
else
    echo "     Audit correctly rejected tampered file with exit code 1."
fi
grep -q "\[FAIL - MISMATCH\]" "${TEST_DIR}/audit_result.txt"
echo "  [PASS] Full cryptographic audit successfully detected timestamp-preserving tampering!"

echo ""
echo "=========================================================================="
echo "=== [PASS] ALL INCREMENTAL HASHER & FULL AUDIT TESTS SUCCEEDED        ==="
echo "=========================================================================="
