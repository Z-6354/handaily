# Remove stale HANDAILY/target at repo root (canonical: hanpet/src-tauri/target)
. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding
$Root = Get-ProjectRoot
$StaleTarget = Join-Path $Root "target"
$Canonical = Get-CargoTargetDir
if (-not (Test-Path $StaleTarget)) {
    Write-Host "No stale root target/. Canonical: $Canonical" -ForegroundColor Green
    exit 0
}
$sizeMb = [math]::Round(((Get-ChildItem $StaleTarget -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum / 1MB), 2)
Write-Host "Remove stale: $StaleTarget (${sizeMb} MB)" -ForegroundColor Yellow
Write-Host "Keep: $Canonical" -ForegroundColor DarkGray
Remove-Item $StaleTarget -Recurse -Force -ErrorAction Stop
Write-Host "Done." -ForegroundColor Green