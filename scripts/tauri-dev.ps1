# 开发启动：Ctrl+C 后清理 Vite，避免 npm「Terminate batch job (Y/N)?」
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Initialize-RustBuildEnv | Out-Null
Set-Location (Get-ProjectRoot)

function Stop-ViteDevServer {
    try {
        Get-NetTCPConnection -LocalPort 1420 -ErrorAction SilentlyContinue |
            ForEach-Object {
                Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue
            }
    } catch {
        # 无 NetTCPConnection 或非 Windows Server 时忽略
    }
    Get-Process -Name "node" -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -like "*HANDAILY*" -or $_.MainWindowTitle -like "*vite*" } |
        ForEach-Object {
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        }
}

try {
    & npx tauri dev @args
    exit $LASTEXITCODE
} finally {
    Stop-ViteDevServer
}
