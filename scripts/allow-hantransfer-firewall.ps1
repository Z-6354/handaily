#Requires -Version 5.1
#Requires -RunAsAdministrator
<#
.SYNOPSIS
  Allow hantransfer LAN access (TCP 7822) through Windows Firewall.

.EXAMPLE
  Right-click PowerShell -> Run as administrator, then:
  .\scripts\allow-hantransfer-firewall.ps1
#>
param(
    [int]$Port = 7822
)

$ErrorActionPreference = "Stop"
$RuleName = "hantransfer TCP $Port"

$existing = netsh advfirewall firewall show rule name="$RuleName" 2>$null
if ($LASTEXITCODE -eq 0) {
    Write-Host "Firewall rule already exists: $RuleName" -ForegroundColor Green
} else {
    netsh advfirewall firewall add rule `
        name="$RuleName" `
        dir=in action=allow protocol=TCP localport=$Port `
        profile=any
    if ($LASTEXITCODE -ne 0) { throw "Failed to add firewall rule" }
    Write-Host "Added firewall rule: $RuleName" -ForegroundColor Green
}

# Hint if Wi-Fi is Public (often blocks LAN)
try {
    $profiles = Get-NetConnectionProfile -ErrorAction SilentlyContinue |
        Where-Object { $_.IPv4Connectivity -eq 'Internet' }
    foreach ($p in $profiles) {
        if ($p.NetworkCategory -eq 'Public') {
            Write-Host ""
            Write-Host "Wi-Fi '$($p.Name)' is Public — consider switching to Private:" -ForegroundColor Yellow
            Write-Host "  Settings -> Network -> Wi-Fi -> $($p.Name) -> Private network" -ForegroundColor Yellow
        }
    }
} catch {}

Write-Host ""
Write-Host "Phone URL (same WiFi):" -ForegroundColor Cyan
$ip = (Get-NetIPAddress -AddressFamily IPv4 |
    Where-Object { $_.IPAddress -notlike '127.*' -and $_.PrefixOrigin -ne 'WellKnown' } |
    Select-Object -First 1).IPAddress
if ($ip) {
    Write-Host "  http://${ip}:${Port}/m/" -ForegroundColor White
} else {
    Write-Host "  http://<PC_IP>:${Port}/m/" -ForegroundColor White
}
