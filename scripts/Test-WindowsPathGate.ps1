param(
    [string] $OutputPath,
    [string] $UncRoot,
    [string] $DriveLetter
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
Initialize-MyAlbunsToolchain

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'The Windows path gate must run on Windows.'
}

$runnerMutex = [System.Threading.Mutex]::new(
    $false,
    'Local\MyAlbuns.WindowsPathGateEvidence.v1'
)
$runnerMutexHeld = $false
try {
    $runnerMutexHeld = $runnerMutex.WaitOne(0)
}
catch [System.Threading.AbandonedMutexException] {
    $runnerMutexHeld = $true
}
if (-not $runnerMutexHeld) {
    $runnerMutex.Dispose()
    throw 'Another Windows path evidence runner is already using the mapped-drive fixture.'
}

try {
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0008-windows-path-gate.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$sourceSnapshotBefore = Get-GateSourceSnapshot `
    -WorkspaceRoot $script:WorkspaceRoot `
    -EvidencePath $OutputPath

$scratchRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot '.scratch\windows-path-gate')
)
$runRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $scratchRoot "run-$PID-$([DateTime]::UtcNow.Ticks)")
)
if (-not $runRoot.StartsWith(
        $scratchRoot + [System.IO.Path]::DirectorySeparatorChar,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The Windows path fixture escaped the workspace scratch root.'
}
New-Item -ItemType Directory -Force -Path $runRoot | Out-Null

if ([string]::IsNullOrWhiteSpace($UncRoot)) {
    $volumeRoot = [System.IO.Path]::GetPathRoot($runRoot)
    if ($volumeRoot -notmatch '^[A-Za-z]:\\$') {
        throw 'The default UNC fixture requires the workspace on a drive-letter volume.'
    }
    $volumeLetter = $volumeRoot.Substring(0, 1)
    $relativeRunRoot = $runRoot.Substring($volumeRoot.Length)
    $UncRoot = "\\127.0.0.1\$volumeLetter`$\$relativeRunRoot"
}
$UncRoot = $UncRoot.TrimEnd('\')

if ([string]::IsNullOrWhiteSpace($DriveLetter)) {
    $usedLetters = @(
        [System.IO.DriveInfo]::GetDrives() |
            ForEach-Object { $_.Name.Substring(0, 1).ToUpperInvariant() }
    )
    $DriveLetter = @('R', 'Q', 'P', 'O', 'N', 'M') |
        Where-Object { $usedLetters -notcontains $_ } |
        Select-Object -First 1
}
$DriveLetter = $DriveLetter.TrimEnd(':').ToUpperInvariant()
if ($DriveLetter -notmatch '^[A-Z]$') {
    throw 'DriveLetter must be one unused letter.'
}
$mappedDrive = "$DriveLetter`:"

$preflightPath = Join-Path $UncRoot 'preflight.tmp'
$evidencePath = Join-Path $runRoot 'path-evidence.json'
$sidecarEvidencePath = Join-Path $runRoot 'sidecar-evidence.json'
$targetDirectory = Join-Path $script:WorkspaceRoot 'target\windows-path-gate'
$builtProcessorPath = Join-Path $targetDirectory 'debug\myalbuns-imaging.exe'
$processorPath = Join-Path $runRoot 'bin\myalbuns-imaging.exe'
$protocolSourcePath = Join-Path `
    $script:WorkspaceRoot `
    'crates\myalbuns-imaging-protocol\src\lib.rs'
$protocolSource = Get-Content -LiteralPath $protocolSourcePath -Raw
if ($protocolSource -notmatch `
        'pub const IMAGING_PROTOCOL_VERSION:\s*u32\s*=\s*(\d+)\s*;') {
    throw 'The authoritative Imaging protocol version could not be read.'
}
$expectedProtocol = $Matches[1]
$environmentNames = [ordered]@{
    MYALBUNS_PATH_GATE_LOCAL_ROOT = $runRoot
    MYALBUNS_PATH_GATE_UNC_ROOT = $UncRoot
    MYALBUNS_PATH_GATE_DRIVE = $mappedDrive
    MYALBUNS_PATH_GATE_EVIDENCE = $evidencePath
    MYALBUNS_PATH_GATE_SIDECAR_EVIDENCE = $sidecarEvidencePath
    MYALBUNS_REAL_IMAGING_PROCESSOR = $processorPath
}
$previousEnvironment = @{}
$results = [System.Collections.Generic.List[object]]::new()
$pathEvidence = $null
$sidecarEvidence = $null
$locationWasPushed = $false

try {
    [System.IO.File]::WriteAllBytes($preflightPath, [byte[]](1, 2, 3))
    if (-not [System.IO.File]::Exists($preflightPath)) {
        throw 'The UNC fixture did not expose the preflight file.'
    }
    [System.IO.File]::Delete($preflightPath)

    foreach ($entry in $environmentNames.GetEnumerator()) {
        $previousEnvironment[$entry.Key] =
            [System.Environment]::GetEnvironmentVariable(
                $entry.Key,
                [System.EnvironmentVariableTarget]::Process
            )
        [System.Environment]::SetEnvironmentVariable(
            $entry.Key,
            $entry.Value,
            [System.EnvironmentVariableTarget]::Process
        )
    }

    $checks = @(
        [ordered]@{
            name = 'path-contract'
            arguments = @('test', '-p', 'myalbuns-paths', '--test', 'path_resolution')
        },
        [ordered]@{
            name = 'path-policy'
            arguments = @('test', '-p', 'myalbuns-paths', '--test', 'app_paths')
        },
        [ordered]@{
            name = 'real-mapped-unc'
            arguments = @(
                'test',
                '-p',
                'myalbuns-paths',
                '--test',
                'windows_path_gate',
                'real_windows_paths_freeze_mapped_bindings_and_keep_unc_export_recoverable',
                '--',
                '--ignored',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'imaging-protocol'
            arguments = @(
                'test',
                '-p',
                'myalbuns-imaging-protocol',
                '--test',
                'protocol'
            )
        },
        [ordered]@{
            name = 'imaging-sidecar-build'
            arguments = @(
                'build',
                '-p',
                'myalbuns-imaging',
                '--target-dir',
                $targetDirectory
            )
        },
        [ordered]@{
            name = 'desktop-host-build'
            arguments = @(
                'build',
                '-p',
                'myalbuns-desktop',
                '--target-dir',
                $targetDirectory
            )
        },
        [ordered]@{
            name = 'path-io-thread'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'path_io::tests::path_binding_capture_runs_on_the_blocking_pool',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'real-sidecar-frozen-plan'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'imaging_recovery_integration::real_processor_consumes_the_frozen_unc_plan_after_the_drive_is_unmapped',
                '--',
                '--ignored',
                '--exact',
                '--nocapture'
            )
        }
    )

    Push-Location $script:WorkspaceRoot
    $locationWasPushed = $true
    foreach ($check in $checks) {
        $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        & $script:CargoExecutable @($check.arguments)
        $exitCode = $LASTEXITCODE
        $stopwatch.Stop()
        if ($exitCode -ne 0) {
            throw "Windows path check '$($check.name)' failed with exit code $exitCode."
        }
        $results.Add([ordered]@{
            name = $check.name
            passed = $true
            elapsedMs = $stopwatch.ElapsedMilliseconds
        })
        if ($check.name -eq 'imaging-sidecar-build') {
            $advertisedProtocol = (& $builtProcessorPath --protocol-version).Trim()
            if ($LASTEXITCODE -ne 0 -or $advertisedProtocol -ne $expectedProtocol) {
                throw "The built Imaging sidecar advertises protocol '$advertisedProtocol', expected '$expectedProtocol'."
            }
            New-Item `
                -ItemType Directory `
                -Force `
                -Path (Split-Path -Parent $processorPath) |
                Out-Null
            Copy-Item `
                -LiteralPath $builtProcessorPath `
                -Destination $processorPath `
                -Force
            $isolatedProtocol = (& $processorPath --protocol-version).Trim()
            if ($LASTEXITCODE -ne 0 -or $isolatedProtocol -ne $expectedProtocol) {
                throw 'The isolated Imaging sidecar copy failed protocol verification.'
            }
            $results.Add([ordered]@{
                name = 'sidecar-protocol-preflight'
                passed = $true
                elapsedMs = 0
            })
        }
    }

    $manifestChecks = @(
        [ordered]@{
            name = 'desktop-long-path-manifest'
            executable = Join-Path $targetDirectory 'debug\myalbuns-desktop.exe'
        },
        [ordered]@{
            name = 'sidecar-long-path-manifest'
            executable = $processorPath
        }
    )
    foreach ($manifestCheck in $manifestChecks) {
        $manifestPath = Join-Path $runRoot "$($manifestCheck.name).xml"
        $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        & mt.exe "-inputresource:$($manifestCheck.executable);#1" "-out:$manifestPath"
        $exitCode = $LASTEXITCODE
        $stopwatch.Stop()
        if ($exitCode -ne 0) {
            throw "Manifest extraction '$($manifestCheck.name)' failed."
        }
        $manifest = Get-Content -LiteralPath $manifestPath -Raw
        if ($manifest -notmatch '<[^>]*longPathAware[^>]*>true</' `
                -or $manifest -notmatch 'Microsoft.Windows.Common-Controls') {
            throw "Manifest '$($manifestCheck.name)' does not opt into long paths."
        }
        $results.Add([ordered]@{
            name = $manifestCheck.name
            passed = $true
            elapsedMs = $stopwatch.ElapsedMilliseconds
        })
    }

    if (-not (Test-Path -LiteralPath $evidencePath -PathType Leaf)) {
        throw 'The real mapped/UNC test did not produce evidence.'
    }
    if (-not (Test-Path -LiteralPath $sidecarEvidencePath -PathType Leaf)) {
        throw 'The real sidecar UNC test did not produce evidence.'
    }
    $pathEvidence = Get-Content -LiteralPath $evidencePath -Raw | ConvertFrom-Json
    $sidecarEvidence =
        Get-Content -LiteralPath $sidecarEvidencePath -Raw | ConvertFrom-Json
    if (-not $pathEvidence.planRoundTripLossless `
            -or -not $pathEvidence.plansDistinct `
            -or $pathEvidence.mappedAndUncIdentity -ne 'same' `
            -or $pathEvidence.mappedAndLocalIdentity -ne 'same' `
            -or $pathEvidence.identityFailure -ne 'indeterminate' `
            -or -not $pathEvidence.physicalAliasLockConflict `
            -or -not $pathEvidence.readOnlyBatchAllowedWhileLocked `
            -or -not $pathEvidence.lockReleasedAfterOwnerDrop `
            -or -not $pathEvidence.bindingReusedAfterRemapWithinAttempt `
            -or -not $pathEvidence.verbatimMappedBindingFrozenAsUnc `
            -or -not $pathEvidence.uncExportPublished `
            -or -not $pathEvidence.stagingInsideDestination `
            -or -not $pathEvidence.cacheUnderLocalAppData `
            -or $pathEvidence.unavailableBinding -ne 'unavailable' `
            -or -not $pathEvidence.explicitRetryRecaptured) {
        throw 'The observed Windows path evidence does not satisfy the gate.'
    }
    if (-not $sidecarEvidence.mappingRemovedBeforeDispatch `
            -or -not $sidecarEvidence.processorUsedFrozenOperationalPath `
            -or -not $sidecarEvidence.outputPublishedOnUnc `
            -or -not $sidecarEvidence.stagingRemoved `
            -or -not $sidecarEvidence.unavailableBindingFailedAtPrepare `
            -or $sidecarEvidence.unavailableAttemptStartedProcessor `
            -or -not $sidecarEvidence.explicitRetryPublished `
            -or $sidecarEvidence.processorPid -lt 1 `
            -or $sidecarEvidence.rootBindingPlanSha256 -notmatch '^[0-9a-f]{64}$' `
            -or $sidecarEvidence.outputSha256 -notmatch '^[0-9a-f]{64}$') {
        throw 'The observed sidecar path evidence does not satisfy the gate.'
    }
}
finally {
    if ($locationWasPushed) {
        Pop-Location
    }
    & cmd.exe /d /c "net use $mappedDrive /delete /y >nul 2>&1"
    foreach ($entry in $environmentNames.GetEnumerator()) {
        [System.Environment]::SetEnvironmentVariable(
            $entry.Key,
            $previousEnvironment[$entry.Key],
            [System.EnvironmentVariableTarget]::Process
        )
    }
    if ([System.IO.File]::Exists($preflightPath)) {
        [System.IO.File]::Delete($preflightPath)
    }
    Remove-GateScratchDirectory `
        -Path $runRoot `
        -AllowedParent $scratchRoot
}

$sourceSnapshotAfter = Get-GateSourceSnapshot `
    -WorkspaceRoot $script:WorkspaceRoot `
    -EvidencePath $OutputPath
$report = [ordered]@{
    schemaVersion = 2
    collectedAtUtc = [DateTime]::UtcNow.ToString('o')
    gitCommit = $sourceSnapshotBefore.gitCommit
    sourceInputsDirty = Test-GateSourceSnapshotsDirty `
        -Before $sourceSnapshotBefore `
        -After $sourceSnapshotAfter
    platform = [ordered]@{
        operatingSystem = [System.Environment]::OSVersion.VersionString
        architecture =
            [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        uncProvider = 'loopback-administrative-share'
    }
    checks = @($results)
    evidence = [ordered]@{
        paths = $pathEvidence
        sidecar = $sidecarEvidence
        libraryBoundaries = [ordered]@{
            directories = [ordered]@{
                responsibility = 'known_folder_discovery'
                operatingSystemBases = [ordered]@{
                    roaming = 'BaseDirs::data_dir (%APPDATA%)'
                    local = 'BaseDirs::data_local_dir (%LOCALAPPDATA%)'
                }
                finalProductDirectorySuffix = 'MyAlbuns'
                temporaryDevelopmentDirectorySuffix = 'MyAlbuns2'
                temporaryUntilFullProgramCompletion = $true
                behindAppPaths = $true
                productOwnsDirectorySuffix = $true
                preservesFinalTreeContract = $true
                exposedAsProductContract = $false
            }
            windowsSys = [ordered]@{
                responsibility = 'native_path_and_handle_adapter'
                physicalIdentityByHandleTested = $true
                mappedDriveResolutionTested = $true
                longPathAwareExecutablesTested = $true
                exposedAsProductContract = $false
            }
            sameFile = [ordered]@{
                selected = $false
                directPathDependency = $false
                usedByPathContract = $false
                reason = 'does_not_define_product_tristate_or_mapped_drive_binding'
            }
            dunce = [ordered]@{
                selected = $false
                directPathDependency = $false
                usedByPathContract = $false
                reason = 'textual_simplification_is_not_operational_identity'
            }
        }
        longPathAware = [ordered]@{
            desktop = $true
            imagingSidecar = $true
        }
        pathIoRunsOutsideCallerThread = $true
    }
}
$json = $report | ConvertTo-Json -Depth 6
$outputDirectory = Split-Path -Parent $OutputPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
[System.IO.File]::WriteAllText(
    $OutputPath,
    $json + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)

Write-Output "Windows path gate report: $OutputPath"
Write-Output $json
}
finally {
    if ($runnerMutexHeld) {
        $runnerMutex.ReleaseMutex()
        $runnerMutexHeld = $false
    }
    $runnerMutex.Dispose()
}
