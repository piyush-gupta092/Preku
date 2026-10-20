# Renders src/dev/preview.ts in headless Edge and saves a PNG. Needs `pnpm dev` running.
#   ./scripts/preview-shot.ps1 -Query "state=expanded&items=2" -Width 504 -Height 372 -Out shot.png
param(
  [string]$Query = "window=island&state=expanded&items=1",
  [int]$Width = 504,
  [int]$Height = 372,
  [double]$Scale = 1,
  [int]$Budget = 3000,
  [Parameter(Mandatory = $true)][string]$Out
)

$edge = "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe"
if (-not (Test-Path $edge)) { $edge = "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe" }
$edgeProfile = Join-Path $env:TEMP "preku-preview-edge"
if (Test-Path $Out) { Remove-Item $Out }
$edgeArgs = @("--headless=new", "--disable-gpu", "--hide-scrollbars", "--user-data-dir=$edgeProfile",
  "--force-device-scale-factor=$Scale", "--window-size=$Width,$Height", "--virtual-time-budget=$Budget",
  "--screenshot=$Out", "http://localhost:1420/preview.html?$Query")
# msedge.exe hands off to a background process, so wait for the file rather than the launcher.
Start-Process -FilePath $edge -ArgumentList $edgeArgs -WindowStyle Hidden -Wait
for ($i = 0; $i -lt 50 -and -not (Test-Path $Out); $i++) { Start-Sleep -Milliseconds 200 }
if (-not (Test-Path $Out)) { throw "Screenshot failed: $Out" }
