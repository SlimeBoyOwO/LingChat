$ErrorActionPreference = 'Stop'

$roots = @(
    'src-tauri/src',
    'src-tauri/crates/sbv2-local-tts/src'
)
$utf8 = [System.Text.UTF8Encoding]::new($false, $true)
$missingBom = @()
$invalidUtf8 = @()

foreach ($root in $roots) {
    Get-ChildItem -LiteralPath $root -Recurse -File -Filter '*.rs' | ForEach-Object {
        $bytes = [System.IO.File]::ReadAllBytes($_.FullName)
        try {
            $text = $utf8.GetString($bytes)
        } catch {
            $invalidUtf8 += $_.FullName
            return
        }

        if ($text -match '[^\x00-\x7F]') {
            $hasBom = $bytes.Length -ge 3 `
                -and $bytes[0] -eq 0xEF `
                -and $bytes[1] -eq 0xBB `
                -and $bytes[2] -eq 0xBF
            if (-not $hasBom) {
                $missingBom += $_.FullName
            }
        }
    }
}

if ($invalidUtf8.Count -gt 0) {
    throw "Invalid UTF-8 Rust sources:`n$($invalidUtf8 -join "`n")"
}
if ($missingBom.Count -gt 0) {
    throw "Non-ASCII Rust sources without UTF-8 BOM:`n$($missingBom -join "`n")"
}

Write-Host 'Rust source encoding tests passed'
