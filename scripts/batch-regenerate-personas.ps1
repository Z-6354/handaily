# Batch AI persona profile update via HANDAILY Agent API (port 1421)
param(
    [int]$BatchSize = 1,
    [switch]$All,
    [string]$BaseUrl = "http://127.0.0.1:1421"
)

. "$PSScriptRoot\_common.ps1"
Initialize-ScriptEncoding

function Test-AgentReady {
    try {
        $s = Invoke-RestMethod -Uri "$BaseUrl/api/status" -TimeoutSec 5
        if (-not $s.running) {
            throw "Agent service not running. Enable it in Agent Connect page."
        }
        return $s
    } catch {
        Write-Error "Cannot connect to $BaseUrl : $_"
        exit 1
    }
}

function Invoke-BatchRegenerate {
    param([int]$Limit, [bool]$OnlyMissing)
    $only = if ($OnlyMissing) { "true" } else { "false" }
    $uri = "$BaseUrl/api/personas/batch-regenerate?limit=$Limit" + "&" + "only_missing=$only"
    Invoke-RestMethod -Uri $uri -Method POST -TimeoutSec 3600
}

$status = Test-AgentReady
Write-Host "Agent: $($status.base_url) | batch size $BatchSize"

$round = 0
$result = $null
do {
    $round++
    Write-Host ""
    Write-Host "--- round $round ---"
    try {
        $pending = Invoke-RestMethod -Uri "$BaseUrl/api/personas/regenerate-pending?only_missing=true" -TimeoutSec 10
        Write-Host "pending: $($pending.pending)"
        if ($pending.pending -le 0) { break }
    } catch { }
    $result = Invoke-BatchRegenerate -Limit $BatchSize -OnlyMissing (-not $All)
    Write-Host $result.message
    if ($result.last_error) {
        Write-Host "last error ($($result.last_id)): $($result.last_error)" -ForegroundColor Red
    }
    if ($result.failed -gt 0) {
        Write-Host "failed in batch: $($result.failed), continuing..."
    }
    if ($result.remaining -le 0) { break }
    if (-not $All -and $result.ok -eq 0 -and $result.skipped -eq $BatchSize) {
        Write-Host "no pending personas, done."
        break
    }
    Start-Sleep -Seconds 2
} while ($result.remaining -gt 0)

Write-Host ""
Write-Host "done. remaining ~ $($result.remaining). re-run script if > 0."
