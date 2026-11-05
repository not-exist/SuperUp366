#requires -Version 5
<#
  Safe, background launch of the up366 client for runtime inspection.
  Isolates the Electron profile, records TCP peers before/after, waits for CDP,
  prints targets, then stops the process unless -Keep is given.
  Never writes to the installation directory.
#>
param(
  [int]$Port = 9222,
  [string]$Profile = "$env:TEMP\u3prof",
  [int]$WaitSec = 30,
  [switch]$Keep
)

$ErrorActionPreference = 'Stop'
$exe = 'C:\Program Files (x86)\up366\client\up366.exe'
$outDir = Join-Path $PSScriptRoot '..\recon'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

function Get-Conn {
  Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue |
    Where-Object { $_.RemoteAddress -notin @('127.0.0.1','::1','0.0.0.0','::') } |
    Select-Object -ExpandProperty RemoteAddress -Unique | Sort-Object
}

$before = Get-Conn
$started = Get-Date
Write-Host "[launch] profile=$Profile port=$Port"

$argv = @("--remote-debugging-port=$Port", "--user-data-dir=$Profile", "--no-first-run")
$proc = Start-Process -FilePath $exe -ArgumentList $argv -WindowStyle Minimized -PassThru
Write-Host "[launch] pid=$($proc.Id)"

$version = $null
$deadline = (Get-Date).AddSeconds($WaitSec)
while ((Get-Date) -lt $deadline) {
  try { $version = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/version" -TimeoutSec 2; break } catch { Start-Sleep -Milliseconds 700 }
}

$log = [ordered]@{
  started_utc = $started.ToUniversalTime().ToString('o')
  pid         = $proc.Id
  port        = $Port
  cdp_version = $version
  targets     = $null
  new_remote  = @()
}
if ($version) {
  $log.targets = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/list" -TimeoutSec 3
} else {
  Write-Host "[launch] CDP endpoint did not come up within $($WaitSec)s"
}

Start-Sleep -Seconds 3
$log.new_remote = @(Get-Conn | Where-Object { $before -notcontains $_ })

$logPath = Join-Path $outDir 'runtime-launch.json'
$log | ConvertTo-Json -Depth 6 | Set-Content -Path $logPath -Encoding UTF8
Write-Host "[launch] wrote $logPath"

if ($log.targets) {
  $log.targets | ForEach-Object { "  target id={0} type={1} url={2}" -f $_.id, $_.type, ($_.url -replace '\?.*','') }
}
if ($log.new_remote.Count -gt 0) { Write-Host "[net] new remote peers: $($log.new_remote -join ', ')" } else { Write-Host "[net] no new remote peers observed" }

if (-not $Keep) {
  Write-Host "[launch] stopping pid $($proc.Id)"
  Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
  Get-Process up366 -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}
