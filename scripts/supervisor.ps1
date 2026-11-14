# scripts/supervisor.ps1 - Windows Operator CLI for the Autonomous Engineering Supervisor
param (
    [Parameter(Position=0)]
    [ValidateSet("status", "run", "report", "test", "help")]
    [string]$Command = "status"
)

$wslPath = "."

switch ($Command) {
    "status" {
        wsl -e bash -c "cd '$wslPath' && dune exec tools/supervisor/main.exe status"
    }
    "run" {
        Write-Host "Starting Autonomous Engineering Supervisor..." -ForegroundColor Cyan
        wsl -e bash -c "cd '$wslPath' && dune exec tools/supervisor/main.exe run"
    }
    "report" {
        wsl -e bash -c "cd '$wslPath' && dune exec tools/supervisor/main.exe report"
    }
    "test" {
        Write-Host "Running Supervisor Acceptance Test..." -ForegroundColor Cyan
        wsl -e bash -c "cd '$wslPath' && dune exec tools/supervisor/main.exe acceptance-test"
    }
    Default {
        Write-Host "Usage: .\scripts\supervisor.ps1 [status | run | report | test]"
    }
}
