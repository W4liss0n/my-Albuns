param(
    [string] $OutputPath,
    [string] $BuildManifestPath = '.tools\native-gate-build.json',
    [switch] $AllowVisibleWindows
)

$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'Invoke-FocusedNativeGate.ps1') -GateName 'saved-original-close' -Scenario 'saved-original-close' @PSBoundParameters
