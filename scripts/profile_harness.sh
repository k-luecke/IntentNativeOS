#!/usr/bin/env bash
# scripts/profile_harness.sh - Deep Performance Profiler for Gate UI-03.1 Acceptance Pipeline
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

CONVERTER="${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp"
AUDITOR="${ROOT_DIR}/tools/contrast_audit"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

now_ms() {
    echo $(($(date +%s%N) / 1000000))
}

echo "=========================================================================="
echo "=== [PHASE A: PERFORMANCE PROFILING] GATE UI-03.1 ACCEPTANCE PIPELINE ==="
echo "=========================================================================="

# --- 1. PROFILE: Kernel & Toolchain Build ---
T0=$(now_ms)
echo "[PROFILE 1] Measuring kernel build..."
bash "${SCRIPT_DIR}/build_kernel.sh" > /dev/null 2>&1
T1=$(now_ms)
D_BUILD=$((T1 - T0))
echo "  -> Kernel build time: ${D_BUILD} ms"

# --- 2. PROFILE: Phase 1 (Model-Free Habitable Desktop) ---
echo "[PROFILE 2] Measuring Phase 1 (Model-Free Habitable Desktop, Zero Model Drive)..."
dd if=/dev/zero of="${DISK_MODELFREE}" bs=1M count=16 status=none
LOG_MF="/tmp/profile_mf.log"
MON_MF="/tmp/profile_mf.sock"
rm -f "${LOG_MF}" "${MON_MF}" /tmp/profile_0.ppm

T0=$(now_ms)
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
    -chardev socket,id=mon0,path="${MON_MF}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_MF}" &
QEMU_PID=$!

(
    for _ in {1..50}; do if [ -S "${MON_MF}" ]; then break; fi; sleep 0.1; done
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_MF}" 2>/dev/null; then
            (for key in d r a f t dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_MF}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
    for _ in {1..300}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_MF}" 2>/dev/null; then
            (for key in h a b i t a b l e spc w o r k s p a c e ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_MF}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
    for _ in {1..300}; do
        if grep -q -E "(\[UI_03_1_SCREEN0_HABITABLE_DESKTOP_RENDERED\]|\[MODEL_FREE_DESKTOP_RENDERED\])" "${LOG_MF}" 2>/dev/null; then
            echo "screendump /tmp/profile_0.ppm" | socat - UNIX-CONNECT:"${MON_MF}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &
wait "${QEMU_PID}" || true
T1=$(now_ms)
D_PHASE1=$((T1 - T0))
echo "  -> Phase 1 (Model-free QEMU lifecycle): ${D_PHASE1} ms"

# --- 3. PROFILE: Phase 2 Stage 1 (With 144 MiB Model Weights Attached) ---
echo "[PROFILE 3] Measuring Phase 2 Stage 1 (With 144 MiB Model Weights on ATA port 0x170)..."
git checkout "${OVMF_VARS}" 2>/dev/null || true
dd if=/dev/zero of="${DISK_INTERACTIVE}" bs=1M count=16 status=none
LOG_S1="/tmp/profile_s1.log"
MON_S1="/tmp/profile_s1.sock"
rm -f "${LOG_S1}" "${MON_S1}"

T_S1_START=$(now_ms)
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
    -chardev socket,id=mon0,path="${MON_S1}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_S1}" &
QEMU_PID=$!

T_STREAM_START=0
T_STREAM_END=0

(
    for _ in {1..50}; do if [ -S "${MON_S1}" ]; then break; fi; sleep 0.1; done
    # Wait for document name prompt
    for _ in {1..2500}; do
        if grep -q "\[PRETRAINED_MODEL\] Streaming " "${LOG_S1}" 2>/dev/null && [ "${T_STREAM_START}" -eq 0 ]; then
            T_STREAM_START=$(now_ms)
        fi
        if grep -q "\[PRETRAINED_MODEL\] Streamed " "${LOG_S1}" 2>/dev/null && [ "${T_STREAM_END}" -eq 0 ]; then
            T_STREAM_END=$(now_ms)
        fi
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_S1}" 2>/dev/null; then
            (for key in d r a f t dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_S1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
    for _ in {1..300}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_S1}" 2>/dev/null; then
            (for key in p r o j e c t spc d r a f t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_S1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &
wait "${QEMU_PID}" || true
T_S1_END=$(now_ms)
D_STAGE1_TOTAL=$((T_S1_END - T_S1_START))

# Extract timestamps from serial log lines if available
echo "  -> Phase 2 Stage 1 (Total with 144MB model): ${D_STAGE1_TOTAL} ms"

# --- 4. PROFILE: Phase 2 Stage 1 WITHOUT Model Weights (Model-Free Equivalent) ---
echo "[PROFILE 4] Measuring Phase 2 Stage 1 WITHOUT Model Weights (Bypassing 144MB ATA Streaming)..."
git checkout "${OVMF_VARS}" 2>/dev/null || true
dd if=/dev/zero of="/tmp/disk_interactive_nomodel.img" bs=1M count=16 status=none
LOG_S1_NOMODEL="/tmp/profile_s1_nomodel.log"
MON_S1_NOMODEL="/tmp/profile_s1_nomodel.sock"
rm -f "${LOG_S1_NOMODEL}" "${MON_S1_NOMODEL}"

T0=$(now_ms)
stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="/tmp/disk_interactive_nomodel.img",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_S1_NOMODEL}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_S1_NOMODEL}" &
QEMU_PID=$!

(
    for _ in {1..50}; do if [ -S "${MON_S1_NOMODEL}" ]; then break; fi; sleep 0.1; done
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_S1_NOMODEL}" 2>/dev/null; then
            (for key in d r a f t dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_S1_NOMODEL}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
    for _ in {1..300}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_S1_NOMODEL}" 2>/dev/null; then
            (for key in p r o j e c t spc d r a f t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_S1_NOMODEL}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &
wait "${QEMU_PID}" || true
T1=$(now_ms)
D_STAGE1_NOMODEL=$((T1 - T0))
D_SAVED=$((D_STAGE1_TOTAL - D_STAGE1_NOMODEL))
echo "  -> Phase 2 Stage 1 (WITHOUT model drive): ${D_STAGE1_NOMODEL} ms"
echo "  -> Time consumed strictly by 144MB ATA PIO transfer + in-guest SHA-256: ${D_SAVED} ms (~$((D_SAVED / 1000)) seconds)"

# --- 5. PROFILE: Phase 2 Stage 2 (Interactive Modal & Authorization) ---
echo "[PROFILE 5] Measuring Phase 2 Stage 2 (Interactive Modal, Tab, Confirm, Screendumps)..."
git checkout "${OVMF_VARS}" 2>/dev/null || true
LOG_S2="/tmp/profile_s2.log"
MON_S2="/tmp/profile_s2.sock"
rm -f "${LOG_S2}" "${MON_S2}" /tmp/profile_1.ppm /tmp/profile_2.ppm /tmp/profile_3.ppm

T0=$(now_ms)
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
    -chardev socket,id=mon0,path="${MON_S2}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_S2}" &
QEMU_PID=$!

(
    for _ in {1..50}; do if [ -S "${MON_S2}" ]; then break; fi; sleep 0.1; done
    for _ in {1..2500}; do
        if grep -q "\[F106_AWAITING_INTENT_INPUT\]" "${LOG_S2}" 2>/dev/null; then
            (for key in s a f e l y spc d u p l i c a t e spc d r a f t dot t x t spc t o spc b a c k u p ret; do
                echo "sendkey ${key}"
                sleep 0.01
            done) | socat - UNIX-CONNECT:"${MON_S2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    for _ in {1..500}; do
        if grep -q "\[UI_03_1_SCREEN1_MODAL_DEFAULT_RENDERED\]" "${LOG_S2}" 2>/dev/null; then
            echo "screendump /tmp/profile_1.ppm" | socat - UNIX-CONNECT:"${MON_S2}" 2>/dev/null || true
            sleep 0.8
            (echo "sendkey tab"; sleep 0.05) | socat - UNIX-CONNECT:"${MON_S2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    for _ in {1..300}; do
        if grep -q "\[UI_03_1_SCREEN2_BUTTON_FOCUS_RENDERED\]" "${LOG_S2}" 2>/dev/null; then
            echo "screendump /tmp/profile_2.ppm" | socat - UNIX-CONNECT:"${MON_S2}" 2>/dev/null || true
            sleep 0.8
            (echo "sendkey 1"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_S2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    for _ in {1..300}; do
        if grep -q -E "(\[UI_03_1_SCREEN3_RECEIPT_RENDERED\]|\[UI_03_1_RECEIPT_CARD_RENDERED\])" "${LOG_S2}" 2>/dev/null; then
            echo "screendump /tmp/profile_3.ppm" | socat - UNIX-CONNECT:"${MON_S2}" 2>/dev/null || true
            sleep 0.8
            break
        fi
        sleep 0.1
    done

    for _ in {1..300}; do
        if grep -q "\[F106_AWAITING_UNDO_INPUT\]" "${LOG_S2}" 2>/dev/null; then
            (echo "sendkey u"; sleep 0.02; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_S2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &
wait "${QEMU_PID}" || true
T1=$(now_ms)
D_STAGE2=$((T1 - T0))
echo "  -> Phase 2 Stage 2 (Total with 144MB model, modal, screendumps): ${D_STAGE2} ms"

# --- 6. PROFILE: Phase 2 Stage 3 (Cold Reboot #2 Persistence) ---
echo "[PROFILE 6] Measuring Phase 2 Stage 3 (Cold Reboot #2, No Model Weights)..."
git checkout "${OVMF_VARS}" 2>/dev/null || true
LOG_S3="/tmp/profile_s3.log"
MON_S3="/tmp/profile_s3.sock"
rm -f "${LOG_S3}" "${MON_S3}" /tmp/profile_4.ppm

T0=$(now_ms)
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
    -chardev socket,id=mon0,path="${MON_S3}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_S3}" &
QEMU_PID=$!

(
    for _ in {1..50}; do if [ -S "${MON_S3}" ]; then break; fi; sleep 0.1; done
    for _ in {1..2500}; do
        if grep -q -E "(\[UI_03_1_SCREEN4_COLD_REBOOT_VERIFIED_RENDERED\]|\[UI_03_SCREEN5_COLD_REBOOT_VERIFIED_RENDERED\])" "${LOG_S3}" 2>/dev/null; then
            echo "screendump /tmp/profile_4.ppm" | socat - UNIX-CONNECT:"${MON_S3}" 2>/dev/null || true
            sleep 1.0
            break
        fi
        sleep 0.1
    done
) &
wait "${QEMU_PID}" || true
T1=$(now_ms)
D_STAGE3=$((T1 - T0))
echo "  -> Phase 2 Stage 3 (Cold reboot, persistence check): ${D_STAGE3} ms"

# --- 7. PROFILE: Host-side Image Processing & Conversions ---
echo "[PROFILE 7] Measuring image conversion overhead (5 screendumps)..."
T0=$(now_ms)
for i in 0 1 2 3 4; do
    if [ -f "/tmp/profile_${i}.ppm" ]; then
        "${CONVERTER}" "/tmp/profile_${i}.ppm" "/tmp/profile_${i}.bmp"
        convert "/tmp/profile_${i}.bmp" -alpha off "/tmp/profile_${i}.png" 2>/dev/null || true
    fi
done
T1=$(now_ms)
D_IMAGES=$((T1 - T0))
echo "  -> 5 screendumps PPM->BMP->PNG conversion: ${D_IMAGES} ms"

# --- 8. PROFILE: Host-side Artifact Hashing (Cold vs Warm Cache) ---
echo "[PROFILE 8] Measuring host-side artifact hashing..."
ARTIFACT_LIST=(
    "${WEIGHTS}"
    "${BUILD_DIR}/esp/EFI/BOOT/BOOTX64.EFI"
    "${DISK_INTERACTIVE}"
    "${EVIDENCE_DIR}/ui_03_1_0_modelfree_desktop.bmp"
    "${EVIDENCE_DIR}/ui_03_1_0_modelfree_desktop.png"
    "${EVIDENCE_DIR}/ui_03_1_1_habitable_modal.bmp"
    "${EVIDENCE_DIR}/ui_03_1_1_habitable_modal.png"
    "${EVIDENCE_DIR}/ui_03_1_2_modal_button_focus.bmp"
    "${EVIDENCE_DIR}/ui_03_1_2_modal_button_focus.png"
    "${EVIDENCE_DIR}/ui_03_1_3_receipt_card.bmp"
    "${EVIDENCE_DIR}/ui_03_1_3_receipt_card.png"
    "${EVIDENCE_DIR}/ui_03_1_4_cold_reboot_verified.bmp"
    "${EVIDENCE_DIR}/ui_03_1_4_cold_reboot_verified.png"
)

# Run 1 (Initial / Cache Warm-up)
T0=$(now_ms)
sha256sum "${ARTIFACT_LIST[@]}" > /tmp/profile_hashes.txt
T1=$(now_ms)
D_HASH_WARM1=$((T1 - T0))

# Run 2 (Warm Cache)
T0=$(now_ms)
sha256sum "${ARTIFACT_LIST[@]}" > /tmp/profile_hashes2.txt
T1=$(now_ms)
D_HASH_WARM2=$((T1 - T0))
echo "  -> Full sha256sum Run 1 (144MB weight + 16MB disk + 10 images): ${D_HASH_WARM1} ms"
echo "  -> Full sha256sum Run 2 (warm cache): ${D_HASH_WARM2} ms"

# Run without the 144MB model weight
ARTIFACT_LIST_NOWEIGHTS=("${ARTIFACT_LIST[@]:1}")
T0=$(now_ms)
sha256sum "${ARTIFACT_LIST_NOWEIGHTS[@]}" > /tmp/profile_hashes_noweights.txt
T1=$(now_ms)
D_HASH_NOWEIGHTS=$((T1 - T0))
echo "  -> Full sha256sum WITHOUT 144MB weights: ${D_HASH_NOWEIGHTS} ms"
echo "  -> sha256sum time consumed strictly by 144MB weights: $((D_HASH_WARM1 - D_HASH_NOWEIGHTS)) ms"

# --- 9. SUMMARY REPORT ---
echo ""
echo "=========================================================================="
echo "=== [PROFILING RESULTS SUMMARY] ==="
echo "=========================================================================="
echo "1. Kernel Build:                               ${D_BUILD} ms"
echo "2. Phase 1 QEMU (Model-Free, no model drive):   ${D_PHASE1} ms"
echo "3. Phase 2 Stage 1 (With 144MB ATA weights):    ${D_STAGE1_TOTAL} ms"
echo "   - Phase 2 Stage 1 WITHOUT model drive:       ${D_STAGE1_NOMODEL} ms"
echo "   - ATA PIO 144MB Streaming Overhead (Stage 1):${D_SAVED} ms (~$((D_SAVED/1000))s)"
echo "4. Phase 2 Stage 2 (With 144MB ATA weights):    ${D_STAGE2} ms"
echo "5. Phase 2 Stage 3 (Cold reboot, persistence):  ${D_STAGE3} ms"
echo "6. Image Conversions (5 screens, BMP+PNG):      ${D_IMAGES} ms"
echo "7. Host Hashing (with 144MB model weights):     ${D_HASH_WARM1} ms"
echo "   - Host Hashing (WITHOUT 144MB weights):      ${D_HASH_NOWEIGHTS} ms"
echo "=========================================================================="
TOTAL_RUN=$((D_BUILD + D_PHASE1 + D_STAGE1_TOTAL + D_STAGE2 + D_STAGE3 + D_IMAGES + D_HASH_WARM1))
TOTAL_ATA_OVERHEAD=$((D_SAVED * 2))
echo "Total Acceptance Run Duration:                  ${TOTAL_RUN} ms (~$((TOTAL_RUN/1000))s)"
echo "Total ATA 144MB Model Streaming (Stage 1 + 2):  ${TOTAL_ATA_OVERHEAD} ms (~$((TOTAL_ATA_OVERHEAD/1000))s)"
echo "Percentage of Test Time in ATA PIO Streaming:   $((TOTAL_ATA_OVERHEAD * 100 / TOTAL_RUN))%"
echo "=========================================================================="
