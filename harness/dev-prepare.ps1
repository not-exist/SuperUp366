param([switch]$Resync)
$ErrorActionPreference = 'Stop'
$root = 'H:\project\up366'
$src  = 'C:\Program Files (x86)\up366\client'
$dst  = Join-Path $root 'work\client'

if ($Resync -or -not (Test-Path (Join-Path $dst 'up366.exe'))) {
  Write-Host '[prepare] mirroring install to work\client'
  robocopy $src $dst /MIR /NFL /NDL /NJH /NJS /R:1 /W:1 | Out-Null
}

python (Join-Path $root 'harness\inject.py') (Join-Path $dst 'resources\res')
Write-Host '[prepare] done'
