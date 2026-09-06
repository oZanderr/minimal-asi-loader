# Builds every proxy variant and drops the correctly-named DLLs into dist/.
# Use the one matching the name the game imports (e.g. version.dll).
$ErrorActionPreference = "Stop"
$names = @("version", "dsound", "dxgi", "winmm", "dinput8")

New-Item -ItemType Directory -Force -Path dist | Out-Null
foreach ($n in $names) {
    Write-Host "Building $n.dll ..."
    cargo build --release --features $n
    Copy-Item "target\release\minimal_asi_loader.dll" "dist\$n.dll" -Force
}
Write-Host "`nDone. Artifacts in dist\:"
Get-ChildItem dist\*.dll | Select-Object Name, @{N = "KB"; E = { [int]($_.Length / 1KB) } }
