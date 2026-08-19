param([string] $OutputPath)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
. (Join-Path $PSScriptRoot 'Gate-OwnedProcessJob.ps1')
. (Join-Path $PSScriptRoot 'Gate-ProcessScope.ps1')
. (Join-Path $PSScriptRoot 'Gate-EvidenceReport.ps1')
. (Join-Path $PSScriptRoot 'Issue45-MediaCacheGateScratch.ps1')
. (Join-Path $PSScriptRoot 'Issue45-MediaCacheGateProof.ps1')
Initialize-MyAlbunsToolchain

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'The issue 45 Media and Cache gate must run on Windows.'
}

$workspaceRoot = $script:WorkspaceRoot
$canonicalOutputPath = [System.IO.Path]::GetFullPath(
    (Join-Path `
        $workspaceRoot `
        'docs\research\artifacts\0036-issue-45-media-cache-integration.json')
)
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = $canonicalOutputPath
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $workspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
if (-not [string]::Equals(
        $OutputPath,
        $canonicalOutputPath,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The issue 45 gate writes only its canonical versioned evidence artifact.'
}

$fixedPoint = 'f6518d63b2c75656a58b6769e87abc318a913e23'
$scratchScope = New-Issue45MediaCacheScratchScope -WorkspaceRoot $workspaceRoot
$scratchContainer = $scratchScope.ScratchContainer
$runRoot = $null
$distPath = $scratchScope.DistPath
$preparedSidecarPath = $scratchScope.PreparedSidecarPath
$sharedCargoTarget = $scratchScope.SharedCargoTarget
$windowsPathScratch = $null
$gateTarget = $null
$independentWindowsPathScratchProbe = $null
$independentWindowsPathScratchPreserved = $false
$independentWindowsPathScratchProbeRemoved = $false
$ownedOutputPreflightPaths = $scratchScope.OwnedOutputPreflightPaths
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
$previousModulePath = $env:PSModulePath
$previousTargetDirectory = $env:CARGO_TARGET_DIR
$processScope = $null

try {
$windowsPowerShell = Join-Path `
    $env:SystemRoot `
    'System32\WindowsPowerShell\v1.0\powershell.exe'
[void] (Initialize-Issue45MediaCacheScratch -Scope $scratchScope)
$runRoot = $scratchScope.RunRoot
$windowsPathScratch = $scratchScope.WindowsPathScratch
$gateTarget = $scratchScope.GateTarget
$env:CARGO_TARGET_DIR = $gateTarget
$processScope = New-GateProcessScope `
    -WorkspaceRoot $workspaceRoot `
    -RunRoot $runRoot `
    -WindowsPowerShell $windowsPowerShell
$preexistingProcesses = @(Get-GateWorkspaceProcesses -Scope $processScope)
if ($preexistingProcesses.Count -ne 0) {
    $identifiers = @($preexistingProcesses | ForEach-Object { $_.ProcessId }) -join ', '
    throw "The issue 45 gate requires zero pre-existing worktree processes: $identifiers."
}

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

$npm = (Get-Command npm.cmd -ErrorAction Stop).Source
$standardModulePath = Join-Path `
    $env:SystemRoot `
    'System32\WindowsPowerShell\v1.0\Modules'
if ($standardModulePath -notin @($env:PSModulePath -split ';')) {
    $env:PSModulePath = "$standardModulePath;$env:PSModulePath"
}
$checks = [System.Collections.Generic.List[object]]::new()
$json = $null

try {
    $independentWindowsPathScratchProbe =
        New-IndependentWindowsPathScratchProbe -Scope $scratchScope
    $outputPreflightAssertionCount = Test-Issue45OwnedOutputPreflightContracts -Scope $scratchScope
    $preflightCoversSharedCargoTarget =
        $ownedOutputPreflightPaths -contains $sharedCargoTarget
    $preflightCoversOwnedScratch =
        $ownedOutputPreflightPaths -contains $scratchContainer
    $checks.Add([ordered]@{
        name = 'fail-closed-preexisting-output-preflight'
        passed = ($outputPreflightAssertionCount -eq 4 -and
            $ownedOutputPreflightPaths.Count -eq 4 -and
            $preflightCoversSharedCargoTarget -and
            $preflightCoversOwnedScratch)
        assertionCount = $outputPreflightAssertionCount + 3
        requiredOutputPathCount = $ownedOutputPreflightPaths.Count
        sharedCargoTargetRequiredAbsent = $preflightCoversSharedCargoTarget
        ownedScratchRequiredAbsent = $preflightCoversOwnedScratch
    })

    $proofParserAssertionCount = Test-ProofParserContracts
    $checks.Add([ordered]@{
        name = 'fail-closed-named-proof-parser'
        passed = ($proofParserAssertionCount -eq 8)
        assertionCount = $proofParserAssertionCount
    })

    $imagingCheckSetAssertionCount = Test-ExactPassedCheckSetContracts `
        -ExpectedNames $expectedImagingRecoveryCheckNames
    $checks.Add([ordered]@{
        name = 'fail-closed-imaging-recovery-check-set'
        passed = ($imagingCheckSetAssertionCount -eq 16)
        assertionCount = $imagingCheckSetAssertionCount
    })

    $windowsCheckSetAssertionCount = Test-ExactPassedCheckSetContracts `
        -ExpectedNames $expectedWindowsPathCheckNames
    $checks.Add([ordered]@{
        name = 'fail-closed-windows-path-check-set'
        passed = ($windowsCheckSetAssertionCount -eq 18)
        assertionCount = $windowsCheckSetAssertionCount
    })

    $postProofMutationAssertionCount = Test-GatePostProofSourceMutationContract `
        -FixtureRoot (Join-Path $runRoot 'post-proof-provenance-fixture')
    $checks.Add([ordered]@{
        name = 'fail-closed-post-proof-source-mutation'
        passed = ($postProofMutationAssertionCount -eq 1)
        assertionCount = $postProofMutationAssertionCount
    })

    $cleanupProbe = Invoke-GateProcessCleanupProbe -Scope $processScope
    $checks.Add([ordered]@{
        name = 'owned-process-listener-cleanup-probe'
        passed = ($cleanupProbe.stoppedProcessCount -ge 1 -and
            $cleanupProbe.listenersBefore -ge 1 -and
            $cleanupProbe.processesAfter -eq 0 -and
            $cleanupProbe.listenersAfter -eq 0 -and
            $cleanupProbe.independentSentinelSurvived)
        assertionCount = 6
        stoppedProcessCount = $cleanupProbe.stoppedProcessCount
        observedListenerCount = $cleanupProbe.listenersBefore
        independentSentinelSurvived = $cleanupProbe.independentSentinelSurvived
    })

    $sidecarPreparationRun = Invoke-GateScopedCommand -Scope $processScope `
        -Name 'debug-sidecar-preparation' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Prepare-Sidecar.ps1'),
            '-Profile',
            'debug'
        )
    if (-not (Test-Path -LiteralPath $preparedSidecarPath -PathType Leaf)) {
        throw 'The clean evidence run did not prepare the required debug sidecar.'
    }
    Test-GateExclusiveRead -Path $preparedSidecarPath
    $checks.Add([ordered]@{
        name = 'clean-debug-sidecar-preparation'
        passed = $true
        assertionCount = 2
        elapsedMs = $sidecarPreparationRun.elapsedMs
    })

    $contractRun = Invoke-GateScopedCommand -Scope $processScope `
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

    $frontendResultsPath = Join-Path $runRoot 'frontend-test-results.json'
    $frontendRun = Invoke-GateScopedCommand -Scope $processScope `
        -Name 'frontend-tests' `
        -FilePath $npm `
        -Arguments @(
            'test',
            '--',
            '--reporter=verbose',
            '--reporter=json',
            "--outputFile.json=$frontendResultsPath"
        )
    if (-not (Test-Path -LiteralPath $frontendResultsPath -PathType Leaf)) {
        throw 'The frontend gate did not produce its machine-readable test report.'
    }
    $frontendResults = Get-Content `
        -LiteralPath $frontendResultsPath `
        -Raw `
        -Encoding UTF8 |
        ConvertFrom-Json
    $frontendFileCount = [int] $frontendResults.numPassedTestSuites
    $frontendTestCount = [int] $frontendResults.numPassedTests
    if (-not $frontendResults.success `
            -or $frontendFileCount -lt 1 `
            -or $frontendTestCount -lt 1 `
            -or [int] $frontendResults.numFailedTestSuites -ne 0 `
            -or [int] $frontendResults.numFailedTests -ne 0 `
            -or [int] $frontendResults.numPendingTests -ne 0 `
            -or [int] $frontendResults.numTodoTests -ne 0) {
        throw 'The frontend machine report is empty, failed, pending, or incomplete.'
    }
    $frontendAssertions = @(
        $frontendResults.testResults |
            ForEach-Object { $_.assertionResults }
    )
    $checks.Add([ordered]@{
        name = 'frontend-tests'
        passed = $true
        assertionCount = $frontendTestCount
        fileCount = $frontendFileCount
        elapsedMs = $frontendRun.elapsedMs
    })

    $typecheckRun = Invoke-GateScopedCommand -Scope $processScope `
        -Name 'frontend-typecheck' `
        -FilePath $npm `
        -Arguments @('run', 'typecheck')
    $checks.Add([ordered]@{
        name = 'frontend-typecheck'
        passed = $true
        assertionCount = 1
        elapsedMs = $typecheckRun.elapsedMs
    })

    $rustRun = Invoke-GateScopedCommand -Scope $processScope `
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

    $qualityRun = Invoke-GateScopedCommand -Scope $processScope `
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
    $imagingRun = Invoke-GateScopedCommand -Scope $processScope `
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
    $imagingChecks = @($imagingEvidence.checks)
    $imagingCheckCount = $imagingChecks.Count
    $imagingFailedCheckCount = @($imagingChecks | Where-Object {
            $_.passed -isnot [System.Boolean] -or $_.passed -ne $true
        }).Count
    if (-not (Test-ExactFalseBoolean $imagingEvidence.sourceInputsDirty) `
            -or $imagingEvidence.gitCommit -ne $sourceBefore.gitCommit `
            -or $imagingFailedCheckCount -ne 0 `
            -or -not (Test-ExactPassedCheckSet `
                -Checks $imagingChecks `
                -ExpectedNames $expectedImagingRecoveryCheckNames)) {
        throw "The real Processor/Cache/Canvas recovery evidence is not authoritative: sourceInputsDirty=$($imagingEvidence.sourceInputsDirty), gitCommit=$($imagingEvidence.gitCommit), expectedCommit=$($sourceBefore.gitCommit), checks=$imagingCheckCount, failed=$imagingFailedCheckCount."
    }
    $checks.Add([ordered]@{
        name = 'real-processor-cache-canvas-recovery'
        passed = $true
        assertionCount = $imagingCheckCount
        elapsedMs = $imagingRun.elapsedMs
    })

    $windowsEvidencePath = Join-Path $runRoot 'windows-paths.json'
    $windowsRun = Invoke-GateScopedCommand -Scope $processScope `
        -Name 'windows-paths' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-WindowsPathGate.ps1'),
            '-OutputPath',
            $windowsEvidencePath,
            '-ScratchRoot',
            $windowsPathScratch
        )
    $windowsEvidence = Get-Content -LiteralPath $windowsEvidencePath -Raw |
        ConvertFrom-Json
    $windowsChecks = @($windowsEvidence.checks)
    $windowsCheckCount = $windowsChecks.Count
    $windowsFailedCheckCount = @($windowsChecks | Where-Object {
            $_.passed -isnot [System.Boolean] -or $_.passed -ne $true
        }).Count
    if (-not (Test-ExactFalseBoolean $windowsEvidence.sourceInputsDirty) `
            -or $windowsEvidence.gitCommit -ne $sourceBefore.gitCommit `
            -or $windowsFailedCheckCount -ne 0 `
            -or -not (Test-ExactPassedCheckSet `
                -Checks $windowsChecks `
                -ExpectedNames $expectedWindowsPathCheckNames)) {
        throw "The Windows local/UNC/mapped/long-path evidence is not authoritative: sourceInputsDirty=$($windowsEvidence.sourceInputsDirty), gitCommit=$($windowsEvidence.gitCommit), expectedCommit=$($sourceBefore.gitCommit), checks=$windowsCheckCount, failed=$windowsFailedCheckCount."
    }
    $checks.Add([ordered]@{
        name = 'windows-local-unc-mapped-long-paths'
        passed = $true
        assertionCount = $windowsCheckCount
        elapsedMs = $windowsRun.elapsedMs
    })

    $releaseTarget = Join-Path $runRoot 'release-target'
    $env:CARGO_TARGET_DIR = $releaseTarget
    $releaseRun = Invoke-GateScopedCommand -Scope $processScope `
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
    $env:CARGO_TARGET_DIR = $gateTarget

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
        Get-GateReleaseArtifact `
            -Name 'desktop-release' `
            -Path (Join-Path $releaseTarget 'release\myalbuns-desktop.exe')
        Get-GateReleaseArtifact `
            -Name 'imaging-release' `
            -Path $builtSidecarPath
        Get-GateReleaseArtifact `
            -Name 'prepared-sidecar' `
            -Path $preparedSidecarPath
        Get-GateReleaseArtifact `
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
        Test-GateExclusiveRead -Path $path
    }
    $checks.Add([ordered]@{
        name = 'release-build-and-nsis-package'
        passed = $true
        assertionCount = $releaseArtifacts.Count
        elapsedMs = $releaseRun.elapsedMs
    })

    $finalOwnedCleanup = Stop-GateProcessScope -Scope $processScope
    $remainingProcesses = @(Get-ActiveGateProcesses -Scope $processScope)
    $remainingListeners = @(Get-GateProcessListeners -Processes $remainingProcesses)
    $untrackedWorkspaceProcesses = @(Get-GateWorkspaceProcesses -Scope $processScope)
    $claimedPreexistingIdentities = @(
        $processScope.OwnedProcessRecords.Keys |
            Where-Object { $processScope.PreexistingProcessIdentities.Contains($_) }
    )
    if ($remainingProcesses.Count -ne 0 `
            -or $remainingListeners.Count -ne 0 `
            -or $untrackedWorkspaceProcesses.Count -ne 0 `
            -or $claimedPreexistingIdentities.Count -ne 0) {
        $identifiers = @(
            $remainingProcesses
            $untrackedWorkspaceProcesses
        ) | ForEach-Object { $_.ProcessId } | Sort-Object -Unique
        $identifiers = @($identifiers) -join ', '
        throw "The issue 45 gate left worktree processes alive: $identifiers."
    }
    $ownedProcessCountAfter = $remainingProcesses.Count
    $ownedListenerCountAfter = $remainingListeners.Count
    $checks.Add([ordered]@{
        name = 'owned-process-lock-listener-cleanup'
        passed = ($ownedProcessCountAfter -eq 0 -and
            $ownedListenerCountAfter -eq 0 -and
            $claimedPreexistingIdentities.Count -eq 0)
        assertionCount = 4
        ownedProcessCount = $ownedProcessCountAfter
        ownedListenerCount = $ownedListenerCountAfter
        claimedPreexistingProcessIdentityCount = $claimedPreexistingIdentities.Count
        stoppedOwnedProcessCount = $finalOwnedCleanup.stoppedProcessCount
        observedOwnedListenerCountBeforeCleanup = $finalOwnedCleanup.listenersBefore
        exclusiveArtifactLockFailures = 0
    })

    Clear-Issue45GateOutputs -Scope $scratchScope

    $independentScratchAssertionCount =
        Test-IndependentWindowsPathScratchProbe `
            -Probe $independentWindowsPathScratchProbe
    $independentWindowsPathScratchPreserved = $true
    $checks.Add([ordered]@{
        name = 'independent-windows-path-scratch-preservation'
        passed = $true
        assertionCount = $independentScratchAssertionCount
    })
    Remove-IndependentWindowsPathScratchProbe `
        -Probe $independentWindowsPathScratchProbe
    $independentWindowsPathScratchProbeRemoved =
        -not (Test-Path `
            -LiteralPath $independentWindowsPathScratchProbe.path)
    if (-not $independentWindowsPathScratchProbeRemoved) {
        throw 'The issue 45 gate retained its independent scratch probe file.'
    }

    $sharedCargoTargetUntouched = -not (Test-Path -LiteralPath $sharedCargoTarget)
    $isolatedCargoTargetRemoved = -not (Test-Path -LiteralPath $gateTarget)
    $runScratchRemoved = -not (Test-Path -LiteralPath $runRoot)
    $ownedScratchContainerRemoved = -not (Test-Path -LiteralPath $scratchContainer)
    $windowsPathScratchRemoved = -not (Test-Path -LiteralPath $windowsPathScratch)
    if (-not $sharedCargoTargetUntouched `
            -or -not $isolatedCargoTargetRemoved `
            -or -not $runScratchRemoved `
            -or -not $ownedScratchContainerRemoved `
            -or -not $windowsPathScratchRemoved) {
        throw 'The issue 45 gate touched the shared Cargo target or retained owned scratch.'
    }
    $checks.Add([ordered]@{
        name = 'isolated-cargo-target-cleanup'
        passed = $true
        assertionCount = 5
        sharedCargoTargetUntouched = $sharedCargoTargetUntouched
        isolatedCargoTargetRemoved = $isolatedCargoTargetRemoved
        runScratchRemoved = $runScratchRemoved
        ownedScratchContainerRemoved = $ownedScratchContainerRemoved
        windowsPathScratchRemoved = $windowsPathScratchRemoved
    })

    $imagingProofText = @($imagingEvidence.checks | ForEach-Object { $_.name }) -join "`n"
    $windowsProofText = @($windowsEvidence.checks | ForEach-Object { $_.name }) -join "`n"
    $cacheServiceSource = Get-Content `
        -LiteralPath (Join-Path $workspaceRoot 'src-tauri\src\cache_service.rs') `
        -Raw `
        -Encoding UTF8
    $cacheCommandMatches = [regex]::Matches(
        $cacheServiceSource,
        '(?ms)^#\[tauri::command\]\s+pub\(crate\) async fn (cache_service_status|free_closed_project_cache|clear_all_cache)\b'
    )
    $cacheCommandNames = @(
        $cacheCommandMatches | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
    )
    if ($cacheCommandMatches.Count -ne 3 -or $cacheCommandNames.Count -ne 3) {
        throw 'The issue 16 Cache service surface is no longer exactly three narrow commands.'
    }
    $cachePathApiSource = Get-Content `
        -LiteralPath (Join-Path $workspaceRoot 'crates\myalbuns-paths\src\app_paths.rs') `
        -Raw `
        -Encoding UTF8
    if ($cachePathApiSource -match '\binspect_cache_namespaces\b') {
        throw 'The Cache path API reintroduced uncoordinated bulk namespace inspection.'
    }
    $narrowApiProofText = @(
        $cacheCommandNames
        'no-bulk-cache-namespace-inspection'
    ) -join "`n"
    $researchProofText = Get-Content `
        -LiteralPath (Join-Path $workspaceRoot 'docs\research\0036-integracao-final-de-midias-e-cache.md') `
        -Raw `
        -Encoding UTF8
    $design0010Text = Get-Content `
        -LiteralPath (Join-Path $workspaceRoot 'docs\design\0010-armazenamento-local-e-cache.md') `
        -Raw `
        -Encoding UTF8
    $normativeScenarios = @(
        Get-NormativeDesignScenarios -Markdown $design0010Text
    )
    $expectedMatrixProofs = @(
        'cache_consumes_authoritative_identity_transitions_without_owning_them'
        'a_new_authorized_identity_reserves_an_independent_empty_namespace'
        'cache_consumes_authoritative_identity_transitions_without_owning_them'
        'cache_consumes_authoritative_identity_transitions_without_owning_them'
        'monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes'
        'absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state'
        'absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state'
        'corrupted_or_incompatible_index_is_discarded_and_rebuilt'
        'obsolete_job_that_finishes_does_not_publish_and_discards_its_candidate_generation'
        'reopening_after_host_death_recovers_the_contained_processors_temporary'
        'project_open_during_free_space_is_serialized_by_namespace_reservation'
        'schedules_total_cleanup_while_a_project_is_active_and_runs_it_at_safe_startup'
        'export_plan_rejects_missing_originals_at_the_typed_plan_stage'
        'real-mapped-unc'
    )
    if ($normativeScenarios.Count -ne $expectedMatrixProofs.Count) {
        throw 'The normative design scenario and behavioral proof counts diverged.'
    }
    $expectedDesignMatrix = @(
        for ($index = 0; $index -lt $normativeScenarios.Count; $index++) {
            [pscustomobject]@{
                scenario = $normativeScenarios[$index]
                proof = $expectedMatrixProofs[$index]
            }
        }
    )
    $designMatrixRows = @(
        ConvertFrom-DesignMatrix -Markdown $researchProofText
    )
    $designMatrixAssertionCount = Test-DesignMatrixContracts `
        -Rows $designMatrixRows `
        -Expected $expectedDesignMatrix
    $checks.Add([ordered]@{
        name = 'complete-fail-closed-design-0010-matrix'
        passed = ($designMatrixAssertionCount -eq 15)
        assertionCount = $designMatrixAssertionCount
        normativeRowCount = $designMatrixRows.Count
    })
    $expectedTopLevelCheckNames = @(
        'fail-closed-preexisting-output-preflight'
        'fail-closed-named-proof-parser'
        'fail-closed-imaging-recovery-check-set'
        'fail-closed-windows-path-check-set'
        'fail-closed-post-proof-source-mutation'
        'owned-process-listener-cleanup-probe'
        'clean-debug-sidecar-preparation'
        'rust-typescript-contracts'
        'frontend-tests'
        'frontend-typecheck'
        'rust-tests'
        'rust-fmt-clippy-deny-warnings'
        'real-processor-cache-canvas-recovery'
        'windows-local-unc-mapped-long-paths'
        'release-build-and-nsis-package'
        'owned-process-lock-listener-cleanup'
        'independent-windows-path-scratch-preservation'
        'isolated-cargo-target-cleanup'
        'complete-fail-closed-design-0010-matrix'
    )
    $topLevelChecks = @(
        $checks | ForEach-Object { [pscustomobject] $_ }
    )
    if (-not (Test-ExactPassedCheckSet `
            -Checks $topLevelChecks `
            -ExpectedNames $expectedTopLevelCheckNames)) {
        throw 'The issue 45 top-level gate checks are not the exact passing Boolean set.'
    }
    $criteria = @(New-Issue45VerifiedCriteria `
        -RustText $rustRun.output `
        -FrontendData $frontendAssertions `
        -DesignMatrixRows $designMatrixRows `
        -ImagingProofText $imagingProofText `
        -WindowsProofText $windowsProofText `
        -NarrowApiProofText $narrowApiProofText)

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
            ownedProcessesAfter = $ownedProcessCountAfter
            ownedListenersAfter = $ownedListenerCountAfter
            ownedProcessesStoppedAtFinalCleanup = $finalOwnedCleanup.stoppedProcessCount
            ownedListenersObservedBeforeFinalCleanup = $finalOwnedCleanup.listenersBefore
        }
        checks = @($checks)
        criteria = $criteria
        nestedEvidence = [ordered]@{
            imaging = [ordered]@{
                schemaVersion = $imagingEvidence.schemaVersion
                checks = $imagingCheckCount
                checkNames = @($imagingChecks | ForEach-Object { $_.name })
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
            runScratchRemoved = $runScratchRemoved
            ownedScratchContainerRemoved = $ownedScratchContainerRemoved
            windowsPathScratchRemoved = $windowsPathScratchRemoved
            independentWindowsPathScratchPreserved =
                $independentWindowsPathScratchPreserved
            independentWindowsPathScratchProbeRemoved =
                $independentWindowsPathScratchProbeRemoved
            isolatedCargoTargetRemoved = $isolatedCargoTargetRemoved
            sharedCargoTargetUntouched = $sharedCargoTargetUntouched
            preparedSidecarRemoved = -not (Test-Path -LiteralPath $preparedSidecarPath)
            distRemoved = -not (Test-Path -LiteralPath $distPath)
            ownedProcesses = $ownedProcessCountAfter
            ownedListeners = $ownedListenerCountAfter
            claimedPreexistingProcessIdentities = $claimedPreexistingIdentities.Count
            stoppedOwnedProcesses = $finalOwnedCleanup.stoppedProcessCount
            observedOwnedListenersBeforeCleanup = $finalOwnedCleanup.listenersBefore
            artifactLocks = 0
        }
    }
    [void] (Assert-GateSourceUnchanged `
        -Before $sourceBefore `
        -WorkspaceRoot $workspaceRoot `
        -EvidencePath $OutputPath `
        -Stage 'while assembling the final report')
    $json = $report | ConvertTo-Json -Depth 12
}
finally {
    $env:PSModulePath = $previousModulePath
    $env:CARGO_TARGET_DIR = $previousTargetDirectory
}
}
finally {
    $ownedCleanupFailure = $null
    try {
        $terminalCleanup = Stop-GateProcessScope -Scope $processScope
        if ($terminalCleanup.processesAfter -ne 0 -or
                $terminalCleanup.listenersAfter -ne 0) {
            throw 'Owned process or listener state remained after terminal cleanup.'
        }
    }
    catch {
        $ownedCleanupFailure = $_.Exception.Message
    }
    try {
        Clear-Issue45GateOutputs -Scope $scratchScope
    }
    finally {
        try {
            Remove-IndependentWindowsPathScratchProbe `
                -Probe $independentWindowsPathScratchProbe
        }
        finally {
            if ($runnerMutexHeld) {
                $runnerMutex.ReleaseMutex()
            }
            $runnerMutex.Dispose()
        }
    }
    if ($null -ne $ownedCleanupFailure) {
        throw "The issue 45 gate failed closed during terminal process cleanup: $ownedCleanupFailure"
    }
}

if ([string]::IsNullOrWhiteSpace($json)) {
    throw 'The issue 45 gate produced no verified report for publication.'
}
Publish-GateEvidenceReport `
    -Json $json `
    -SourceSnapshot $sourceBefore `
    -WorkspaceRoot $workspaceRoot `
    -EvidencePath $OutputPath
Write-Output "Issue 45 Media and Cache report: $OutputPath"
Write-Output $json
