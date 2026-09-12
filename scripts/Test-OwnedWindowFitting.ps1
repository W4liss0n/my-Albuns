param([string] $OutputPath = '.scratch/ui-acceptance/owned-window-fitting')
$ErrorActionPreference = 'Stop'
$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$edge = & (Join-Path $PSScriptRoot 'Resolve-EdgeWebDriver.ps1') | ConvertFrom-Json
Push-Location $workspaceRoot
try {
    & node (Join-Path $PSScriptRoot 'Run-OwnedWindowFitting.mjs') `
        $edge.edgeExecutable $edge.driverExecutable $OutputPath
    exit $LASTEXITCODE
}
finally { Pop-Location }
