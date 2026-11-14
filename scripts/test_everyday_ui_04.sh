#!/usr/bin/env bash
# scripts/test_everyday_ui_04.sh - Acceptance suite for Gate UI-04: Everyday Computing
# Verifies:
# 1. UI-04.1 (P0): Typed WindowApp/Compositor boundary separating document state from chrome
# 2. UI-04.2 (P0): Multiline editor, UTF-8 boundary navigation, range selection
# 3. UI-04.3 (P1): Native in-guest clipboard (copy/paste), vertical scrolling, reliable dirty save
# 4. UI-04.4 (P1): Native document library and launcher (create, open, browse, rename via INOS-FS)
# 5. UI-04.5 (P1): Persistent workspace restoration across cold reboot via versioned .workspace

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVMF_CODE="${ROOT_DIR}/tools/firmware/OVMF_CODE.fd"
OVMF_VARS="${ROOT_DIR}/tools/firmware/OVMF_VARS.fd"
ESP_DIR="${ROOT_DIR}/build/esp"
BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
DISK_UI04="${BUILD_DIR}/disk_ui04.img"

LOG_STAGE1="${EVIDENCE_DIR}/ui_04_stage1_serial.log"
LOG_STAGE2="${EVIDENCE_DIR}/ui_04_stage2_serial.log"
LOG_STAGE3="${EVIDENCE_DIR}/ui_04_stage3_serial.log"

CONVERTER="${ROOT_DIR}/tools/ppm2bmp/target/release/ppm2bmp"
AUDITOR="${ROOT_DIR}/tools/contrast_audit_ui04"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}"

echo "=========================================================================="
echo "=== [GATE UI-04] EVERYDAY COMPUTING ACCEPTANCE SUITE                   ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Ensuring tools and kernel are freshly compiled..."
if [ ! -f "${CONVERTER}" ]; then
    echo "  -> Compiling native Rust ppm2bmp tool..."
    (cd "${ROOT_DIR}/tools/ppm2bmp" && cargo build --release)
fi

if [ ! -f "${AUDITOR}" ]; then
    echo "  -> Compiling UI-04 contrast auditor..."
    gcc -O2 -Wall "${ROOT_DIR}/tools/contrast_audit_ui04.c" -lm -o "${AUDITOR}"
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
# PHASE 1 - STAGE 1: Multiline Editor, Selection & In-Guest Clipboard
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Stage 1 Setup, Multiline Editor & In-Guest Clipboard Copy <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
echo "[STORAGE] Initializing fresh 16 MB raw ATA disk for UI-04 test..."
dd if=/dev/zero of="${DISK_UI04}" bs=1M count=16 status=none

rm -f "${LOG_STAGE1}" /tmp/ui04_mon1.sock /tmp/ui_04_1_raw.ppm
MON_SOCK1="/tmp/ui04_mon1.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_UI04}",format=raw,index=1,media=disk \
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

    # 1. Wait for document name prompt and send "research.txt"
    for _ in {1..500}; do
        if grep -q "\[G9_AWAITING_DOC_NAME\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[INPUT HW] Dispatching hardware keypresses for 'research.txt'..."
            (for key in r e s e a r c h dot t x t ret; do echo "sendkey ${key}"; sleep 0.01; done) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            break
        fi
        sleep 0.1
    done

    # 2. Wait for Screendump 1 (Multiline Editor with Selection)
    for _ in {1..300}; do
        if grep -q "\[UI_04_SCREEN1_MULTILINE_SELECTION_RENDERED\]" "${LOG_STAGE1}" 2>/dev/null; then
            echo "[SCREEN 1] Capturing Screendump #1 (Multiline Editor with Selection Highlight)..."
            (echo "screendump /tmp/ui_04_1_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK1}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID1}" || true
rm -f "${MON_SOCK1}"

convert_screenshot "/tmp/ui_04_1_raw.ppm" "ui_04_1_multiline_selection"

echo "[VERIFY] Checking UI-04 Stage 1 Serial Invariants..."
grep -q "\[UI_04_MULTILINE_EDITOR_ACTIVE\]" "${LOG_STAGE1}"
grep -q "\[UI_04_SELECTION_COPIED_TO_CLIPBOARD\]" "${LOG_STAGE1}"
grep -q "\[UI_04_STAGE1_SAVED_OK\]" "${LOG_STAGE1}"
echo "  [PASS] Stage 1 Verified: Multiline editor mounted, selection highlighted, copied to clipboard."

# =========================================================================
# PHASE 2 - STAGE 2: Clipboard Paste, Scrolling, Rename & Workspace Persist
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Stage 2 Paste, Scrolling, Document Rename & Workspace Save <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_STAGE2}" /tmp/ui04_mon2.sock /tmp/ui_04_2_raw.ppm /tmp/ui_04_3_raw.ppm
MON_SOCK2="/tmp/ui04_mon2.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_UI04}",format=raw,index=1,media=disk \
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

    # 1. Capture Screen 2: Pasted into notes.txt and scrolled research.txt
    for _ in {1..500}; do
        if grep -q "\[UI_04_SCREEN2_PASTED_SCROLLED_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 2] Capturing Screendump #2 (Clipboard Pasted & Multiline Scrolled)..."
            (echo "screendump /tmp/ui_04_2_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done

    # 2. Capture Screen 3: Document Library & Circular Launcher
    for _ in {1..300}; do
        if grep -q "\[UI_04_SCREEN3_DOCUMENT_LIBRARY_RENDERED\]" "${LOG_STAGE2}" 2>/dev/null; then
            echo "[SCREEN 3] Capturing Screendump #3 (Document Library & Launcher Menu)..."
            (echo "screendump /tmp/ui_04_3_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK2}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID2}" || true
rm -f "${MON_SOCK2}"

convert_screenshot "/tmp/ui_04_2_raw.ppm" "ui_04_2_clipboard_pasted_scrolled"
convert_screenshot "/tmp/ui_04_3_raw.ppm" "ui_04_3_document_library_launcher"

echo "[VERIFY] Checking UI-04 Stage 2 Serial Invariants..."
grep -q "\[UI_04_WORKSPACE_LOADED_OK\]" "${LOG_STAGE2}"
grep -q "\[UI_04_CLIPBOARD_PASTE_VERIFIED\]" "${LOG_STAGE2}"
grep -q "\[UI_04_MULTILINE_SCROLL_VERIFIED\]" "${LOG_STAGE2}"
grep -q "\[UI_04_DOCUMENT_RENAMED_VERIFIED\]" "${LOG_STAGE2}"
grep -q "\[UI_04_LAUNCHER_LIBRARY_UPDATED\]" "${LOG_STAGE2}"
grep -q "\[UI_04_WORKSPACE_SAVED_VERIFIED\]" "${LOG_STAGE2}"
echo "  [PASS] Stage 2 Verified: Clipboard paste, vertical scrolling, atomic rename, and workspace saved."

# =========================================================================
# PHASE 3 - STAGE 3: Cold Reboot Restoration Verification
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: Stage 3 Cold Reboot Restoration Verification              <<<"
echo "--------------------------------------------------------------------------"

git checkout "${OVMF_VARS}" 2>/dev/null || true
rm -f "${LOG_STAGE3}" /tmp/ui04_mon3.sock /tmp/ui_04_4_raw.ppm
MON_SOCK3="/tmp/ui04_mon3.sock"

stdbuf -oL -eL qemu-system-x86_64 \
    -m 1024M \
    -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -drive file="fat:rw:${ESP_DIR}",format=raw \
    -drive file="${DISK_UI04}",format=raw,index=1,media=disk \
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

    # Capture Screen 4: Cold Reboot Restored Desktop
    for _ in {1..300}; do
        if grep -q "\[UI_04_SCREEN4_COLD_REBOOT_RESTORED_RENDERED\]" "${LOG_STAGE3}" 2>/dev/null; then
            echo "[SCREEN 4] Capturing Screendump #4 (Cold Reboot Workspace Completely Restored)..."
            (echo "screendump /tmp/ui_04_4_raw.ppm"; sleep 0.8) | socat - UNIX-CONNECT:"${MON_SOCK3}" 2>/dev/null || true
            sleep 0.5
            break
        fi
        sleep 0.1
    done
) &

wait "${QEMU_PID3}" || true
rm -f "${MON_SOCK3}"

convert_screenshot "/tmp/ui_04_4_raw.ppm" "ui_04_4_cold_reboot_workspace_restored"

echo "[VERIFY] Checking UI-04 Stage 3 Serial Invariants..."
grep -q "\[UI_04_DOCUMENTS_RESTORED_INTACT\]" "${LOG_STAGE3}"
grep -q "\[UI_04_WORKSPACE_CHECKSUM_VERIFIED\]" "${LOG_STAGE3}"
grep -q "\[UI_04_WINDOW_TITLES_VERIFIED\]" "${LOG_STAGE3}"
grep -q "\[UI_04_SCROLL_AND_CURSOR_RESTORED\]" "${LOG_STAGE3}"
grep -q "\[UI_04_COLD_REBOOT_WORKSPACE_VERIFIED\]" "${LOG_STAGE3}"
grep -q "\[UI_04_EVERYDAY_COMPUTING_ALL_STAGES_VERIFIED_MARKER\]" "${LOG_STAGE3}"
echo "  [PASS] Stage 3 Verified: Full workspace layout, document contents, and scroll states restored."

# =========================================================================
# PHASE 4: Contrast & Typography Hierarchy Audit
# =========================================================================
echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 4: Contrast & Typography Hierarchy Verification (WCAG AA/AAA) <<<"
echo "--------------------------------------------------------------------------"

"${AUDITOR}" "${EVIDENCE_DIR}/ui_04_1_multiline_selection.bmp"
"${AUDITOR}" "${EVIDENCE_DIR}/ui_04_4_cold_reboot_workspace_restored.bmp"

echo ""
echo "=========================================================================="
echo "=== [GATE UI-04: EVERYDAY COMPUTING COMPLETE AND ACCEPTED]            ==="
echo "=========================================================================="
