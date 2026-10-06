$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    & $script:CargoExecutable fmt --all --check
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    # The feature only gates the `myalbuns-dev` binary, so one pass covers it.
    & $script:CargoExecutable clippy `
        --workspace `
        --all-targets `
        --features myalbuns-desktop/dev-supervisor `
        -- `
        -D warnings
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
