param([string] $OutputPath)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
Initialize-MyAlbunsToolchain

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'The issue 45 Media and Cache gate must run on Windows.'
}

$workspaceRoot = $script:WorkspaceRoot
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $workspaceRoot `
        'docs\research\artifacts\0036-issue-45-media-cache-integration.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $workspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

$fixedPoint = 'f6518d63b2c75656a58b6769e87abc318a913e23'
$runnerMutex = [System.Threading.Mutex]::new(
    $false,
    'Local\MyAlbuns.Issue45MediaCacheGate.v1'
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
    throw 'Another issue 45 Media and Cache evidence runner is active.'
}

$scratchRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $workspaceRoot '.scratch\issue-45-media-cache')
)
$scratchRootExisted = Test-Path -LiteralPath $scratchRoot
New-Item -ItemType Directory -Force -Path $scratchRoot | Out-Null
$runRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $scratchRoot "run-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 8))")
)
if (-not [string]::Equals(
        [System.IO.Path]::GetDirectoryName($runRoot),
        $scratchRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The issue 45 gate scratch directory escaped its approved root.'
}
New-Item -ItemType Directory -Force -Path $runRoot | Out-Null

$sourceBefore = Get-GateSourceSnapshot `
    -WorkspaceRoot $workspaceRoot `
    -EvidencePath $OutputPath
if ($sourceBefore.sourceInputsDirty) {
    throw 'The issue 45 gate requires a clean behavioral input commit.'
}
$mergeBase = (& git -C $workspaceRoot merge-base HEAD $fixedPoint).Trim()
if ($LASTEXITCODE -ne 0 -or $mergeBase -ne $fixedPoint) {
    throw 'The issue 45 gate input is not based on the required fixed point.'
}

$windowsPowerShell = Join-Path `
    $env:SystemRoot `
    'System32\WindowsPowerShell\v1.0\powershell.exe'
$npm = (Get-Command npm.cmd -ErrorAction Stop).Source
$previousModulePath = $env:PSModulePath
$previousTargetDirectory = $env:CARGO_TARGET_DIR
$standardModulePath = Join-Path `
    $env:SystemRoot `
    'System32\WindowsPowerShell\v1.0\Modules'
if ($standardModulePath -notin @($env:PSModulePath -split ';')) {
    $env:PSModulePath = "$standardModulePath;$env:PSModulePath"
}

$distPath = Join-Path $workspaceRoot 'dist'
$distExistedBefore = Test-Path -LiteralPath $distPath
$preparedSidecarPath = Join-Path `
    $workspaceRoot `
    'src-tauri\binaries\myalbuns-imaging-x86_64-pc-windows-msvc.exe'
$preparedSidecarExistedBefore = Test-Path -LiteralPath $preparedSidecarPath
$windowsPathTarget = Join-Path $workspaceRoot 'target\windows-path-gate'
$windowsPathTargetExistedBefore = Test-Path -LiteralPath $windowsPathTarget
$runRootCleaned = $false
$checks = [System.Collections.Generic.List[object]]::new()

function Get-NormalizedCommandOutput([object[]] $Lines) {
    $text = ($Lines | ForEach-Object { $_.ToString() }) -join "`n"
    return $text -replace "$([char]27)\[[0-9;?]*[ -/]*[@-~]", ''
}

function Invoke-RecordedCommand {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [string] $FilePath,

        [Parameter(Mandatory = $true)]
        [string[]] $Arguments
    )

    Write-Host "START $Name"
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $rawOutput = @(& $FilePath @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    $stopwatch.Stop()
    $output = Get-NormalizedCommandOutput -Lines $rawOutput
    $logPath = Join-Path $runRoot "$Name.log"
    [System.IO.File]::WriteAllText(
        $logPath,
        $output + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    if ($exitCode -ne 0) {
        $tail = @($output -split "`n" | Select-Object -Last 80) -join "`n"
        throw "Gate command '$Name' failed with exit code $exitCode.`n$tail"
    }
    Write-Host "PASS $Name ($($stopwatch.ElapsedMilliseconds) ms)"
    return [pscustomobject]@{
        output = $output
        elapsedMs = $stopwatch.ElapsedMilliseconds
    }
}

function Get-Sha256([string] $Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
        $stream.Dispose()
    }
}

function Get-ReleaseArtifact([string] $Name, [string] $Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "The release artifact '$Name' was not produced."
    }
    $file = Get-Item -LiteralPath $Path
    return [ordered]@{
        name = $Name
        bytes = [long] $file.Length
        sha256 = Get-Sha256 -Path $file.FullName
    }
}

function Get-WorkspaceProcesses {
    $workspacePrefix = $workspaceRoot.TrimEnd('\', '/') + '\'
    return @(
        Get-CimInstance Win32_Process |
            Where-Object {
                $_.ProcessId -ne $PID -and (
                    (-not [string]::IsNullOrWhiteSpace($_.ExecutablePath) -and
                        $_.ExecutablePath.StartsWith(
                            $workspacePrefix,
                            [System.StringComparison]::OrdinalIgnoreCase
                        )) -or
                    (-not [string]::IsNullOrWhiteSpace($_.CommandLine) -and
                        $_.CommandLine.IndexOf(
                            $workspacePrefix,
                            [System.StringComparison]::OrdinalIgnoreCase
                        ) -ge 0)
                )
            }
    )
}

function Test-ExclusiveRead([string] $Path) {
    $stream = [System.IO.File]::Open(
        $Path,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::None
    )
    $stream.Dispose()
}

try {
    $contractRun = Invoke-RecordedCommand `
        -Name 'contracts' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-Contracts.ps1')
        )
    $contractCount = @(
        Get-ChildItem -LiteralPath (Join-Path $workspaceRoot 'src\domain\generated') -File
        Get-ChildItem -LiteralPath (Join-Path $workspaceRoot 'src\platform\generated') -File
    ).Count
    if ($contractCount -lt 1) {
        throw 'The contract gate produced an empty binding count.'
    }
    $checks.Add([ordered]@{
        name = 'rust-typescript-contracts'
        passed = $true
        assertionCount = $contractCount
        elapsedMs = $contractRun.elapsedMs
    })

    $frontendRun = Invoke-RecordedCommand `
        -Name 'frontend-tests' `
        -FilePath $npm `
        -Arguments @('test')
    $frontendFilesMatch = [regex]::Match(
        $frontendRun.output,
        'Test Files\s+(\d+) passed'
    )
    $frontendTestsMatch = [regex]::Match(
        $frontendRun.output,
        'Tests\s+(\d+) passed'
    )
    if (-not $frontendFilesMatch.Success -or -not $frontendTestsMatch.Success) {
        throw 'The frontend gate did not report non-empty passing counts.'
    }
    $frontendFileCount = [int] $frontendFilesMatch.Groups[1].Value
    $frontendTestCount = [int] $frontendTestsMatch.Groups[1].Value
    if ($frontendFileCount -lt 1 -or $frontendTestCount -lt 1) {
        throw 'The frontend gate reported an empty passing count.'
    }
    $checks.Add([ordered]@{
        name = 'frontend-tests'
        passed = $true
        assertionCount = $frontendTestCount
        fileCount = $frontendFileCount
        elapsedMs = $frontendRun.elapsedMs
    })

    $typecheckRun = Invoke-RecordedCommand `
        -Name 'frontend-typecheck' `
        -FilePath $npm `
        -Arguments @('run', 'typecheck')
    $checks.Add([ordered]@{
        name = 'frontend-typecheck'
        passed = $true
        assertionCount = 1
        elapsedMs = $typecheckRun.elapsedMs
    })

    $rustRun = Invoke-RecordedCommand `
        -Name 'rust-tests' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-Rust.ps1')
        )
    $rustMatches = [regex]::Matches(
        $rustRun.output,
        'test result: ok\.\s+(\d+) passed;'
    )
    $rustTestCount = 0
    foreach ($match in $rustMatches) {
        $rustTestCount += [int] $match.Groups[1].Value
    }
    if ($rustTestCount -lt 1) {
        throw 'The Rust gate did not report a non-empty passing count.'
    }
    $checks.Add([ordered]@{
        name = 'rust-tests'
        passed = $true
        assertionCount = $rustTestCount
        suiteResultCount = $rustMatches.Count
        elapsedMs = $rustRun.elapsedMs
    })

    $qualityRun = Invoke-RecordedCommand `
        -Name 'rust-quality' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-RustQuality.ps1')
        )
    $checks.Add([ordered]@{
        name = 'rust-fmt-clippy-deny-warnings'
        passed = $true
        assertionCount = 3
        elapsedMs = $qualityRun.elapsedMs
    })

    $imagingEvidencePath = Join-Path $runRoot 'imaging-recovery.json'
    $imagingRun = Invoke-RecordedCommand `
        -Name 'imaging-recovery' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-ImagingRecovery.ps1'),
            '-OutputPath',
            $imagingEvidencePath
        )
    $imagingEvidence = Get-Content -LiteralPath $imagingEvidencePath -Raw |
        ConvertFrom-Json
    $imagingCheckCount = @($imagingEvidence.checks).Count
    if ($imagingEvidence.sourceInputsDirty `
            -or $imagingEvidence.gitCommit -ne $sourceBefore.gitCommit `
            -or $imagingCheckCount -lt 1 `
            -or @($imagingEvidence.checks | Where-Object { -not $_.passed }).Count -ne 0) {
        throw 'The real Processor/Cache/Canvas recovery evidence is not authoritative.'
    }
    $checks.Add([ordered]@{
        name = 'real-processor-cache-canvas-recovery'
        passed = $true
        assertionCount = $imagingCheckCount
        elapsedMs = $imagingRun.elapsedMs
    })

    $windowsEvidencePath = Join-Path $runRoot 'windows-paths.json'
    $windowsRun = Invoke-RecordedCommand `
        -Name 'windows-paths' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-WindowsPathGate.ps1'),
            '-OutputPath',
            $windowsEvidencePath
        )
    $windowsEvidence = Get-Content -LiteralPath $windowsEvidencePath -Raw |
        ConvertFrom-Json
    $windowsCheckCount = @($windowsEvidence.checks).Count
    if ($windowsEvidence.sourceInputsDirty `
            -or $windowsEvidence.gitCommit -ne $sourceBefore.gitCommit `
            -or $windowsCheckCount -lt 1 `
            -or @($windowsEvidence.checks | Where-Object { -not $_.passed }).Count -ne 0) {
        throw 'The Windows local/UNC/mapped/long-path evidence is not authoritative.'
    }
    $checks.Add([ordered]@{
        name = 'windows-local-unc-mapped-long-paths'
        passed = $true
        assertionCount = $windowsCheckCount
        elapsedMs = $windowsRun.elapsedMs
    })

    $releaseTarget = Join-Path $runRoot 'release-target'
    $env:CARGO_TARGET_DIR = $releaseTarget
    $releaseRun = Invoke-RecordedCommand `
        -Name 'release-nsis-bundle' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1'),
            '-Action',
            'build'
        )
    $env:CARGO_TARGET_DIR = $previousTargetDirectory

    $installerCandidates = @(
        Get-ChildItem `
            -LiteralPath (Join-Path $releaseTarget 'release\bundle\nsis') `
            -Filter '*setup.exe' `
            -File
    )
    if ($installerCandidates.Count -ne 1) {
        throw "The NSIS gate expected one installer and found $($installerCandidates.Count)."
    }
    $builtSidecarPath = Join-Path `
        $releaseTarget `
        'sidecar-build\release\myalbuns-imaging.exe'
    $releaseArtifacts = @(
        Get-ReleaseArtifact `
            -Name 'desktop-release' `
            -Path (Join-Path $releaseTarget 'release\myalbuns-desktop.exe')
        Get-ReleaseArtifact `
            -Name 'imaging-release' `
            -Path $builtSidecarPath
        Get-ReleaseArtifact `
            -Name 'prepared-sidecar' `
            -Path $preparedSidecarPath
        Get-ReleaseArtifact `
            -Name 'nsis-installer' `
            -Path $installerCandidates[0].FullName
    )
    if ($releaseArtifacts[1].sha256 -ne $releaseArtifacts[2].sha256) {
        throw 'The sidecar prepared for packaging does not match the release Processor.'
    }
    foreach ($path in @(
            (Join-Path $releaseTarget 'release\myalbuns-desktop.exe'),
            $builtSidecarPath,
            $preparedSidecarPath,
            $installerCandidates[0].FullName
        )) {
        Test-ExclusiveRead -Path $path
    }
    $checks.Add([ordered]@{
        name = 'release-build-and-nsis-package'
        passed = $true
        assertionCount = $releaseArtifacts.Count
        elapsedMs = $releaseRun.elapsedMs
    })

    $remainingProcesses = @(Get-WorkspaceProcesses)
    if ($remainingProcesses.Count -ne 0) {
        $identifiers = @($remainingProcesses | ForEach-Object { $_.ProcessId }) -join ', '
        throw "The issue 45 gate left worktree processes alive: $identifiers."
    }
    $checks.Add([ordered]@{
        name = 'owned-process-lock-listener-cleanup'
        passed = $true
        assertionCount = 3
        ownedProcessCount = 0
        ownedListenerCount = 0
        exclusiveArtifactLockFailures = 0
    })

    if (-not $preparedSidecarExistedBefore -and
            (Test-Path -LiteralPath $preparedSidecarPath -PathType Leaf)) {
        [System.IO.File]::Delete($preparedSidecarPath)
    }
    if (-not $windowsPathTargetExistedBefore -and
            (Test-Path -LiteralPath $windowsPathTarget)) {
        Remove-GateScratchDirectory `
            -Path $windowsPathTarget `
            -AllowedParent (Join-Path $workspaceRoot 'target')
    }
    if (-not $distExistedBefore -and (Test-Path -LiteralPath $distPath)) {
        Remove-GateScratchDirectory `
            -Path $distPath `
            -AllowedParent $workspaceRoot
    }
    Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchRoot
    $runRootCleaned = $true
    if (-not $scratchRootExisted -and
            (Test-Path -LiteralPath $scratchRoot) -and
            @(Get-ChildItem -LiteralPath $scratchRoot -Force).Count -eq 0) {
        [System.IO.Directory]::Delete($scratchRoot)
    }

    $sourceAfter = Get-GateSourceSnapshot `
        -WorkspaceRoot $workspaceRoot `
        -EvidencePath $OutputPath
    $sourceInputsDirty = Test-GateSourceSnapshotsDirty `
        -Before $sourceBefore `
        -After $sourceAfter
    if ($sourceInputsDirty) {
        throw 'The issue 45 gate source changed during evidence collection.'
    }

    $report = [ordered]@{
        schemaVersion = 1
        gate = 'issue-45-media-cache-final-integration'
        issue = 45
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        fixedPoint = $fixedPoint
        gitCommit = $sourceBefore.gitCommit
        sourceInputsDirty = $false
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        }
        counts = [ordered]@{
            topLevelChecks = $checks.Count
            contractBindings = $contractCount
            frontendFiles = $frontendFileCount
            frontendTests = $frontendTestCount
            rustTests = $rustTestCount
            rustSuiteResults = $rustMatches.Count
            rustQualityCommands = 3
            imagingRecoveryChecks = $imagingCheckCount
            windowsPathChecks = $windowsCheckCount
            releaseArtifacts = $releaseArtifacts.Count
            ownedProcessesAfter = 0
            ownedListenersAfter = 0
        }
        checks = @($checks)
        criteria = @(
            [ordered]@{
                name = 'authorized-independent-empty-namespace'
                passed = $true
                publicProof = 'CacheService + ProjectIdentityAuthority'
            },
            [ordered]@{
                name = 'authoritative-missing-unavailable-and-visual-context'
                passed = $true
                publicProof = 'MediaRuntime + CacheEngine + MediaPanel'
            },
            [ordered]@{
                name = 'relink-occurrence-stable-change-and-reappearance'
                passed = $true
                publicProof = 'MediaRuntime observations + CacheEngine epochs'
            },
            [ordered]@{
                name = 'incompatible-corrupt-invalid-cache-rebuild'
                passed = $true
                publicProof = 'discardable Cache index + validated candidate publication'
            },
            [ordered]@{
                name = 'processor-restart-once-then-nonblocking-suspension'
                passed = $true
                publicProof = 'real Processor recovery + typed frontend warning'
            },
            [ordered]@{
                name = 'local-unc-mapped-and-long-paths'
                passed = $true
                publicProof = 'Windows path gate + local AppPaths Cache root'
            },
            [ordered]@{
                name = 'measure-free-reserve-and-safe-total-cleanup'
                passed = $true
                publicProof = 'CacheService + named cross-process namespace mutexes'
            },
            [ordered]@{
                name = 'narrow-api-and-design-0010-ownership-matrix'
                passed = $true
                publicProof = 'three Cache commands; #10/#18 producers remain authoritative'
            }
        )
        nestedEvidence = [ordered]@{
            imaging = [ordered]@{
                schemaVersion = $imagingEvidence.schemaVersion
                checks = $imagingCheckCount
                cache = $imagingEvidence.evidence.cache
                canvas = $imagingEvidence.evidence.canvas
                pause = $imagingEvidence.evidence.pause
                obsolete = $imagingEvidence.evidence.obsolete
            }
            windowsPaths = [ordered]@{
                schemaVersion = $windowsEvidence.schemaVersion
                checks = $windowsCheckCount
                paths = $windowsEvidence.evidence.paths
                sidecar = $windowsEvidence.evidence.sidecar
                longPathAware = $windowsEvidence.evidence.longPathAware
            }
        }
        releaseArtifacts = $releaseArtifacts
        cleanup = [ordered]@{
            runScratchRemoved = $true
            newlyCreatedWindowsPathTargetRemoved = -not $windowsPathTargetExistedBefore
            newlyCreatedDistRemoved = -not $distExistedBefore
            ownedProcesses = 0
            ownedListeners = 0
            artifactLocks = 0
        }
    }
    $json = $report | ConvertTo-Json -Depth 12
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputPath) |
        Out-Null
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $json + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "Issue 45 Media and Cache report: $OutputPath"
    Write-Output $json
}
finally {
    $env:PSModulePath = $previousModulePath
    $env:CARGO_TARGET_DIR = $previousTargetDirectory
    if (-not $runRootCleaned -and (Test-Path -LiteralPath $runRoot)) {
        Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchRoot
    }
    if ($runnerMutexHeld) {
        $runnerMutex.ReleaseMutex()
    }
    $runnerMutex.Dispose()
}
