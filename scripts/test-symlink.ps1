$src = "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\target\aarch64-linux-android\release\libling_chat_lib.so"
$dstDir = "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\gen\android\app\src\main\jniLibs\arm64-v8a"
$dst = Join-Path $dstDir "libling_chat_lib.so"
Get-Item $src | Select-Object Length, FullName
New-Item -ItemType Directory -Force -Path $dstDir | Out-Null
if (Test-Path $dst) { Remove-Item $dst -Force }
try {
  New-Item -ItemType SymbolicLink -Path $dst -Target $src -ErrorAction Stop
  Write-Host "SYMLINK OK (PS)"
  if (Test-Path $dst) { Remove-Item $dst -Force }
} catch {
  Write-Host "SYMLINK FAIL (PS): $($_.Exception.Message)"
}
try {
  $out = cmd /c mklink "$dst" "$src" 2>&1
  Write-Host "MKLINK: $out"
  if (Test-Path $dst) { Remove-Item $dst -Force }
} catch {
  Write-Host "MKLINK FAIL: $($_.Exception.Message)"
}
