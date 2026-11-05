param([int]$WaitSec = 15)
$ErrorActionPreference = 'SilentlyContinue'
$exe = 'C:\Program Files (x86)\up366\client\up366.exe'
$out = 'H:\project\up366\recon\runtime-access.md'
$prof = "$env:TEMP\u3probe"
$hook = "$env:TEMP\u3hook.js"
$marker = "$env:TEMP\u3hook-alive.txt"
$NL = [char]10
Remove-Item $marker -Force -EA SilentlyContinue
$hookBody = 'require("fs").writeFileSync(process.env.U3_MARKER || "C:\\up3_marker.txt", "pid=" + process.pid + " type=" + (process.type||"node"));'
Set-Content -Path $hook -Encoding UTF8 -Value $hookBody

function Stop-All {
  Get-Process up366 -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
  Start-Sleep -Milliseconds 900
}

$lines = @()
$lines += "# 运行时通道可用性"
$lines += ""
$lines += "测试时间: $(Get-Date -Format o)"
$lines += ""
$lines += "| 通道 | 结果 | 证据 |"
$lines += "|---|---|---|"

Stop-All
$p = Start-Process -FilePath $exe -ArgumentList '--inspect=9229', "--user-data-dir=$prof" -WindowStyle Minimized -PassThru
$okA = $false
$detA = 'no listener within ' + $WaitSec + 's'
$dl = (Get-Date).AddSeconds($WaitSec)
while ((Get-Date) -lt $dl) {
  try { $j = Invoke-RestMethod 'http://127.0.0.1:9229/json/list' -TimeoutSec 2; $okA = $true; $detA = ($j | ConvertTo-Json -Depth 4 -Compress); break } catch { Start-Sleep -Milliseconds 700 }
}
$lines += "| --inspect=9229 | $okA | $($detA.Substring(0,[Math]::Min(120,$detA.Length))) |"
Stop-All

$p = Start-Process -FilePath $exe -ArgumentList '--remote-debugging-port=9222', "--user-data-dir=$prof" -WindowStyle Minimized -PassThru
$okB = $false
$detB = 'no listener'
$dl = (Get-Date).AddSeconds($WaitSec)
while ((Get-Date) -lt $dl) {
  try { $j = Invoke-RestMethod 'http://127.0.0.1:9222/json/list' -TimeoutSec 2; $okB = $true; $detB = ($j | ConvertTo-Json -Depth 4 -Compress); break } catch { Start-Sleep -Milliseconds 700 }
}
$lines += "| --remote-debugging-port=9222 | $okB | $($detB.Substring(0,[Math]::Min(120,$detB.Length))) |"
Stop-All

$env:NODE_OPTIONS = '--require "' + $hook + '"'
$env:U3_MARKER = $marker
$p = Start-Process -FilePath $exe -ArgumentList "--user-data-dir=$prof" -WindowStyle Minimized -PassThru
Start-Sleep -Seconds 8
$okC = Test-Path $marker
$detC = 'marker missing'
if ($okC) { $detC = (Get-Content $marker -Raw).Trim() }
$lines += "| NODE_OPTIONS=--require | $okC | $detC |"
Stop-All
Remove-Item Env:NODE_OPTIONS -EA SilentlyContinue
Remove-Item Env:U3_MARKER -EA SilentlyContinue

$env:ELECTRON_RUN_AS_NODE = '1'
$detD = (& $exe -e "console.log(process.versions.electron + '|' + process.versions.node + '|' + process.versions.chrome)" 2>&1 | Out-String).Trim()
$okD = ($detD -match '\|')
$lines += "| ELECTRON_RUN_AS_NODE=1 | $okD | $($detD.Substring(0,[Math]::Min(120,$detD.Length))) |"
Remove-Item Env:ELECTRON_RUN_AS_NODE -EA SilentlyContinue

$lines += ""
$lines | Set-Content -Path $out -Encoding UTF8
Write-Output ($lines -join $NL)
Get-Process up366 -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
