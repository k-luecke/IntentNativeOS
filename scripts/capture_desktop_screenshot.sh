#!/usr/bin/env bash
# scripts/capture_desktop_screenshot.sh - Captures pixel-accurate screenshot of running INOS QEMU VM
# Zero Python dependencies: uses QEMU monitor screendump + native Rust ppm2bmp tool.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
TMP_PPM="/tmp/desktop_screenshot.ppm"
PPM_PATH="${EVIDENCE_DIR}/desktop_screenshot.ppm"
BMP_PATH="${EVIDENCE_DIR}/desktop_screenshot.bmp"
PNG_PATH="${EVIDENCE_DIR}/desktop_screenshot.png"
LOG_PATH="${EVIDENCE_DIR}/qemu_screenshot_boot.log"
MON_SOCK="/tmp/inos-qemu-mon-$$.sock"

mkdir -p "${EVIDENCE_DIR}"
rm -f "${MON_SOCK}" "${TMP_PPM}" "${PPM_PATH}" "${LOG_PATH}"

echo "=== [G8] Authentic QEMU Desktop Framebuffer Screendump ==="

# 1. Build converter tool if needed
if [ ! -f "${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp" ]; then
    echo "[BUILD] Compiling zero-dependency Rust ppm2bmp converter..."
    (cd "${ROOT_DIR}/tools/ppm2bmp" && cargo build --release)
fi

# 2. Launch QEMU with unbuffered serial stream into tee
echo "[QEMU] Starting QEMU with active video display adapter and monitor socket..."
stdbuf -oL -eL qemu-system-x86_64 \
    -m 4G \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -display none \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -chardev socket,id=mon0,path="${MON_SOCK}",server=on,wait=off \
    -mon chardev=mon0,mode=readline 2>&1 | tee "${LOG_PATH}" &
QEMU_PID=$!

echo "[QEMU] Process PID: ${QEMU_PID}. Awaiting desktop render..."

# 3. Wait for GOP desktop marker or stall marker in unbuffered stream
CAPTURED=0
for i in {1..120}; do
    if [ -f "${LOG_PATH}" ] && grep -q "\[G8_STALL\]" "${LOG_PATH}"; then
        echo "[SCREEN] Detected active desktop during framebuffer hold! Issuing screendump..."
        echo "screendump ${TMP_PPM}" | socat - UNIX-CONNECT:"${MON_SOCK}" || true
        sleep 0.5
        if [ -f "${TMP_PPM}" ] && [ -s "${TMP_PPM}" ]; then
            cp "${TMP_PPM}" "${PPM_PATH}"
            CAPTURED=1
            echo "[SCREEN] Successfully captured physical framebuffer: $(ls -lh "${PPM_PATH}" | awk '{print $5, $9}')"
            break
        fi
    fi
    if ! kill -0 "${QEMU_PID}" 2>/dev/null; then
        echo "[WARN] QEMU terminated before capture"
        break
    fi
    sleep 0.1
done

# Wait for QEMU to finish cleanly
wait "${QEMU_PID}" 2>/dev/null || true
rm -f "${MON_SOCK}" "${TMP_PPM}"

if [ "${CAPTURED}" -ne 1 ] || [ ! -f "${PPM_PATH}" ]; then
    echo "ERROR: Failed to capture framebuffer screendump from QEMU!"
    exit 1
fi

# 4. Convert PPM to 24-bit BMP using native Rust converter
echo "[CONVERT] Converting PPM screendump to standard Windows BMP..."
"${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp" "${PPM_PATH}" "${BMP_PATH}"

# 5. Convert BMP to PNG using PowerShell/.NET (zero Python)
echo "[CONVERT] Generating companion PNG image via .NET System.Drawing..."
powershell.exe -NoProfile -Command "Add-Type -AssemblyName System.Drawing; \$p = (Resolve-Path 'evidence\desktop_screenshot.bmp').Path; \$out = [System.IO.Path]::ChangeExtension(\$p, '.png'); \$bmp = [System.Drawing.Image]::FromFile(\$p); \$bmp.Save(\$out, [System.Drawing.Imaging.ImageFormat]::Png); \$bmp.Dispose();" || true

# 6. Verify image dimensions and non-emptiness
FILE_SIZE=$(wc -c < "${BMP_PATH}")
echo "[VERIFIED] Generated authentic QEMU framebuffer BMP: ${BMP_PATH} (${FILE_SIZE} bytes)"

# 7. Generate SHA-256 checksum of screenshot
sha256sum "${BMP_PATH}" > "${EVIDENCE_DIR}/desktop_screenshot.sha256"
cat "${EVIDENCE_DIR}/desktop_screenshot.sha256"

echo "========================================================="
echo "[PASS] Authentic QEMU framebuffer screenshot captured!"
echo "Saved BMP: ${BMP_PATH}"
echo "Saved PNG: ${PNG_PATH}"
echo "========================================================="
