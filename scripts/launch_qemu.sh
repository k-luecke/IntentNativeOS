#!/usr/bin/env bash
# scripts/launch_qemu.sh - Launches INOS kernel under QEMU x86-64 with OVMF UEFI firmware

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"

if [ ! -f "${ESP_DIR}/EFI/BOOT/BOOTX64.EFI" ]; then
    echo "Error: Boot image not found. Run scripts/build_kernel.sh first."
    exit 1
fi

qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${ROOT_DIR}/models/pretrained/stories15M.bin",format=raw,if=ide,index=2,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    "$@"
