# Started by a scheduled task inside the interactive session of the lab user:
# a GUI app started from PowerShell Direct would run without a desktop.
$ErrorActionPreference = 'Stop'
$launch = Get-Content -LiteralPath 'C:\Lab\launch.json' -Raw -Encoding UTF8 | ConvertFrom-Json
# WebView2 reads this variable for every WebView of the app. Port 0 lets each
# WebView pick a free port and write it to DevToolsActivePort in its profile.
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $launch.browserArguments
$started = [DateTimeOffset]::UtcNow
$arguments = @($launch.projects | ForEach-Object { '"{0}"' -f $_ })
$process = Start-Process -FilePath $launch.executable -ArgumentList $arguments -PassThru
# Written beside the marker and renamed, so the waiting runner never reads a
# partly written file.
$temporary = "$($launch.markerPath).tmp"
[ordered] @{
    launchedAt = $started.ToUnixTimeMilliseconds()
    launchedAtIso = $started.ToString('o')
    processId = $process.Id
} | ConvertTo-Json | Set-Content -LiteralPath $temporary -Encoding UTF8
Move-Item -LiteralPath $temporary -Destination $launch.markerPath -Force
