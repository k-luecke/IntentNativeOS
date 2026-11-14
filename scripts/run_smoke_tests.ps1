# scripts/run_smoke_tests.ps1 - Run smoke tests and record evidence
Write-Host "=== Running Intent-Native OS Smoke Tests ===" -ForegroundColor Cyan

$wslPath = "."

# 1. Structural check
Write-Host "[1/3] Verifying monorepo structure..." -ForegroundColor Yellow
wsl -e bash -c "cd '$wslPath' && dune exec scripts/verify_repo.exe"
if ($LASTEXITCODE -ne 0) {
    Write-Host "Structural verification failed!" -ForegroundColor Red
    exit 1
}

# 2. Build targets
Write-Host "[2/3] Building all targets with Dune..." -ForegroundColor Yellow
wsl -e bash -c "cd '$wslPath' && dune build @all"
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

# 3. Run test suite
Write-Host "[3/3] Running tests with Dune..." -ForegroundColor Yellow
wsl -e bash -c "cd '$wslPath' && dune runtest"
if ($LASTEXITCODE -ne 0) {
    Write-Host "Tests failed!" -ForegroundColor Red
    exit 1
}

Write-Host "=== All smoke tests passed successfully! ===" -ForegroundColor Green
