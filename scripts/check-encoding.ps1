# 检测 scripts 目录下脚本的编码问题
# 用法: .\scripts\check-encoding.ps1

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$issues = @()

function Test-ScriptEncoding {
    param([string]$Path)

    $name = Split-Path $Path -Leaf
    $bytes = [IO.File]::ReadAllBytes($Path)
    $hasBom = ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF)
    $ext = [IO.Path]::GetExtension($Path).ToLowerInvariant()

    if ($ext -eq ".ps1") {
        if (-not $hasBom) {
            $issues += "$name : 缺少 UTF-8 BOM（PowerShell 5.1 会把中文读成乱码）"
        }
        $text = [IO.File]::ReadAllText($Path, [Text.UTF8Encoding]::new($true))
        if ($text -match [char]0x2014) {
            $issues += "$name : 含 em dash（—），部分控制台会显示为 �"
        }
        if ($text -match '[^\x09\x0A\x0D\x20-\x7E\u4e00-\u9fff\u3000-\u303f\uff00-\uffef]') {
            $issues += "$name : 含非常规字符（建议检查是否为乱码残留）"
        }
    }

    if ($ext -eq ".bat") {
        if (-not $hasBom) {
            # bat 无中文时可无 BOM；检查是否含非 ASCII
            $asciiOnly = $true
            foreach ($b in $bytes) { if ($b -gt 127) { $asciiOnly = $false; break } }
            if (-not $asciiOnly) {
                $issues += "$name : bat 含非 ASCII 但未使用 UTF-8 BOM"
            }
        }
    }
}

Get-ChildItem -LiteralPath $ScriptDir -File | ForEach-Object {
    Test-ScriptEncoding -Path $_.FullName
}

Write-Host "编码检测报告 - scripts/" -ForegroundColor Cyan
Write-Host ""

if ($issues.Count -eq 0) {
    Write-Host "未发现编码问题。" -ForegroundColor Green
    exit 0
}

Write-Host "发现 $($issues.Count) 个问题:" -ForegroundColor Yellow
$issues | ForEach-Object { Write-Host "  - $_" -ForegroundColor Yellow }
exit 1
