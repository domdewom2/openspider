# Tauri signCommand: payloads, NSIS uninstaller and both installers.
[CmdletBinding()]
param([Parameter(Mandatory, Position=0)][string]$Path)
$ErrorActionPreference = 'Stop'
foreach ($name in @('OPENSPIDER_SIGNTOOL', 'OPENSPIDER_SIGN_DLIB', 'OPENSPIDER_SIGN_METADATA')) {
    if (-not [Environment]::GetEnvironmentVariable($name)) { throw "Signierkonfiguration fehlt: $name" }
}
if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw 'Signierdatei fehlt' }
# No /debug or /v: credential diagnostics and metadata must not enter logs.
& $env:OPENSPIDER_SIGNTOOL sign /fd SHA256 /td SHA256 /tr http://timestamp.acs.microsoft.com /dlib $env:OPENSPIDER_SIGN_DLIB /dmdf $env:OPENSPIDER_SIGN_METADATA $Path
if ($LASTEXITCODE -ne 0) { throw 'Authenticode-Signierung fehlgeschlagen' }
$sig = Get-AuthenticodeSignature -LiteralPath $Path
if ($sig.Status -ne 'Valid' -or -not $sig.TimeStamperCertificate) {
    throw 'Authenticode-Signatur oder Zeitstempel ungültig'
}
