# 小寒日报 - 构建
# 用法:
#   .\scripts\build.ps1           # release-fast + NSIS（默认，较快）
#   .\scripts\build.ps1 -ExeOnly  # 仅便携 exe，不打安装包
#   .\scripts\build.ps1 -Full     # release + LTO，最慢、适合正式发布
#   .\scripts\build.ps1 -Small    # 体积优先
#   .\scripts\build.ps1 -SkipFe   # 强制跳过前端（dist 须已存在）

param(
    [switch]$ExeOnly,
    [switch]$Full,
    [switch]$Small,
    [switch]$SkipFe
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
} elseif ($Full) {
    $env:FE_BUILD_FULL = "1"
}

if ($Small) {
    Invoke-NpmCli run tauri:build:small
} elseif ($ExeOnly) {
    Invoke-NpmCli run tauri:build:exe
} elseif ($Full) {
    Invoke-NpmCli run tauri:build:full
} else {
    Invoke-NpmCli run tauri:build
}

Write-Host "构建完成。" -ForegroundColor Green
Write-Host "exe (release-fast): src-tauri\target\release-fast\xiaohan-daily.exe" -ForegroundColor DarkGray
Write-Host "exe (release):      src-tauri\target\release\xiaohan-daily.exe" -ForegroundColor DarkGray
if (-not $ExeOnly -and -not $Small) {
    Write-Host "NSIS: src-tauri\target\release-fast\bundle\nsis\" -ForegroundColor DarkGray
}
