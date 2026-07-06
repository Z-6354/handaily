# 小寒日报 - 构建（统一 release-fast 配置，见 src-tauri/Cargo.toml [profile.release]）
# 用法:
#   .\scripts\build.ps1           # NSIS 安装包 + exe
#   .\scripts\build.ps1 -ExeOnly  # 仅便携 exe，不打安装包
#   .\scripts\build.ps1 -SkipFe   # 强制跳过前端（dist 须已存在）
#   .\scripts\build.ps1 -TypeCheck  # 打包前跑 tsc + vite（默认仅 vite）

param(
    [switch]$ExeOnly,
    [switch]$SkipFe,
    [switch]$TypeCheck
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

if ($SkipFe) {
    $env:SKIP_FE_BUILD = "1"
    Write-Host "SkipFe: 强制跳过前端构建" -ForegroundColor DarkGray
} elseif ($TypeCheck) {
    $env:FE_BUILD_FULL = "1"
    Write-Host "TypeCheck: 打包前执行 tsc + vite" -ForegroundColor DarkGray
}

if ($ExeOnly) {
    Invoke-NpmCli run tauri:build:exe
} else {
    Invoke-NpmCli run tauri:build
}

Write-Host "构建完成。" -ForegroundColor Green
Write-Host "exe:  src-tauri\target\release\xiaohan-daily.exe" -ForegroundColor DarkGray
if (-not $ExeOnly) {
    Write-Host "NSIS: src-tauri\target\release\bundle\nsis\" -ForegroundColor DarkGray
}
