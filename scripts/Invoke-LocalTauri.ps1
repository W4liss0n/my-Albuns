param(
    [ValidateSet('dev', 'build')]
    [string] $Action = 'dev',

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $TauriArguments
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

$sidecarProfile = if (
    $Action -eq 'build' -and
    $TauriArguments -notcontains '--debug'
) {
    'release'
}
else {
    'debug'
}
& (Join-Path $PSScriptRoot 'Prepare-Sidecar.ps1') -Profile $sidecarProfile
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$tauriCommand = Join-Path $script:WorkspaceRoot 'node_modules\.bin\tauri.cmd'
if (-not (Test-Path -LiteralPath $tauriCommand)) {
    throw 'A CLI local do Tauri não existe. Execute npm run setup:local.'
}

Push-Location $script:WorkspaceRoot
try {
    & $tauriCommand $Action @TauriArguments
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
