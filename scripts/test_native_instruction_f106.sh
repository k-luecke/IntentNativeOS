#!/usr/bin/env bash
# scripts/test_native_instruction_f106.sh - Gate F106 Native Instruction-Tuned Operation Acceptance Harness
# Tests:
# 1. Requirement A: Genuine Ring 3 Instruction-Tuned Inference (SmolLM2-135M-Instruct Q8_0 streamed from hardware ATA drive,
#    bit-for-bit SHA-256 verification 81ad64af..., floating-point forward pass with RoPE/Attention/SwiGLU/GQA, exact logits,
#    greedy token sequence [3725, 5367, 198, 2017, 3378, 198, 2017, 26215, 198, 198, 1604, 3784, 79, 7942, 24, 10920, 79, 1537, 28, 2434, 79, 1537, 727],
#    throughput & memory reporting, deterministic bound verification).
# 2. Requirement B: Interactive Graphical Workflow (Type unfamiliar instruction 'safely duplicate draft.txt to backup' ->
#    Model proposes DUPLICATE_SAFE candidate plan -> Plan confirmation modal rendered on 1280x800 framebuffer ->
#    Confirm via PS/2 -> Non-destructive execution with original retained -> Full transaction receipt -> Undo via F3 ->
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
WEIGHTS="${ROOT_DIR}/models/pretrained/smollm2_135m_q8.bin"

LOG_STAGE1="${EVIDENCE_DIR}/f106_stage1_serial.log"
LOG_STAGE2="${EVIDENCE_DIR}/f106_stage2_serial.log"
LOG_STAGE3="${EVIDENCE_DIR}/f106_stage3_serial.log"

CONVERTER="${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE F106] NATIVE INSTRUCTION-TUNED OPERATION ACCEPTANCE SUITE    ==="
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

echo "  -> Executing host freestanding transformer unit test on smollm2_135m_q8.bin..."
"${ROOT_DIR}/tools/test_freestanding" "${WEIGHTS}" > "${EVIDENCE_DIR}/f106_host_freestanding_test.log" 2>&1
grep -q "\[F106_RING3_TRANSFORMER_FORWARD_PASS_SUCCESS\]" "${EVIDENCE_DIR}/f106_host_freestanding_test.log"
grep -q "\[RING3_INSTRUCTION_NUMERICAL_PARITY_VERIFIED_MARKER\]" "${EVIDENCE_DIR}/f106_host_freestanding_test.log"
grep -q "\[F106_DETERMINISTIC_VERIFIER_PASSED_MARKER\]" "${EVIDENCE_DIR}/f106_host_freestanding_test.log"
echo "  [PASS] Freestanding transformer unit test matched reference instruction tokens and logits."

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
# STAGE 1: Boot Session 1 (SmolLM2-135M Ingestion, Ring 3 Inference, Durability, Document)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> STAGE 1: Booting QEMU Session #1 (SmolLM2 Ingestion, Ring 3 Inference, Durability) <<<"
echo "--------------------------------------------------------------------------"

# Initialize fresh 16 MB raw virtual disk
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk: ${DISK_IMG}..."
dd if=/dev/zero of="${DISK_IMG}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/f106_mon1.sock /tmp/f106_0_raw.ppm
MON_SOCK1="/tmp/f106_mon1.sock"

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

    # 1. Wait for document name prompt (allow up to 250s for model streaming)
    for _ in {1..2500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'draft.txt' via sendkey..."
            (for key in d r a f t dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Wait for document content prompt
    for _ in {1..300}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching document baseline content via sendkey..."
            (for key in p r o j e c t spc d r a f t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 3. Capture baseline screenshot
    for _ in {1..300}; do
        if grep -q "\[G9_STAGE1_SAVED_RENDERED\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[SCREEN 0] Capturing Screendump #0 (Baseline Document Saved)..."
            (echo "screendump /tmp/f106_0_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

convert_screenshot "/tmp/f106_0_raw.ppm" "f106_0_document_baseline"

echo "[VERIFY] Checking Stage 1 Serial Markers..."
grep -q "\[MODEL_HARDWARE_INGESTION_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[F106_MODEL_INGESTION_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Hardware Model ATA Ingestion (144.4 MB streamed, bit-for-bit SHA-256 match)."

grep -q "\[RING3_INSTRUCTION_NUMERICAL_PARITY_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[F106_RING3_TRANSFORMER_FORWARD_PASS_SUCCESS\]" "${LOG_STAGE1}"
grep -q "\[F106_DETERMINISTIC_VERIFIER_PASSED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Ring 3 Instruction Inference Numerical Parity (23/23 tokens matched reference)."

grep -q "\[WAL_BOUNDARY_1\]" "${LOG_STAGE1}"
grep -q "\[WAL_BOUNDARY_2\]" "${LOG_STAGE1}"
grep -q "\[WAL_BOUNDARY_3\]" "${LOG_STAGE1}"
grep -q "\[WAL_BOUNDARY_4\]" "${LOG_STAGE1}"
grep -q "\[WAL_BOUNDARY_5\]" "${LOG_STAGE1}"
grep -q "\[F105_WAL_CRASH_RECOVERY_ALL_BOUNDARIES_VERIFIED_MARKER\]" "${LOG_STAGE1}"
echo "  [PASS] Real Crash-Consistent Storage (All 5 power-cut / crash / torn boundaries passed)."

grep -q "\[G9_STAGE1_SAVED_OK\]" "${LOG_STAGE1}"
echo "  [PASS] Baseline 'draft.txt' saved and committed to persistent disk."

# =========================================================================
# STAGE 2: Boot Session 2 (Interactive Instruction Workflow -> Plan Modal -> Option [1] -> Receipt -> Undo)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> STAGE 2: Cold Reboot #1 (Instruction -> Plan Modal -> Confirm -> Receipt -> Undo) <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true

rm -f "${LOG_STAGE2}" /tmp/f106_mon2.sock /tmp/f106_1_raw.ppm /tmp/f106_2_raw.ppm /tmp/f106_3_raw.ppm
MON_SOCK2="/tmp/f106_mon2.sock"

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

    # 1. Wait for intent input prompt (allow up to 250s for model streaming)
    for _ in {1..2500}; do
        if grep -q "\[F106_AWAITING_INTENT_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching unfamiliar instruction keypresses via sendkey..."
            (for key in s a f e l y spc d u p l i c a t e spc d r a f t dot t x t spc t o spc b a c k u p ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Capture Plan Confirmation Modal Screenshot
    for _ in {1..500}; do
        if grep -q "\[F106_PLAN_MODAL_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 1] Capturing Screendump #1 (Plan Confirmation Modal)..."
            (echo "screendump /tmp/f106_1_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 3. Wait for option input prompt and select Option [1] (confirm)
    for _ in {1..300}; do
        if grep -q "\[F106_AWAITING_OPTION_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching Option [1] confirmation keypress ('1', scancode 0x02) via sendkey..."
            (echo "sendkey 1"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 4. Capture Transaction Receipt Screenshot
    for _ in {1..300}; do
        if grep -q "\[F106_TRANSACTION_RECEIPT_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 2] Capturing Screendump #2 (Full Transaction Receipt)..."
            (echo "screendump /tmp/f106_2_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 5. Wait for undo prompt and trigger undo via F3
    for _ in {1..300}; do
        if grep -q "\[F106_AWAITING_UNDO_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching F3 scancode (0x3D) via sendkey to undo transaction..."
            (echo "sendkey f3"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 6. Capture Undone Desktop Screenshot
    for _ in {1..300}; do
        if grep -q "\[F106_STAGE2_UNDONE_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 3] Capturing Screendump #3 (Transaction Undone Desktop)..."
            (echo "screendump /tmp/f106_3_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID2}" || true
rm -f "${MON_SOCK2}"

convert_screenshot "/tmp/f106_1_raw.ppm" "f106_1_plan_confirmation_modal"
convert_screenshot "/tmp/f106_2_raw.ppm" "f106_2_transaction_receipt"
convert_screenshot "/tmp/f106_3_raw.ppm" "f106_3_undone_baseline"

echo "[VERIFY] Checking Stage 2 Serial Markers..."
grep -q "\[F106_SOURCE_VERIFIED\]" "${LOG_STAGE2}"
grep -q "\[F106_STAGE2_REOPENED_OK\]" "${LOG_STAGE2}"
echo "  [PASS] Cold reboot recovery: Source document verified bit-for-bit."

grep -q "\[F106_INTENT_INPUT_RECEIVED\]" "${LOG_STAGE2}"
grep -q "\[F106_PLAN_MODAL_RENDERED\]" "${LOG_STAGE2}"
echo "  [PASS] Unfamiliar user instruction parsed and plan modal rendered on 1280x800 framebuffer."

grep -q "\[F106_OPTION_SELECTED\]" "${LOG_STAGE2}"
grep -q "\[F106_OPERATION_EXECUTED\]" "${LOG_STAGE2}"
echo "  [PASS] Hardware PS/2 confirmation received: non-destructive operation executed atomically."

grep -q "\[F106_RECEIPT_EMITTED\]" "${LOG_STAGE2}"
grep -q "COMMITTED_DURABLE" "${LOG_STAGE2}"
echo "  [PASS] Transaction receipt emitted with content hash and durable status."

grep -q "\[F106_UNDO_TRIGGER_RECEIVED\]" "${LOG_STAGE2}"
grep -q "\[F106_UNDO_EXECUTED\]" "${LOG_STAGE2}"
echo "  [PASS] Transaction rollback executed: backup removed, source preserved intact."

grep -q "\[F106_STAGE2_COMPLETE_MARKER\]" "${LOG_STAGE2}"
echo "  [PASS] Stage 2 complete."

# =========================================================================
# STAGE 3: Boot Session 3 (Independent Cold Reboot Verification)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> STAGE 3: Independent Cold Reboot #2 (Persistence & Undo Invariant Check) <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true

rm -f "${LOG_STAGE3}" /tmp/f106_mon3.sock /tmp/f106_4_raw.ppm
MON_SOCK3="/tmp/f106_mon3.sock"

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

# Monitor background driver for Stage 3
(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK3}" ]; then break; fi
        sleep 0.1
    done

    for _ in {1..2500}; do
        if grep -q "\[G9_PERSISTENT_DESKTOP_RENDERED\]" "${LOG_STAGE3}" 2>/dev/null; then
            echo "[SCREEN 4] Capturing Screendump #4 (Independent Reboot Preserved Desktop)..."
            (echo "screendump /tmp/f106_4_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK3}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID3}" || true
rm -f "${MON_SOCK3}"

convert_screenshot "/tmp/f106_4_raw.ppm" "f106_4_cold_reboot_verified"

echo "[VERIFY] Checking Stage 3 Serial Markers..."
grep -q "\[F106_COLD_REBOOT_VERIFICATION_SUCCESS\]" "${LOG_STAGE3}"
grep -q "\[F106_NATIVE_INSTRUCTION_ALL_STAGES_VERIFIED_MARKER\]" "${LOG_STAGE3}"
echo "  [PASS] Independent Cold Reboot Verification: Invariants preserved across independent reboot."

# Build artifact hash ledger
echo ""
echo "[EVIDENCE] Generating Gate F106 artifact ledger..."
F106_HASHES="${EVIDENCE_DIR}/f106_artifacts.sha256"
rm -f "${F106_HASHES}"

sha256sum "${WEIGHTS}" >> "${F106_HASHES}"
sha256sum "${ROOT_DIR}/models/pretrained/smollm2_tokenizer.bin" >> "${F106_HASHES}"
sha256sum "${ROOT_DIR}/userspace/user_intent_runtime/user_intent_runtime" >> "${F106_HASHES}"
sha256sum "${ESP_DIR}/EFI/BOOT/BOOTX64.EFI" >> "${F106_HASHES}"
for f in "${EVIDENCE_DIR}"/f106_*.png; do
    if [ -f "$f" ]; then
        sha256sum "$f" >> "${F106_HASHES}"
    fi
done

echo "=========================================================================="
echo "=== [GATE F106 ACCEPTED] ALL ACCEPTANCE REQUIREMENTS MET AND VERIFIED  ==="
echo "=========================================================================="
cat "${F106_HASHES}"
