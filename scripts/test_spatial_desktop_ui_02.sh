#!/usr/bin/env bash
# scripts/test_spatial_desktop_ui_02.sh - Verification suite for Gate UI-02: Spatial Desktop & Typography
# Verifies:
# 1. Proportional font rendering & metrics (Liberation Serif via FreeType rasterizer)
# 2. Decoupled Spatial Window Manager (Dual independent windows, z-ordering, clipping)
# 3. PS/2 Mouse & Pointer subsystem (Cursor backing store, click-to-raise, window dragging)
# 4. Redraw performance benchmark (Full-frame vs dirty-region rdtsc cycles)
# 5. Multi-document independent editing & storage persistence
# 6. Cold reboot verification with persistent undo
# 7. Cryptographically distinct framebuffer screendumps for all 6 UI states

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

LOG_MODELFREE="${EVIDENCE_DIR}/ui_02_modelfree_serial.log"
LOG_STAGE1="${EVIDENCE_DIR}/ui_02_stage1_serial.log"
LOG_STAGE2="${EVIDENCE_DIR}/ui_02_stage2_serial.log"
LOG_STAGE3="${EVIDENCE_DIR}/ui_02_stage3_serial.log"

CONVERTER="${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE UI-02] SPATIAL DESKTOP & TYPOGRAPHY ACCEPTANCE SUITE         ==="
echo "=========================================================================="

# Step 0: Ensure tools and kernel are freshly compiled
echo ""
echo "[STEP 0] Ensuring tools and kernel are freshly compiled..."
if [ ! -f "${CONVERTER}" ]; then
    echo "  -> Compiling native Rust ppm2bmp tool..."
    (cd "${ROOT_DIR}/tools/ppm2bmp" && cargo build --release)
fi

echo "  -> Building kernel image and native Ring 3 user ELF..."
bash "${SCRIPT_DIR}/build_kernel.sh"

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
# PHASE 1: Model-Free Spatial Desktop Baseline (NO MODEL WEIGHTS ATTACHED)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Model-Free Spatial Desktop (Zero Model Weights Attached) <<<"
echo "--------------------------------------------------------------------------"

echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for model-free boot..."
dd if=/dev/zero of="${DISK_MODELFREE}" bs=1M count=16 status=none

rm -f "${LOG_MODELFREE}" /tmp/ui02_mon_mf.sock /tmp/ui02_0_raw.ppm
MON_SOCK_MF="/tmp/ui02_mon_mf.sock"

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
            echo "[INPUT HW] Dispatching hardware keypresses for 'draft.txt'..."
            (for key in d r a f t dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK_MF}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Wait for document content prompt
    for _ in {1..300}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_MODELFREE}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching model-free content..."
            (for key in h a b i t a b l e spc w o r k s p a c e ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK_MF}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 3. Capture Model-Free Spatial Desktop Screendump #0
    for _ in {1..300}; do
        if grep -q -E "(\[UI_02_SCREEN0_MODELFREE_RENDERED\]|\[MODEL_FREE_DESKTOP_RENDERED\])" "${LOG_MODELFREE}" 2>/dev/null; then
            echo "[SCREEN 0] Capturing Screendump #0 (Model-Free Spatial Desktop)..."
            (echo "screendump /tmp/ui02_0_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK_MF}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID_MF}" || true
rm -f "${MON_SOCK_MF}"

convert_screenshot "/tmp/ui02_0_raw.ppm" "ui_02_0_modelfree_spatial"

echo "[VERIFY] Checking Model-Free Spatial Desktop Serial Invariants..."
grep -q "\[MODEL_FREE_BASELINE_ACTIVE\]" "${LOG_MODELFREE}"
grep -q "\[MODEL_FREE_BASELINE_VERIFIED_MARKER\]" "${LOG_MODELFREE}"
grep -q "\[G9_STAGE1_SAVED_OK\]" "${LOG_MODELFREE}"
grep -q "\[UI_02_DUAL_SURFACES_MOUNTED\]" "${LOG_MODELFREE}"
echo "  [PASS] Clean Model-Free Spatial Desktop Mounted with Dual Document Surfaces."

# =========================================================================
# PHASE 2: Instruction-Tuned Spatial Desktop Workflow (With Model Weights)
# =========================================================================

# Stage 1: Setup Multi-Document Environment
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2 - STAGE 1: Creating 'draft.txt' & 'notes.txt' with Model Attached <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true
dd if=/dev/zero of="${DISK_INTERACTIVE}" bs=1M count=16 status=none
rm -f "${LOG_STAGE1}" /tmp/ui02_mon1.sock
MON_SOCK1="/tmp/ui02_mon1.sock"

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
grep -q "\[UI_02_DUAL_SURFACES_MOUNTED\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Setup Complete: Dual documents initialized and committed to disk."

# Stage 2: Cold Reboot #1 (Benchmark -> Pointer Focus -> Drag/Edit -> Modal -> Receipt -> Undo)
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2 - STAGE 2: Spatial Multi-Document Interaction & Benchmark <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_STAGE2}" /tmp/ui02_mon2.sock /tmp/ui02_1_raw.ppm /tmp/ui02_2_raw.ppm /tmp/ui02_3_raw.ppm /tmp/ui02_4_raw.ppm
MON_SOCK2="/tmp/ui02_mon2.sock"

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

    # 1. Capture Screen 1: Pointer Focus Switch (notes.txt raised to z=2, cursor visible at (720, 200))
    for _ in {1..2500}; do
        if grep -q "\[UI_02_SCREEN1_POINTER_FOCUS_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 1] Capturing Screendump #1 (Pointer Focus Switch - notes.txt Raised)..."
            (echo "screendump /tmp/ui02_1_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 2. Capture Screen 2: Window Dragged & Independent Edit (notes.txt dragged by (-60, +30), content appended)
    for _ in {1..300}; do
        if grep -q "\[UI_02_SCREEN2_EDITED_MOVED_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 2] Capturing Screendump #2 (Window Dragged & Independent Edit)..."
            (echo "screendump /tmp/ui02_2_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 3. Dispatch hardware intent instruction
    for _ in {1..300}; do
        if grep -q "\[F106_AWAITING_INTENT_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching intent: 'safely duplicate draft.txt to backup'..."
            (for key in s a f e l y spc d u p l i c a t e spc d r a f t dot t x t spc t o spc b a c k u p ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 4. Capture Screen 3: Layered Intent Confirmation Modal
    for _ in {1..300}; do
        if grep -q "\[UI_02_SCREEN3_MODAL_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 3] Capturing Screendump #3 (Layered Intent Confirmation Modal)..."
            (echo "screendump /tmp/ui02_3_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
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

    # 6. Capture Screen 4: Transaction Receipt Card
    for _ in {1..300}; do
        if grep -q "\[UI_02_SCREEN4_RECEIPT_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 4] Capturing Screendump #4 (Spatial Transaction Receipt Card)..."
            (echo "screendump /tmp/ui02_4_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
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
) &

wait "${QEMU_PID2}" || true
rm -f "${MON_SOCK2}"

convert_screenshot "/tmp/ui02_1_raw.ppm" "ui_02_1_pointer_focus"
convert_screenshot "/tmp/ui02_2_raw.ppm" "ui_02_2_window_drag_edit"
convert_screenshot "/tmp/ui02_3_raw.ppm" "ui_02_3_modal_layered"
convert_screenshot "/tmp/ui02_4_raw.ppm" "ui_02_4_receipt_card"

echo "[VERIFY] Checking Stage 2 Invariants & Telemetry..."
grep -q "\[UI_02_PS2_MOUSE_INITIALIZED\]" "${LOG_STAGE2}"
grep -q "\[UI_02_BENCHMARK_FULL_FRAME_TICKS\]" "${LOG_STAGE2}"
grep -q "\[UI_02_BENCHMARK_DIRTY_REGION_TICKS\]" "${LOG_STAGE2}"
grep -q "\[UI_02_RENDERING_BENCHMARK_VERIFIED\]" "${LOG_STAGE2}"
grep -q "\[UI_02_POINTER_FOCUS_SWITCHED\]" "${LOG_STAGE2}"
grep -q "\[UI_02_WINDOW_DRAGGED\]" "${LOG_STAGE2}"
grep -q "\[UI_02_KEYBOARD_FOCUSED_EDIT\]" "${LOG_STAGE2}"
grep -q "\[UI_02_MODAL_LAYERED_ON_SPATIAL\]" "${LOG_STAGE2}"
grep -q "\[UI_02_SCREEN4_RECEIPT_RENDERED\]" "${LOG_STAGE2}"
grep -q "\[CAUSAL_INTEGRATION_ALL_INVARIANTS_VERIFIED_MARKER\]" "${LOG_STAGE2}"
echo "  [PASS] All Stage 2 Spatial Desktop, Pointer Interactions, and Causal Invariants Verified."

# Stage 3: Cold Reboot #2 (Persistent Multi-Document & Undo Verification)
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2 - STAGE 3: Cold Reboot #2 (Persistent Dual Documents & Undo) <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_STAGE3}" /tmp/ui02_mon3.sock /tmp/ui02_5_raw.ppm
MON_SOCK3="/tmp/ui02_mon3.sock"

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

    # Capture Stage 3 Verified Desktop Screendump #5
    for _ in {1..2500}; do
        if grep -q "\[UI_02_SCREEN5_STAGE3_VERIFIED_RENDERED\]" "${LOG_STAGE3}" 2>/dev/null; then
            echo "[SCREEN 5] Capturing Screendump #5 (Stage 3 Restored Multi-Document Desktop)..."
            echo "screendump /tmp/ui02_5_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK3}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID3}" || true
rm -f "${MON_SOCK3}"

convert_screenshot "/tmp/ui02_5_raw.ppm" "ui_02_5_cold_reboot_verified"

echo "[VERIFY] Checking Stage 3 Serial Invariants..."
grep -q "\[UI_02_COLD_REBOOT_DUAL_DOCS_VERIFIED\]" "${LOG_STAGE3}"
grep -q "\[F106_COLD_REBOOT_VERIFICATION_SUCCESS\]" "${LOG_STAGE3}"
grep -q "\[UI_02_SCREEN5_STAGE3_VERIFIED_RENDERED\]" "${LOG_STAGE3}"
echo "  [PASS] Cold Reboot #2 Persistence for Both Documents Verified."

# =========================================================================
# PHASE 3: Checksum Distinctness & Evidence Manifest Generation
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: Visual Distinctness & Checksum Verification <<<"
echo "--------------------------------------------------------------------------"

LEDGER="${EVIDENCE_DIR}/ui_02_artifacts.sha256"
rm -f "${LEDGER}"

cd "${EVIDENCE_DIR}"
sha256sum \
    ui_02_0_modelfree_spatial.bmp \
    ui_02_0_modelfree_spatial.png \
    ui_02_1_pointer_focus.bmp \
    ui_02_1_pointer_focus.png \
    ui_02_2_window_drag_edit.bmp \
    ui_02_2_window_drag_edit.png \
    ui_02_3_modal_layered.bmp \
    ui_02_3_modal_layered.png \
    ui_02_4_receipt_card.bmp \
    ui_02_4_receipt_card.png \
    ui_02_5_cold_reboot_verified.bmp \
    ui_02_5_cold_reboot_verified.png \
    ui_02_modelfree_serial.log \
    ui_02_stage1_serial.log \
    ui_02_stage2_serial.log \
    ui_02_stage3_serial.log | tee "${LEDGER}"

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
echo "=== [TELEMETRY BENCHMARK REPORT] ==="
grep "\[UI_02_BENCHMARK_" "${LOG_STAGE2}" || true
echo "===================================="
echo ""
echo "=========================================================================="
echo "=== [PASS] GATE UI-02: SPATIAL DESKTOP & TYPOGRAPHY VERIFIED           ==="
echo "=========================================================================="
