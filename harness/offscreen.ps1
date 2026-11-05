param([int]$X = -4000, [int]$Y = -4000)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class U3Win {
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hAfter, int X, int Y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
}
"@
$SWP_NOSIZE = 0x0001
$SWP_NOZORDER = 0x0004
$SWP_SHOWWINDOW = 0x0040
$moved = 0
Get-Process up366 -EA SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | ForEach-Object {
  [U3Win]::ShowWindow($_.MainWindowHandle, 5) | Out-Null
  [U3Win]::SetWindowPos($_.MainWindowHandle, [IntPtr]::Zero, $X, $Y, 0, 0, ($SWP_NOSIZE -bor $SWP_NOZORDER -bor $SWP_SHOWWINDOW)) | Out-Null
  $moved++
}
Write-Host ('[offscreen] moved ' + $moved + ' window(s)')
