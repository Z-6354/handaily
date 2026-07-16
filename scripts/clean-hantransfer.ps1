# hantransfer build artifacts & release cleanup
# Usage:
#   .\scripts\clean-hantransfer.ps1
#   .\scripts\clean-hantransfer.ps1 -KeepBuilds 2

param(
    [int]$KeepBuilds = 1
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding

$Root = Get-RepoRoot
$Release = Join-Path $Root "hantransfer\release"
$Target = Join-Path $Root "hantransfer\android-maven\target"
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

Write-Host "hantransfer cleanup" -ForegroundColor Cyan
Write-Host ""

function Stop-StaleBuildProcesses {
    $patterns = @(
        "hantransfer\android-maven",
        "build-hantransfer-apk.ps1",
        "hantransfer:apk"
    )
    $killed = 0
    Get-CimInstance Win32_Process -Filter "Name='java.exe'" -ErrorAction SilentlyContinue | ForEach-Object {
        $cmd = $_.CommandLine
        if (-not $cmd) { return }
        foreach ($pattern in $patterns) {
            if ($cmd -like "*$pattern*") {
                Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
                $killed += 1
                break
            }
        }
    }
    if ($killed -gt 0) {
        Write-Host "Stopped $killed stale Maven build process(es)" -ForegroundColor Yellow
        Start-Sleep -Seconds 1
    }
}

Stop-StaleBuildProcesses

if (Test-Path $Release) {
    $keepNames = @("hantransfer-latest-debug.apk", "latest.json", "hantransfer-desktop-0.1.0.exe", "start-console.bat", "start-tray.bat")
    Get-ChildItem $Release -Filter "hantransfer-*-build*-debug.apk" -File |
        Sort-Object LastWriteTime -Descending |
        Select-Object -Skip $KeepBuilds |
        ForEach-Object { Remove-FileIfExists -Path $_.FullName -Label "old apk" }

    Remove-FileIfExists -Path (Join-Path $Release "classes.dex") -Label "stray dex"
    Remove-FileIfExists -Path (Join-Path $Release "mvn-build.log") -Label "build log"
    Remove-FileIfExists -Path (Join-Path $Release "hantransfer-0.1.0-debug.apk") -Label "legacy apk"
}

Remove-TreeIfExists -Path $Target -Label "maven target"

Write-Host ""
Write-Host "Done. Freed about $freedMb MB." -ForegroundColor Green
