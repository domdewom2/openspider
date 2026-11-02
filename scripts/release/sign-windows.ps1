# Tauri signCommand: payloads, NSIS uninstaller and both installers.
[CmdletBinding()]
param([Parameter(Mandatory, Position=0)][string]$Path)
$ErrorActionPreference = 'Stop'
foreach ($name in @('OPENSPIDER_SIGNTOOL', 'OPENSPIDER_SIGN_DLIB', 'OPENSPIDER_SIGN_METADATA')) {
    if (-not [Environment]::GetEnvironmentVariable($name)) { throw "Signierkonfiguration fehlt: $name" }
}
if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw 'Signierdatei fehlt' }
# The GitHub OIDC assertion behind azure/login expires after about five minutes, while
# signing runs much later (CLI build, Tauri bundle). Re-login with a fresh assertion when
# the last login is older than four minutes. Token and az output never reach the log.
if ($env:ACTIONS_ID_TOKEN_REQUEST_URL -and $env:ACTIONS_ID_TOKEN_REQUEST_TOKEN -and $env:AZURE_CLIENT_ID -and $env:AZURE_TENANT_ID) {
    $stamp = Join-Path ($env:RUNNER_TEMP ?? [IO.Path]::GetTempPath()) 'openspider-azure-login'
    $fresh = (Test-Path -LiteralPath $stamp) -and ((Get-Date) - (Get-Item -LiteralPath $stamp).LastWriteTime).TotalMinutes -lt 4
    if (-not $fresh) {
        $uri = "$($env:ACTIONS_ID_TOKEN_REQUEST_URL)&audience=api://AzureADTokenExchange"
        $assertion = (Invoke-RestMethod -Uri $uri -Headers @{ Authorization = "Bearer $($env:ACTIONS_ID_TOKEN_REQUEST_TOKEN)" }).value
        if (-not $assertion) { throw 'OIDC-Token für Azure fehlt' }
        az login --service-principal --username $env:AZURE_CLIENT_ID --tenant $env:AZURE_TENANT_ID --federated-token $assertion --allow-no-subscriptions --output none 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Azure-Anmeldung mit frischem OIDC-Token fehlgeschlagen' }
        if ($env:AZURE_SUBSCRIPTION_ID) { az account set --subscription $env:AZURE_SUBSCRIPTION_ID --output none 2>$null | Out-Null }
        Set-Content -LiteralPath $stamp -Value '' -NoNewline
    }
}
# No /debug or /v: credential diagnostics and metadata must not enter logs.
& $env:OPENSPIDER_SIGNTOOL sign /fd SHA256 /td SHA256 /tr http://timestamp.acs.microsoft.com /dlib $env:OPENSPIDER_SIGN_DLIB /dmdf $env:OPENSPIDER_SIGN_METADATA $Path
if ($LASTEXITCODE -ne 0) { throw 'Authenticode-Signierung fehlgeschlagen' }
$sig = Get-AuthenticodeSignature -LiteralPath $Path
if ($sig.Status -ne 'Valid' -or -not $sig.TimeStamperCertificate) {
    throw 'Authenticode-Signatur oder Zeitstempel ungültig'
}
