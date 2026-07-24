$hashes = @("3c394880b5ee06c2")
$src = "C:\Users\DCY45\AppData\Local\Temp\jpreprocess-dict\extracted\naist-jdic"
$dstBase = "C:\Users\DCY45\Desktop\LingChat-refactor\src-tauri\target\aarch64-linux-android\release\build"
foreach ($h in $hashes) {
  $dictDir = Join-Path $dstBase "jpreprocess-naist-jdic-$h\out\naist-jdic"
  if (-not (Test-Path $dictDir)) {
    New-Item -ItemType Directory -Force -Path $dictDir | Out-Null
    Write-Host "Created $dictDir"
  } else {
    Write-Host "Exists: $dictDir"
  }
  $existing = @(Get-ChildItem $dictDir -ErrorAction SilentlyContinue)
  if ($existing.Count -gt 0) {
    Write-Host "Already populated ($($existing.Count) files), skipping copy"
  } else {
    Copy-Item -Recurse -Force "$src\*" $dictDir
    Write-Host "Copied $($existing.Count) files to $dictDir"
  }
  Get-ChildItem $dictDir | Select-Object Name, Length
}
