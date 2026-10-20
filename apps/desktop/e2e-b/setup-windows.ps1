$ErrorActionPreference = 'Stop'
# Match WebView2 itself (the Edge browser can be on a different update channel).
$runtime = Join-Path ${env:ProgramFiles(x86)} 'Microsoft\EdgeWebView\Application'
$binary = Get-ChildItem $runtime -Filter msedgewebview2.exe -Recurse |
    Sort-Object { [version]$_.VersionInfo.ProductVersion } -Descending | Select-Object -First 1
if (-not $binary) { throw 'WebView2 runtime is not installed' }
$version = $binary.VersionInfo.ProductVersion
$destination = Join-Path $env:RUNNER_TEMP 's02-edgedriver'
New-Item -ItemType Directory -Force $destination | Out-Null
$archive = Join-Path $destination 'driver.zip'
Invoke-WebRequest "https://msedgedriver.microsoft.com/$version/edgedriver_win64.zip" -OutFile $archive
Expand-Archive $archive -DestinationPath $destination -Force
$destination | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
& (Join-Path $destination 'msedgedriver.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'msedgedriver smoke failed' }
