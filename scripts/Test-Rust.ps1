$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
$cargoTargetDirectory = Resolve-MyAlbunsCargoTargetDirectory

# Cargo reports success when a filter matches no test, so a renamed test would
# stop running without any failure. Each call states how many tests it selects.
function Invoke-RealProcessorTests {
    param(
        [Parameter(Mandatory)] [string] $Filter,
        [Parameter(Mandatory)] [int] $ExpectedCount,
        [switch] $Exact
    )

    $testArguments = @('--ignored', '--test-threads=1')
    if ($Exact) {
        $testArguments += '--exact'
    }
    & $script:CargoExecutable test -p myalbuns-desktop $Filter -- @testArguments |
        Tee-Object -Variable testOutput
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $passed = 0
    foreach ($line in $testOutput) {
        if ([string] $line -match 'test result: ok\. (\d+) passed') {
            $passed += [int] $Matches[1]
        }
    }
    if ($passed -ne $ExpectedCount) {
        Write-Error "'$Filter' ran $passed real-Processor tests; expected $ExpectedCount." -ErrorAction Continue
        exit 1
    }
}

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

    & $script:CargoExecutable test `
        -p myalbuns-desktop `
        --bin myalbuns-dev `
        --features dev-supervisor
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $script:CargoExecutable build `
        -p myalbuns-imaging `
        --bin myalbuns-imaging
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    # Desktop builds copy the packaged sidecar into debug/. Preserve the debug
    # Processor before a later desktop build can replace it with a release one,
    # which intentionally ignores the integration tests' isolated data root.
    $testProcessorDirectory = Join-Path $cargoTargetDirectory 'integration-test-processor'
    New-Item -ItemType Directory -Force -Path $testProcessorDirectory | Out-Null
    $testProcessorSource = Join-Path `
        $cargoTargetDirectory `
        'debug\myalbuns-imaging.exe'
    $env:MYALBUNS_TEST_IMAGING_PROCESSOR = Join-Path `
        $testProcessorDirectory `
        'myalbuns-imaging.exe'
    Copy-Item -LiteralPath $testProcessorSource `
        -Destination $env:MYALBUNS_TEST_IMAGING_PROCESSOR -Force
    Invoke-RealProcessorTests `
        -Filter 'project_host::tests::reopened_project_exports_the_frozen_visible_sheet_through_the_real_processor' `
        -ExpectedCount 1 `
        -Exact

    Invoke-RealProcessorTests `
        -Filter 'batch_runner::tests::real_processor_exports_persisted_batches_in_every_format' `
        -ExpectedCount 1 `
        -Exact

    # Every ignored test of the module: the import flow and its two variants.
    Invoke-RealProcessorTests `
        -Filter 'photo_import::native_flow_tests::' `
        -ExpectedCount 3

    & $script:CargoExecutable test -p myalbuns-imaging
    exit $LASTEXITCODE
}
finally {
    $env:MYALBUNS_TEST_IMAGING_PROCESSOR = $previousTestProcessor
    Pop-Location
}
