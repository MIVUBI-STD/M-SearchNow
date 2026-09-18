param(
    [switch]$CompileChecks
)

$ErrorActionPreference = "Stop"

function Write-Check([string]$Name, [bool]$Ok, [string]$Detail) {
    $status = if ($Ok) { "PASS" } else { "CHECK" }
    Write-Host ("[{0}] {1} - {2}" -f $status, $Name, $Detail)
}

Write-Host "SearchNow Windows smoke readiness (non-destructive)"
Write-Host "This script does not launch Minecraft, modify packs, or write SearchNow settings."

$isWindows = $PSVersionTable.Platform -eq "Win32NT" -or $env:OS -eq "Windows_NT"
Write-Check "Windows host" $isWindows ($PSVersionTable.OS)
if (-not $isWindows) {
    throw "This readiness script is intended for Windows only."
}

Write-Check "LOCALAPPDATA" (-not [string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) ($env:LOCALAPPDATA ?? "missing")
Write-Check "APPDATA" (-not [string]::IsNullOrWhiteSpace($env:APPDATA)) ($env:APPDATA ?? "missing")

$candidates = @()
if ($env:APPDATA) {
    $candidates += Join-Path $env:APPDATA "Minecraft Bedrock\users\shared\games\com.mojang"
    $candidates += Join-Path $env:APPDATA "Minecraft Bedrock Preview\users\shared\games\com.mojang"
}
if ($env:LOCALAPPDATA) {
    $candidates += Join-Path $env:LOCALAPPDATA "Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe\LocalState\games\com.mojang"
    $candidates += Join-Path $env:LOCALAPPDATA "Packages\Microsoft.MinecraftWindowsBeta_8wekyb3d8bbwe\LocalState\games\com.mojang"
}

foreach ($candidate in $candidates) {
    Write-Check "Minecraft candidate" (Test-Path -LiteralPath $candidate) $candidate
}

if ($CompileChecks) {
    Write-Host "Running repository compile checks only; no application launch occurs."
    cargo test --locked --manifest-path EngineData/Backend/RustCore/Cargo.toml
    Push-Location EngineData/Frontend/RustApp
    try {
        npm ci --no-audit --no-fund
        npm run build:frontend
    } finally {
        Pop-Location
    }
    cargo check --locked --manifest-path EngineData/Frontend/RustApp/src-tauri/Cargo.toml
}

Write-Host ""
Write-Host "Deferred manual runtime checklist (run only when local testing is convenient):"
Write-Host "  1. Launch SearchNow and inspect safe backend health/diagnostics."
Write-Host "  2. Confirm Minecraft discovery matches the installed Bedrock channel/account root."
Write-Host "  3. Save one harmless settings toggle, restart, and confirm persistence."
Write-Host "  4. Run one approved local download fixture and confirm finalization/recovery."
Write-Host "  5. Confirm diagnostics contain no absolute input path, token, Authorization, cookie, or signed URL."
Write-Host ""
Write-Host "No local smoke evidence is claimed until those runtime checks are actually performed."
