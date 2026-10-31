[CmdletBinding()]
param([Parameter(Mandatory)][string]$Version, [Parameter(Mandatory)][string]$Repository)
$ErrorActionPreference = 'Stop'
# Hosted Server images do not guarantee App Installer. Use Microsoft's MIT client module.
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Install-Module Microsoft.WinGet.Client -RequiredVersion 1.12.440 -Repository PSGallery -Force -Scope CurrentUser
    Import-Module Microsoft.WinGet.Client -RequiredVersion 1.12.440
    Repair-WinGetPackageManager -Version '1.12.440' -AllUsers
    $env:PATH += ";$env:LOCALAPPDATA\Microsoft\WindowsApps"
}
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) { throw 'WinGet bootstrap failed' }
& winget --info
if ($LASTEXITCODE -ne 0) { throw 'WinGet is not functional' }
function Invoke-Winget([string[]]$Arguments) {
    & winget @Arguments
    if ($LASTEXITCODE -ne 0) { throw "winget failed ($LASTEXITCODE): $($Arguments[0])" }
}
$ready = Join-Path $env:RUNNER_TEMP 'p01-url.txt'
Remove-Item $ready -ErrorAction SilentlyContinue
$server = Start-Process node -ArgumentList @('packaging/homebrew/serve.mjs', 'incoming', "`"$ready`"") -PassThru -NoNewWindow
try {
    for ($n = 0; $n -lt 100 -and -not (Test-Path $ready); $n++) { Start-Sleep -Milliseconds 100 }
    $base = Get-Content $ready -Raw
    # This identifier is deliberately confined to the disposable CI manifest.
    node packaging/homebrew/generate.mjs --directory incoming --output generated --version $Version --repository $Repository --mode winget --package-id OpenSpider.CITest --base-url $base
    if ($LASTEXITCODE -ne 0) { throw 'Manifest generation failed' }
    Invoke-Winget -Arguments @('settings', '--enable', 'LocalManifestFiles')
    Invoke-Winget -Arguments @('validate', '--manifest', 'generated', '--disable-interactivity')
    foreach ($scope in @('machine', 'user')) {
        $install = Join-Path $env:RUNNER_TEMP "P01-$scope"
        $installed = $false
        $data = Join-Path $env:APPDATA 'OpenSpider/p01-retain.txt'
        $taskName = "p01-$scope"
        $primaryError = $null
        try {
            Invoke-Winget -Arguments @('install', '--manifest', 'generated', '--scope', $scope, '--architecture', 'x64', '--silent', '--location', $install,
                '--accept-package-agreements', '--accept-source-agreements', '--disable-interactivity')
            $installed = $true
            $cli = Join-Path $install 'openspider.exe'
            if (-not (Test-Path $cli)) { throw 'Installed CLI missing' }
            $installedVersion = (& $cli --version) -join "`n"
            if ($LASTEXITCODE -ne 0) { throw 'Installed CLI failed' }
            Write-Output $installedVersion
            if ($installedVersion -notmatch ('(?:^|\s)' + [regex]::Escape($Version) + '(?:\s|$)')) { throw 'Installed CLI version differs from checkout' }
            $target = if ($scope -eq 'machine') { 'Machine' } else { 'User' }
            $pathEntries = [Environment]::GetEnvironmentVariable('Path', $target) -split ';'
            if (-not ($pathEntries | Where-Object { $_.TrimEnd('\') -ieq $install.TrimEnd('\') })) { throw 'PATH entry missing' }
            New-Item -ItemType Directory -Force (Split-Path $data) | Out-Null
            Set-Content $data 'retain'
            $action = New-ScheduledTaskAction -Execute $cli -Argument '--version'
            $scheduler = New-Object -ComObject Schedule.Service
            $scheduler.Connect()
            try { $null = $scheduler.GetFolder('\OpenSpider') }
            catch { $null = $scheduler.GetFolder('\').CreateFolder('OpenSpider') }
            Register-ScheduledTask -TaskPath '\OpenSpider\' -TaskName $taskName -Action $action -Force | Out-Null
        } catch {
            $primaryError = $_
            throw
        } finally {
            if ($installed) {
                try {
                    # Match the actual ARP registration: local test IDs do not exist in a public source.
                    Invoke-Winget -Arguments @('uninstall', '--name', 'OpenSpider', '--exact', '--silent', '--disable-interactivity')
                    for ($n = 0; $n -lt 30 -and (Test-Path (Join-Path $install 'openspider.exe')); $n++) { Start-Sleep -Seconds 1 }
                    if (Test-Path (Join-Path $install 'openspider.exe')) { throw 'CLI remains after uninstall' }
                    if (Get-ScheduledTask -TaskPath '\OpenSpider\' -TaskName $taskName -ErrorAction SilentlyContinue) { throw 'Scheduled task remains' }
                    $entries = [Environment]::GetEnvironmentVariable('Path', $target) -split ';'
                    if ($entries | Where-Object { $_.TrimEnd('\') -ieq $install.TrimEnd('\') }) { throw 'PATH entry remains' }
                    if ((Get-Content $data -Raw).Trim() -ne 'retain') { throw 'User data removed by default' }
                    $links = @(Get-ChildItem "$env:APPDATA/Microsoft/Windows/Start Menu/Programs", "$env:ProgramData/Microsoft/Windows/Start Menu/Programs" -Recurse -Filter '*OpenSpider*.lnk')
                    if ($links.Count) { throw 'Start menu shortcut remains' }
                } catch {
                    if ($null -ne $primaryError) { Write-Warning "Cleanup also failed: $_" } else { throw }
                }
            }
            Unregister-ScheduledTask -TaskPath '\OpenSpider\' -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
            Remove-Item $data -ErrorAction SilentlyContinue
        }
        "ok: winget $scope silent install/uninstall, CLI, PATH, tasks, start menu and retained user data" >> $env:GITHUB_STEP_SUMMARY
    }
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
