$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$applicationRoots = @(
    (Join-Path ${env:ProgramFiles(x86)} 'Microsoft\Edge\Application'),
    (Join-Path $env:ProgramFiles 'Microsoft\Edge\Application')
) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }

$edgeExecutable = $applicationRoots |
    ForEach-Object { Join-Path $_ 'msedge.exe' } |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
    Select-Object -First 1
if (-not $edgeExecutable) {
    throw 'Microsoft Edge was not found in its standard Windows installation directories.'
}

$edgeVersion = (Get-Item -LiteralPath $edgeExecutable).VersionInfo.ProductVersion
$parsedVersion = $null
if (-not [version]::TryParse($edgeVersion, [ref]$parsedVersion)) {
    throw "Microsoft Edge reported an invalid version: $edgeVersion"
}

$driverRoot = Join-Path $workspaceRoot ".tools\edge-driver\$edgeVersion"
$driverExecutable = Join-Path $driverRoot 'msedgedriver.exe'
if (-not (Test-Path -LiteralPath $driverExecutable -PathType Leaf)) {
    New-Item -ItemType Directory -Force -Path $driverRoot | Out-Null
    $archive = Join-Path $driverRoot 'edgedriver_win64.zip'
    Invoke-WebRequest `
        -UseBasicParsing `
        -Uri "https://msedgedriver.microsoft.com/$edgeVersion/edgedriver_win64.zip" `
        -OutFile $archive `
        -TimeoutSec 120
    Expand-Archive -LiteralPath $archive -DestinationPath $driverRoot -Force
}
if (-not (Test-Path -LiteralPath $driverExecutable -PathType Leaf)) {
    throw "Microsoft Edge WebDriver $edgeVersion was not materialized."
}

$driverVersionOutput = (& $driverExecutable --version).Trim()
if ($LASTEXITCODE -ne 0 -or $driverVersionOutput -notmatch [regex]::Escape($edgeVersion)) {
    throw "Microsoft Edge WebDriver '$driverVersionOutput' does not match Edge $edgeVersion."
}

[ordered]@{
    edgeExecutable = [System.IO.Path]::GetFullPath($edgeExecutable)
    edgeVersion = $edgeVersion
    driverExecutable = [System.IO.Path]::GetFullPath($driverExecutable)
    driverVersion = $edgeVersion
} | ConvertTo-Json -Compress
