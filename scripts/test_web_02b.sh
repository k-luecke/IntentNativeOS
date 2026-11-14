#!/usr/bin/env bash
# scripts/test_web_02b.sh - Acceptance suite for Milestone WEB-02B
# Verifies:
#  1. Hardware CPL=3 isolated execution of native Ring 3 Document Renderer & Parser (PID 310).
#  2. Streaming UTF-8 decoding with incomplete sequence buffering and invalid sequence replacement.
#  3. Streaming HTML tokenization with state preservation across chunk boundaries.
#  4. Representative HTML document parsing into verified DOM tree.
#  5. Chunk-boundary invariance (1-byte, 3-byte, 7-byte chunks produce identical DOMs).
#  6. Deterministic fail-closed resource limits:
#     - Maximum element nesting depth (TreeDepth <= 64)
#     - Maximum attribute value length (AttrValueLength <= 1024)
#     - Maximum payload size (InputBytes <= 512 KiB)
#  7. WHATWG error recovery: broken entities preserved, unclosed tags safely handled.
#  8. Inert script & style elements with passive string attribute handling.
#  9. Zero ambient authority maintained post-parsing.
# 10. Post-termination lifecycle & fault containment.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_APP="${BUILD_DIR}/disk_web02b.img"
LOG_STAGE2="${EVIDENCE_DIR}/web02b_stage2_serial.log"

if [ ! -f "${OVMF_CODE}" ]; then
    echo "[ERROR] OVMF_CODE.fd not found at ${OVMF_CODE}" >&2
    exit 1
fi

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE WEB-02B] STREAMING HTML TOKENIZER & BOUNDED DOM ACCEPTANCE   ==="
echo "=========================================================================="

echo ""
echo "[STEP 1] Running freestanding unit and stress tests (Layer 1)..."
cargo test --manifest-path "${ROOT_DIR}/userspace/apps/web_render/Cargo.toml"

echo ""
echo "[STEP 2] Compiling web_render binary and kernel for UEFI (Layer 2)..."
if [ "${SKIP_BUILD:-0}" != "1" ]; then
    bash "${SCRIPT_DIR}/build_web_render.sh"
    bash "${SCRIPT_DIR}/build_kernel.sh"
else
    echo "[STEP 2] Skipping build (SKIP_BUILD=1)."
fi

cleanup() {
    rm -f /tmp/web02b_mon.sock
}
trap cleanup EXIT INT TERM

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Ring 3 Streaming HTML Tokenization & Bounded DOM Parsing   <<<"
echo "--------------------------------------------------------------------------"

dd if=/dev/zero of="${DISK_APP}" bs=1M count=16 status=none

rm -f "${LOG_STAGE2}" /tmp/web02b_mon.sock
MON_SOCK="/tmp/web02b_mon.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -cpu max \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_APP}",format=raw,index=1,media=disk,file.locking=off \
    -netdev user,id=net0 \
    -device virtio-net-pci,disable-legacy=off,disable-modern=on,romfile="",netdev=net0,mac=52:54:00:12:34:56 \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE2}" &
QEMU_PID=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK}" ]; then break; fi
        sleep 0.1
    done

    # Wait for document name prompt and dispatch "web02b"
    for _ in {1..2000}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'web02b'..."
            (for key in w e b 0 2 b ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK}" 2>/dev/null || true
            break
        fi
        sleep 0.01
    done
) &

wait "${QEMU_PID}" || true
rm -f "${MON_SOCK}"

echo ""
echo "[VERIFY] Checking WEB-02B Serial Invariants..."
grep -q "\[PAGE_TABLES\] PML4_RENDER initialized" "${LOG_STAGE2}"
grep -q "\[PAGE_TABLES\] Physical memory disjointness verification: PASSED\." "${LOG_STAGE2}"
grep -q "\[PAGE_TABLES\] W\^X permission verification: PASSED\." "${LOG_STAGE2}"
grep -q "\[PAGE_TABLES\] Stack guard page verification: PASSED\." "${LOG_STAGE2}"
grep -q "\[WEB_02B_STAGE2_STARTED_MARKER_v1.0\]" "${LOG_STAGE2}"

# Userspace Parser Acceptance Invariants
grep -q "\[WEB_RENDER\] Hardware CPL=3 isolation confirmed\." "${LOG_STAGE2}"
grep -q "\[WEB_RENDER\] Registered Process ID 310 confirmed" "${LOG_STAGE2}"
grep -q "\[WEB_02B_REPRESENTATIVE_DOM_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE2}"
grep -q "\[WEB_02B_CHUNK_INVARIANCE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE2}"
grep -q "\[WEB_02B_MAX_DEPTH_REJECTED_MARKER_v1.0\]" "${LOG_STAGE2}"
grep -q "\[WEB_02B_OVERSIZED_ATTR_REJECTED_MARKER_v1.0\]" "${LOG_STAGE2}"
grep -q "\[WEB_02B_MALFORMED_RECOVERY_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE2}"
grep -q "\[WEB_02B_INERT_SCRIPT_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE2}"
grep -q "\[WEB_02B_ZERO_AMBIENT_MAINTAINED_MARKER_v1.0\]" "${LOG_STAGE2}"
grep -q "\[WEB_02B_ALL_ACCEPTANCE_GATES_PASSED_MARKER_v1.0\]" "${LOG_STAGE2}"

# Post-termination & fault containment invariants
grep -q "\[WEB_02B_POST_TERMINATION_LIFECYCLE_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE2}"
grep -q "\[WEB_02B_FAULT_CONTAINMENT_VERIFIED_MARKER_v1.0\]" "${LOG_STAGE2}"
grep -q "\[WEB_02B_STAGE2_COMPLETE_MARKER_v1.0\]" "${LOG_STAGE2}"

echo ""
echo "=========================================================================="
echo "=== [WEB-02B] ALL ACCEPTANCE GATES PASSED (100% VERIFIED)               ==="
echo "=========================================================================="
