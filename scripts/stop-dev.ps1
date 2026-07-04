# 停止开发实例并释放端口 / exe 锁
. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding
Stop-XiaohanDev
Write-Host "已停止。" -ForegroundColor Green
