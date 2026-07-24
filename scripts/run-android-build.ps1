param([string]$Mode = "build")

$ErrorActionPreference = "Continue"

$logFile = "C:\Users\DCY45\Desktop\LingChat-refactor\build.log"
$errFile = "C:\Users\DCY45\Desktop\LingChat-refactor\build.err.log"
foreach ($f in @($logFile, $errFile)) { if (Test-Path $f) { Remove-Item -LiteralPath $f -Force } }

$env:ANDROID_HOME = "C:\Users\DCY45\AppData\Local\Android\Sdk"
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\28.2.13676358"
$env:ANDROID_NDK_ROOT = $env:ANDROID_NDK_HOME

$ndk = "$env:ANDROID_HOME\ndk\28.2.13676358\toolchains\llvm\prebuilt\windows-x86_64"
$env:PATH = "$ndk\bin;$env:PATH"
$env:CC_aarch64_linux_android = "$ndk\bin\clang.exe"
$env:CXX_aarch64_linux_android = "$ndk\bin\clang++.exe"
$env:AR_aarch64_linux_android = "$ndk\bin\llvm-ar.exe"

$env:JAVA_HOME = "C:\Program Files\Java\jdk-25"

Write-Host "=== Env ==="
Write-Host "ANDROID_HOME=$env:ANDROID_HOME"
Write-Host "ANDROID_NDK_HOME=$env:ANDROID_NDK_HOME"
Write-Host "JAVA_HOME=$env:JAVA_HOME"
Write-Host "CC_aarch64_linux_android=$env:CC_aarch64_linux_android"

Set-Location "C:\Users\DCY45\Desktop\LingChat-refactor"

if ($Mode -eq "build") {
  & pnpm exec tauri android build --target aarch64 2>&1 | Out-File -FilePath $logFile -Encoding utf8
  Write-Host "ExitCode=$LASTEXITCODE"
  Write-Host "---- tail of build.log ----"
  Get-Content -Path $logFile -Tail 80 -ErrorAction SilentlyContinue
}
