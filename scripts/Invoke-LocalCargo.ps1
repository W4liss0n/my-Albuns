param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $CargoArguments
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    & $script:CargoExecutable @CargoArguments
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
