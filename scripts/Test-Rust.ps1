$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
$cargoTargetDirectory = Resolve-MyAlbunsCargoTargetDirectory

# Builds every test executable of the workspace with one Cargo invocation and
# returns them. A second `cargo test` would compile the desktop crate again:
# Tauri's code generation writes into OUT_DIR while the library compiles, so
# the next invocation finds that output newer than its fingerprint.
function Build-WorkspaceTests {
    $messages = & $script:CargoExecutable test `
        --workspace `
        --exclude myalbuns-imaging `
        --features myalbuns-desktop/dev-supervisor `
        --no-run `
        --message-format=json
    $exitCode = $LASTEXITCODE
    $executables = @()
    foreach ($line in $messages) {
        if (-not ([string] $line).StartsWith('{')) {
            continue
        }
        $message = $line | ConvertFrom-Json
        if ($message.reason -eq 'compiler-message' -and $message.message.rendered) {
            # Diagnostics go to the host so only executables reach the caller.
            Write-Host $message.message.rendered
        }
        elseif ($message.reason -eq 'compiler-artifact' -and $message.executable -and $message.profile.test) {
            $executables += [pscustomobject]@{
                Path = $message.executable
                Directory = Split-Path -Parent $message.manifest_path
                Package = Split-Path -Leaf (Split-Path -Parent $message.manifest_path)
                Target = $message.target.name
            }
        }
    }
    if ($exitCode -ne 0) {
        exit $exitCode
    }
    $executables
}

# Cargo runs each test executable from its package directory.
function Invoke-TestExecutable {
    param(
        [Parameter(Mandatory)] $Executable,
        [string[]] $Arguments = @()
    )

    Push-Location $Executable.Directory
    try {
        Write-Host "Running $($Executable.Package) $($Executable.Target)"
        & $Executable.Path @Arguments | Out-Host
        return $LASTEXITCODE
    }
    finally {
        Pop-Location
    }
}

# A filter that matches no test still succeeds, so a renamed test would stop
# running without any failure. Each call states how many tests it selects.
function Invoke-RealProcessorTests {
    param(
        [Parameter(Mandatory)] $Executable,
        [Parameter(Mandatory)] [string] $Filter,
        [Parameter(Mandatory)] [int] $ExpectedCount,
        [switch] $Exact
    )

    $testArguments = @($Filter, '--ignored', '--test-threads=1')
    if ($Exact) {
        $testArguments += '--exact'
    }
    Push-Location $Executable.Directory
    try {
        & $Executable.Path @testArguments | Tee-Object -Variable testOutput | Out-Host
        $exitCode = $LASTEXITCODE
    }
    finally {
        Pop-Location
    }
    if ($exitCode -ne 0) {
        exit $exitCode
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
    $executables = Build-WorkspaceTests
    foreach ($executable in $executables) {
        $exitCode = Invoke-TestExecutable -Executable $executable
        if ($exitCode -ne 0) {
            exit $exitCode
        }
    }
    $desktopLibrary = $executables |
        Where-Object { $_.Target -eq 'myalbuns_desktop_lib' } |
        Select-Object -First 1
    if (-not $desktopLibrary) {
        throw 'The desktop library test executable was not built.'
    }

    # Cargo 1.97 can rematerialize a stale top-level binary while testing the
    # complete workspace, even after an explicit build. Keep the processor out
    # of that pass and test it package-scoped, so CARGO_BIN_EXE_myalbuns-imaging
    # names the executable just produced; that build is then reused below.
    & $script:CargoExecutable test -p myalbuns-imaging
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
        -Executable $desktopLibrary `
        -Filter 'project_host::tests::reopened_project_exports_the_frozen_visible_sheet_through_the_real_processor' `
        -ExpectedCount 1 `
        -Exact

    Invoke-RealProcessorTests `
        -Executable $desktopLibrary `
        -Filter 'batch_runner::tests::real_processor_exports_persisted_batches_in_every_format' `
        -ExpectedCount 1 `
        -Exact

    # Every ignored test of the module: the import flow and its two variants.
    Invoke-RealProcessorTests `
        -Executable $desktopLibrary `
        -Filter 'photo_import::native_flow_tests::' `
        -ExpectedCount 3
    exit 0
}
finally {
    $env:MYALBUNS_TEST_IMAGING_PROCESSOR = $previousTestProcessor
    Pop-Location
}
