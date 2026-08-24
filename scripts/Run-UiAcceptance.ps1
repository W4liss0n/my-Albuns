param(
    [string] $OutputPath
)

$ErrorActionPreference = 'Stop'
$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$node = (Get-Command node.exe -ErrorAction Stop).Source
$viteEntry = Join-Path $workspaceRoot 'node_modules\vite\bin\vite.js'
if (-not (Test-Path -LiteralPath $viteEntry -PathType Leaf)) {
    throw 'Vite is not installed. Run npm ci before the UI acceptance workflow.'
}

$runRoot = if ($OutputPath) {
    [System.IO.Path]::GetFullPath($OutputPath)
}
else {
    $stamp = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff')
    Join-Path $workspaceRoot ".scratch\ui-acceptance\$stamp"
}
New-Item -ItemType Directory -Force -Path $runRoot | Out-Null

$unitTests = Join-Path $PSScriptRoot 'Test-UiAcceptance.mjs'
& $node --test $unitTests
if ($LASTEXITCODE -ne 0) {
    throw "The UI acceptance contract tests failed with exit code $LASTEXITCODE."
}

$edge = & (Join-Path $PSScriptRoot 'Resolve-EdgeWebDriver.ps1') |
    ConvertFrom-Json
$env:MYALBUNS_UI_BROWSER_VERSION = $edge.edgeVersion
$env:MYALBUNS_UI_DRIVER_VERSION = $edge.driverVersion

$runner = Join-Path $PSScriptRoot 'Run-UiAcceptance.mjs'
& $node `
    $runner `
    $workspaceRoot `
    $runRoot `
    $edge.edgeExecutable `
    $edge.driverExecutable
if ($LASTEXITCODE -ne 0) {
    throw "UI acceptance failed. Inspect retained evidence at $runRoot"
}
