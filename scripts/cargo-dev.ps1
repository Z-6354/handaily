# 日常 Rust 开发：并行编译 + 可选 sccache（check/test/run 统一入口）
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Initialize-RustBuildEnv
Set-Location (Get-ProjectRoot)

if ($CargoArgs.Count -eq 0) {
    Write-Error "用法: cargo-dev.ps1 check --manifest-path src-tauri/Cargo.toml --lib"
    exit 1
}

& cargo @CargoArgs
exit $LASTEXITCODE
