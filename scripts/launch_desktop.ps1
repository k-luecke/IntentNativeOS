# scripts/launch_desktop.ps1 - Windows PowerShell launcher for INOS Interactive Desktop
# Launches QEMU with VNC display enabled on port 5900 and attaches persistent storage.

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir

Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host "=== INTENT-NATIVE OPERATING SYSTEM (INOS) INTERACTIVE RUNTIME     ===" -ForegroundColor Cyan
Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host "Display:   VNC server active at localhost:5900 (Display :0)" -ForegroundColor Green
Write-Host "Connect:   Open any VNC client (TightVNC, UltraVNC, RealVNC) to localhost:5900" -ForegroundColor Yellow
Write-Host "Storage:   16 MB Persistent ATA Disk mounted (build/persistent_disk.img)" -ForegroundColor Green
Write-Host "Press Ctrl+C to terminate the QEMU virtual machine." -ForegroundColor DarkGray
Write-Host "======================================================================" -ForegroundColor Cyan

$WslRoot = (wsl wslpath -u $RootDir).Trim()
wsl -e bash -c "cd '$WslRoot' && bash scripts/launch_interactive.sh"
