# hantransfer - dev console startup
# Usage: .\scripts\start-hantransfer.ps1

. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding

$Repo = Get-RepoRoot
Set-Location $Repo

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo not found. Install Rust toolchain first."
    exit 1
}

Write-Host "hantransfer desktop startup..." -ForegroundColor Cyan
Write-Host "repo: $Repo" -ForegroundColor DarkGray
Write-Host "  cargo run -p hantransfer-desktop" -ForegroundColor DarkGray
Write-Host "  manage: http://127.0.0.1:7822/" -ForegroundColor DarkGray
Write-Host "  mobile: http://<LAN_IP>:7822/m/" -ForegroundColor DarkGray
Write-Host ""

Initialize-RustBuildEnv
& cargo run -p hantransfer-desktop
exit $LASTEXITCODE