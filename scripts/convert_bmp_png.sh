#!/usr/bin/env bash
set -euo pipefail
BMP="$1"
PNG="${BMP%.*}.png"
WIN_BMP=$(wslpath -w "$BMP")
WIN_PNG=$(wslpath -w "$PNG")
powershell.exe -NoProfile -Command "Add-Type -AssemblyName System.Drawing; \$bmp = [System.Drawing.Image]::FromFile('${WIN_BMP}'); \$bmp.Save('${WIN_PNG}', [System.Drawing.Imaging.ImageFormat]::Png); \$bmp.Dispose();"
