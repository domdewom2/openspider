# Called by NSIS and MSI in the installing/removing user's context.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Install', 'Uninstall')][string]$Mode,
    [Parameter(Mandatory)][string]$InstallDir,
    [string]$RemoveUserData = '0',
    [switch]$Machine
)
$ErrorActionPreference = 'Stop'
if ($Mode -eq 'Uninstall') {
    & (Join-Path $InstallDir 'openspider.exe') --uninstall-cleanup
    if ($LASTEXITCODE -ne 0) { throw 'Aufgabenbereinigung fehlgeschlagen' }
}
# MSI owns its machine PATH component (including removal on uninstall).
if (-not $Machine) {
    $entries = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' |
        Where-Object { $_ -and $_.TrimEnd('\') -ine $InstallDir.TrimEnd('\') })
    if ($Mode -eq 'Install') { $entries += $InstallDir }
    [Environment]::SetEnvironmentVariable('Path', ($entries -join ';'), 'User')
}
if ($Mode -eq 'Uninstall' -and $RemoveUserData -eq '1') {
    # Never follow an override to an arbitrary directory or delete another user's profile.
    foreach ($base in @($env:APPDATA, $env:LOCALAPPDATA) | Select-Object -Unique) {
        if (-not $base) { throw 'Benutzerverzeichnis fehlt' }
        $data = Join-Path $base 'OpenSpider'
        if (Test-Path -LiteralPath $data) {
            if ((Get-Item -LiteralPath $data).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Benutzerdaten sind ein Verweis; bitte manuell entfernen'
            }
            Remove-Item -LiteralPath $data -Recurse -Force
        }
    }
}
