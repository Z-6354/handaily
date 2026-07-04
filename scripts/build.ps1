# 小寒日报 - 构建
# 用法: .\scripts\build.ps1 [-Fast] [-Small]

param(
    [switch]$Fast,
    [switch]$Small
)

. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding

$Root = Get-ProjectRoot
Set-Location $Root

Write-Host "小寒日报 - 开始构建..." -ForegroundColor Cyan

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    Write-Error "未找到 npm，请先安装 Node.js"
    exit 1
}

Initialize-MsvcEnvironment | Out-Null

Invoke-NpmCli run build

if ($Small) {
    Invoke-NpmCli run tauri:build:small
} elseif ($Fast) {
    Invoke-NpmCli run tauri:build:fast
} else {
    Invoke-NpmCli run tauri:build
}

Write-Host "构建完成。" -ForegroundColor Green
Write-Host "运行: .\scripts\start.bat" -ForegroundColor DarkGray
