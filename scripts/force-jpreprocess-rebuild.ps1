
$paths = @(
  "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\target\debug\.fingerprint\jpreprocess-naist-jdic-*",
  "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\target\debug\build\jpreprocess-naist-jdic-*",
  "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\target\release\.fingerprint\jpreprocess-naist-jdic-*",
  "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\target\release\build\jpreprocess-naist-jdic-*",
  "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\target\aarch64-linux-android\release\.fingerprint\jpreprocess-naist-jdic-*",
  "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\target\aarch64-linux-android\release\build\jpreprocess-naist-jdic-*"
)
$total = 0
foreach ($p in $paths) {
  $items = Get-Item -Path $p -ErrorAction SilentlyContinue
  foreach ($it in $items) {
    Remove-Item -LiteralPath $it.FullName -Recurse -Force -ErrorAction SilentlyContinue
    $total++
  }
}
Write-Host "Removed $total cached dirs"

$buildRs = "C:\Users\DCY45\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\jpreprocess-naist-jdic-0.13.2\build.rs"
(Get-Item $buildRs).LastWriteTime = Get-Date
Get-Item $buildRs | Select-Object LastWriteTime, FullName
