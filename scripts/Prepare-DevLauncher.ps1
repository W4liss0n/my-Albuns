$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

& (Join-Path $PSScriptRoot 'Prepare-Sidecar.ps1') -Profile debug
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Push-Location $script:WorkspaceRoot
try {
    & $script:CargoExecutable build `
        -p myalbuns-desktop `
        --bin myalbuns-dev `
        --features dev-supervisor
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $launcher = Join-Path $script:WorkspaceRoot 'target\debug\myalbuns-dev.exe'
    if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) {
        throw "The development supervisor was not produced at $launcher."
    }
}
finally {
    Pop-Location
}
