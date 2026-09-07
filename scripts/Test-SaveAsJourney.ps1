param([string] $OutputPath, [switch] $AllowVisibleWindows)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = 'docs\research\artifacts\0038-issue-18-save-as-journey.json'
}

& (Join-Path $PSScriptRoot 'Test-ProductiveJourney.ps1') `
    -OutputPath $OutputPath `
    -AllowVisibleWindows:$AllowVisibleWindows
if ($LASTEXITCODE -ne 0) {
    throw "The Issue #18 Save As journey failed with exit code $LASTEXITCODE."
}
