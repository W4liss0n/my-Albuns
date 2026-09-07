param([string] $OutputPath, [switch] $AllowVisibleWindows)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = 'docs\research\artifacts\0039-issue-15-session-recovery.json'
}

& (Join-Path $PSScriptRoot 'Test-ProductiveJourney.ps1') `
    -OutputPath $OutputPath `
    -AllowVisibleWindows:$AllowVisibleWindows
if ($LASTEXITCODE -ne 0) {
    throw "The Issue #15 session recovery journey failed with exit code $LASTEXITCODE."
}
