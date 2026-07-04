# 脚本公共函数（由其他 .ps1 dot-source 引入）

$script:DevPort = 1420

function Initialize-ScriptEncoding {
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    $script:OutputEncoding = [System.Text.Encoding]::UTF8
}

function Get-ProjectRoot {
    Split-Path -Parent $PSScriptRoot
}

function Initialize-MsvcEnvironment {
    # Tauri Windows 构建需要 RC.EXE（资源编译器），普通 PowerShell 会话常未配置 PATH
    if (Get-Command rc.exe -ErrorAction SilentlyContinue) {
        return $true
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vsPath = & $vswhere -latest -products * `
            -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
            -property installationPath 2>$null
        $devShell = if ($vsPath) {
            Join-Path $vsPath "Common7\Tools\Launch-VsDevShell.ps1"
        } else { $null }
        if ($devShell -and (Test-Path $devShell)) {
            Write-Host "加载 Visual Studio 编译环境 (RC.EXE)..." -ForegroundColor DarkGray
            & $devShell -Arch amd64 -SkipAutomaticLocation *> $null
            if (Get-Command rc.exe -ErrorAction SilentlyContinue) {
                return $true
            }
        }
    }

    $kitsBin = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    if (Test-Path $kitsBin) {
        $rc = Get-ChildItem -Path $kitsBin -Filter rc.exe -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.DirectoryName -match '\\x64\\' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($rc) {
            $env:PATH = "$($rc.DirectoryName);$env:PATH"
            $env:RC = $rc.FullName
            Write-Host "已加入 Windows SDK rc.exe" -ForegroundColor DarkGray
            return $true
        }
    }

    Write-Host ""
    Write-Host "警告: 未找到 RC.EXE，Rust 编译可能失败" -ForegroundColor Yellow
    Write-Host "  请安装 Visual Studio Build Tools，勾选「使用 C++ 的桌面开发」" -ForegroundColor Yellow
    Write-Host "  下载: https://visualstudio.microsoft.com/visual-cpp-build-tools/" -ForegroundColor DarkGray
    Write-Host ""
    return $false
}

function Get-DevExePath {
    Join-Path (Get-ProjectRoot) "src-tauri\target\debug\xiaohan-daily.exe"
}

function Get-PortListeners {
    param(
        [Parameter(Mandatory = $true)]
        [int]$Port
    )

    $listeners = @{}

    try {
        Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
            ForEach-Object {
                $procId = [int]$_.OwningProcess
                if ($procId -gt 0) { $listeners[$procId] = $true }
            }
    } catch {
        $pattern = ":$Port\s+.*LISTENING"
        netstat -ano | Select-String $pattern | ForEach-Object {
            $parts = ($_ -split '\s+') | Where-Object { $_ -ne '' }
            $procId = [int]$parts[-1]
            if ($procId -gt 0) { $listeners[$procId] = $true }
        }
    }

    foreach ($procId in @($listeners.Keys)) {
        $proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
        [PSCustomObject]@{
            Pid  = $procId
            Name = if ($proc) { $proc.ProcessName } else { 'unknown' }
            Path = if ($proc) { $proc.Path } else { '' }
        }
    }
}

function Test-PortOccupied {
    param([int]$Port = $script:DevPort)
    return (@(Get-PortListeners -Port $Port)).Count -gt 0
}

function Test-ExeLocked {
    param([string]$ExePath = (Get-DevExePath))

    if (-not (Test-Path $ExePath)) { return $false }
    try {
        $fs = [System.IO.File]::Open($ExePath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
        $fs.Close()
        return $false
    } catch {
        return $true
    }
}

function Stop-XiaohanProcesses {
    # 停止所有 xiaohan-daily 实例，等待 exe 文件解锁（解决 Cargo 拒绝访问 os error 5）
    $killed = @()

    Get-Process -Name 'xiaohan-daily' -ErrorAction SilentlyContinue | ForEach-Object {
        Write-Host "停止 xiaohan-daily (PID $($_.Id))..." -ForegroundColor Yellow
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        $killed += $_.Id
    }

    if ($killed.Count -eq 0) { return 0 }

    $exePath = Get-DevExePath
    for ($i = 1; $i -le 8; $i++) {
        if (-not (Test-ExeLocked -ExePath $exePath)) {
            Write-Host "exe 已解锁。" -ForegroundColor Green
            return $killed.Count
        }
        # 仍有残留则再杀一次
        Get-Process -Name 'xiaohan-daily' -ErrorAction SilentlyContinue | ForEach-Object {
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        }
        Write-Host "等待 exe 解锁... ($i/8)" -ForegroundColor DarkGray
        Start-Sleep -Seconds 1
    }

    Write-Host "警告: xiaohan-daily.exe 仍被锁定，请关闭托盘中的「小寒日报」后重试" -ForegroundColor Red
    return $killed.Count
}

function Stop-PortOccupiers {
    param(
        [int]$Port = $script:DevPort,
        [int]$MaxWaitSec = 8
    )

    $listeners = @(Get-PortListeners -Port $Port)
    if ($listeners.Count -eq 0) {
        return 0
    }

    Write-Host "检测到端口 $Port 被占用，自动停止占用进程..." -ForegroundColor Yellow
    foreach ($item in $listeners) {
        $detail = if ($item.Path) { $item.Path } else { $item.Name }
        Write-Host "  - PID $($item.Pid)  $detail" -ForegroundColor DarkGray
        Stop-Process -Id $item.Pid -Force -ErrorAction SilentlyContinue
    }

    Stop-XiaohanProcesses | Out-Null

    for ($i = 1; $i -le $MaxWaitSec; $i++) {
        if (-not (Test-PortOccupied -Port $Port)) {
            Write-Host "端口 $Port 已释放。" -ForegroundColor Green
            return $listeners.Count
        }
        Write-Host "等待端口释放... ($i/$MaxWaitSec)" -ForegroundColor DarkGray
        Start-Sleep -Seconds 1
    }

    Write-Host "错误: 端口 $Port 仍被占用，请运行 .\scripts\stop-dev.ps1" -ForegroundColor Red
    exit 1
}

function Ensure-DevEnvironmentClean {
    param([switch]$NoKill)

    if ($NoKill) {
        if ((Get-Process -Name 'xiaohan-daily' -ErrorAction SilentlyContinue) -or (Test-PortOccupied)) {
            Write-Host "错误: 已有实例在运行。去掉 -NoKill 可自动清理，或运行 .\scripts\stop-dev.ps1" -ForegroundColor Red
            exit 1
        }
        return
    }

    # 始终先杀残留 xiaohan-daily（即使端口未被占用，exe 仍可能锁定导致 Cargo 失败）
    if (Get-Process -Name 'xiaohan-daily' -ErrorAction SilentlyContinue) {
        Stop-XiaohanProcesses | Out-Null
    } elseif (Test-ExeLocked) {
        Write-Host "检测到 exe 被锁定，尝试清理..." -ForegroundColor Yellow
        Stop-XiaohanProcesses | Out-Null
    }

    if (Test-PortOccupied) {
        Stop-PortOccupiers | Out-Null
    }
}

function Stop-XiaohanDev {
    Stop-XiaohanProcesses | Out-Null
    if (Test-PortOccupied) {
        Stop-PortOccupiers | Out-Null
    } else {
        Write-Host "端口 $($script:DevPort) 未被占用。" -ForegroundColor DarkGray
    }
}

function Invoke-NpmCli {
    param(
        [Parameter(Mandatory = $true, Position = 0)]
        [string]$Command,
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$Rest
    )
    $ErrorActionPreference = 'Continue'
    if ($Rest -and $Rest.Count -gt 0) {
        npm $Command @Rest
    } else {
        npm $Command
    }
    return $LASTEXITCODE
}

function Invoke-TauriDev {
    param([switch]$NoKill)

    Initialize-MsvcEnvironment | Out-Null

    $code = Invoke-NpmCli run tauri:dev

    # Cargo exit 101 + os error 5 = exe 被旧进程锁定（tauri dev 热重载时常见）
    if ($code -eq 101 -and -not $NoKill) {
        Write-Host ""
        Write-Host "检测到编译失败 (exit 101)，可能是 exe 被占用..." -ForegroundColor Yellow
        Ensure-DevEnvironmentClean
        Write-Host "正在自动重试..." -ForegroundColor Cyan
        Write-Host ""
        $code = Invoke-NpmCli run tauri:dev
    }

    if ($code -ne 0) {
        Write-Host ""
        Write-Host "npm 失败，退出码: $code" -ForegroundColor Red
        if ($code -eq 101) {
            if (-not (Get-Command rc.exe -ErrorAction SilentlyContinue)) {
                Write-Host "提示: 可能缺少 RC.EXE，请安装 VS Build Tools 后重试" -ForegroundColor Yellow
            }
            Write-Host "提示: 运行 .\scripts\stop-dev.ps1 后重新 .\scripts\start-dev.bat" -ForegroundColor Yellow
            Write-Host "      修改 Rust 代码后请等上一次编译完成再保存，避免连续触发热重载" -ForegroundColor Yellow
        }
        exit $code
    }
}
