#!/usr/bin/env bash
# scripts/launch_interactive.sh - Launches INOS kernel with graphical display and persistent disk
# Supports VNC (port 5900) for native remote/local desktop interaction.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
DISK_IMG="${ROOT_DIR}/build/persistent_disk.img"

if [ ! -f "${ESP_DIR}/EFI/BOOT/BOOTX64.EFI" ]; then
    echo "Error: Boot image not found. Running build_kernel.sh..."
    bash "${SCRIPT_DIR}/build_kernel.sh"
fi

if [ ! -f "${DISK_IMG}" ]; then
    echo "Initializing 16 MB raw persistent disk: ${DISK_IMG}..."
    dd if=/dev/zero of="${DISK_IMG}" bs=1M count=16 status=none
fi

echo "======================================================================"
echo "=== INTENT-NATIVE OPERATING SYSTEM (INOS) INTERACTIVE RUNTIME     ==="
echo "======================================================================"
echo "Display:   VNC server active at 0.0.0.0:5900 (Display :0)"
echo "Connect:   Open any VNC client (e.g. RealVNC, TightVNC) -> 127.0.0.1:5900"
echo "Serial:    Interactive console attached to stdio"
echo "Storage:   16 MB Persistent ATA Disk mounted (/build/persistent_disk.img)"
echo "Press Ctrl+C to stop VM."
echo "======================================================================"

qemu-system-x86_64 \
    -m 256M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_IMG}",format=raw,index=1,media=disk \
    -vnc 0.0.0.0:0 \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    "$@"
