# Project cleanup: logs, temp, stale caches, optional debug / full cargo clean
# Usage:
#   .\scripts\clean-project.ps1
#   .\scripts\clean-project.ps1 -Debug
#   .\scripts\clean-project.ps1 -All

param(
    [switch]$Debug,
    [switch]$All
)

. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding

$Root = Get-ProjectRoot
$Target = Get-CargoTargetDir
$freedMb = 0.0

function Get-TreeSizeMb {
    param([string]$Path)
    if (-not (Test-Path $Path)) { return 0.0 }
    return [math]::Round(((Get-ChildItem $Path -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum / 1MB), 1)
}

function Remove-TreeIfExists {
    param([string]$Path, [string]$Label)
    if (-not (Test-Path $Path)) { return }
    $mb = Get-TreeSizeMb $Path
    Write-Host "Remove $Label ($mb MB): $Path" -ForegroundColor Yellow
    Remove-Item $Path -Recurse -Force -ErrorAction Stop
    $script:freedMb += $mb
}

function Remove-FileIfExists {
    param([string]$Path, [string]$Label)
    if (-not (Test-Path $Path)) { return }
    $mb = [math]::Round((Get-Item $Path).Length / 1MB, 2)
    Write-Host "Remove $Label ($mb MB): $Path" -ForegroundColor Yellow
    Remove-Item $Path -Force -ErrorAction Stop
    $script:freedMb += $mb
}

Write-Host "HANDAILY project cleanup" -ForegroundColor Cyan
Write-Host "Target: $Target" -ForegroundColor DarkGray
Write-Host ""

if ($All -or $Debug) {
    if (Get-Process -Name 'xiaohan-daily' -ErrorAction SilentlyContinue) {
        Write-Host "Close xiaohan-daily before -Debug / -All" -ForegroundColor Red
        exit 1
    }
}

if ($All) {
    Set-Location $Root
    Write-Host "cargo clean..." -ForegroundColor Yellow
    cargo clean --manifest-path src-tauri/Cargo.toml
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host "cargo clean done." -ForegroundColor Green
}

& "$PSScriptRoot\clean-stale-target.ps1" | Out-Null
Remove-TreeIfExists -Path (Join-Path $Target "release-fast") -Label "release-fast"

Get-ChildItem $Root -Filter "*.log" -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
    Remove-FileIfExists -Path $_.FullName -Label "log"
}

Get-ChildItem $Root -Filter "tmp-*" -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
    Remove-FileIfExists -Path $_.FullName -Label "temp"
}
Get-ChildItem $Root -Filter "*.tmp" -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
    Remove-FileIfExists -Path $_.FullName -Label "temp"
}

Get-ChildItem $Root -Filter "*.tsbuildinfo" -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
    Remove-FileIfExists -Path $_.FullName -Label "tsbuildinfo"
}
Remove-FileIfExists -Path (Join-Path $Root "vite.config.d.ts") -Label "vite d.ts"
Remove-TreeIfExists -Path (Join-Path $Root "node_modules\.vite") -Label "vite cache"

Remove-TreeIfExists -Path (Join-Path $Root "src-tauri\gen") -Label "tauri gen"
Remove-TreeIfExists -Path (Join-Path $Root "src-tauri\WixTools") -Label "WixTools"
Remove-TreeIfExists -Path (Join-Path $Root ".iterative-hardening") -Label "hardening session"

if ($Debug -and -not $All) {
    Remove-TreeIfExists -Path (Join-Path $Target "debug") -Label "debug"
}

Write-Host ""
Write-Host "Done. Freed about $freedMb MB (-All cargo clean not counted)." -ForegroundColor Green
if (-not $Debug -and -not $All) {
    Write-Host "More: npm run clean:debug  or  npm run clean:all" -ForegroundColor DarkGray
}
