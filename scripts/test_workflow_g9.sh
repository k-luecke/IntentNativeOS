#!/usr/bin/env bash
# scripts/test_workflow_g9.sh - Automated Human-Operated Native Workflow Harness (Gate G9)
# Demonstrates: Type -> Create -> Save -> Cold Reboot -> Reopen -> Modify -> Undo -> Cold Reboot -> Verify
# All input dispatched via QEMU monitor sendkey into emulated i8042 PS/2 controller.
# Zero Python: pure Bash, native Rust ppm2bmp, QEMU, and PowerShell/.NET.

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

LOG_STAGE1="${EVIDENCE_DIR}/g9_stage1_serial.log"
LOG_STAGE2="${EVIDENCE_DIR}/g9_stage2_serial.log"
LOG_STAGE3="${EVIDENCE_DIR}/g9_stage3_serial.log"

CONVERTER="${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE G9] HUMAN-OPERATED NATIVE WORKFLOW INDEPENDENT VERIFICATION ==="
echo "=========================================================================="

# Ensure kernel and converter are built
if [ ! -f "${ESP_DIR}/EFI/BOOT/BOOTX64.EFI" ]; then
    echo "[BUILD] Building bootable kernel..."
    bash "${SCRIPT_DIR}/build_kernel.sh"
fi

if [ ! -f "${CONVERTER}" ]; then
    echo "[BUILD] Compiling native Rust ppm2bmp tool..."
    (cd "${ROOT_DIR}/tools/ppm2bmp" && cargo build --release)
fi

# Initialize fresh 16 MB raw virtual disk
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk: ${DISK_IMG}..."
dd if=/dev/zero of="${DISK_IMG}" bs=1M count=16 status=none
echo "[STORAGE] Virtual disk initialized (32,768 zeroed sectors)."

# Helper to convert PPM to BMP and PNG
convert_screenshot() {
    local src_ppm="$1"
    local base_name="$2"
    local dst_bmp="${EVIDENCE_DIR}/${base_name}.bmp"
    local dst_png="${EVIDENCE_DIR}/${base_name}.png"

    if [ -f "${src_ppm}" ] && [ -s "${src_ppm}" ]; then
        "${CONVERTER}" "${src_ppm}" "${dst_bmp}"
        powershell.exe -NoProfile -Command "Add-Type -AssemblyName System.Drawing; \$p = (Resolve-Path '${dst_bmp}').Path; \$out = [System.IO.Path]::ChangeExtension(\$p, '.png'); \$bmp = [System.Drawing.Image]::FromFile(\$p); \$bmp.Save(\$out, [System.Drawing.Imaging.ImageFormat]::Png); \$bmp.Dispose();" 2>/dev/null || true
        echo "[SCREENSHOT] Generated ${dst_bmp} and ${dst_png} ($(wc -c < "${dst_bmp}") bytes)"
        rm -f "${src_ppm}"
    else
        echo "[WARN] Screenshot source ${src_ppm} not found or empty."
    fi
}

# =========================================================================
# STAGE 1: Type Arbitrary Document Name and Content via QEMU sendkey, Save
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> STAGE 1: Booting QEMU Session #1 (Interactive Typing & INOS-FS Save) <<<"
echo "--------------------------------------------------------------------------"
rm -f "${LOG_STAGE1}" /tmp/g9_mon1.sock /tmp/g9_1_raw.ppm /tmp/g9_2_raw.ppm
MON_SOCK1="/tmp/g9_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 256M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_IMG}",format=raw,index=1,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK1}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_STAGE1}" &
QEMU_PID1=$!

# Monitor background driver for Stage 1
(
    # Wait for QEMU monitor socket to be ready
    for _ in {1..50}; do
        if [ -S "${MON_SOCK1}" ]; then break; fi
        sleep 0.1
    done

    # 1. Wait for document name prompt
    for _ in {1..150}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for document name 'report.txt' via sendkey..."
            (for key in r e p o r t dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.05
    done

    # 2. Wait for document content prompt
    for _ in {1..150}; do
        if grep -q "\[G9_AWAITING_DOC_CONTENT\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching arbitrary content keypresses via sendkey..."
            (for key in r e v e n u e spc 1 0 4 5 0 0 0 spc u s d ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.05
    done

    # 3. Capture Screenshot 1 (Typed)
    for _ in {1..150}; do
        if grep -q "\[G9_STAGE1_TYPED_RENDERED\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[SCREEN 1] Capturing Screendump #1 (Document Typed)..."
            echo "screendump /tmp/g9_1_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            sleep 0.2
            break
        fi
        sleep 0.05
    done

    # 4. Capture Screenshot 2 (Saved)
    for _ in {1..150}; do
        if grep -q "\[G9_STAGE1_SAVED_RENDERED\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[SCREEN 2] Capturing Screendump #2 (Document Saved)..."
            echo "screendump /tmp/g9_2_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            sleep 0.2
            break
        fi
        sleep 0.05
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

convert_screenshot "/tmp/g9_1_raw.ppm" "g9_1_document_typed"
convert_screenshot "/tmp/g9_2_raw.ppm" "g9_2_document_saved"

if ! grep -q "\[G9_STAGE1_SAVED_OK\]" "${LOG_STAGE1}"; then
    echo "ERROR: Stage 1 save failed!"
    cat "${LOG_STAGE1}"
    exit 1
fi
echo "[STAGE 1 PASS] Document created, typed via PS/2 port 0x60, and saved to INOS-FS."

# =========================================================================
# STAGE 2: Cold Reboot, Reopen, Modify, and Persistent Snapshot Undo
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> STAGE 2: Cold Reboot #1 (Reopen -> Modify via sendkey -> Undo)    <<<"
echo "--------------------------------------------------------------------------"
rm -f "${LOG_STAGE2}" /tmp/g9_mon2.sock /tmp/g9_3_raw.ppm /tmp/g9_4_raw.ppm /tmp/g9_5_raw.ppm
MON_SOCK2="/tmp/g9_mon2.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 256M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_IMG}",format=raw,index=1,media=disk \
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

    # 1. Capture Screenshot 3 (Reopened from Cold Disk)
    for _ in {1..150}; do
        if grep -q "\[G9_STAGE2_REOPENED_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 3] Capturing Screendump #3 (Document Reopened After Cold Reboot)..."
            echo "screendump /tmp/g9_3_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Wait for modification input prompt
    for _ in {1..150}; do
        if grep -q "\[G9_AWAITING_MOD_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching modification keypresses via sendkey..."
            (for key in spc a m e n d m e n t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.05
    done

    # 3. Capture Screenshot 4 (Modified)
    for _ in {1..150}; do
        if grep -q "\[G9_STAGE2_MODIFIED_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 4] Capturing Screendump #4 (Document Modified)..."
            echo "screendump /tmp/g9_4_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.2
            break
        fi
        sleep 0.05
    done

    # 4. Wait for undo trigger prompt
    for _ in {1..150}; do
        if grep -q "\[G9_AWAITING_UNDO_INPUT\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching undo keypress ('u') via sendkey..."
            (echo "sendkey u"; sleep 0.01; echo "sendkey ret") | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            break
        fi
        sleep 0.05
    done

    # 5. Capture Screenshot 5 (Undone)
    for _ in {1..150}; do
        if grep -q "\[G9_STAGE2_UNDONE_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 5] Capturing Screendump #5 (Document Undone to Persistent Snapshot)..."
            echo "screendump /tmp/g9_5_raw.ppm" | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.2
            break
        fi
        sleep 0.05
    done
) &

wait "${QEMU_PID2}" || true
rm -f "${MON_SOCK2}"

convert_screenshot "/tmp/g9_3_raw.ppm" "g9_3_reopened_reboot"
convert_screenshot "/tmp/g9_4_raw.ppm" "g9_4_document_modified"
convert_screenshot "/tmp/g9_5_raw.ppm" "g9_5_document_undone_reboot"

if ! grep -q "\[G9_STAGE2_UNDONE_OK\]" "${LOG_STAGE2}"; then
    echo "ERROR: Stage 2 undo failed!"
    cat "${LOG_STAGE2}"
    exit 1
fi
echo "[STAGE 2 PASS] Cold reboot reopen, modification, and snapshot undo succeeded."

# =========================================================================
# STAGE 3: Cold Reboot #2 (Verify Persistent Undo Across Process Lifecycles)
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> STAGE 3: Cold Reboot #2 (Verify Undone State Survived Reboot)     <<<"
echo "--------------------------------------------------------------------------"
rm -f "${LOG_STAGE3}"

set +e
timeout 30s qemu-system-x86_64 \
    -m 256M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_IMG}",format=raw,index=1,media=disk \
    -display none \
    -serial file:"${LOG_STAGE3}" \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04
STAGE3_EXIT=$?
set -e

if ! grep -q "\[G9_PERSISTENT_UNDO_VERIFIED_AFTER_REBOOT\]" "${LOG_STAGE3}" || \
   ! grep -q "\[GATE_G9_WORKFLOW_VERIFIED_MARKER_v0.1\]" "${LOG_STAGE3}"; then
    echo "ERROR: Stage 3 persistent undo verification failed!"
    cat "${LOG_STAGE3}"
    exit 1
fi
echo "[STAGE 3 PASS] Verified restored baseline content survived second cold reboot!"

# =========================================================================
# Physical Virtual Disk Inspection & Hash Ledger
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHYSICAL VIRTUAL DISK & EVIDENCE ARTIFACTS AUDIT                   <<<"
echo "--------------------------------------------------------------------------"
echo "[DISK AUDIT] Inspecting Superblock (LBA 0):"
hexdump -C -n 64 "${DISK_IMG}"

echo "[DISK AUDIT] Inspecting Directory Table (LBA 7):"
hexdump -C -s 3584 -n 64 "${DISK_IMG}"

echo ""
echo "[HASH LEDGER] Generating SHA-256 signatures for Gate G9 artifacts..."
sha256sum \
    "${EVIDENCE_DIR}/g9_1_document_typed.bmp" \
    "${EVIDENCE_DIR}/g9_2_document_saved.bmp" \
    "${EVIDENCE_DIR}/g9_3_reopened_reboot.bmp" \
    "${EVIDENCE_DIR}/g9_4_document_modified.bmp" \
    "${EVIDENCE_DIR}/g9_5_document_undone_reboot.bmp" \
    "${DISK_IMG}" > "${EVIDENCE_DIR}/g9_artifacts.sha256"

cat "${EVIDENCE_DIR}/g9_artifacts.sha256"

echo ""
echo "=========================================================================="
echo "=== [PASS] GATE G9 COMPLETE: HUMAN-OPERATED NATIVE WORKFLOW VERIFIED! ==="
echo "=========================================================================="
exit 0
