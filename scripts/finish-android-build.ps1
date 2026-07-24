$root = "C:\Users\DCY45\Desktop\LingChat-refactor"
$ndk = "C:\Users\DCY45\AppData\Local\Android\Sdk\ndk\28.2.13676358\toolchains\llvm\prebuilt\windows-x86_64\sysroot\usr\lib"
$so = Join-Path $root "src-tauri\target\aarch64-linux-android\release\libling_chat_lib.so"
$jni = Join-Path $root "src-tauri\gen\android\app\src\main\jniLibs\arm64-v8a\libling_chat_lib.so"
$jniDir = Join-Path $root "src-tauri\gen\android\app\src\main\jniLibs\arm64-v8a"
$cxxSharedSrc = Join-Path $ndk "aarch64-linux-android\libc++_shared.so"
$androidDir = Join-Path $root "src-tauri\gen\android"

$env:ANDROID_HOME = "C:\Users\DCY45\AppData\Local\Android\Sdk"
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\28.2.13676358"
$env:ANDROID_NDK_ROOT = $env:ANDROID_NDK_HOME
$env:JAVA_HOME = "C:\Program Files\Java\jdk-21.0.10"

# 1) Pre-place the .so where AGP picks it up (avoids tauri-cli symlink)
New-Item -ItemType Directory -Force -Path (Split-Path $jni) | Out-Null
if (Test-Path $jni) { Remove-Item -LiteralPath $jni -Force }
Copy-Item -LiteralPath $so -Destination $jni
Write-Host "Copied $so -> $jni ($((Get-Item $jni).Length) bytes)"
# Always ship the NDK libc++_shared.so alongside (wry/ort link against it)
$cxxDst = Join-Path $jniDir "libc++_shared.so"
if (Test-Path $cxxSharedSrc) {
  if (Test-Path $cxxDst) { Remove-Item -LiteralPath $cxxDst -Force }
  Copy-Item -LiteralPath $cxxSharedSrc -Destination $cxxDst
  Write-Host "Copied libc++_shared.so ($((Get-Item $cxxDst).Length) bytes)"
} else {
  Write-Host "WARN: $cxxSharedSrc not found"
}

# 2) Run gradle directly, skipping the rustBuild tasks (those invoke tauri-cli which tries to symlink)
$log = Join-Path $root "gradle.log"
if (Test-Path $log) { Remove-Item -LiteralPath $log -Force }
Set-Location $androidDir
$args = @("assembleArm64Release",
  "-x", "rustBuildArm64Release",
  "-x", "rustBuildUniversalRelease",
  "-x", "rustBuildArm64Debug",
  "-x", "rustBuildUniversalDebug",
  "--no-daemon",
  "--stacktrace")
& .\gradlew.bat $args 2>&1 | Out-File -FilePath $log -Encoding utf8
Write-Host "Gradle exit=$LASTEXITCODE"
Write-Host "---- tail of gradle.log ----"
Get-Content $log -Tail 80 -ErrorAction SilentlyContinue
