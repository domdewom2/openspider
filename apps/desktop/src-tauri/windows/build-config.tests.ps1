# Deterministic fixtures for K3-07, no real signing key.
$ErrorActionPreference = 'Stop'
$script = Join-Path $PSScriptRoot 'build-config.ps1'
$root = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force (Join-Path $root 'apps/desktop/src-tauri') | Out-Null
Push-Location $root
try {
    $previous = -1
    foreach ($case in @(
        @('1.0.0-beta.0', '1.0.0', 0), @('1.0.0-beta.1', '1.0.1', 1), @('1.0.0-beta.499', '1.0.499', 499),
        @('1.0.0-rc.1', '1.0.500', 500), @('1.0.0-rc.499', '1.0.998', 998),
        @('1.0.0', '1.0.999', 999), @('1.0.1-beta.1', '1.0.1001', 1001)
    )) {
        @{ version = $case[0] } | ConvertTo-Json | Set-Content apps/desktop/src-tauri/tauri.conf.json
        & $script -Output config.json -Channel beta
        $config = Get-Content config.json -Raw | ConvertFrom-Json
        if ($config.bundle.windows.wix.version -ne $case[1]) { throw 'MSI-Versionsabbildung falsch' }
        if ($case[2] -le $previous) { throw 'MSI-Version nicht monoton' }
        if ($config.bundle.createUpdaterArtifacts) { throw 'Updater ohne Schlüssel aktiviert' }
        if ($config.plugins.updater.endpoints[0] -notlike '*/latest-beta.json') { throw 'Beta-URL falsch' }
        $previous = $case[2]
    }
    # Missing event inputs must produce the same beta config as an explicit beta build.
    foreach ($arguments in @(@{}, @{ Channel = '' }, @{ Channel = 'stable' })) {
        & $script -Output config.json @arguments
        $config = Get-Content config.json -Raw | ConvertFrom-Json
        $manifest = if ($arguments.Channel -eq 'stable') { 'latest.json' } else { 'latest-beta.json' }
        if ($config.plugins.updater.endpoints[0] -notlike "*/$manifest") { throw 'Kanal-Rückfall falsch' }
        if ($config.bundle.windows.webviewInstallMode) { throw 'Offline-WebView ohne Eingabe aktiviert' }
        if ($config.bundle.createUpdaterArtifacts) { throw 'Updater ohne Schlüssel aktiviert' }
    }
    foreach ($version in @('256.0.0', '1.256.0', '1.0.65', '1.0.0-rc.0', '1.0.0-beta.500', '1.0.0-rc.500', '1.0.0-other.1')) {
        @{ version = $version } | ConvertTo-Json | Set-Content apps/desktop/src-tauri/tauri.conf.json
        $rejected = $false
        try { & $script -Output config.json } catch { $rejected = $true }
        if (-not $rejected) { throw "Ungültige Version akzeptiert: $version" }
    }
    Write-Output 'OK: MSI-Versionen, Grenzen, Kanal-Rückfall, Offline-Standard und fehlende Updater-Schlüssel'
} finally {
    Pop-Location
    Remove-Item -LiteralPath $root -Recurse -Force
}
