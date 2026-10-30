[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$tools = Join-Path $env:RUNNER_TEMP 'artifact-signing'
New-Item -ItemType Directory -Force $tools | Out-Null
# Official Microsoft SDK + Artifact Signing dlib; tools stay outside the repository.
& nuget install Microsoft.ArtifactSigning.Client -OutputDirectory $tools -NonInteractive -Verbosity quiet
if ($LASTEXITCODE -ne 0) { throw 'Artifact Signing Client konnte nicht installiert werden' }
& nuget install Microsoft.Windows.SDK.BuildTools -OutputDirectory $tools -NonInteractive -Verbosity quiet
if ($LASTEXITCODE -ne 0) { throw 'Windows SDK konnte nicht installiert werden' }
$dlib = @(Get-ChildItem $tools -Recurse -Filter Azure.CodeSigning.Dlib.dll | Where-Object { $_.Directory.Name -eq 'x64' })
$signTool = @(Get-ChildItem $tools -Recurse -Filter signtool.exe | Where-Object { $_.Directory.Name -eq 'x64' })
if ($dlib.Count -ne 1 -or $signTool.Count -ne 1) { throw 'Signierwerkzeuge nicht eindeutig gefunden' }
$metadata = Join-Path $tools 'metadata.json'
@{
    Endpoint = $env:ARTIFACT_SIGNING_ENDPOINT
    CodeSigningAccountName = $env:ARTIFACT_SIGNING_ACCOUNT
    CertificateProfileName = $env:ARTIFACT_SIGNING_PROFILE
    ExcludeCredentials = @(
        'EnvironmentCredential', 'WorkloadIdentityCredential', 'ManagedIdentityCredential',
        'SharedTokenCacheCredential', 'VisualStudioCredential', 'VisualStudioCodeCredential',
        'AzurePowerShellCredential', 'AzureDeveloperCliCredential', 'InteractiveBrowserCredential'
    )
} | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $metadata -Encoding utf8
# AzureCliCredential uses the OIDC login established by azure/login; no client secret.
"OPENSPIDER_SIGNTOOL=$($signTool[0].FullName)" >> $env:GITHUB_ENV
"OPENSPIDER_SIGN_DLIB=$($dlib[0].FullName)" >> $env:GITHUB_ENV
"OPENSPIDER_SIGN_METADATA=$metadata" >> $env:GITHUB_ENV
