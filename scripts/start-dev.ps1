# 小寒日报 - 开发模式启动
# 用法: .\scripts\start-dev.ps1 [-NoKill]

param(
    [switch]$NoKill
)

. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding

$Root = Get-ProjectRoot
Set-Location $Root

Write-Host "小寒日报 - 开发模式启动..." -ForegroundColor Cyan
Write-Host "项目目录: $Root" -ForegroundColor DarkGray

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    Write-Error "未找到 npm，请先安装 Node.js"
    exit 1
}

Ensure-DevEnvironmentClean -NoKill:$NoKill

Initialize-MsvcEnvironment | Out-Null

if (-not (Test-Path "$Root\node_modules")) {
    Write-Host "首次运行，正在安装依赖..." -ForegroundColor Yellow
    $code = Invoke-NpmCli install
    if ($code -ne 0) { exit $code }
}

Ensure-DevEnvironmentClean -NoKill:$NoKill

Write-Host ""
Write-Host "即将启动 Tauri 开发环境..." -ForegroundColor Cyan
Write-Host "  [1/2] Vite 前端 -> http://127.0.0.1:1420 (pet.html 同端口)" -ForegroundColor DarkGray
Write-Host "  [2/2] Cargo 编译 Rust (首次约 1-3 分钟)" -ForegroundColor DarkGray
Write-Host "  编译完成后主窗口与桌宠会自动弹出" -ForegroundColor DarkGray
Write-Host "  修改 Rust 后请等编译完成再保存，避免 exe 占用错误" -ForegroundColor DarkGray
Write-Host "  若桌宠不可见: 设置 -> 桌宠 -> 立即更新" -ForegroundColor DarkGray
Write-Host ""

Invoke-TauriDev -NoKill:$NoKill
