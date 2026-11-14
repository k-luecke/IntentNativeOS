#!/usr/bin/env bash
# scripts/benchmark_model_transport.sh - Standalone Model Transport Benchmark
# Compares Batched ATA PIO vs Legacy VirtIO-blk DMA under identical QEMU TCG conditions.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
WEIGHTS="${ROOT_DIR}/models/pretrained/smollm2_135m_q8.bin"
DISK_DUMMY="${BUILD_DIR}/disk_dummy.img"

mkdir -p "${BUILD_DIR}"
dd if=/dev/zero of="${DISK_DUMMY}" bs=1M count=16 status=none

now_ms() {
    echo $(($(date +%s%N) / 1000000))
}

echo "=========================================================================="
echo "=== [STANDALONE BENCHMARK: MODEL WEIGHT INGESTION TRANSPORTS]          ==="
echo "=========================================================================="
echo "Platform: AMD Ryzen 5 5500U | Accelerator: QEMU TCG (Software Emulation)"
echo "Model: SmolLM2-135M-Instruct-Q8 (151,430,428 bytes / 295,763 sectors)"
echo "Target SHA-256: 81ad64afffe84375655ac23c8c924c30b2c474f484104f4feaa248a5d786f727"
echo ""

# --------------------------------------------------------------------------
# BENCHMARK 1: Batched ATA PIO (Secondary IDE Channel 0x170)
# --------------------------------------------------------------------------
echo ">>> [BENCHMARK 1] Measuring Batched ATA PIO Transport (Port 0x170) <<<"
LOG_ATA="/tmp/bench_ata.log"
rm -f "${LOG_ATA}" /tmp/bench_ata.sock
git checkout "${OVMF_VARS}" 2>/dev/null || true

T0_ATA=$(now_ms)
stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_DUMMY}",format=raw,index=1,media=disk \
    -drive file="${WEIGHTS}",format=raw,if=ide,index=2,media=disk \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path=/tmp/bench_ata.sock,server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_ATA}" &
QPID_ATA=$!

(
    for _ in {1..50}; do if [ -S /tmp/bench_ata.sock ]; then break; fi; sleep 0.1; done
    # Once model ingestion completes and awaits intent input, send debug exit
    for _ in {1..3000}; do
        if grep -q "\[F106_MODEL_INGESTION_VERIFIED_MARKER\]" "${LOG_ATA}" 2>/dev/null; then
            echo "quit" | socat - UNIX-CONNECT:/tmp/bench_ata.sock 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &
wait "${QPID_ATA}" || true
T1_ATA=$(now_ms)
D_ATA_TOTAL=$((T1_ATA - T0_ATA))

echo "  -> Batched ATA Wall-Clock Duration: ${D_ATA_TOTAL} ms (~$((D_ATA_TOTAL / 1000))s)"
grep -E "(\[MODEL_TRANSPORT\]|\[PRETRAINED_MODEL\] Streamed|In-guest SHA-256|Bit-for-bit weight integrity)" "${LOG_ATA}" || true
echo ""

# --------------------------------------------------------------------------
# BENCHMARK 2: VirtIO-blk Legacy PCI (DMA via Split VirtQueue)
# --------------------------------------------------------------------------
echo ">>> [BENCHMARK 2] Measuring VirtIO-blk Legacy PCI DMA Transport <<<"
LOG_VIRTIO="/tmp/bench_virtio.log"
rm -f "${LOG_VIRTIO}" /tmp/bench_virtio.sock
git checkout "${OVMF_VARS}" 2>/dev/null || true

T0_VIRTIO=$(now_ms)
stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_DUMMY}",format=raw,index=1,media=disk \
    -drive file="${WEIGHTS}",if=none,id=modeldisk,format=raw \
    -device virtio-blk-pci,drive=modeldisk,disable-legacy=off,disable-modern=on \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path=/tmp/bench_virtio.sock,server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_VIRTIO}" &
QPID_VIRTIO=$!

(
    for _ in {1..50}; do if [ -S /tmp/bench_virtio.sock ]; then break; fi; sleep 0.1; done
    for _ in {1..3000}; do
        if grep -q "\[F106_MODEL_INGESTION_VERIFIED_MARKER\]" "${LOG_VIRTIO}" 2>/dev/null; then
            echo "quit" | socat - UNIX-CONNECT:/tmp/bench_virtio.sock 2>/dev/null || true
            break
        fi
        sleep 0.1
    done
) &
wait "${QPID_VIRTIO}" || true
T1_VIRTIO=$(now_ms)
D_VIRTIO_TOTAL=$((T1_VIRTIO - T0_VIRTIO))

echo "  -> VirtIO-blk DMA Wall-Clock Duration: ${D_VIRTIO_TOTAL} ms (~$((D_VIRTIO_TOTAL / 1000))s)"
grep -E "(\[MODEL_TRANSPORT\]|\[VIRTIO_BLK|\[PRETRAINED_MODEL\] Streamed|In-guest SHA-256|Bit-for-bit weight integrity)" "${LOG_VIRTIO}" || true
echo ""

echo "=========================================================================="
echo "=== [STANDALONE BENCHMARK COMPARISON] ==="
echo "=========================================================================="
echo "1. Batched ATA PIO:  ${D_ATA_TOTAL} ms (~$((D_ATA_TOTAL / 1000))s)"
echo "2. VirtIO-blk DMA:   ${D_VIRTIO_TOTAL} ms (~$((D_VIRTIO_TOTAL / 1000))s)"
if [ "${D_VIRTIO_TOTAL}" -gt 0 ]; then
    SPEEDUP=$((D_ATA_TOTAL * 10 / D_VIRTIO_TOTAL))
    echo "Speedup Factor:      $((SPEEDUP / 10)).$((SPEEDUP % 10))x"
fi
echo "=========================================================================="
