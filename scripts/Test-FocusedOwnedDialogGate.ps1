param(
    [string] $OutputPath,
    [ValidateSet('all', 'external-copy-opening-owner', 'late-graphics-project-dialog')]
    [string] $Scenario = 'all',
    [string] $BuildManifestPath = '.tools\native-gate-build.json',
    [switch] $AllowVisibleWindows
)

$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'Invoke-FocusedNativeGate.ps1') -GateName 'focused-owned-dialogs' @PSBoundParameters
