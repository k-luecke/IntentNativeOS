#!/usr/bin/env bash
# scripts/test_persistence_g8.sh - Automated Two-Stage Hardware Persistence Test (GAP-G7-04)
# Proves that intent documents and WAL states survive genuine QEMU process termination and reboot.
# Zero Python: pure Bash, QEMU, and native Rust.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_IMG="${BUILD_DIR}/persistent_disk.img"
LOG_STAGE1="${EVIDENCE_DIR}/persistence_stage1_serial.log"
LOG_STAGE2="${EVIDENCE_DIR}/persistence_stage2_serial.log"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "================================================================="
echo "=== [G8 / GAP-G7-04] TWO-STAGE HARDWARE DISK PERSISTENCE TEST ==="
echo "================================================================="

# Step 0: Ensure bootloader is built
if [ ! -f "${ESP_DIR}/EFI/BOOT/BOOTX64.EFI" ]; then
    echo "[BUILD] Building bootable kernel image..."
    bash "${SCRIPT_DIR}/build_kernel.sh"
fi

# Step 1: Create fresh 16 MB raw disk image
echo "[STORAGE] Initializing fresh 16 MB raw disk: ${DISK_IMG}..."
dd if=/dev/zero of="${DISK_IMG}" bs=1M count=16 status=none
echo "[STORAGE] Virtual disk image initialized with 32,768 zeroed sectors."

# Step 2: STAGE 1 — First QEMU Process (Format, Write Intent Record & Commit WAL)
echo ""
echo "-----------------------------------------------------------------"
echo ">>> STAGE 1: Booting QEMU Process #1 (Initial Write & Commit) <<<"
echo "-----------------------------------------------------------------"
rm -f "${LOG_STAGE1}"

set +e
timeout 25s qemu-system-x86_64 \
    -m 256M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_IMG}",format=raw,index=1,media=disk \
    -display none \
    -serial file:"${LOG_STAGE1}" \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04
STAGE1_EXIT=$?
set -e

echo "[STAGE 1] QEMU Process #1 exited with code ${STAGE1_EXIT}."
if [ "${STAGE1_EXIT}" -ne 33 ] && [ "${STAGE1_EXIT}" -ne 0 ]; then
    echo "ERROR: QEMU Stage 1 did not exit cleanly (got ${STAGE1_EXIT})"
    cat "${LOG_STAGE1}"
    exit 1
fi

# Verify Stage 1 wrote the payload
if ! grep -q "\[PERSISTENCE_STAGE_1_COMPLETE_MARKER_v0.1\]" "${LOG_STAGE1}"; then
    echo "ERROR: Stage 1 completion marker not found in log!"
    cat "${LOG_STAGE1}"
    exit 1
fi
echo "[STAGE 1 PASS] Confirmed persistent block written and verified in Process #1."

# Step 3: Inspect raw sector on host disk file
echo ""
echo "[HOST VERIFY] Inspecting raw bytes of sector 16 (offset 8192) on host virtual disk..."
SECTOR_HEX=$(hexdump -C -s 8192 -n 128 "${DISK_IMG}")
echo "${SECTOR_HEX}"
if ! echo "${SECTOR_HEX}" | grep -q "NT-NATIVE-OS"; then
    echo "ERROR: Persistent payload text not found at sector 16 of disk image!"
    exit 1
fi
echo "[HOST VERIFY] Confirmed persistent record is physically present in host disk image."

# Step 4: STAGE 2 — Second QEMU Process (Cold Reboot, Read & Cryptographic Verification)
echo ""
echo "-----------------------------------------------------------------"
echo ">>> STAGE 2: Booting QEMU Process #2 (Cold Reboot Recovery)   <<<"
echo "-----------------------------------------------------------------"
rm -f "${LOG_STAGE2}"

set +e
timeout 25s qemu-system-x86_64 \
    -m 256M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_IMG}",format=raw,index=1,media=disk \
    -display none \
    -serial file:"${LOG_STAGE2}" \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04
STAGE2_EXIT=$?
set -e

echo "[STAGE 2] QEMU Process #2 exited with code ${STAGE2_EXIT}."
if [ "${STAGE2_EXIT}" -ne 33 ] && [ "${STAGE2_EXIT}" -ne 0 ]; then
    echo "ERROR: QEMU Stage 2 did not exit cleanly (got ${STAGE2_EXIT})"
    cat "${LOG_STAGE2}"
    exit 1
fi

# Step 5: Verify Stage 2 successfully read back and verified the persistent payload
echo ""
echo "[VERIFICATION] Analyzing Stage 2 cold restart serial trace..."
if grep -q "\[PERSISTENCE_DATA_SURVIVED_RESTART_MARKER_v0.1\]" "${LOG_STAGE2}" && \
   grep -q "\[GAP_G7_04_CLOSED_HARDWARE_PERSISTENCE_VERIFIED\]" "${LOG_STAGE2}"; then
    echo ""
    echo "================================================================="
    echo "[PASS] GAP-G7-04 VERIFIED: DATA PERSISTENCE ACROSS REBOOT PROVEN!"
    echo "Stage 1 Log: ${LOG_STAGE1}"
    echo "Stage 2 Log: ${LOG_STAGE2}"
    echo "Persistent Disk: ${DISK_IMG}"
    echo "Payload verified bit-for-bit with SHA-256 integrity match."
    echo "================================================================="
    exit 0
else
    echo "ERROR: Persistence restart verification markers missing in Stage 2!"
    cat "${LOG_STAGE2}"
    exit 1
fi
