# 重启开发环境（先清理再启动）
. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding
Stop-XiaohanDev
Start-Sleep -Seconds 1
& "$PSScriptRoot\start-dev.ps1"
