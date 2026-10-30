[CmdletBinding()]
param([Parameter(Mandatory)][string[]]$Path, [switch]$AllowUnsigned)
$ErrorActionPreference = 'Stop'
$files = @($Path | ForEach-Object {
    $item = Get-Item -LiteralPath $_
    if ($item.PSIsContainer) { Get-ChildItem -LiteralPath $_ -Recurse -File } else { $item }
} | Where-Object { $_.Extension -in @('.exe', '.dll', '.msi') } | Sort-Object FullName -Unique)
if ($files.Count -eq 0) { throw 'Keine Windows-Binärdateien gefunden' }
foreach ($file in $files) {
    $sig = Get-AuthenticodeSignature -LiteralPath $file.FullName
    if ($AllowUnsigned -and $sig.Status -eq 'NotSigned') {
        Write-Output "UNSIGNIERT: $($file.Name)"
        continue
    }
    if ($sig.Status -ne 'Valid' -or -not $sig.TimeStamperCertificate) {
        throw "Signatur/Zeitstempel ungültig: $($file.Name) ($($sig.Status))"
    }
    Write-Output "VALID: $($file.Name)"
}
