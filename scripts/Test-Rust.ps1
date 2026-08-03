$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    # CLI integration tests launch CARGO_BIN_EXE_myalbuns-imaging. Build the
    # executable explicitly so an incremental workspace test cannot reuse an
    # older sidecar after only the shared protocol crate changes.
    & $script:CargoExecutable build `
        -p myalbuns-imaging `
        --bin myalbuns-imaging
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $script:CargoExecutable test --workspace
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
