param([string]$Profile = "$env:TEMP\u3dev", [int]$WaitSec = 10, [switch]$Keep, [switch]$Offscreen)
$ErrorActionPreference = 'Stop'
$exe = 'H:\project\up366\work\client\up366.exe'
if (-not (Test-Path $exe)) { throw 'run dev-prepare.ps1 first' }
$style = if ($Offscreen) { 'Normal' } else { 'Minimized' }
$p = Start-Process -FilePath $exe -ArgumentList "--user-data-dir=$Profile" -WindowStyle $style -PassThru
Start-Sleep -Seconds $WaitSec
if ($Offscreen) {
  & (Join-Path $PSScriptRoot 'offscreen.ps1') | Out-Host
}
$titles = @(Get-Process up366 -EA SilentlyContinue | Where-Object { $_.MainWindowTitle } | ForEach-Object { $_.MainWindowTitle })
Write-Host ('[launch] pid=' + $p.Id)
Write-Host ('[launch] windows: ' + ($titles -join ' | '))
if (-not $Keep) {
  Get-Process up366 -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
  Write-Host '[launch] stopped'
}
