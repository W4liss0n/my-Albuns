param([string] $OutputPath)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = 'docs\research\artifacts\0039-issue-15-session-recovery.json'
}

& (Join-Path $PSScriptRoot 'Test-ProductiveJourney.ps1') `
    -OutputPath $OutputPath
if ($LASTEXITCODE -ne 0) {
    throw "The Issue #15 session recovery journey failed with exit code $LASTEXITCODE."
}
