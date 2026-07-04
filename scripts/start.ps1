# 小寒日报 - 运行已构建程序
# 用法: .\scripts\start.ps1 [-Debug]

param(
    [switch]$Debug
)

. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding

$Root = Get-ProjectRoot
Set-Location $Root

$candidates = @()
if ($Debug) {
    $candidates += Join-Path $Root "src-tauri\target\debug\xiaohan-daily.exe"
} else {
    $candidates += Join-Path $Root "src-tauri\target\release\xiaohan-daily.exe"
    $candidates += Join-Path $Root "src-tauri\target\release\bundle\nsis\*.exe"
    $candidates += Join-Path $Root "src-tauri\target\debug\xiaohan-daily.exe"
}

$exe = $null
foreach ($path in $candidates) {
    if ($path.Contains('*')) {
        $found = Get-Item $path -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) { $exe = $found.FullName; break }
    } elseif (Test-Path $path) {
        $exe = $path
        break
    }
}

if (-not $exe) {
    Write-Host "未找到可执行文件，请先构建:" -ForegroundColor Yellow
    Write-Host "  .\scripts\build.bat" -ForegroundColor DarkGray
    Write-Host "  .\scripts\build.ps1 -Fast   # 仅编译 exe" -ForegroundColor DarkGray
    exit 1
}

Write-Host "启动: $exe" -ForegroundColor Cyan
Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe -Parent)
