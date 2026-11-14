#!/usr/bin/env bash
# scripts/test_native_maturity_f105.sh - Gate F105 Native Intelligence Acceptance Verification Harness
# Tests:
# 1. Requirement A: Genuine Ring 3 Pretrained Inference (stories15M.bin streamed from hardware ATA drive,
#    bit-for-bit SHA-256 verification, floating-point forward pass with RoPE/Attention/SwiGLU, exact logits,
#    greedy token sequence [9038, 2501, 263, 931, 29892, 727], throughput & memory reporting).
# 2. Requirement B: Interactive Graphical Workflow (Type 'archive report.txt' -> Clarify -> Select Option [2]
#    via PS/2 -> Non-destructive execution with original retained -> Full transaction receipt -> Undo via F3 ->
#    Independent cold reboot verification).
# 3. Requirement C: Real Crash-Consistent Storage (5 power-cut / crash / torn boundaries, ATA cache flush 0xE7).
# Zero Python across all tools, tests, and CI.

set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_IMG="${BUILD_DIR}/persistent_disk.img"
WEIGHTS="${ROOT_DIR}/models/pretrained/stories15M.bin"

LOG_STAGE1="${EVIDENCE_DIR}/f105_stage1_serial.log"
LOG_STAGE2="${EVIDENCE_DIR}/f105_stage2_serial.log"
LOG_STAGE3="${EVIDENCE_DIR}/f105_stage3_serial.log"

CONVERTER="${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE F105] NATIVE INTELLIGENCE ACCEPTANCE VERIFICATION SUITE       ==="
echo "=========================================================================="

# Step 0: Ensure tools, freestanding transformer unit test, and kernel are built
echo ""
echo "[STEP 0] Compiling tools and building freestanding transformer unit test..."
if [ ! -f "${CONVERTER}" ]; then
    echo "  -> Compiling native Rust ppm2bmp tool..."
    (cd "${ROOT_DIR}/tools/ppm2bmp" && cargo build --release)
fi

gcc -O2 -DHOST_TEST -o "${ROOT_DIR}/tools/test_freestanding" \
    "${ROOT_DIR}/tools/test_freestanding.c" \
    "${ROOT_DIR}/userspace/user_intent_runtime/src/freestanding_transformer.c"

echo "  -> Executing host freestanding transformer unit test on stories15M.bin..."
"${ROOT_DIR}/tools/test_freestanding" "${WEIGHTS}" > "${EVIDENCE_DIR}/f105_host_freestanding_test.log" 2>&1
grep -q "\[F105_RING3_TRANSFORMER_FORWARD_PASS_SUCCESS\]" "${EVIDENCE_DIR}/f105_host_freestanding_test.log"
echo "  [PASS] Freestanding transformer unit test matched reference tokens and logits."

echo "  -> Building kernel image and native Ring 3 user ELF..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# Restore clean OVMF NVRAM variables
git checkout "${OVMF_VARS}" 2>/dev/null || true

# Helper to convert PPM to BMP and PNG
convert_screenshot() {
    local src_ppm="$1"
    local base_name="$2"
    local dst_bmp="${EVIDENCE_DIR}/${base_name}.bmp"
    local dst_png="${EVIDENCE_DIR}/${base_name}.png"

    if [ -f "${src_ppm}" ] && [ -s "${src_ppm}" ]; then
        "${CONVERTER}" "${src_ppm}" "${dst_bmp}"
        win_bmp=$(wslpath -w "${dst_bmp}")
        powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "Add-Type -AssemblyName System.Drawing; \$bmp = [System.Drawing.Image]::FromFile('${win_bmp}'); \$out = [System.IO.Path]::ChangeExtension('${win_bmp}', '.png'); \$bmp.Save(\$out, [System.Drawing.Imaging.ImageFormat]::Png); \$bmp.Dispose();" 2>/dev/null || true
        echo "[SCREENSHOT] Generated ${dst_bmp} and ${dst_png} ($(wc -c < "${dst_bmp}") bytes)"
        rm -f "${src_ppm}"
    else
        echo "[WARN] Screenshot source ${src_ppm} not found or empty."
    fi
}

# =========================================================================
# STAGE 1: Boot Session 1 (Model Ingestion, Ring 3 Inference, WAL Durability, Initial Document)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> STAGE 1: Booting QEMU Session #1 (Hardware Ingestion, Ring 3 Inference, Durability) <<<"
echo "--------------------------------------------------------------------------"

# Initialize fresh 16 MB raw virtual disk
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk: ${DISK_IMG}..."
dd if=/dev/zero of="${DISK_IMG}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/f105_mon1.sock /tmp/f105_1_raw.ppm /tmp/f105_2_raw.ppm
MON_SOCK1="/tmp/f105_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_IMG}",format=raw,index=1,media=disk \
    -drive file="${WEIGHTS}",format=raw,if=ide,index=2,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK1}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE1}" &
QEMU_PID1=$!

# Monitor background driver for Stage 1
(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK1}" ]; then break; fi
        sleep 0.1
    done

    # 1. Wait for document name prompt
    for _ in {1..300}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'report.txt' via sendkey..."
            (for key in r e p o r t dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Wait for document content prompt
    for _ in {1..300}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching document baseline content via sendkey..."
            (for key in r e v e n u e spc 1 0 4 5 0 0 0 spc u s d ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 3. Capture baseline screenshot
    for _ in {1..300}; do
        if grep -q "\[G9_STAGE1_SAVED_RENDERED\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[SCREEN 0] Capturing Screendump #0 (Baseline Document Saved)..."
            echo "screendump /tmp/f105_0_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

convert_screenshot "/tmp/f105_0_raw.ppm" "f105_0_document_baseline"

echo "[VERIFY] Checking Stage 1 Serial Markers..."
grep -q "\[MODEL_HARDWARE_INGESTION_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Hardware Model ATA Ingestion (58 MB streamed, bit-for-bit SHA-256 match)."

grep -q "\[RING3_PRETRAINED_NUMERICAL_PARITY_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[F105_RING3_TRANSFORMER_FORWARD_PASS_SUCCESS\]" "${LOG_STAGE1}"
echo "  [PASS] Ring 3 Pretrained Inference Numerical Parity (Greedy sequence: 'Once upon a time, there')."

grep -q "\[WAL_BOUNDARY_1\]" "${LOG_STAGE1}"
grep -q "\[WAL_BOUNDARY_2\]" "${LOG_STAGE1}"
grep -q "\[WAL_BOUNDARY_3\]" "${LOG_STAGE1}"
grep -q "\[WAL_BOUNDARY_4\]" "${LOG_STAGE1}"
grep -q "\[WAL_BOUNDARY_5\]" "${LOG_STAGE1}"
grep -q "\[F105_WAL_CRASH_RECOVERY_ALL_BOUNDARIES_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Real Crash-Consistent Storage (All 5 power-cut / crash / torn boundaries passed)."

grep -q "\[G9_STAGE1_SAVED_OK\]" "${LOG_STAGE1}"
echo "  [PASS] Baseline 'report.txt' saved and committed to persistent disk."

# =========================================================================
# STAGE 2: Boot Session 2 (Interactive Workflow: Clarify -> Option [2] -> Execute -> Receipt -> Undo)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> STAGE 2: Cold Reboot #1 (Interactive Intent -> Clarify -> Option [2] -> Receipt -> Undo) <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true

rm -f "${LOG_STAGE2}" /tmp/f105_mon2.sock /tmp/f105_1_raw.ppm /tmp/f105_2_raw.ppm /tmp/f105_3_raw.ppm
MON_SOCK2="/tmp/f105_mon2.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_IMG}",format=raw,index=1,media=disk \
    -drive file="${WEIGHTS}",format=raw,if=ide,index=2,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK2}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE2}" &
QEMU_PID2=$!

# Monitor background driver for Stage 2
(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK2}" ]; then break; fi
        sleep 0.1
    done

    # 1. Wait for intent input prompt
    for _ in {1..300}; do
        if grep -q "\[F105_AWAITING_INTENT_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching intent keypresses for 'archive report.txt' via sendkey..."
            (for key in a r c h i v e spc r e p o r t dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Capture Clarification Modal Screenshot
    for _ in {1..300}; do
        if grep -q "\[F105_CLARIFICATION_MODAL_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 1] Capturing Screendump #1 (Clarification Modal Popup)..."
            echo "screendump /tmp/f105_1_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 3. Wait for option input prompt and select Option [2]
    for _ in {1..300}; do
        if grep -q "\[F105_AWAITING_OPTION_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching Option [2] selection keypress ('2', scancode 0x03) via sendkey..."
            (echo "sendkey 2"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 4. Capture Transaction Receipt Screenshot
    for _ in {1..300}; do
        if grep -q "\[F105_TRANSACTION_RECEIPT_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 2] Capturing Screendump #2 (Transaction Receipt)..."
            echo "screendump /tmp/f105_2_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 5. Wait for undo input prompt and dispatch F3 / undo trigger
    for _ in {1..300}; do
        if grep -q "\[F105_AWAITING_UNDO_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching Undo keypress ('f3', scancode 0x3D) via sendkey..."
            (echo "sendkey f3"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 6. Capture Undone State Screenshot
    for _ in {1..300}; do
        if grep -q "\[F105_STAGE2_UNDONE_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 3] Capturing Screendump #3 (Transaction Undone)..."
            echo "screendump /tmp/f105_3_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID2}" || true
rm -f "${MON_SOCK2}"

convert_screenshot "/tmp/f105_1_raw.ppm" "f105_1_clarification_modal"
convert_screenshot "/tmp/f105_2_raw.ppm" "f105_2_transaction_receipt"
convert_screenshot "/tmp/f105_3_raw.ppm" "f105_3_transaction_undone"

echo "[VERIFY] Checking Stage 2 Serial Markers..."
grep -q "\[F105_SOURCE_VERIFIED\]" "${LOG_STAGE2}"
echo "  [PASS] Target existence verified prior to execution."

grep -q "\[F105_INTENT_INPUT_RECEIVED\]" "${LOG_STAGE2}"
echo "  [PASS] Intent 'archive report.txt' typed via PS/2 port 0x60."

grep -q "\[F105_CLARIFICATION_MODAL_RENDERED\]" "${LOG_STAGE2}"
echo "  [PASS] Clarification modal rendered on 1280x800 framebuffer."

grep -q "\[F105_OPTION_SELECTED\]" "${LOG_STAGE2}"
echo "  [PASS] Option [2] (non-destructive archive with original retention) selected."

grep -q "\[F105_OPERATION_EXECUTED\]" "${LOG_STAGE2}"
echo "  [PASS] Non-destructive archive executed: report.txt retained, report.txt.bak created."

grep -q "\[F105_RECEIPT_EMITTED\]" "${LOG_STAGE2}"
grep -q "\[F105_TRANSACTION_RECEIPT_RENDERED\]" "${LOG_STAGE2}"
echo "  [PASS] Complete transaction receipt rendered on framebuffer and serial."

grep -q "\[F105_UNDO_EXECUTED\]" "${LOG_STAGE2}"
grep -q "\[F105_STAGE2_COMPLETE_MARKER\]" "${LOG_STAGE2}"
echo "  [PASS] Undo executed: rollback committed and disk synchronized."

# =========================================================================
# STAGE 3: Independent Cold Reboot Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> STAGE 3: Cold Reboot #2 (Independent Cold Reboot Verification)     <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true

rm -f "${LOG_STAGE3}" /tmp/f105_mon3.sock /tmp/f105_4_raw.ppm
MON_SOCK3="/tmp/f105_mon3.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_IMG}",format=raw,index=1,media=disk \
    -drive file="${WEIGHTS}",format=raw,if=ide,index=2,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK3}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE3}" &
QEMU_PID3=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK3}" ]; then break; fi
        sleep 0.1
    done

    for _ in {1..300}; do
        if grep -q -E "(\[G9_STAGE3_VERIFIED_RENDERED\]|\[G8_STALL\])" "${LOG_STAGE3}" 2>/dev/null; then
            echo "[SCREEN 4] Capturing Screendump #4 (Cold Reboot Verification)..."
            echo "screendump /tmp/f105_4_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK3}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID3}" || true
rm -f "${MON_SOCK3}"

convert_screenshot "/tmp/f105_4_raw.ppm" "f105_4_cold_reboot_verified"

echo "[VERIFY] Checking Stage 3 Serial Markers..."
grep -q "\[F105_COLD_REBOOT_VERIFICATION_SUCCESS\]" "${LOG_STAGE3}"
grep -q "\[F105_INTERACTIVE_WORKFLOW_ALL_STAGES_VERIFIED_MARKER\]" "${LOG_STAGE3}"
echo "  [PASS] Cold reboot verified: report.txt preserved, rollback confirmed, 0 data corruption."

# =========================================================================
# Artifact Ledger & SHA-256 Hashes
# =========================================================================
echo ""
echo "=========================================================================="
echo "=== [HASH LEDGER] Generating SHA-256 signatures for Gate F105 artifacts ==="
echo "=========================================================================="
F105_HASHES="${EVIDENCE_DIR}/f105_artifacts.sha256"
sha256sum "${WEIGHTS}" > "${F105_HASHES}"
sha256sum "${ROOT_DIR}/userspace/user_intent_runtime/user_intent_runtime" >> "${F105_HASHES}"
sha256sum "${ROOT_DIR}/build/esp/EFI/BOOT/BOOTX64.EFI" >> "${F105_HASHES}"
sha256sum "${DISK_IMG}" >> "${F105_HASHES}"
cat "${F105_HASHES}"

echo ""
echo "=========================================================================="
echo "=== [PASS] GATE F105 NATIVE INTELLIGENCE ACCEPTANCE FULLY VERIFIED    ==="
echo "=========================================================================="
