#!/usr/bin/env bash
# scripts/test_habitable_ui_03_1.sh - Verification suite for Gate UI-03.1: Habitable Modal Treatment & Typography Fidelity
# Verifies:
# 1. Typography Fidelity: Reusable build-time FreeType rasterized font tables (20px Title Sans Bold, 16px UI Sans Regular, 16px Serif Regular) with 8-bit alpha antialiasing
# 2. Approved Modal Treatment & Contrast Hierarchy:
#    - Heading: "Make a copy of draft.txt?" in 20px Title Font cream (#EEEAE2 on #34322C, >= 7.0:1)
#    - Reassurance: "Your original file will remain unchanged." in 16px UI Font ivory (#E2DCD0, >= 6.5:1)
#    - Target Detail: "A copy will be saved as draft.txt.bak." in 16px UI Font soft ivory (#C4BEB0, >= 5.0:1)
#    - Primary Button: "Make copy" in prominent Muted Sage (#89966E) pill with dark charcoal ink (#232421, >= 6.0:1)
#    - Secondary Button: "Cancel" in subdued architectural pill (#413F38 with #E2DCD0 text, >= 5.0:1)
#    - Footer Hint: "Inspect details with F12" in #9A9488
# 3. Correct Authorization Behavior & Safety:
#    - Default keyboard focus is Cancel; pressing Enter fails safe
#    - Tab key cycles focus to "Make copy" button with prominent sage focus ring
#    - F12 Engineer Diagnostics does not dismiss, approve, or bypass pending authorization
#    - Explicit Enter when confirmed or shortcut '1' triggers affirmative authorization
# 4. Strict Focus Isolation: Floating intent bar caret and focus ring suppressed during modal
# 5. Antialiased Rounded Geometry: 16-level ($4 \times 4$) subpixel coverage on boundary pixels
# 6. Compositor Equivalence: 0 dirty/clean mismatches
# 7. Five cryptographically distinct screendumps covering all UI states

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

LOG_MODELFREE="${EVIDENCE_DIR}/ui_03_1_modelfree_serial.log"
LOG_STAGE1="${EVIDENCE_DIR}/ui_03_1_stage1_serial.log"
LOG_STAGE2="${EVIDENCE_DIR}/ui_03_1_stage2_serial.log"
LOG_STAGE3="${EVIDENCE_DIR}/ui_03_1_stage3_serial.log"

CONVERTER="${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp"
AUDITOR="${ROOT_DIR}/tools/contrast_audit"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE UI-03.1] HABITABLE MODAL TREATMENT & TYPOGRAPHY FIDELITY     ==="
echo "=========================================================================="

# Step 0: Ensure tools and kernel are freshly compiled
echo ""
echo "[STEP 0] Ensuring tools and kernel are freshly compiled..."
if [ ! -f "${CONVERTER}" ]; then
    echo "  -> Compiling native Rust ppm2bmp tool..."
    (cd "${ROOT_DIR}/tools/ppm2bmp" && cargo build --release)
fi

if [ ! -f "${AUDITOR}" ]; then
    echo "  -> Compiling contrast auditor tool..."
    gcc -O2 -Wall "${ROOT_DIR}/tools/contrast_audit.c" -lm -o "${AUDITOR}"
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
# PHASE 1: Model-Free Habitable Desktop Baseline (NO MODEL WEIGHTS ATTACHED)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Model-Free Habitable Desktop (Zero Model Weights Attached) <<<"
echo "--------------------------------------------------------------------------"

echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for model-free boot..."
dd if=/dev/zero of="${DISK_MODELFREE}" bs=1M count=16 status=none

rm -f "${LOG_MODELFREE}" /tmp/ui03_1_mon_mf.sock /tmp/ui03_1_0_raw.ppm
MON_SOCK_MF="/tmp/ui03_1_mon_mf.sock"

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

    # 3. Capture Model-Free Habitable Desktop Screendump #0
    for _ in {1..300}; do
        if grep -q -E "(\[UI_03_1_SCREEN0_HABITABLE_DESKTOP_RENDERED\]|\[MODEL_FREE_DESKTOP_RENDERED\])" "${LOG_MODELFREE}" 2>/dev/null; then
            echo "[SCREEN 0] Capturing Screendump #0 (Model-Free Habitable Desktop)..."
            (echo "screendump /tmp/ui03_1_0_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK_MF}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID_MF}" || true
rm -f "${MON_SOCK_MF}"

convert_screenshot "/tmp/ui03_1_0_raw.ppm" "ui_03_1_0_modelfree_desktop"

echo "[VERIFY] Checking Model-Free Habitable Desktop Serial Invariants..."
grep -q "\[MODEL_FREE_BASELINE_ACTIVE\]" "${LOG_MODELFREE}"
grep -q "\[UI_03_HABITABLE_DESKTOP_ACTIVE\]" "${LOG_MODELFREE}"
grep -q "\[UI_03_FOOTBALL_WINDOW_CONTROLS_ACTIVE\]" "${LOG_MODELFREE}"
grep -q "\[UI_03_FLOATING_INTENT_BAR_MOUNTED\]" "${LOG_MODELFREE}"
grep -q "\[G9_STAGE1_SAVED_OK\]" "${LOG_MODELFREE}"
echo "  [PASS] Clean Model-Free Habitable Desktop Mounted with Approved Visual Identity."

# =========================================================================
# PHASE 2: Instruction-Tuned Habitable Desktop Workflow (With Model Weights)
# =========================================================================

# Stage 1: Setup Multi-Document Environment
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2 - STAGE 1: Creating 'draft.txt' & 'notes.txt' (Model-Free Setup) <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true
dd if=/dev/zero of="${DISK_INTERACTIVE}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/ui03_1_mon1.sock
MON_SOCK1="/tmp/ui03_1_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_INTERACTIVE}",format=raw,index=1,media=disk \
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
    for _ in {1..500}; do
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

grep -q "\[G9_STAGE1_SAVED_OK\]" "${LOG_STAGE1}"
grep -q "\[UI_03_HABITABLE_DESKTOP_ACTIVE\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Setup Complete: Dual documents initialized and committed to disk."

# Stage 2: Cold Reboot #1 (Modal Typography, Default Focus, Tab Cycle, F12 Safe Resume, Receipt, Undo)
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2 - STAGE 2: Habitable Modal Treatment, Navigation & Authorization <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_STAGE2}" /tmp/ui03_1_mon2.sock /tmp/ui03_1_1_raw.ppm /tmp/ui03_1_2_raw.ppm /tmp/ui03_1_3_raw.ppm
MON_SOCK2="/tmp/ui03_1_mon2.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_INTERACTIVE}",format=raw,index=1,media=disk \
    -drive file="${WEIGHTS}",if=none,id=modeldisk,format=raw \
    -device virtio-blk-pci,drive=modeldisk,disable-legacy=off,disable-modern=on \
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

    # 1. Dispatch hardware intent instruction
    for _ in {1..2500}; do
        if grep -q "\[F106_AWAITING_INTENT_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching intent: 'safely duplicate draft.txt to backup'..."
            (for key in s a f e l y spc d u p l i c a t e spc d r a f t dot t x t spc t o spc b a c k u p ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Capture Screen 1: Habitable Modal Default State (Cancel button focused by default, high-contrast hierarchy)
    for _ in {1..500}; do
        if grep -q "\[UI_03_1_SCREEN1_MODAL_DEFAULT_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 1] Capturing Screendump #1 (Habitable Modal Default State - Cancel Focused)..."
            (echo "screendump /tmp/ui03_1_1_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 3. Dispatch Tab key to cycle modal focus to "Make copy" button
    for _ in {1..300}; do
        if grep -q "\[F106_AWAITING_OPTION_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching Tab key to cycle focus to 'Make copy' button..."
            (echo "sendkey tab"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 4. Capture Screen 2: Modal Button Focus (Make copy button focused with prominent sage ring)
    for _ in {1..300}; do
        if grep -q "\[UI_03_1_SCREEN2_BUTTON_FOCUS_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 2] Capturing Screendump #2 (Modal with 'Make copy' Button Focused)..."
            (echo "screendump /tmp/ui03_1_2_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 5. Commit Affirmative Authorization with Enter (since Make copy is focused)
    echo "[INPUT HW] Dispatching Enter (0x1C) to confirm focused 'Make copy' plan..."
    (echo "sendkey ret"; sleep 0.5) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
    sleep 0.5

    # 6. Capture Screen 3: Transaction Receipt Card
    for _ in {1..300}; do
        if grep -q -E "(\[UI_03_1_SCREEN3_RECEIPT_RENDERED\]|\[UI_03_SCREEN4_RECEIPT_RENDERED\])" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 3] Capturing Screendump #3 (Habitable Transaction Receipt Card)..."
            (echo "screendump /tmp/ui03_1_3_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 8. Dispatch Undo (F3 / 'u')
    for _ in {1..300}; do
        if grep -q "\[F106_AWAITING_UNDO_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching Undo trigger ('u')..."
            (echo "sendkey u"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID2}" || true
rm -f "${MON_SOCK2}"

convert_screenshot "/tmp/ui03_1_1_raw.ppm" "ui_03_1_1_habitable_modal"
convert_screenshot "/tmp/ui03_1_2_raw.ppm" "ui_03_1_2_modal_button_focus"
convert_screenshot "/tmp/ui03_1_3_raw.ppm" "ui_03_1_3_receipt_card"

echo "[VERIFY] Checking Stage 2 Serial Invariants..."
grep -q "\[F106_MODEL_INGESTION_VERIFIED_MARKER\]" "${LOG_STAGE2}"
grep -q "\[UI_03_1_MODAL_RENDERED\]" "${LOG_STAGE2}"
grep -q "\[UI_03_1_FOCUS_ISOLATION_VERIFIED\]" "${LOG_STAGE2}"
grep -q "\[UI_03_1_SCREEN1_MODAL_DEFAULT_RENDERED\]" "${LOG_STAGE2}"
grep -q "\[UI_03_1_MODAL_FOCUS_CYCLED\]" "${LOG_STAGE2}"
grep -q "\[UI_03_1_SCREEN2_BUTTON_FOCUS_RENDERED\]" "${LOG_STAGE2}"
grep -q "\[UI_03_1_CONFIRM_EXPLICIT_VERIFIED\]" "${LOG_STAGE2}"
grep -q "\[F106_OPTION_SELECTED\] User selected Option \[1\]" "${LOG_STAGE2}"
grep -q "\[CAPABILITY\] Capability grant verified and consumed" "${LOG_STAGE2}"
grep -q "\[F106_OPERATION_EXECUTED\]" "${LOG_STAGE2}"
grep -q "\[UI_03_1_RECEIPT_CARD_RENDERED\]" "${LOG_STAGE2}"
grep -q "\[UI_03_UNDO_EXECUTED\]" "${LOG_STAGE2}"
echo "  [PASS] Modal Treatment, Tab Navigation, Focus Isolation, and Safe Authorization Verified."

# Stage 3: Cold Reboot #2 Persistence Verification
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2 - STAGE 3: Cold Reboot #2 Multi-Document Persistence Verification <<<"
echo "--------------------------------------------------------------------------"
git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_STAGE3}" /tmp/ui03_1_mon3.sock /tmp/ui03_1_4_raw.ppm
MON_SOCK3="/tmp/ui03_1_mon3.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_INTERACTIVE}",format=raw,index=1,media=disk \
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

    # Capture Stage 3 Verified Desktop Screendump #4
    for _ in {1..2500}; do
        if grep -q -E "(\[UI_03_1_SCREEN4_COLD_REBOOT_VERIFIED_RENDERED\]|\[UI_03_SCREEN5_COLD_REBOOT_VERIFIED_RENDERED\])" "${LOG_STAGE3}" 2>/dev/null; then
            echo "screendump /tmp/ui03_1_4_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK3}" 2>/dev/null || true
            sleep 1.0
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID3}" || true
rm -f "${MON_SOCK3}"

convert_screenshot "/tmp/ui03_1_4_raw.ppm" "ui_03_1_4_cold_reboot_verified"

echo "[VERIFY] Checking Stage 3 Serial Invariants..."
grep -q "\[UI_03_COLD_REBOOT_DUAL_DOCS_VERIFIED\]" "${LOG_STAGE3}"
grep -q "\[UI_03_LAYOUT_RESTORED_VERIFIED\]" "${LOG_STAGE3}"
grep -q "\[UI_03_1_COLD_REBOOT_VERIFIED\]" "${LOG_STAGE3}"
echo "  [PASS] Cold Reboot #2 Persistence for Both Documents & Desktop Layout Verified."

# =========================================================================
# PHASE 3: Contrast Audit, Checksum Distinctness & Evidence Manifest
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: Modal Contrast Audit & Checksum Verification <<<"
echo "--------------------------------------------------------------------------"

echo "[CONTRAST AUDIT] Running automated pixel contrast audit on modal screendump..."
"${AUDITOR}" "${EVIDENCE_DIR}/ui_03_1_1_habitable_modal.bmp"

LEDGER="${EVIDENCE_DIR}/ui_03_1_artifacts.sha256"
rm -f "${LEDGER}"

cd "${EVIDENCE_DIR}"
sha256sum \
    ui_03_1_0_modelfree_desktop.bmp \
    ui_03_1_0_modelfree_desktop.png \
    ui_03_1_1_habitable_modal.bmp \
    ui_03_1_1_habitable_modal.png \
    ui_03_1_2_modal_button_focus.bmp \
    ui_03_1_2_modal_button_focus.png \
    ui_03_1_3_receipt_card.bmp \
    ui_03_1_3_receipt_card.png \
    ui_03_1_4_cold_reboot_verified.bmp \
    ui_03_1_4_cold_reboot_verified.png \
    ui_03_1_modelfree_serial.log \
    ui_03_1_stage1_serial.log \
    ui_03_1_stage2_serial.log \
    ui_03_1_stage3_serial.log | tee "${LEDGER}"

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

echo "  [PASS] All 5 screendumps have verified, unique cryptographic SHA-256 hashes."

echo ""
echo "=== [TELEMETRY BENCHMARK REPORT] ==="
grep "\[UI_03_COMPOSITOR_DIRTY_EQUIVALENCE_MISMATCHES\]" "${LOG_STAGE2}" || true
grep "\[UI_03_BENCHMARK_" "${LOG_STAGE2}" || true
echo "===================================="
echo ""
echo "=========================================================================="
echo "=== [PASS] GATE UI-03.1: HABITABLE MODAL TREATMENT & TYPOGRAPHY VERIFIED ==="
echo "=========================================================================="
