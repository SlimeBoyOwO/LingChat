$pkg = "com.noiq.lingchat"
$dev = "95379e0c"
$log = "C:\Users\DCY45\Desktop\LingChat-refactor\adb-crash.log"

& adb -s $dev shell am force-stop $pkg
& adb -s $dev logcat -c

# Launch via monkey (handles activity resolution correctly)
& adb -s $dev shell monkey -p $pkg -c android.intent.category.LAUNCHER 1 2>&1 | Select-Object -First 5
Start-Sleep -Seconds 8

# Capture full logcat at INFO+ (need to see the crash details)
& adb -s $dev logcat -d -v threadtime | Out-File -FilePath $log -Encoding utf8
Write-Host "Log: $((Get-Item $log).Length) bytes"

# Find app PID and inspect its logs
$pids = & adb -s $dev shell pidof $pkg
Write-Host "PID: $pids"

Write-Host ""
Write-Host "=== AndroidRuntime / FATAL ==="
Get-Content $log | Select-String -Pattern "FATAL|AndroidRuntime|JNI DETECTED|UnsatisfiedLink|JniError|tombstone|signal 11|signal 6|backtrace|libling_chat_lib" | Select-Object -First 80

Write-Host ""
Write-Host "=== com.noiq.lingchat (last 80) ==="
Get-Content $log | Where-Object { $_ -match "com\.noiq\.lingchat" } | Select-Object -Last 80
