# scripts/verify_repo.ps1 - Windows PowerShell wrapper for structural verification
Write-Host "Running Intent-Native OS structural verification..." -ForegroundColor Cyan

$wslPath = "."
wsl -e bash -c "cd '$wslPath' && dune exec scripts/verify_repo.exe"

if ($LASTEXITCODE -eq 0) {
    Write-Host "Structural verification passed!" -ForegroundColor Green
} else {
    Write-Host "Structural verification failed with code $LASTEXITCODE" -ForegroundColor Red
    exit 1
}
