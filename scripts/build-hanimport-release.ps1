# Build portable hanimport package (exe + web + bat)
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$HanimportDir = Join-Path $RepoRoot "hanimport"
$OutDir = Join-Path $HanimportDir "release\portable"
$TplDir = Join-Path $HanimportDir "release\templates"
$TargetExe = Join-Path $RepoRoot "hanpet\src-tauri\target\release\hanimport.exe"

Write-Host "[hanimport-release] repo: $RepoRoot"

if (-not $SkipBuild) {
    Write-Host "[hanimport-release] cargo build --release -p hanimport"
    Push-Location $RepoRoot
    try {
        cargo build --release -p hanimport
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path $TargetExe)) {
    throw "hanimport.exe not found: $TargetExe"
}

Write-Host "[hanimport-release] preparing $OutDir"
if (Test-Path $OutDir) {
    Remove-Item -Recurse -Force $OutDir
}
New-Item -ItemType Directory -Path $OutDir | Out-Null
New-Item -ItemType Directory -Path (Join-Path $OutDir "scripts") | Out-Null
New-Item -ItemType Directory -Path (Join-Path $OutDir "web") | Out-Null
New-Item -ItemType Directory -Path (Join-Path $OutDir "data\model") | Out-Null

Copy-Item $TargetExe (Join-Path $OutDir "hanimport.exe")
Copy-Item (Join-Path $HanimportDir "scripts\unpack_bundle.py") (Join-Path $OutDir "scripts\")
Copy-Item (Join-Path $HanimportDir "scripts\serve_web.py") (Join-Path $OutDir "scripts\")
Copy-Item (Join-Path $HanimportDir "scripts\build_model_config.py") (Join-Path $OutDir "scripts\")
Copy-Item (Join-Path $HanimportDir "web\*") (Join-Path $OutDir "web\") -Recurse

Copy-Item (Join-Path $TplDir "portable-web.bat") (Join-Path $OutDir "StartWeb.bat")
Copy-Item (Join-Path $TplDir "portable-menu.bat") (Join-Path $OutDir "Hanimport.bat")
Copy-Item (Join-Path $TplDir "readme.txt") (Join-Path $OutDir "README.txt")

$ZipPath = Join-Path $HanimportDir "release\hanimport-portable.zip"
if (Test-Path $ZipPath) { Remove-Item -Force $ZipPath }
Compress-Archive -Path "$OutDir\*" -DestinationPath $ZipPath

Write-Host "[hanimport-release] done"
Write-Host "  folder: $OutDir"
Write-Host "  zip:    $ZipPath"
