#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

gcc "${ROOT_DIR}/tools/font_rasterizer/rasterize.c" $(pkg-config --cflags --libs freetype2) -o "${ROOT_DIR}/tools/font_rasterizer/rasterize"
"${ROOT_DIR}/tools/font_rasterizer/rasterize" "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf" "${ROOT_DIR}/kernel/src/graphics/proportional_font.rs"
echo "[FONT] Proportional font table generated successfully."
