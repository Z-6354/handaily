# Rust dev helper: parallel jobs + optional sccache (check/test/run)
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Initialize-RustBuildEnv
Set-Location (Get-RepoRoot)

if ($CargoArgs.Count -eq 0) {
    Write-Error "Usage: cargo-dev.ps1 check -p xiaohan-daily --lib"
    exit 1
}

& cargo @CargoArgs
exit $LASTEXITCODE