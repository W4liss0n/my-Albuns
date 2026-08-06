$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    & $script:CargoExecutable fmt --all --check
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $script:CargoExecutable clippy --workspace --all-targets -- -D warnings
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
