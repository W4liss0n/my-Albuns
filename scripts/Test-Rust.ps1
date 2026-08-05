$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    # Cargo 1.97 can rematerialize a stale top-level binary while testing the
    # complete workspace, even after an explicit build. Keep the processor out
    # of that pass, then build and test it in one package-scoped sequence so
    # CARGO_BIN_EXE_myalbuns-imaging names the executable just produced.
    & $script:CargoExecutable test `
        --workspace `
        --exclude myalbuns-imaging
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $script:CargoExecutable build `
        -p myalbuns-imaging `
        --bin myalbuns-imaging
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $script:CargoExecutable test -p myalbuns-imaging
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
