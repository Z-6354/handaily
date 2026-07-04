# hanagent - 小寒日报子命令
# 用法: .\scripts\hanagent.ps1 xiaohan dev|build|pack|stop

param(
    [Parameter(Position = 0)]
    [string]$Channel,

    [Parameter(Position = 1)]
    [string]$Action = "dev"
)

. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding

$Root = Get-ProjectRoot
Set-Location $Root

if ($Channel -ne "xiaohan") {
    Write-Error "未知频道: $Channel (当前仅支持 xiaohan)"
    exit 1
}

switch ($Action) {
    "dev" {
        & "$PSScriptRoot\start-dev.ps1"
        exit $LASTEXITCODE
    }
    "stop" {
        Stop-XiaohanDev
        Write-Host "已停止开发实例。" -ForegroundColor Green
    }
    "build" {
        & "$PSScriptRoot\build.ps1"
        exit $LASTEXITCODE
    }
    "pack" {
        Invoke-NpmCli run build
        Invoke-NpmCli run tauri:pack
    }
    default {
        Write-Error "未知动作: $Action (支持 dev|stop|build|pack)"
        exit 1
    }
}
