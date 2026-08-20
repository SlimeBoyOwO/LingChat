param(
  [Parameter(Mandatory = $true)]
  [string]$ExePath,
  [string]$DataDir = (Join-Path $PSScriptRoot "..\.dev-data\live2d"),
  [int]$DebugPort = 9237
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$exe = (Resolve-Path $ExePath).Path
$data = [System.IO.Path]::GetFullPath($DataDir)

if (-not (Test-Path (Join-Path $data "game_data"))) {
  New-Item -ItemType Directory -Force -Path $data | Out-Null
  Copy-Item -Recurse -Force (Join-Path $repoRoot "data\game_data") (Join-Path $data "game_data")
  if (Test-Path (Join-Path $repoRoot "data\third_party")) {
    Copy-Item -Recurse -Force (Join-Path $repoRoot "data\third_party") (Join-Path $data "third_party")
  }
}

$isolatedAppData = Join-Path $data ".appdata"
New-Item -ItemType Directory -Force -Path $isolatedAppData | Out-Null
$env:LINGCHAT_DATA_DIR = $data
$env:APPDATA = $isolatedAppData
$env:WEBVIEW2_USER_DATA_FOLDER = (Join-Path $data ".webview2")
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$DebugPort"

$frontendLog = Join-Path $data "vite.log"
$frontendErrorLog = Join-Path $data "vite-error.log"
$frontend = Start-Process `
  -FilePath "corepack.cmd" `
  -ArgumentList @("pnpm", "dev") `
  -WorkingDirectory $repoRoot `
  -RedirectStandardOutput $frontendLog `
  -RedirectStandardError $frontendErrorLog `
  -PassThru

try {
  $ready = $false
  for ($attempt = 0; $attempt -lt 60; $attempt++) {
    try {
      Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:1420" -TimeoutSec 1 | Out-Null
      $ready = $true
      break
    } catch {
      Start-Sleep -Milliseconds 500
    }
  }
  if (-not $ready) {
    throw "Vite did not start. See $frontendErrorLog"
  }

  $hostProcess = Start-Process -FilePath $exe -PassThru
  Write-Host "LingChat Dev Host started with data: $data"
  Write-Host "Frontend HMR: http://127.0.0.1:1420"
  Write-Host "WebView diagnostics: http://127.0.0.1:$DebugPort"
  Wait-Process -Id $hostProcess.Id
} finally {
  if ($frontend) {
    & taskkill.exe /PID $frontend.Id /T /F 2>$null | Out-Null
  }
}
