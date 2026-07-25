$env:JAVA_HOME = "C:\Program Files\Java\jdk-21.0.10"
$env:PATH = "$env:JAVA_HOME\bin;$env:PATH"

$pkg = "com.noiq.lingchat"
$dev = "95379e0c"
$apkUnsigned = "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\gen\android\app\build\outputs\apk\arm64\release\app-arm64-release-unsigned.apk"
$apkSigned = "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\gen\android\app\build\outputs\apk\arm64\release\app-arm64-release.apk"
$keystore = "C:\Users\DCY45\.android\debug.keystore"
$apksigner = "C:\Users\DCY45\AppData\Local\Android\Sdk\build-tools\35.0.0\apksigner.bat"
$zipalign = "C:\Users\DCY45\AppData\Local\Android\Sdk\build-tools\35.0.0\zipalign.exe"
$log = "C:\Users\DCY45\Desktop\LingChat-refactor\adb-test.log"
$aligned = $apkSigned -replace "\.apk$", "-aligned.apk"

# 1) Sign
if (Test-Path $aligned) { Remove-Item -LiteralPath $aligned -Force }
if (Test-Path $apkSigned) { Remove-Item -LiteralPath $apkSigned -Force }
& $zipalign -f -p 4 $apkUnsigned $aligned 2>&1 | Select-Object -Last 2
& $apksigner sign --ks $keystore --ks-pass pass:android --key-pass pass:android --ks-key-alias androiddebugkey `
  --v2-signing-enabled true --v3-signing-enabled true --out $apkSigned $aligned 2>&1 | Select-Object -Last 2
Remove-Item -LiteralPath $aligned -Force -ErrorAction SilentlyContinue
Write-Host "Signed: $((Get-Item $apkSigned).Length) bytes"

# 2) Install (replace existing)
& adb -s $dev uninstall $pkg 2>&1 | Select-Object -First 2
& adb -s $dev install -r $apkSigned 2>&1 | Select-Object -First 5

# 3) Reset logcat & launch
& adb -s $dev shell am force-stop $pkg
& adb -s $dev logcat -c
& adb -s $dev shell monkey -p $pkg -c android.intent.category.LAUNCHER 1 2>&1 | Select-Object -First 2
Start-Sleep -Seconds 6

# 4) Dump & analyze
& adb -s $dev logcat -d -v threadtime | Out-File -FilePath $log -Encoding utf8
Write-Host "Log: $((Get-Item $log).Length) bytes"
Write-Host "PID: $((& adb -s $dev shell pidof $pkg).Trim())"
Write-Host ""
Write-Host "=== FATAL / native crash ==="
Get-Content $log | Select-String -Pattern "FATAL EXCEPTION|AndroidRuntime: |JNI DETECTED|UnsatisfiedLink|tombstone|signal 11|signal 6|libling_chat|libc\+\+_shared|backtrace:|DEBUG " | Select-Object -First 60
Write-Host ""
Write-Host "=== app-specific (last 60) ==="
Get-Content $log | Where-Object { $_ -match "com\.noiq\.lingchat|noiq|libling|TauriActivity|WryActivity|WryLifecycleObserver" } | Select-Object -Last 60
