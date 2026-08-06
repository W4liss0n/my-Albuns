$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
$previousTestProcessor = $env:MYALBUNS_TEST_IMAGING_PROCESSOR
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

    $env:MYALBUNS_TEST_IMAGING_PROCESSOR = Join-Path `
        $script:WorkspaceRoot `
        'target\debug\myalbuns-imaging.exe'
    & $script:CargoExecutable test `
        -p myalbuns-desktop `
        'project_host::tests::reopened_project_exports_the_frozen_visible_sheet_through_the_real_processor' `
        -- `
        --ignored `
        --exact `
        --test-threads=1
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $script:CargoExecutable test -p myalbuns-imaging
    exit $LASTEXITCODE
}
finally {
    $env:MYALBUNS_TEST_IMAGING_PROCESSOR = $previousTestProcessor
    Pop-Location
}
