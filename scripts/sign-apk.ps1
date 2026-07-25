$env:JAVA_HOME = "C:\Program Files\Java\jdk-21.0.10"
$env:PATH = "$env:JAVA_HOME\bin;$env:PATH"

$keystore = "C:\Users\DCY45\.android\debug.keystore"
$apkIn = "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\gen\android\app\build\outputs\apk\arm64\release\app-arm64-release-unsigned.apk"
$apkOut = "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\gen\android\app\build\outputs\apk\arm64\release\app-arm64-release.apk"
$apksigner = "C:\Users\DCY45\AppData\Local\Android\Sdk\build-tools\35.0.0\apksigner.bat"
$zipalign = "C:\Users\DCY45\AppData\Local\Android\Sdk\build-tools\35.0.0\zipalign.exe"
$aligned = $apkOut -replace "\.apk$", "-aligned.apk"

# 1) Ensure debug keystore exists
$ksDir = Split-Path $keystore
if (-not (Test-Path $ksDir)) { New-Item -ItemType Directory -Force -Path $ksDir | Out-Null }
if (-not (Test-Path $keystore)) {
  Write-Host "Generating debug keystore..."
  & keytool -genkey -v -keystore $keystore -storepass android -alias androiddebugkey -keypass android `
    -keyalg RSA -keysize 2048 -validity 10000 `
    -dname "CN=Android Debug,O=Android,C=US" 2>&1 | Select-Object -Last 5
} else {
  Write-Host "Keystore exists: $keystore"
}

# 2) Clean targets
if (Test-Path $aligned) { Remove-Item -LiteralPath $aligned -Force }
if (Test-Path $apkOut) { Remove-Item -LiteralPath $apkOut -Force }

# 3) zipalign 4-byte (preserves .so alignment for native libs)
& $zipalign -f -p 4 $apkIn $aligned 2>&1 | Select-Object -Last 3
Write-Host "aligned: $((Get-Item $aligned).Length) bytes"

# 4) Sign with v1+v2+v3 (max compatibility, Android 7+)
& $apksigner sign --ks $keystore --ks-pass pass:android --key-pass pass:android --ks-key-alias androiddebugkey `
  --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true `
  --out $apkOut $aligned 2>&1 | Select-Object -Last 5
Write-Host "signed: $((Get-Item $apkOut).Length) bytes"

# 5) Verify
& $apksigner verify --verbose --print-certs $apkOut 2>&1 | Select-Object -First 20
Remove-Item -LiteralPath $aligned -Force -ErrorAction SilentlyContinue
Write-Host ""
Write-Host "=== final ==="
Get-Item $apkOut | Select-Object FullName, Length, LastWriteTime
