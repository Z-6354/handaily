# Merge repo-root live2d/ into data/live2d/ (per slug subfolder)
# Usage:
#   .\scripts\migrate-live2d.ps1           # preview
#   .\scripts\migrate-live2d.ps1 -Apply    # run migration

param(
    [switch]$Apply
)

. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding

$Repo = Get-RepoRoot
$Legacy = Join-Path $Repo "live2d"
$Target = Join-Path $Repo "data\live2d"

if (-not (Test-Path $Legacy)) {
    Write-Host "No legacy live2d/ directory." -ForegroundColor Green
    exit 0
}

$legacyDirs = @(Get-ChildItem $Legacy -Directory -ErrorAction SilentlyContinue)
if ($legacyDirs.Count -eq 0) {
    Write-Host "live2d/ is empty." -ForegroundColor Green
    exit 0
}

New-Item -ItemType Directory -Force -Path $Target | Out-Null

$conflicts = @()
$toMove = @()
foreach ($dir in $legacyDirs) {
    $dest = Join-Path $Target $dir.Name
    if (Test-Path $dest) {
        $conflicts += $dir.Name
    } else {
        $toMove += $dir
    }
}

Write-Host "legacy live2d/: $($legacyDirs.Count) slug folders" -ForegroundColor Cyan
Write-Host "target: $Target" -ForegroundColor DarkGray
Write-Host "movable: $($toMove.Count)" -ForegroundColor Green
if ($conflicts.Count -gt 0) {
    Write-Host "conflicts (skip): $($conflicts.Count)" -ForegroundColor Yellow
    $conflicts | Select-Object -First 10 | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
    if ($conflicts.Count -gt 10) { Write-Host "  ..." -ForegroundColor DarkGray }
}

if (-not $Apply) {
    Write-Host ""
    Write-Host "Preview only. Run: .\scripts\migrate-live2d.ps1 -Apply" -ForegroundColor Cyan
    exit 0
}

$moved = 0
foreach ($dir in $toMove) {
    $dest = Join-Path $Target $dir.Name
    Move-Item -Path $dir.FullName -Destination $dest
    $moved++
}

$remaining = @(Get-ChildItem $Legacy -Force -ErrorAction SilentlyContinue)
if ($remaining.Count -eq 0) {
    Remove-Item $Legacy -Force -ErrorAction SilentlyContinue
    Write-Host "Removed empty live2d/" -ForegroundColor DarkGray
}

Write-Host "Done. Moved $moved folders to data/live2d/." -ForegroundColor Green
if ($conflicts.Count -gt 0) {
    Write-Host "Resolve $($conflicts.Count) conflicts manually, then remove live2d/." -ForegroundColor Yellow
}