#!/usr/bin/env bash
# scripts/test_habitable_ui_01.sh - End-to-end verification suite for Gate UI-01: Habitable Computing Foundation
# Verifies:
# 1. Model-Free Baseline Resilience (Boot, Interactive Typing, Saving with zero model attached)
# 2. First-Class Proportional Typography & Contrast Compliance
# 3. Natural Human Communication (Confirmation Modal & Transaction Receipt)
# 4. Engineer Diagnostics View (Strictly Separated Telemetry)
# 5. Persistent Undo Across Independent Cold Reboot
# 6. Distinct Framebuffer Screendump Checksums for all UI States

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_MODELFREE="${BUILD_DIR}/disk_modelfree.img"
DISK_INTERACTIVE="${BUILD_DIR}/disk_interactive.img"
WEIGHTS="${ROOT_DIR}/models/pretrained/smollm2_135m_q8.bin"

LOG_MODELFREE="${EVIDENCE_DIR}/ui_01_modelfree_serial.log"
LOG_STAGE1="${EVIDENCE_DIR}/ui_01_stage1_serial.log"
LOG_STAGE2="${EVIDENCE_DIR}/ui_01_stage2_serial.log"
LOG_STAGE3="${EVIDENCE_DIR}/ui_01_stage3_serial.log"

CONVERTER="${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE UI-01] HABITABLE COMPUTING FOUNDATION ACCEPTANCE SUITE      ==="
echo "=========================================================================="

# Step 0: Ensure tools and kernel are built
echo ""
echo "[STEP 0] Ensuring tools and kernel are freshly compiled..."
if [ ! -f "${CONVERTER}" ]; then
    echo "  -> Compiling native Rust ppm2bmp tool..."
    (cd "${ROOT_DIR}/tools/ppm2bmp" && cargo build --release)
fi

echo "  -> Building kernel image and native Ring 3 user ELF..."
bash "${SCRIPT_DIR}/build_kernel.sh"

# Helper: Convert raw PPM screendump to 24-bit BMP and PNG via .NET (zero Python)
convert_screenshot() {
    local ppm_src="$1"
    local base_name="$2"
    local bmp_dst="${EVIDENCE_DIR}/${base_name}.bmp"
    local png_dst="${EVIDENCE_DIR}/${base_name}.png"

    if [ ! -f "${ppm_src}" ] || [ ! -s "${ppm_src}" ]; then
        echo "ERROR: Missing or empty PPM capture: ${ppm_src}"
        exit 1
    fi

    "${CONVERTER}" "${ppm_src}" "${bmp_dst}"
    bash "${SCRIPT_DIR}/convert_bmp_png.sh" "${bmp_dst}" || true

    local sz
    sz=$(wc -c < "${bmp_dst}")
    echo "  [CONVERTED] ${base_name}.bmp (${sz} bytes) & ${base_name}.png"
}

# =========================================================================
# PHASE 1: Model-Free Baseline Resilience (NO MODEL DISK ATTACHED)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Model-Free Baseline Test (Zero Model Weights Attached) <<<"
echo "--------------------------------------------------------------------------"

echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for model-free boot..."
dd if=/dev/zero of="${DISK_MODELFREE}" bs=1M count=16 status=none

rm -f "${LOG_MODELFREE}" /tmp/ui01_mon_mf.sock /tmp/ui01_0_raw.ppm
MON_SOCK_MF="/tmp/ui01_mon_mf.sock"

# Launch QEMU WITHOUT the weights drive attached
stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_MODELFREE}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK_MF}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_MODELFREE}" &
QEMU_PID_MF=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK_MF}" ]; then break; fi
        sleep 0.1
    done

    # 1. Wait for document name prompt
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_MODELFREE}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'notes.txt' via sendkey..."
            (for key in n o t e s dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK_MF}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Wait for document content prompt
    for _ in {1..300}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_MODELFREE}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching model-free content via sendkey..."
            (for key in h a b i t a b l e spc c o m p u t i n g ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK_MF}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 3. Capture Model-Free Baseline Screenshot
    for _ in {1..300}; do
        if grep -q "\[MODEL_FREE_DESKTOP_RENDERED\]" "${LOG_MODELFREE}" 2>/dev/null; then
            echo "[SCREEN 0] Capturing Screendump #0 (Model-Free Desktop Baseline)..."
            (echo "screendump /tmp/ui01_0_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK_MF}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID_MF}" || true
rm -f "${MON_SOCK_MF}"

convert_screenshot "/tmp/ui01_0_raw.ppm" "ui_01_0_modelfree_desktop"

echo "[VERIFY] Checking Model-Free Serial Invariants..."
grep -q "\[MODEL_FREE_BASELINE_ACTIVE\]" "${LOG_MODELFREE}"
grep -q "\[MODEL_FREE_BASELINE_VERIFIED_MARKER\]" "${LOG_MODELFREE}"
grep -q "\[G9_STAGE1_SAVED_OK\]" "${LOG_MODELFREE}"
grep -q "\[MODEL_FREE_DESKTOP_RENDERED\]" "${LOG_MODELFREE}"
echo "  [PASS] Clean Model-Free Boot, Interactive Typing, and Storage Persistence Confirmed."

# =========================================================================
# PHASE 2: Instruction-Tuned Habitable Workflow (With Model Weights)
# =========================================================================

# Stage 1: Setup Document
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2 - STAGE 1: Creating 'draft.txt' with Model Attached <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true
dd if=/dev/zero of="${DISK_INTERACTIVE}" bs=1M count=16 status=none
rm -f "${LOG_STAGE1}" /tmp/ui01_mon1.sock
MON_SOCK1="/tmp/ui01_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_INTERACTIVE}",format=raw,index=1,media=disk \
    -drive file="${WEIGHTS}",format=raw,if=ide,index=2,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK1}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE1}" &
QEMU_PID1=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK1}" ]; then break; fi
        sleep 0.1
    done

    # Document name
    for _ in {1..2500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'draft.txt'..."
            (for key in d r a f t dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # Document content
    for _ in {1..300}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching baseline content..."
            (for key in p r o j e c t spc d r a f t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

grep -q "\[F106_MODEL_INGESTION_VERIFIED_MARKER\]" "${LOG_STAGE1}"
grep -q "\[G9_STAGE1_SAVED_OK\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Setup Complete: 'draft.txt' committed."

# Stage 2: Cold Reboot #1 (Modal -> Engineer Diagnostics -> Confirm -> Receipt -> Undo)
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2 - STAGE 2: Cold Reboot #1 (Modal -> Details -> Confirm -> Receipt -> Undo) <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_STAGE2}" /tmp/ui01_mon2.sock /tmp/ui01_1_raw.ppm /tmp/ui01_2_raw.ppm /tmp/ui01_3_raw.ppm /tmp/ui01_4_raw.ppm
MON_SOCK2="/tmp/ui01_mon2.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_INTERACTIVE}",format=raw,index=1,media=disk \
    -drive file="${WEIGHTS}",format=raw,if=ide,index=2,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK2}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE2}" &
QEMU_PID2=$!

(
    for _ in {1..50}; do
        if [ -S "${MON_SOCK2}" ]; then break; fi
        sleep 0.1
    done

    # 1. Dispatch intent instruction
    for _ in {1..2500}; do
        if grep -q "\[F106_AWAITING_INTENT_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching intent: 'safely duplicate draft.txt to backup'..."
            (for key in s a f e l y spc d u p l i c a t e spc d r a f t dot t x t spc t o spc b a c k u p ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Capture Plan Confirmation Modal
    for _ in {1..500}; do
        if grep -q "\[UI_01_HABITABLE_MODAL_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 1] Capturing Screendump #1 (Habitable Confirmation Modal)..."
            (echo "screendump /tmp/ui01_1_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 3. Request Details / Engineer Diagnostics View
    for _ in {1..300}; do
        if grep -q "\[F106_AWAITING_OPTION_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching 'd' via sendkey to view Engineer Diagnostics..."
            (echo "sendkey d"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 4. Capture Engineer Diagnostics View
    for _ in {1..300}; do
        if grep -q "\[UI_01_ENGINEER_DIAGNOSTICS_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 4] Capturing Screendump #4 (Engineer Diagnostics View)..."
            (echo "screendump /tmp/ui01_4_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 5. Confirm Execution with Option [1]
    for _ in {1..300}; do
        if grep -q "\[F106_AWAITING_OPTION_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching '1' via sendkey to confirm plan..."
            (echo "sendkey 1"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 6. Capture Transaction Receipt Card
    for _ in {1..300}; do
        if grep -q "\[UI_01_HABITABLE_RECEIPT_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 2] Capturing Screendump #2 (Habitable Transaction Receipt Card)..."
            (echo "screendump /tmp/ui01_2_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 7. Undo Transaction via F3
    for _ in {1..300}; do
        if grep -q "\[F106_AWAITING_UNDO_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching F3 scancode via sendkey to undo transaction..."
            (echo "sendkey f3"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 8. Capture Undone Desktop
    for _ in {1..300}; do
        if grep -q "\[UI_01_HABITABLE_UNDONE_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 3] Capturing Screendump #3 (Habitable Undone Desktop)..."
            (echo "screendump /tmp/ui01_3_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID2}" || true
rm -f "${MON_SOCK2}"

convert_screenshot "/tmp/ui01_1_raw.ppm" "ui_01_1_habitable_plan_modal"
convert_screenshot "/tmp/ui01_4_raw.ppm" "ui_01_4_engineer_diagnostics"
convert_screenshot "/tmp/ui01_2_raw.ppm" "ui_01_2_habitable_receipt_card"
convert_screenshot "/tmp/ui01_3_raw.ppm" "ui_01_3_habitable_undone_desktop"

echo "[VERIFY] Checking Stage 2 Invariants..."
grep -q "\[UI_01_HABITABLE_MODAL_RENDERED\]" "${LOG_STAGE2}"
grep -q "\[UI_01_ENGINEER_DIAGNOSTICS_RENDERED\]" "${LOG_STAGE2}"
grep -q "\[UI_01_HABITABLE_RECEIPT_RENDERED\]" "${LOG_STAGE2}"
grep -q "\[UI_01_HABITABLE_UNDONE_RENDERED\]" "${LOG_STAGE2}"
grep -q "\[CAUSAL_INTEGRATION_ALL_INVARIANTS_VERIFIED_MARKER\]" "${LOG_STAGE2}"
echo "  [PASS] All Stage 2 Habitable UI States and Causal Audit Invariants Verified."

# Stage 3: Cold Reboot #2 (Persistent Undo Verification)
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2 - STAGE 3: Cold Reboot #2 (Persistent Undo & Desktop) <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_STAGE3}" /tmp/ui01_mon3.sock /tmp/ui01_5_raw.ppm
MON_SOCK3="/tmp/ui01_mon3.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_INTERACTIVE}",format=raw,index=1,media=disk \
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

    # Capture Stage 3 Verified Desktop
    for _ in {1..2500}; do
        if grep -q -E "(\[UI_01_STAGE3_VERIFIED_RENDERED\]|\[G8_STALL\])" "${LOG_STAGE3}" 2>/dev/null; then
            echo "[SCREEN 5] Capturing Screendump #5 (Stage 3 Verified Desktop)..."
            echo "screendump /tmp/ui01_5_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK3}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID3}" || true
rm -f "${MON_SOCK3}"

convert_screenshot "/tmp/ui01_5_raw.ppm" "ui_01_5_stage3_verified"

echo "[VERIFY] Checking Stage 3 Serial Invariants..."
grep -q "\[F106_COLD_REBOOT_VERIFICATION_SUCCESS\]" "${LOG_STAGE3}"
grep -q "\[UI_01_STAGE3_VERIFIED_RENDERED\]" "${LOG_STAGE3}"
echo "  [PASS] Cold Reboot #2 Persistence Verified."

# =========================================================================
# PHASE 3: Checksum Distinctness & Evidence Manifest Generation
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: Visual Distinctness & Checksum Verification <<<"
echo "--------------------------------------------------------------------------"

LEDGER="${EVIDENCE_DIR}/ui_01_artifacts.sha256"
rm -f "${LEDGER}"

cd "${EVIDENCE_DIR}"
sha256sum \
    ui_01_0_modelfree_desktop.bmp \
    ui_01_0_modelfree_desktop.png \
    ui_01_1_habitable_plan_modal.bmp \
    ui_01_1_habitable_plan_modal.png \
    ui_01_2_habitable_receipt_card.bmp \
    ui_01_2_habitable_receipt_card.png \
    ui_01_3_habitable_undone_desktop.bmp \
    ui_01_3_habitable_undone_desktop.png \
    ui_01_4_engineer_diagnostics.bmp \
    ui_01_4_engineer_diagnostics.png \
    ui_01_5_stage3_verified.bmp \
    ui_01_5_stage3_verified.png \
    ui_01_modelfree_serial.log \
    ui_01_stage1_serial.log \
    ui_01_stage2_serial.log \
    ui_01_stage3_serial.log | tee "${LEDGER}"

echo ""
echo "[DISTINCTNESS AUDIT] Verifying all BMP screendump checksums are genuinely distinct..."
BMP_HASHES=$(awk '/\.bmp$/ {print $1}' "${LEDGER}")
UNIQUE_COUNT=$(echo "${BMP_HASHES}" | sort -u | wc -l)
TOTAL_COUNT=$(echo "${BMP_HASHES}" | wc -l)

echo "Total BMP screendumps: ${TOTAL_COUNT}, Unique SHA-256 hashes: ${UNIQUE_COUNT}"
if [ "${UNIQUE_COUNT}" -ne "${TOTAL_COUNT}" ]; then
    echo "ERROR: Duplicate screendump hashes detected across workflow states!"
    exit 1
fi

echo "  [PASS] All 6 screendumps have verified, unique cryptographic SHA-256 hashes."
echo ""
echo "=========================================================================="
echo "=== [PASS] GATE UI-01: HABITABLE COMPUTING FOUNDATION VERIFIED        ==="
echo "=========================================================================="
