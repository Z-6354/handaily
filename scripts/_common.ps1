# Shared helpers for project scripts

$script:DevPort = 1420

function Initialize-ScriptEncoding {
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    $script:OutputEncoding = [System.Text.Encoding]::UTF8
}

function Get-ProjectRoot {
    Split-Path -Parent $PSScriptRoot
}

function Get-CargoTargetDir {
    Join-Path (Get-ProjectRoot) "src-tauri\target"
}

function Initialize-RustBuildEnv {
    $jobs = [Environment]::ProcessorCount
    if ($jobs -lt 1) { $jobs = 4 }
    $env:CARGO_BUILD_JOBS = "$jobs"
    $env:CARGO_INCREMENTAL = "1"
    if (Get-Command sccache -ErrorAction SilentlyContinue) {
        $env:RUSTC_WRAPPER = "sccache"
    }
}

function Initialize-MsvcEnvironment {
    if (Get-Command rc.exe -ErrorAction SilentlyContinue) { return $true }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
        $devShell = if ($vsPath) { Join-Path $vsPath "Common7\Tools\Launch-VsDevShell.ps1" } else { $null }
        if ($devShell -and (Test-Path $devShell)) {
            Write-Host "Loading VS build environment..." -ForegroundColor DarkGray
            & $devShell -Arch amd64 -SkipAutomaticLocation *> $null
            if (Get-Command rc.exe -ErrorAction SilentlyContinue) { return $true }
        }
    }
    $kitsBin = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    if (Test-Path $kitsBin) {
        $rc = Get-ChildItem -Path $kitsBin -Filter rc.exe -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.DirectoryName -match '\\x64\\' } | Sort-Object FullName -Descending | Select-Object -First 1
        if ($rc) {
            $env:PATH = "$($rc.DirectoryName);$env:PATH"
            $env:RC = $rc.FullName
            return $true
        }
    }
    Write-Host "Warning: RC.EXE not found" -ForegroundColor Yellow
    return $false
}

function Get-DevExePath { Join-Path (Get-ProjectRoot) "src-tauri\target\debug\xiaohan-daily.exe" }

function Get-PortListeners {
    param([Parameter(Mandatory = $true)][int]$Port)
    $listeners = @{}
    try {
        Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue | ForEach-Object {
            $procId = [int]$_.OwningProcess
            if ($procId -gt 0) { $listeners[$procId] = $true }
        }
    } catch {
        netstat -ano | Select-String ":$Port\s+.*LISTENING" | ForEach-Object {
            $parts = ($_ -split '\s+') | Where-Object { $_ -ne '' }
            $procId = [int]$parts[-1]
            if ($procId -gt 0) { $listeners[$procId] = $true }
        }
    }
    foreach ($procId in @($listeners.Keys)) {
        $proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
        [PSCustomObject]@{ Pid = $procId; Name = if ($proc) { $proc.ProcessName } else { 'unknown' }; Path = if ($proc) { $proc.Path } else { '' } }
    }
}

function Test-PortOccupied { param([int]$Port = $script:DevPort); return (@(Get-PortListeners -Port $Port)).Count -gt 0 }

function Test-ExeLocked {
    param([string]$ExePath = (Get-DevExePath))
    if (-not (Test-Path $ExePath)) { return $false }
    try { $fs = [System.IO.File]::Open($ExePath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None); $fs.Close(); return $false } catch { return $true }
}

function Stop-XiaohanProcesses {
    $killed = @()
    Get-Process -Name 'xiaohan-daily' -ErrorAction SilentlyContinue | ForEach-Object {
        Write-Host "Stopping xiaohan-daily (PID $($_.Id))..." -ForegroundColor Yellow
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        $killed += $_.Id
    }
    if ($killed.Count -eq 0) { return 0 }
    $exePath = Get-DevExePath
    for ($i = 1; $i -le 8; $i++) {
        if (-not (Test-ExeLocked -ExePath $exePath)) { return $killed.Count }
        Get-Process -Name 'xiaohan-daily' -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Seconds 1
    }
    Write-Host "Warning: xiaohan-daily.exe still locked" -ForegroundColor Red
    return $killed.Count
}

function Stop-PortOccupiers {
    param([int]$Port = $script:DevPort, [int]$MaxWaitSec = 8)
    $listeners = @(Get-PortListeners -Port $Port)
    if ($listeners.Count -eq 0) { return 0 }
    foreach ($item in $listeners) { Stop-Process -Id $item.Pid -Force -ErrorAction SilentlyContinue }
    Stop-XiaohanProcesses | Out-Null
    for ($i = 1; $i -le $MaxWaitSec; $i++) {
        if (-not (Test-PortOccupied -Port $Port)) { return $listeners.Count }
        Start-Sleep -Seconds 1
    }
    exit 1
}

function Ensure-DevEnvironmentClean {
    param([switch]$NoKill)
    if ($NoKill) {
        if ((Get-Process -Name 'xiaohan-daily' -ErrorAction SilentlyContinue) -or (Test-PortOccupied)) { exit 1 }
        return
    }
    if (Get-Process -Name 'xiaohan-daily' -ErrorAction SilentlyContinue) { Stop-XiaohanProcesses | Out-Null }
    elseif (Test-ExeLocked) { Stop-XiaohanProcesses | Out-Null }
    if (Test-PortOccupied) { Stop-PortOccupiers | Out-Null }
}

function Stop-XiaohanDev {
    Stop-XiaohanProcesses | Out-Null
    if (Test-PortOccupied) { Stop-PortOccupiers | Out-Null }
}

function Invoke-NpmCli {
    param([Parameter(Mandatory = $true, Position = 0)][string]$Command, [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
    if ($Rest -and $Rest.Count -gt 0) { npm $Command @Rest } else { npm $Command }
    return $LASTEXITCODE
}

function Invoke-TauriDev {
    param([switch]$NoKill)
    Initialize-MsvcEnvironment | Out-Null
    $code = Invoke-NpmCli run tauri:dev
    if ($code -eq 101 -and -not $NoKill) {
        Ensure-DevEnvironmentClean
        $code = Invoke-NpmCli run tauri:dev
    }
    if ($code -ne 0) { exit $code }
}