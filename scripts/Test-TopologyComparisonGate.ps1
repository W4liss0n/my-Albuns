param(
    [switch] $SkipCollection,
    [string] $ProtocolPath,
    [string] $RunAbPath,
    [string] $RunBaPath,
    [string] $OutputPath,
    [ValidateRange(10, 120)]
    [int] $WindowTimeoutSeconds = 45,
    [ValidateRange(30, 1800)]
    [int] $CacheTimeoutSeconds = 900,
    [ValidateRange(30, 1800)]
    [int] $PerformanceTimeoutSeconds = 300
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

function Resolve-WorkspacePath {
    param(
        [AllowEmptyString()]
        [string] $Path,
        [Parameter(Mandatory = $true)]
        [string] $DefaultRelativePath
    )

    $candidate = if ([string]::IsNullOrWhiteSpace($Path)) {
        Join-Path $script:WorkspaceRoot $DefaultRelativePath
    }
    elseif ([System.IO.Path]::IsPathRooted($Path)) {
        $Path
    }
    else {
        Join-Path $script:WorkspaceRoot $Path
    }
    return [System.IO.Path]::GetFullPath($candidate)
}

function Get-WorkspaceRelativePath {
    param([Parameter(Mandatory = $true)][string] $Path)

    $workspacePrefix = [System.IO.Path]::GetFullPath(
        $script:WorkspaceRoot
    ).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    Assert-Condition `
        -Condition $fullPath.StartsWith(
            $workspacePrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        ) `
        -Message 'Topology comparison paths must stay inside the workspace.'
    return $fullPath.
        Substring($workspacePrefix.Length).
        Replace([System.IO.Path]::DirectorySeparatorChar, '/')
}

function Read-JsonObject {
    param([Parameter(Mandatory = $true)][string] $Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required topology comparison input is missing: $Path"
    }
    try {
        return Get-Content -LiteralPath $Path -Raw -Encoding utf8 |
            ConvertFrom-Json
    }
    catch {
        throw "Topology comparison input is not valid JSON: $Path"
    }
}

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)]
        [bool] $Condition,
        [Parameter(Mandatory = $true)]
        [string] $Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Assert-JsonNumber {
    param(
        $Value,
        [Parameter(Mandatory = $true)]
        [string] $Field,
        [double] $Minimum = 0
    )

    if ($null -eq $Value -or $Value -is [string] -or $Value -is [bool]) {
        throw "Final topology evidence field must be numeric: $Field"
    }
    try {
        $number = [double]$Value
    }
    catch {
        throw "Final topology evidence field must be numeric: $Field"
    }
    if (
        [double]::IsNaN($number) -or
        [double]::IsInfinity($number) -or
        $number -lt $Minimum
    ) {
        throw "Final topology evidence field is outside its range: $Field"
    }
}

function Assert-JsonString {
    param(
        $Value,
        [Parameter(Mandatory = $true)]
        [string] $Field,
        [string] $Pattern
    )

    if ($Value -isnot [string] -or [string]::IsNullOrWhiteSpace($Value)) {
        throw "Final topology evidence field must be a non-empty string: $Field"
    }
    if (-not [string]::IsNullOrWhiteSpace($Pattern) -and $Value -cnotmatch $Pattern) {
        throw "Final topology evidence field has an invalid format: $Field"
    }
}

function Assert-JsonBoolean {
    param(
        $Value,
        [Parameter(Mandatory = $true)]
        [string] $Field
    )

    if ($Value -isnot [bool]) {
        throw "Final topology evidence field must be boolean: $Field"
    }
}

function Assert-TimingSummary {
    param(
        [Parameter(Mandatory = $true)] $Summary,
        [Parameter(Mandatory = $true)][string] $Field
    )

    Assert-JsonNumber -Value $Summary.sampleCount -Field "$Field.sampleCount" -Minimum 1
    foreach ($name in @(
        'durationMs',
        'firstFrameLatencyMs',
        'meanFrameMs',
        'p50FrameMs',
        'p95FrameMs',
        'p99FrameMs',
        'maxFrameMs',
        'framesOver16Ms',
        'framesOver33Ms'
    )) {
        Assert-JsonNumber -Value $Summary.$name -Field "$Field.$name"
    }
}

function Assert-ContinuityProbeContract {
    param(
        [Parameter(Mandatory = $true)] $Probe,
        [Parameter(Mandatory = $true)][string] $Field
    )

    foreach ($name in @(
        'expectedCompletions',
        'observedCompletions',
        'duplicateCompletions',
        'missingCompletions'
    )) {
        Assert-JsonNumber -Value $Probe.$name -Field "$Field.$name"
    }
    foreach ($entry in @($Probe.evidence)) {
        Assert-JsonString -Value $entry.projectId -Field "$Field.evidence.projectId"
        Assert-JsonNumber -Value $entry.persistedRevision -Field "$Field.evidence.persistedRevision"
        Assert-JsonNumber -Value $entry.reopenedRevision -Field "$Field.evidence.reopenedRevision"
        Assert-JsonBoolean -Value $entry.dirty -Field "$Field.evidence.dirty"
        Assert-JsonBoolean `
            -Value $entry.globalAvailable `
            -Field "$Field.evidence.globalAvailable"
        if ($null -ne $entry.globalProcessId) {
            Assert-JsonNumber `
                -Value $entry.globalProcessId `
                -Field "$Field.evidence.globalProcessId" `
                -Minimum 1
        }
    }
}

function Assert-TerminationContract {
    param(
        [Parameter(Mandatory = $true)] $Termination,
        [Parameter(Mandatory = $true)][string] $Field
    )

    foreach ($name in @(
        'exitObserved',
        'executableValidated',
        'descendantsExited'
    )) {
        Assert-JsonBoolean `
            -Value $Termination.$name `
            -Field "$Field.$name"
    }
    foreach ($name in @(
        'exitObservationMs',
        'descendantProcessCount',
        'forcedDescendantCleanupCount',
        'remainingDescendantProcessCount',
        'descendantCleanupMs'
    )) {
        Assert-JsonNumber `
            -Value $Termination.$name `
            -Field "$Field.$name"
    }
    Assert-Condition `
        -Condition (
            [int]$Termination.forcedDescendantCleanupCount -le
                [int]$Termination.descendantProcessCount
        ) `
        -Message "Final topology termination counts are inconsistent: $Field"
}

function Assert-TopologyAlternativeContract {
    param(
        [Parameter(Mandatory = $true)] $Alternative,
        [Parameter(Mandatory = $true)][string] $Field,
        [Parameter(Mandatory = $true)][int] $ExpectedHostProcessCount,
        [Parameter(Mandatory = $true)][int] $ExpectedHostGlobalLinks
    )

    Assert-JsonNumber -Value $Alternative.ready.elapsedMs -Field "$Field.ready.elapsedMs"
    Assert-Condition `
        -Condition (@($Alternative.ready.windows).Count -eq 2) `
        -Message "Final topology evidence must contain two Windows: $Field.ready.windows"

    foreach ($name in @(
        'readyElapsedMs',
        'cacheWallTimeMs',
        'projectCount',
        'mediaCount',
        'photoCount',
        'decorativeCount',
        'generatedCount',
        'reusedCount',
        'sourceBytes',
        'previewBytes',
        'sourceBytesPerSecond'
    )) {
        Assert-JsonNumber -Value $Alternative.cache.$name -Field "$Field.cache.$name"
    }
    Assert-Condition `
        -Condition (
            [int]$Alternative.cache.projectCount -eq 2 -and
            [int]$Alternative.cache.mediaCount -eq 174 -and
            [int]$Alternative.cache.photoCount -eq 172 -and
            [int]$Alternative.cache.decorativeCount -eq 2
        ) `
        -Message "Final topology evidence must contain two cached Projects: $Field.cache"

    foreach ($name in @(
        'hostProcessCount',
        'processTreeCount',
        'workingSetBytes',
        'privateMemoryBytes',
        'handleCount',
        'threadCount'
    )) {
        Assert-JsonNumber -Value $Alternative.processes.$name -Field "$Field.processes.$name"
    }
    $projectHostPriorityClasses = @($Alternative.processes.rootPriorityClasses)
    Assert-Condition `
        -Condition ($projectHostPriorityClasses.Count -eq $ExpectedHostProcessCount) `
        -Message "Final topology evidence must record every Project Host priority: $Field.processes"
    foreach ($priorityClass in $projectHostPriorityClasses) {
        Assert-JsonString `
            -Value $priorityClass `
            -Field "$Field.processes.rootPriorityClasses" `
            -Pattern '^(Idle|BelowNormal|Normal|AboveNormal|High|RealTime)$'
    }

    $globalPriorityClasses = @(
        $Alternative.forcedFailure.globalProcess.initial.processes.rootPriorityClasses
    )
    Assert-Condition `
        -Condition ($globalPriorityClasses.Count -eq 1) `
        -Message "Final topology evidence must record the global process priority: $Field.forcedFailure.globalProcess.initial.processes"
    Assert-JsonString `
        -Value $globalPriorityClasses[0] `
        -Field "$Field.forcedFailure.globalProcess.initial.processes.rootPriorityClasses" `
        -Pattern '^(Idle|BelowNormal|Normal|AboveNormal|High|RealTime)$'

    $global = $Alternative.forcedFailure.globalProcess
    Assert-TerminationContract `
        -Termination $global.termination `
        -Field "$Field.forcedFailure.globalProcess.termination"
    foreach ($entry in @(
        [ordered]@{ value = $global.initial.status.available; name = 'initial.status.available' },
        [ordered]@{ value = $global.initial.singleton.ownerPreserved; name = 'initial.singleton.ownerPreserved' },
        [ordered]@{ value = $global.unavailableBeforeExplicitRestart.available; name = 'unavailableBeforeExplicitRestart.available' },
        [ordered]@{ value = $global.noAutomaticRestartObserved; name = 'noAutomaticRestartObserved' },
        [ordered]@{ value = $global.explicitRestart.pidChanged; name = 'explicitRestart.pidChanged' },
        [ordered]@{ value = $global.explicitRestart.status.available; name = 'explicitRestart.status.available' },
        [ordered]@{ value = $global.explicitRestart.singleton.ownerPreserved; name = 'explicitRestart.singleton.ownerPreserved' }
    )) {
        Assert-JsonBoolean `
            -Value $entry.value `
            -Field "$Field.forcedFailure.globalProcess.$($entry.name)"
    }
    foreach ($entry in @(
        [ordered]@{ value = $global.initial.visibleWindowCount; name = 'initial.visibleWindowCount' },
        [ordered]@{ value = $global.initial.singleton.rejectedExitCode; name = 'initial.singleton.rejectedExitCode' },
        [ordered]@{ value = $global.windowsWhileUnavailable.expectedCount; name = 'windowsWhileUnavailable.expectedCount' },
        [ordered]@{ value = $global.windowsWhileUnavailable.observedCount; name = 'windowsWhileUnavailable.observedCount' },
        [ordered]@{ value = $global.explicitRestart.singleton.rejectedExitCode; name = 'explicitRestart.singleton.rejectedExitCode' }
    )) {
        Assert-JsonNumber `
            -Value $entry.value `
            -Field "$Field.forcedFailure.globalProcess.$($entry.name)"
    }
    Assert-ContinuityProbeContract `
        -Probe $global.offlineContinuity `
        -Field "$Field.forcedFailure.globalProcess.offlineContinuity"
    Assert-ContinuityProbeContract `
        -Probe $global.onlineContinuity `
        -Field "$Field.forcedFailure.globalProcess.onlineContinuity"

    $projectHost = $Alternative.forcedFailure.projectHost
    Assert-TerminationContract `
        -Termination $projectHost.termination `
        -Field "$Field.forcedFailure.projectHost.termination"
    foreach ($entry in @(
        [ordered]@{ value = $projectHost.hostSurvived; name = 'hostSurvived' },
        [ordered]@{ value = $projectHost.otherHostSurvived; name = 'otherHostSurvived' },
        [ordered]@{ value = $projectHost.noAutomaticRestartObserved; name = 'noAutomaticRestartObserved' },
        [ordered]@{ value = $projectHost.explicitRestart.pidChanged; name = 'explicitRestart.pidChanged' },
        [ordered]@{ value = $projectHost.explicitRestart.globalStatus.available; name = 'explicitRestart.globalStatus.available' }
    )) {
        Assert-JsonBoolean `
            -Value $entry.value `
            -Field "$Field.forcedFailure.projectHost.$($entry.name)"
    }
    foreach ($entry in @(
        [ordered]@{ value = $projectHost.remainingWindowCount; name = 'remainingWindowCount' },
        [ordered]@{ value = $projectHost.explicitRestart.reopen.expectedProjects; name = 'explicitRestart.reopen.expectedProjects' },
        [ordered]@{ value = $projectHost.explicitRestart.reopen.observedProjects; name = 'explicitRestart.reopen.observedProjects' }
    )) {
        Assert-JsonNumber `
            -Value $entry.value `
            -Field "$Field.forcedFailure.projectHost.$($entry.name)"
    }
    if ($null -ne $projectHost.survivorContinuity) {
        Assert-ContinuityProbeContract `
            -Probe $projectHost.survivorContinuity `
            -Field "$Field.forcedFailure.projectHost.survivorContinuity"
    }

    $graphics = $Alternative.interaction.canvas.aggregate.graphics
    Assert-JsonNumber -Value $graphics.webglVersion -Field "$Field.graphics.webglVersion"
    Assert-JsonString `
        -Value $graphics.contextRecovery.mechanism `
        -Field "$Field.graphics.contextRecovery.mechanism"
    foreach ($name in @('projectCount', 'lostCount', 'restoredCount', 'glError')) {
        Assert-JsonNumber `
            -Value $graphics.contextRecovery.$name `
            -Field "$Field.graphics.contextRecovery.$name"
    }
    Assert-JsonNumber `
        -Value $Alternative.interaction.postProbeGpuMemory.totalBytes `
        -Field "$Field.interaction.postProbeGpuMemory.totalBytes"
    Assert-JsonNumber `
        -Value $Alternative.interaction.canvas.allProjectsReadyElapsedMs `
        -Field "$Field.interaction.canvas.allProjectsReadyElapsedMs"
    Assert-Condition `
        -Condition (@($Alternative.interaction.canvas.projects).Count -eq 2) `
        -Message "Final topology evidence must contain two Canvas Projects: $Field.interaction.canvas"

    foreach ($metric in @('pan', 'zoom', 'navigation')) {
        Assert-JsonNumber `
            -Value $Alternative.interaction.canvas.aggregate.$metric.worstProjectP95FrameMs `
            -Field "$Field.interaction.canvas.aggregate.$metric.worstProjectP95FrameMs"
        Assert-JsonNumber `
            -Value $Alternative.interaction.canvas.aggregate.$metric.framesOver33Ms `
            -Field "$Field.interaction.canvas.aggregate.$metric.framesOver33Ms"
    }
    foreach ($project in @($Alternative.interaction.canvas.projects)) {
        Assert-JsonString -Value $project.projectId -Field "$Field.canvas.project.projectId"
        Assert-JsonString -Value $project.frameId -Field "$Field.canvas.project.frameId"
        Assert-TimingSummary -Summary $project.pan -Field "$Field.canvas.$($project.projectId).pan"
        Assert-TimingSummary -Summary $project.zoom -Field "$Field.canvas.$($project.projectId).zoom"
        Assert-TimingSummary `
            -Summary $project.navigation.timings `
            -Field "$Field.canvas.$($project.projectId).navigation.timings"
        foreach ($name in @(
            'sheetCount',
            'cycleCount',
            'maxResidentSheetCount',
            'maxResidentTextureCount',
            'maxResidentTexturePixelCount'
        )) {
            Assert-JsonNumber `
                -Value $project.navigation.$name `
                -Field "$Field.canvas.$($project.projectId).navigation.$name"
        }
        foreach ($targetSheetId in @($project.navigation.targetSheetIds)) {
            Assert-JsonString `
                -Value $targetSheetId `
                -Field "$Field.canvas.$($project.projectId).navigation.targetSheetIds"
        }
        Assert-Condition `
            -Condition (
                [int]$project.pan.sampleCount -eq 120 -and
                [int]$project.zoom.sampleCount -eq 120 -and
                [int]$project.navigation.sheetCount -eq 100 -and
                [int]$project.navigation.cycleCount -eq 10 -and
                @($project.navigation.targetSheetIds).Count -eq 3
            ) `
            -Message "Final topology evidence changed the frozen interaction workload: $Field"
    }

    foreach ($name in @(
        'elapsedMs',
        'widthPx',
        'heightPx',
        'dpi',
        'sourceCount',
        'sourceBytes',
        'outputBytes'
    )) {
        Assert-JsonNumber -Value $Alternative.interaction.export.$name -Field "$Field.export.$name"
    }
    Assert-JsonString `
        -Value $Alternative.interaction.export.outputSha256 `
        -Field "$Field.export.outputSha256" `
        -Pattern '^[0-9a-f]{64}$'
    Assert-Condition `
        -Condition ([int]$Alternative.interaction.export.dpi -eq 300) `
        -Message "Final topology evidence changed the frozen Export DPI: $Field"

    foreach ($name in @(
        'projectHostToGlobalLinkCount',
        'linksInterruptedByGlobalCrash',
        'minimumProjectHostCommandsPerProjectProbe',
        'minimumCorrelatedInteractionsPerProjectProbe'
    )) {
        Assert-JsonNumber `
            -Value $Alternative.forcedFailure.ipc.$name `
            -Field "$Field.forcedFailure.ipc.$name"
    }
    foreach ($name in @(
        'streamCount',
        'startEvents',
        'singletonRejectionEvents',
        'statusEvents',
        'missingRequiredFields'
    )) {
        Assert-JsonNumber `
            -Value $Alternative.forcedFailure.logs.global.$name `
            -Field "$Field.forcedFailure.logs.global.$name"
    }
    foreach ($name in @(
        'streamCount',
        'continuityCompletionEvents',
        'continuityFailureEvents',
        'reopenEvents',
        'missingRequiredFields'
    )) {
        Assert-JsonNumber `
            -Value $Alternative.forcedFailure.logs.projectHosts.$name `
            -Field "$Field.forcedFailure.logs.projectHosts.$name"
    }
    Assert-Condition `
        -Condition (
            [int]$Alternative.processes.hostProcessCount -eq $ExpectedHostProcessCount -and
            [int]$Alternative.forcedFailure.ipc.projectHostToGlobalLinkCount -eq
                $ExpectedHostGlobalLinks
        ) `
        -Message "Final topology evidence changed the expected process topology: $Field"
}

function Assert-TopologyRunContract {
    param(
        [Parameter(Mandatory = $true)] $Report,
        [Parameter(Mandatory = $true)][ValidateSet('AB', 'BA')][string] $ExpectedOrder
    )

    Assert-Condition `
        -Condition ($Report.schemaVersion -eq 13) `
        -Message 'Final topology runs must use measurement schema 13.'
    Assert-Condition `
        -Condition ($Report.execution.order -ceq $ExpectedOrder) `
        -Message "Final topology report did not execute order $ExpectedOrder."
    Assert-JsonString -Value $Report.collectedAtUtc -Field "$ExpectedOrder.collectedAtUtc"

    foreach ($name in @(
        'gitCommit',
        'buildInputDigestSha256',
        'executableSha256',
        'imagingExecutableSha256',
        'profile'
    )) {
        Assert-JsonString -Value $Report.build.$name -Field "$ExpectedOrder.build.$name"
    }
    Assert-JsonBoolean -Value $Report.build.workingTreeDirty -Field "$ExpectedOrder.build.workingTreeDirty"
    Assert-JsonBoolean -Value $Report.build.buildInputsDirty -Field "$ExpectedOrder.build.buildInputsDirty"
    Assert-JsonBoolean `
        -Value $Report.build.currentBuildInputsMatchManifest `
        -Field "$ExpectedOrder.build.currentBuildInputsMatchManifest"

    foreach ($name in @(
        'albumCount',
        'mediaCount',
        'photoCount',
        'decorativeCount',
        'sourceBytes'
    )) {
        Assert-JsonNumber -Value $Report.corpus.$name -Field "$ExpectedOrder.corpus.$name" -Minimum 1
    }
    Assert-JsonString `
        -Value $Report.corpus.corpusSha256 `
        -Field "$ExpectedOrder.corpus.corpusSha256" `
        -Pattern '^[0-9a-f]{64}$'
    Assert-JsonBoolean -Value $Report.corpus.integrity.verified -Field "$ExpectedOrder.corpus.integrity.verified"
    Assert-Condition `
        -Condition (
            $Report.corpus.integrity.verified -and
            $Report.corpus.integrity.beforeSha256 -ceq $Report.corpus.corpusSha256 -and
            $Report.corpus.integrity.afterSha256 -ceq $Report.corpus.corpusSha256 -and
            [int]$Report.corpus.albumCount -eq 2 -and
            [int]$Report.corpus.mediaCount -eq 173 -and
            [int]$Report.corpus.photoCount -eq 172 -and
            [int]$Report.corpus.decorativeCount -eq 1 -and
            [long]$Report.corpus.sourceBytes -eq 1469084414 -and
            $Report.corpus.corpusSha256 -ceq
                'c160ef3e3a2a1c7401f10574e3383fddb830678eb3fbc984dbca28dc59ebd01f'
        ) `
        -Message "Final topology report does not match the frozen corpus: $ExpectedOrder"

    Assert-JsonString `
        -Value $Report.hardware.operatingSystem.caption `
        -Field "$ExpectedOrder.hardware.operatingSystem.caption"
    Assert-JsonString `
        -Value $Report.hardware.operatingSystem.version `
        -Field "$ExpectedOrder.hardware.operatingSystem.version"
    Assert-JsonNumber `
        -Value $Report.hardware.totalPhysicalMemoryBytes `
        -Field "$ExpectedOrder.hardware.totalPhysicalMemoryBytes" `
        -Minimum 1
    Assert-Condition `
        -Condition (@($Report.hardware.cpu).Count -gt 0 -and @($Report.hardware.gpu).Count -gt 0) `
        -Message "Final topology report has incomplete hardware inventory: $ExpectedOrder"

    Assert-TopologyAlternativeContract `
        -Alternative $Report.alternatives.independentHosts `
        -Field "$ExpectedOrder.alternatives.independentHosts" `
        -ExpectedHostProcessCount 2 `
        -ExpectedHostGlobalLinks 2
    Assert-TopologyAlternativeContract `
        -Alternative $Report.alternatives.multiwindowHost `
        -Field "$ExpectedOrder.alternatives.multiwindowHost" `
        -ExpectedHostProcessCount 1 `
        -ExpectedHostGlobalLinks 1
    Assert-JsonBoolean -Value $Report.failureGate.passed -Field "$ExpectedOrder.failureGate.passed"
    Assert-JsonBoolean `
        -Value $Report.failureGate.imagingProcessor.validated `
        -Field "$ExpectedOrder.failureGate.imagingProcessor.validated"
    Assert-JsonString `
        -Value $Report.failureGate.imagingProcessor.artifactSha256 `
        -Field "$ExpectedOrder.failureGate.imagingProcessor.artifactSha256" `
        -Pattern '^[0-9a-f]{64}$'
    Assert-JsonNumber `
        -Value $Report.failureGate.imagingProcessor.artifactSchemaVersion `
        -Field "$ExpectedOrder.failureGate.imagingProcessor.artifactSchemaVersion" `
        -Minimum 1
    foreach ($name in @(
        'sourceInputsDirty',
        'sameGitCommitAsTopologyBuild',
        'cacheRecoveredAfterOneExplicitRestart',
        'exportFailedSafelyUntilExplicitRetry'
    )) {
        Assert-JsonBoolean `
            -Value $Report.failureGate.imagingProcessor.$name `
            -Field "$ExpectedOrder.failureGate.imagingProcessor.$name"
    }
}

function Assert-CanonicalArtifactPath {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $ArtifactDirectory
    )

    $parent = [System.IO.Path]::GetFullPath((Split-Path -Parent $Path))
    Assert-Condition `
        -Condition (
            [string]::Equals(
                $parent,
                $ArtifactDirectory,
                [System.StringComparison]::OrdinalIgnoreCase
            ) -and
            [System.IO.Path]::GetExtension($Path) -ceq '.json'
        ) `
        -Message 'Conclusive topology evidence must be a JSON file in docs/research/artifacts.'
}

function New-ObservedRange {
    param(
        [Parameter(Mandatory = $true)]
        [double] $Ab,
        [Parameter(Mandatory = $true)]
        [double] $Ba
    )

    return [ordered]@{
        byExecutionOrder = [ordered]@{
            AB = $Ab
            BA = $Ba
        }
        minimum = [Math]::Min($Ab, $Ba)
        maximum = [Math]::Max($Ab, $Ba)
        median = ($Ab + $Ba) / 2
    }
}

function New-MetricComparison {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Unit,
        [Parameter(Mandatory = $true)]
        [ValidateSet('lower', 'higher')]
        [string] $Better,
        [Parameter(Mandatory = $true)]
        [double] $IndependentAb,
        [Parameter(Mandatory = $true)]
        [double] $MultiwindowAb,
        [Parameter(Mandatory = $true)]
        [double] $IndependentBa,
        [Parameter(Mandatory = $true)]
        [double] $MultiwindowBa
    )

    $deltaAb = $MultiwindowAb - $IndependentAb
    $deltaBa = $MultiwindowBa - $IndependentBa
    $sameDirection = (
        ($deltaAb -gt 0 -and $deltaBa -gt 0) -or
        ($deltaAb -lt 0 -and $deltaBa -lt 0)
    )
    $minimumGap = [Math]::Min([Math]::Abs($deltaAb), [Math]::Abs($deltaBa))
    $withinAlternativeRange = [Math]::Max(
        [Math]::Abs($IndependentAb - $IndependentBa),
        [Math]::Abs($MultiwindowAb - $MultiwindowBa)
    )
    $consistent = $sameDirection -and $minimumGap -gt $withinAlternativeRange
    $winner = $null
    if ($consistent) {
        $independentLower = $deltaAb -gt 0
        if ($Better -eq 'lower') {
            $winner = if ($independentLower) { 'independent' } else { 'multiwindow' }
        }
        else {
            $winner = if ($independentLower) { 'multiwindow' } else { 'independent' }
        }
    }

    return [ordered]@{
        unit = $Unit
        better = $Better
        independent = New-ObservedRange -Ab $IndependentAb -Ba $IndependentBa
        multiwindow = New-ObservedRange -Ab $MultiwindowAb -Ba $MultiwindowBa
        interpretation = [ordered]@{
            classification = if ($consistent) { 'consistent' } else { 'inconclusive' }
            winner = $winner
            minimumCrossTopologyGap = $minimumGap
            maximumWithinTopologyRange = $withinAlternativeRange
        }
    }
}

function New-ContinuityFacts {
    param(
        [Parameter(Mandatory = $true)] $Probe,
        [Parameter(Mandatory = $true)][int] $ExpectedCompletions,
        [Parameter(Mandatory = $true)][bool] $ExpectedGlobalAvailable
    )

    $evidence = @($Probe.evidence)
    $reportedExpectedCompletions = [int]$Probe.expectedCompletions
    $revisionsRecovered = (
        $evidence.Count -eq $ExpectedCompletions -and
        @(
            $evidence | Where-Object {
                [int]$_.persistedRevision -ne [int]$_.reopenedRevision
            }
        ).Count -eq 0
    )
    $globalAvailabilityMatched = @(
        $evidence | Where-Object {
            [bool]$_.globalAvailable -ne $ExpectedGlobalAvailable
        }
    ).Count -eq 0
    $globalProcessIdentityMatched = @(
        $evidence | Where-Object {
            if ($ExpectedGlobalAvailable) {
                $null -eq $_.globalProcessId -or [int]$_.globalProcessId -le 0
            }
            else {
                $null -ne $_.globalProcessId
            }
        }
    ).Count -eq 0
    $cleanPersistedState = @(
        $evidence | Where-Object { [bool]$_.dirty }
    ).Count -eq 0
    $passed = (
        $reportedExpectedCompletions -eq $ExpectedCompletions -and
        [int]$Probe.observedCompletions -eq $ExpectedCompletions -and
        [int]$Probe.duplicateCompletions -eq 0 -and
        [int]$Probe.missingCompletions -eq 0 -and
        $revisionsRecovered -and
        $cleanPersistedState -and
        $globalAvailabilityMatched -and
        $globalProcessIdentityMatched
    )

    return [ordered]@{
        expectedCompletions = $ExpectedCompletions
        reportedExpectedCompletions = $reportedExpectedCompletions
        observedCompletions = [int]$Probe.observedCompletions
        duplicateCompletions = [int]$Probe.duplicateCompletions
        missingCompletions = [int]$Probe.missingCompletions
        revisionsRecovered = $revisionsRecovered
        cleanPersistedState = $cleanPersistedState
        expectedGlobalAvailable = $ExpectedGlobalAvailable
        globalAvailabilityMatched = $globalAvailabilityMatched
        globalProcessIdentityMatched = $globalProcessIdentityMatched
        passed = $passed
    }
}

function New-AlternativeRobustnessFacts {
    param(
        [Parameter(Mandatory = $true)] $Alternative,
        [Parameter(Mandatory = $true)][int] $ExpectedRemainingWindows,
        [Parameter(Mandatory = $true)][int] $ExpectedReopenedProjects,
        [Parameter(Mandatory = $true)][bool] $ExpectedOtherHostSurvived
    )

    $global = $Alternative.forcedFailure.globalProcess
    $projectHostSource = $Alternative.forcedFailure.projectHost
    $offlineContinuity = New-ContinuityFacts `
        -Probe $global.offlineContinuity `
        -ExpectedCompletions 2 `
        -ExpectedGlobalAvailable $false
    $onlineContinuity = New-ContinuityFacts `
        -Probe $global.onlineContinuity `
        -ExpectedCompletions 2 `
        -ExpectedGlobalAvailable $true
    $reopenedProjects = @($projectHostSource.explicitRestart.reopen.projects)
    $onlineEvidence = @($global.onlineContinuity.evidence)
    $reopenedRevisionsRecovered = (
        $reopenedProjects.Count -eq $ExpectedReopenedProjects -and
        @(
            $reopenedProjects | Where-Object {
                $reopenedProject = $_
                @(
                    $onlineEvidence | Where-Object {
                        $_.projectId -ceq $reopenedProject.projectId -and
                        [int]$_.persistedRevision -eq [int]$reopenedProject.revision
                    }
                ).Count -ne 1
            }
        ).Count -eq 0
    )
    $survivorApplicable = $ExpectedRemainingWindows -gt 0
    $survivorContinuity = if ($survivorApplicable) {
        New-ContinuityFacts `
            -Probe $projectHostSource.survivorContinuity `
            -ExpectedCompletions 1 `
            -ExpectedGlobalAvailable $true
    }
    else {
        $null
    }

    $globalInitial = [ordered]@{
        available = [bool]$global.initial.status.available
        visibleWindowCount = [int]$global.initial.visibleWindowCount
        processTreeCount = [int]$global.initial.processes.processTreeCount
        workingSetBytes = [long]$global.initial.processes.workingSetBytes
        priorityClasses = @($global.initial.processes.rootPriorityClasses)
        singletonOwnerPreserved = [bool]$global.initial.singleton.ownerPreserved
        singletonRejectedExitCode = [int]$global.initial.singleton.rejectedExitCode
    }
    $globalInitial.passed = (
        $globalInitial.available -and
        $globalInitial.visibleWindowCount -eq 0 -and
        $globalInitial.priorityClasses.Count -eq 1 -and
        $globalInitial.singletonOwnerPreserved -and
        $globalInitial.singletonRejectedExitCode -eq 73
    )

    $globalOutage = [ordered]@{
        terminationObserved = [bool]$global.termination.exitObserved
        terminationExecutableValidated = [bool]$global.termination.executableValidated
        descendantProcessCount = [int]$global.termination.descendantProcessCount
        forcedDescendantCleanupCount =
            [int]$global.termination.forcedDescendantCleanupCount
        descendantsExited = [bool]$global.termination.descendantsExited
        remainingDescendantProcessCount =
            [int]$global.termination.remainingDescendantProcessCount
        descendantCleanupMs = [long]$global.termination.descendantCleanupMs
        unavailableBeforeExplicitRestart = -not [bool]$global.unavailableBeforeExplicitRestart.available
        noAutomaticRestartObserved = [bool]$global.noAutomaticRestartObserved
        expectedWindows = [int]$global.windowsWhileUnavailable.expectedCount
        observedWindows = [int]$global.windowsWhileUnavailable.observedCount
        unexpectedProcesses = @($global.processSetWhileUnavailable.unexpectedProcessIds).Count
        offlineContinuity = $offlineContinuity
    }
    $globalOutage.passed = (
        $globalOutage.terminationObserved -and
        $globalOutage.terminationExecutableValidated -and
        $globalOutage.descendantsExited -and
        $globalOutage.remainingDescendantProcessCount -eq 0 -and
        $globalOutage.unavailableBeforeExplicitRestart -and
        $globalOutage.noAutomaticRestartObserved -and
        $globalOutage.expectedWindows -eq 2 -and
        $globalOutage.observedWindows -eq 2 -and
        $globalOutage.unexpectedProcesses -eq 0 -and
        $offlineContinuity.passed
    )

    $globalRestart = [ordered]@{
        pidChanged = [bool]$global.explicitRestart.pidChanged
        available = [bool]$global.explicitRestart.status.available
        singletonOwnerPreserved = [bool]$global.explicitRestart.singleton.ownerPreserved
        singletonRejectedExitCode = [int]$global.explicitRestart.singleton.rejectedExitCode
        onlineContinuity = $onlineContinuity
    }
    $globalRestart.passed = (
        $globalRestart.pidChanged -and
        $globalRestart.available -and
        $globalRestart.singletonOwnerPreserved -and
        $globalRestart.singletonRejectedExitCode -eq 73 -and
        $onlineContinuity.passed
    )

    $projectHost = [ordered]@{
        terminationObserved = [bool]$projectHostSource.termination.exitObserved
        terminationExecutableValidated = [bool]$projectHostSource.termination.executableValidated
        descendantProcessCount =
            [int]$projectHostSource.termination.descendantProcessCount
        forcedDescendantCleanupCount =
            [int]$projectHostSource.termination.forcedDescendantCleanupCount
        descendantsExited =
            [bool]$projectHostSource.termination.descendantsExited
        remainingDescendantProcessCount =
            [int]$projectHostSource.termination.remainingDescendantProcessCount
        descendantCleanupMs =
            [long]$projectHostSource.termination.descendantCleanupMs
        terminatedHostSurvived = [bool]$projectHostSource.hostSurvived
        otherHostSurvived = [bool]$projectHostSource.otherHostSurvived
        expectedOtherHostSurvived = $ExpectedOtherHostSurvived
        noAutomaticRestartObserved = [bool]$projectHostSource.noAutomaticRestartObserved
        expectedRemainingWindows = $ExpectedRemainingWindows
        observedRemainingWindows = [int]$projectHostSource.remainingWindowCount
        unexpectedProcesses = @($projectHostSource.processSetAfterCrash.unexpectedProcessIds).Count
        survivorContinuityApplicable = $survivorApplicable
        survivorContinuity = $survivorContinuity
        restartPidChanged = [bool]$projectHostSource.explicitRestart.pidChanged
        expectedReopenedProjects = $ExpectedReopenedProjects
        reportedExpectedReopenedProjects = [int]$projectHostSource.explicitRestart.reopen.expectedProjects
        observedReopenedProjects = [int]$projectHostSource.explicitRestart.reopen.observedProjects
        reopenedRevisionsRecovered = $reopenedRevisionsRecovered
        globalAvailableAfterRestart = [bool]$projectHostSource.explicitRestart.globalStatus.available
    }
    $survivorPassed = if ($survivorApplicable) {
        $null -ne $survivorContinuity -and $survivorContinuity.passed
    }
    else {
        $null -eq $projectHostSource.survivorContinuity
    }
    $projectHost.passed = (
        $projectHost.terminationObserved -and
        $projectHost.terminationExecutableValidated -and
        $projectHost.descendantsExited -and
        $projectHost.remainingDescendantProcessCount -eq 0 -and
        -not $projectHost.terminatedHostSurvived -and
        $projectHost.otherHostSurvived -eq $ExpectedOtherHostSurvived -and
        $projectHost.noAutomaticRestartObserved -and
        $projectHost.observedRemainingWindows -eq $ExpectedRemainingWindows -and
        $projectHost.unexpectedProcesses -eq 0 -and
        $survivorPassed -and
        $projectHost.restartPidChanged -and
        $projectHost.reportedExpectedReopenedProjects -eq $ExpectedReopenedProjects -and
        $projectHost.observedReopenedProjects -eq $ExpectedReopenedProjects -and
        $projectHost.reopenedRevisionsRecovered -and
        $projectHost.globalAvailableAfterRestart
    )

    $graphicsSource = $Alternative.interaction.canvas.aggregate.graphics
    $graphics = [ordered]@{
        webglVersion = [int]$graphicsSource.webglVersion
        contextRecoveryMechanism = [string]$graphicsSource.contextRecovery.mechanism
        projectCount = [int]$graphicsSource.contextRecovery.projectCount
        contextLostCount = [int]$graphicsSource.contextRecovery.lostCount
        contextRestoredCount = [int]$graphicsSource.contextRecovery.restoredCount
        glError = [int]$graphicsSource.contextRecovery.glError
    }
    $graphics.passed = (
        $graphics.webglVersion -eq 2 -and
        $graphics.contextRecoveryMechanism -ceq 'webgl_lose_context' -and
        $graphics.projectCount -eq 2 -and
        $graphics.contextLostCount -eq $graphics.projectCount -and
        $graphics.contextRestoredCount -eq $graphics.projectCount -and
        $graphics.glError -eq 0
    )

    $globalLogs = $Alternative.forcedFailure.logs.global
    $projectLogs = $Alternative.forcedFailure.logs.projectHosts
    $logs = [ordered]@{
        globalStreamCount = [int]$globalLogs.streamCount
        globalMissingRequiredFields = [int]$globalLogs.missingRequiredFields
        projectHostStreamCount = [int]$projectLogs.streamCount
        projectHostContinuityFailureEvents = [int]$projectLogs.continuityFailureEvents
        projectHostMissingRequiredFields = [int]$projectLogs.missingRequiredFields
    }
    $logs.passed = (
        $logs.globalStreamCount -gt 0 -and
        $logs.globalMissingRequiredFields -eq 0 -and
        $logs.projectHostStreamCount -gt 0 -and
        $logs.projectHostContinuityFailureEvents -eq 0 -and
        $logs.projectHostMissingRequiredFields -eq 0
    )

    $passed = (
        $globalInitial.passed -and
        $globalOutage.passed -and
        $globalRestart.passed -and
        $projectHost.passed -and
        $graphics.passed -and
        $logs.passed
    )
    return [ordered]@{
        globalProcess = [ordered]@{
            initial = $globalInitial
            outage = $globalOutage
            explicitRestart = $globalRestart
        }
        projectHost = $projectHost
        graphics = $graphics
        logs = $logs
        passed = $passed
    }
}

function New-RunRobustnessFacts {
    param([Parameter(Mandatory = $true)] $Run)

    $independent = New-AlternativeRobustnessFacts `
        -Alternative $Run.alternatives.independentHosts `
        -ExpectedRemainingWindows 1 `
        -ExpectedReopenedProjects 1 `
        -ExpectedOtherHostSurvived $true
    $multiwindow = New-AlternativeRobustnessFacts `
        -Alternative $Run.alternatives.multiwindowHost `
        -ExpectedRemainingWindows 0 `
        -ExpectedReopenedProjects 2 `
        -ExpectedOtherHostSurvived $false
    $imagingSource = $Run.failureGate.imagingProcessor
    $imaging = [ordered]@{
        validated = [bool]$imagingSource.validated
        artifactSha256 = [string]$imagingSource.artifactSha256
        artifactSchemaVersion = [int]$imagingSource.artifactSchemaVersion
        sourceInputsDirty = [bool]$imagingSource.sourceInputsDirty
        sameGitCommitAsTopologyBuild = [bool]$imagingSource.sameGitCommitAsTopologyBuild
        cacheRecoveredAfterOneExplicitRestart = [bool]$imagingSource.cacheRecoveredAfterOneExplicitRestart
        exportFailedSafelyUntilExplicitRetry = [bool]$imagingSource.exportFailedSafelyUntilExplicitRetry
    }
    $imaging.passed = (
        $imaging.validated -and
        -not $imaging.sourceInputsDirty -and
        $imaging.sameGitCommitAsTopologyBuild -and
        $imaging.cacheRecoveredAfterOneExplicitRestart -and
        $imaging.exportFailedSafelyUntilExplicitRetry
    )
    $rawFailureGatePassed = [bool]$Run.failureGate.passed
    $factsPassed = (
        $independent.passed -and
        $multiwindow.passed -and
        $imaging.passed
    )

    return [ordered]@{
        rawFailureGatePassed = $rawFailureGatePassed
        rawFailureGateConsistent = $rawFailureGatePassed -eq $factsPassed
        independent = $independent
        multiwindow = $multiwindow
        imagingProcessor = $imaging
        factsRecorded = $true
        passed = (
            $rawFailureGatePassed -and
            $factsPassed -and
            $rawFailureGatePassed -eq $factsPassed
        )
    }
}

$ProtocolPath = Resolve-WorkspacePath `
    -Path $ProtocolPath `
    -DefaultRelativePath 'docs\research\0027-protocolo-da-comparacao-final-de-topologias.md'
$RunAbPath = Resolve-WorkspacePath `
    -Path $RunAbPath `
    -DefaultRelativePath 'docs\research\artifacts\0019-topology-final-ab.json'
$RunBaPath = Resolve-WorkspacePath `
    -Path $RunBaPath `
    -DefaultRelativePath 'docs\research\artifacts\0020-topology-final-ba.json'
$OutputPath = Resolve-WorkspacePath `
    -Path $OutputPath `
    -DefaultRelativePath 'docs\research\artifacts\0021-topology-final-comparison.json'
$artifactDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'docs\research\artifacts')
)
foreach ($artifactPath in @($RunAbPath, $RunBaPath, $OutputPath)) {
    Assert-CanonicalArtifactPath `
        -Path $artifactPath `
        -ArtifactDirectory $artifactDirectory
}
Assert-Condition `
    -Condition (
        -not [string]::Equals($RunAbPath, $RunBaPath, [System.StringComparison]::OrdinalIgnoreCase) -and
        -not [string]::Equals($RunAbPath, $OutputPath, [System.StringComparison]::OrdinalIgnoreCase) -and
        -not [string]::Equals($RunBaPath, $OutputPath, [System.StringComparison]::OrdinalIgnoreCase)
    ) `
    -Message 'AB, BA, and consolidated topology evidence require distinct files.'

Assert-Condition `
    -Condition (Test-Path -LiteralPath $ProtocolPath -PathType Leaf) `
    -Message "Frozen topology comparison protocol is missing: $ProtocolPath"
$protocolRelativePath = Get-WorkspaceRelativePath -Path $ProtocolPath
& git -C $script:WorkspaceRoot ls-files --error-unmatch -- $protocolRelativePath |
    Out-Null
$protocolTracked = $LASTEXITCODE -eq 0
Assert-Condition `
    -Condition $protocolTracked `
    -Message 'The topology comparison protocol must be committed before collection.'
& git -C $script:WorkspaceRoot diff --quiet HEAD -- $protocolRelativePath
$protocolUnchanged = $LASTEXITCODE -eq 0
Assert-Condition `
    -Condition $protocolUnchanged `
    -Message 'The topology comparison protocol changed after it was frozen.'
$criteriaFrozenBeforeFinalExecution = $protocolTracked -and $protocolUnchanged

$headCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
Assert-Condition `
    -Condition ($headCommit -match '^[0-9a-f]{40}$') `
    -Message 'Could not identify the commit used by the comparison.'
$protocolSha256 = (
    Get-FileHash -LiteralPath $ProtocolPath -Algorithm SHA256
).Hash.ToLowerInvariant()
$protocolText = Get-Content -LiteralPath $ProtocolPath -Raw -Encoding utf8
foreach ($requiredProtocolText in @(
    '## Neutraliza',
    '`AB`: A seguida de B',
    '`BA`: B seguida de A',
    '## Custo de implementa',
    '33,33 ms'
)) {
    Assert-Condition `
        -Condition $protocolText.Contains($requiredProtocolText) `
        -Message "The frozen protocol lost a required rule: $requiredProtocolText"
}

if (-not $SkipCollection) {
    $collectionDirectory = Join-Path `
        $script:WorkspaceRoot `
        'target\topology-final-comparison'
    New-Item -ItemType Directory -Force -Path $collectionDirectory | Out-Null
    $imagingRecoveryPath = Join-Path $collectionDirectory 'imaging-recovery.json'

    & (Join-Path $PSScriptRoot 'Test-ImagingRecovery.ps1') `
        -OutputPath $imagingRecoveryPath
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & (Join-Path $PSScriptRoot 'Measure-TopologySpike.ps1') `
        -ExecutionOrder AB `
        -ImagingRecoveryPath $imagingRecoveryPath `
        -WindowTimeoutSeconds $WindowTimeoutSeconds `
        -CacheTimeoutSeconds $CacheTimeoutSeconds `
        -PerformanceTimeoutSeconds $PerformanceTimeoutSeconds `
        -OutputPath $RunAbPath
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & (Join-Path $PSScriptRoot 'Measure-TopologySpike.ps1') `
        -SkipBuild `
        -ExecutionOrder BA `
        -ImagingRecoveryPath $imagingRecoveryPath `
        -WindowTimeoutSeconds $WindowTimeoutSeconds `
        -CacheTimeoutSeconds $CacheTimeoutSeconds `
        -PerformanceTimeoutSeconds $PerformanceTimeoutSeconds `
        -OutputPath $RunBaPath
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

$runAb = Read-JsonObject -Path $RunAbPath
$runBa = Read-JsonObject -Path $RunBaPath

Assert-TopologyRunContract -Report $runAb -ExpectedOrder AB
Assert-TopologyRunContract -Report $runBa -ExpectedOrder BA
$balancedOrdersPassed = (
    $runAb.execution.order -ceq 'AB' -and
    $runBa.execution.order -ceq 'BA'
)

$buildIdentityFields = @(
    'gitCommit',
    'buildInputDigestSha256',
    'executableSha256',
    'imagingExecutableSha256',
    'profile'
)
foreach ($field in $buildIdentityFields) {
    Assert-Condition `
        -Condition ($runAb.build.$field -ceq $runBa.build.$field) `
        -Message "Final topology runs used different build field: $field"
}
$sameCleanReleaseBuild = (
    $runAb.build.gitCommit -ceq $headCommit -and
    -not $runAb.build.buildInputsDirty -and
    -not $runBa.build.buildInputsDirty -and
    $runAb.build.currentBuildInputsMatchManifest -and
    $runBa.build.currentBuildInputsMatchManifest -and
    $runAb.build.profile -ceq 'release'
)
Assert-Condition `
    -Condition $sameCleanReleaseBuild `
    -Message 'Final topology runs did not use clean, unchanged build inputs.'

$sameCorpus = (
    $runAb.corpus.corpusSha256 -ceq $runBa.corpus.corpusSha256 -and
    $runAb.corpus.mediaCount -eq $runBa.corpus.mediaCount -and
    $runAb.corpus.sourceBytes -eq $runBa.corpus.sourceBytes -and
    $runAb.corpus.integrity.verified -and
    $runBa.corpus.integrity.verified
)
Assert-Condition `
    -Condition $sameCorpus `
    -Message 'Final topology runs did not use the same intact corpus.'
$hardwareAb = $runAb.hardware | ConvertTo-Json -Compress -Depth 8
$hardwareBa = $runBa.hardware | ConvertTo-Json -Compress -Depth 8
$sameHardware = $hardwareAb -ceq $hardwareBa
Assert-Condition `
    -Condition $sameHardware `
    -Message 'Final topology runs were not collected on the same hardware.'
$robustness = [ordered]@{
    method = 'normalized_facts_from_raw_runs'
    byExecutionOrder = [ordered]@{
        AB = New-RunRobustnessFacts -Run $runAb
        BA = New-RunRobustnessFacts -Run $runBa
    }
}
$robustness.factsRecorded = (
    $robustness.byExecutionOrder.AB.factsRecorded -and
    $robustness.byExecutionOrder.BA.factsRecorded
)
$robustness.passed = (
    $robustness.factsRecorded -and
    $robustness.byExecutionOrder.AB.passed -and
    $robustness.byExecutionOrder.BA.passed
)
$robustnessFactsRecorded = [bool]$robustness.factsRecorded
$resilienceGatesPassed = [bool]$robustness.passed

$runPairs = @(
    [ordered]@{ order = 'AB'; report = $runAb },
    [ordered]@{ order = 'BA'; report = $runBa }
)
$allP95WithinTarget = $true
$sameCanvasTargets = $true
$sameExportOutputs = $true
$exportHashes = [System.Collections.Generic.List[string]]::new()
$frameTargetSignatures = [System.Collections.Generic.List[string]]::new()
$navigationTargetSignatures = [System.Collections.Generic.List[string]]::new()
foreach ($runPair in $runPairs) {
    foreach ($alternativeName in @('independentHosts', 'multiwindowHost')) {
        $alternative = $runPair.report.alternatives.$alternativeName
        foreach ($project in $alternative.interaction.canvas.projects) {
            if (
                [double]$project.pan.p95FrameMs -gt 33.33 -or
                [double]$project.zoom.p95FrameMs -gt 33.33
            ) {
                $allP95WithinTarget = $false
            }
        }
    }

    $independent = $runPair.report.alternatives.independentHosts
    $multiwindow = $runPair.report.alternatives.multiwindowHost
    $exportHashes.Add([string]$independent.interaction.export.outputSha256)
    $exportHashes.Add([string]$multiwindow.interaction.export.outputSha256)
    $sameExportOutputs = (
        $sameExportOutputs -and
        (
            $independent.interaction.export.outputSha256 -ceq
                $multiwindow.interaction.export.outputSha256 -and
            $independent.interaction.export.widthPx -eq
                $multiwindow.interaction.export.widthPx -and
            $independent.interaction.export.heightPx -eq
                $multiwindow.interaction.export.heightPx
        )
    )
    $independentFrames = @(
        $independent.interaction.canvas.projects |
            Sort-Object projectId |
            ForEach-Object { "$($_.projectId):$($_.frameId)" }
    ) -join '|'
    $multiwindowFrames = @(
        $multiwindow.interaction.canvas.projects |
            Sort-Object projectId |
            ForEach-Object { "$($_.projectId):$($_.frameId)" }
    ) -join '|'
    $independentNavigation = @(
        $independent.interaction.canvas.projects |
            Sort-Object projectId |
            ForEach-Object {
                "$($_.projectId):$($_.navigation.targetSheetIds -join ',')"
            }
    ) -join '|'
    $multiwindowNavigation = @(
        $multiwindow.interaction.canvas.projects |
            Sort-Object projectId |
            ForEach-Object {
                "$($_.projectId):$($_.navigation.targetSheetIds -join ',')"
            }
    ) -join '|'
    $targetsMatch = (
        $independentFrames -ceq $multiwindowFrames -and
        $independentNavigation -ceq $multiwindowNavigation
    )
    $sameCanvasTargets = $sameCanvasTargets -and $targetsMatch
    $frameTargetSignatures.Add($independentFrames)
    $navigationTargetSignatures.Add($independentNavigation)
}
$sameTargetsAcrossOrders = (
    @($frameTargetSignatures | Select-Object -Unique).Count -eq 1 -and
    @($navigationTargetSignatures | Select-Object -Unique).Count -eq 1
)
$sameCanvasTargets = $sameCanvasTargets -and $sameTargetsAcrossOrders
$identicalExports = (
    $sameExportOutputs -and
    @($exportHashes | Select-Object -Unique).Count -eq 1
)

function New-FinalMetric {
    param(
        [Parameter(Mandatory = $true)][string] $Unit,
        [Parameter(Mandatory = $true)][ValidateSet('lower', 'higher')][string] $Better,
        [Parameter(Mandatory = $true)][scriptblock] $Selector
    )

    return New-MetricComparison `
        -Unit $Unit `
        -Better $Better `
        -IndependentAb ([double](& $Selector $runAb.alternatives.independentHosts)) `
        -MultiwindowAb ([double](& $Selector $runAb.alternatives.multiwindowHost)) `
        -IndependentBa ([double](& $Selector $runBa.alternatives.independentHosts)) `
        -MultiwindowBa ([double](& $Selector $runBa.alternatives.multiwindowHost))
}

$metrics = [ordered]@{
    windowsReady = New-FinalMetric -Unit 'ms' -Better lower -Selector {
        param($alternative) $alternative.ready.elapsedMs
    }
    cacheReady = New-FinalMetric -Unit 'ms' -Better lower -Selector {
        param($alternative) $alternative.cache.readyElapsedMs
    }
    cacheWallTime = New-FinalMetric -Unit 'ms' -Better lower -Selector {
        param($alternative) $alternative.cache.cacheWallTimeMs
    }
    cacheThroughput = New-FinalMetric -Unit 'bytes_per_second' -Better higher -Selector {
        param($alternative) $alternative.cache.sourceBytesPerSecond
    }
    canvasReady = New-FinalMetric -Unit 'ms' -Better lower -Selector {
        param($alternative) $alternative.interaction.canvas.allProjectsReadyElapsedMs
    }
    panWorstProjectP95 = New-FinalMetric -Unit 'ms' -Better lower -Selector {
        param($alternative) $alternative.interaction.canvas.aggregate.pan.worstProjectP95FrameMs
    }
    panFramesOver33 = New-FinalMetric -Unit 'frames' -Better lower -Selector {
        param($alternative) $alternative.interaction.canvas.aggregate.pan.framesOver33Ms
    }
    zoomWorstProjectP95 = New-FinalMetric -Unit 'ms' -Better lower -Selector {
        param($alternative) $alternative.interaction.canvas.aggregate.zoom.worstProjectP95FrameMs
    }
    zoomFramesOver33 = New-FinalMetric -Unit 'frames' -Better lower -Selector {
        param($alternative) $alternative.interaction.canvas.aggregate.zoom.framesOver33Ms
    }
    navigationWorstProjectP95 = New-FinalMetric -Unit 'ms' -Better lower -Selector {
        param($alternative) $alternative.interaction.canvas.aggregate.navigation.worstProjectP95FrameMs
    }
    export = New-FinalMetric -Unit 'ms' -Better lower -Selector {
        param($alternative) $alternative.interaction.export.elapsedMs
    }
    workingSet = New-FinalMetric -Unit 'bytes' -Better lower -Selector {
        param($alternative) $alternative.processes.workingSetBytes
    }
    privateMemory = New-FinalMetric -Unit 'bytes' -Better lower -Selector {
        param($alternative) $alternative.processes.privateMemoryBytes
    }
    gpuPostProbe = New-FinalMetric -Unit 'bytes' -Better lower -Selector {
        param($alternative) $alternative.interaction.postProbeGpuMemory.totalBytes
    }
    processTree = New-FinalMetric -Unit 'processes' -Better lower -Selector {
        param($alternative) $alternative.processes.processTreeCount
    }
}

function Select-ImplementationCostSnapshot {
    param([Parameter(Mandatory = $true)] $Alternative)

    return [ordered]@{
        hostProcessCount = [int]$Alternative.processes.hostProcessCount
        projectProcessTreeCount = [int]$Alternative.processes.processTreeCount
        projectWorkingSetBytes = [long]$Alternative.processes.workingSetBytes
        projectHostPriorityClasses = @($Alternative.processes.rootPriorityClasses)
        globalProcessTreeCount = [int]$Alternative.forcedFailure.globalProcess.initial.processes.processTreeCount
        globalWorkingSetBytes = [long]$Alternative.forcedFailure.globalProcess.initial.processes.workingSetBytes
        globalVisibleWindowCount = [int]$Alternative.forcedFailure.globalProcess.initial.visibleWindowCount
        globalPriorityClasses = @(
            $Alternative.forcedFailure.globalProcess.initial.processes.rootPriorityClasses
        )
        projectHostToGlobalLinkCount = [int]$Alternative.forcedFailure.ipc.projectHostToGlobalLinkCount
        logStreamCount = [int]$Alternative.forcedFailure.logs.projectHosts.streamCount
        windowsRemainingAfterHostCrash = [int]$Alternative.forcedFailure.projectHost.remainingWindowCount
        projectsReopenedAfterHostRestart = [int]$Alternative.forcedFailure.projectHost.explicitRestart.reopen.observedProjects
    }
}

function Select-ImplementationCostFacts {
    param(
        [Parameter(Mandatory = $true)] $AbAlternative,
        [Parameter(Mandatory = $true)] $BaAlternative
    )

    return [ordered]@{
        byExecutionOrder = [ordered]@{
            AB = Select-ImplementationCostSnapshot -Alternative $AbAlternative
            BA = Select-ImplementationCostSnapshot -Alternative $BaAlternative
        }
    }
}

$implementationCost = [ordered]@{
    method = 'observable_responsibilities_without_synthetic_score_or_effort_estimate'
    responsibilityModel = $protocolRelativePath
    independent = Select-ImplementationCostFacts `
        -AbAlternative $runAb.alternatives.independentHosts `
        -BaAlternative $runBa.alternatives.independentHosts
    multiwindow = Select-ImplementationCostFacts `
        -AbAlternative $runAb.alternatives.multiwindowHost `
        -BaAlternative $runBa.alternatives.multiwindowHost
}

$rawMeasurementsPreserved = (
    (Test-Path -LiteralPath $RunAbPath -PathType Leaf) -and
    (Test-Path -LiteralPath $RunBaPath -PathType Leaf) -and
    (Get-WorkspaceRelativePath -Path $RunAbPath).StartsWith(
        'docs/research/artifacts/',
        [System.StringComparison]::Ordinal
    ) -and
    (Get-WorkspaceRelativePath -Path $RunBaPath).StartsWith(
        'docs/research/artifacts/',
        [System.StringComparison]::Ordinal
    )
)
$implementationCostRecorded = $true
foreach ($cost in @($implementationCost.independent, $implementationCost.multiwindow)) {
    foreach ($order in @('AB', 'BA')) {
        $snapshot = $cost.byExecutionOrder.$order
        $implementationCostRecorded = (
            $implementationCostRecorded -and
            $null -ne $snapshot -and
            @($snapshot.projectHostPriorityClasses).Count -eq [int]$snapshot.hostProcessCount -and
            @($snapshot.globalPriorityClasses).Count -eq 1
        )
    }
}

$checks = @(
    [ordered]@{ name = 'criteria-frozen-before-final-execution'; passed = $criteriaFrozenBeforeFinalExecution },
    [ordered]@{ name = 'balanced-ab-ba-orders'; passed = $balancedOrdersPassed },
    [ordered]@{ name = 'same-clean-release-build'; passed = $sameCleanReleaseBuild },
    [ordered]@{ name = 'same-hardware-and-corpus'; passed = ($sameHardware -and $sameCorpus) },
    [ordered]@{ name = 'same-canvas-targets'; passed = $sameCanvasTargets },
    [ordered]@{ name = 'identical-exports-across-runs'; passed = $identicalExports },
    [ordered]@{ name = 'robustness-facts-recorded'; passed = $robustnessFactsRecorded },
    [ordered]@{ name = 'resilience-gates'; passed = $resilienceGatesPassed },
    [ordered]@{ name = 'pan-zoom-p95-at-most-33ms'; passed = $allP95WithinTarget },
    [ordered]@{ name = 'raw-measurements-preserved'; passed = $rawMeasurementsPreserved },
    [ordered]@{ name = 'implementation-cost-recorded'; passed = $implementationCostRecorded }
)
$ticketCriterionSatisfied = @($checks | Where-Object { -not $_.passed }).Count -eq 0

$artifact = [ordered]@{
    schemaVersion = 3
    suite = 'topology_final_comparison'
    collectedAtUtc = [DateTime]::UtcNow.ToString('o')
    gitCommit = $headCommit
    sourceInputsDirty = -not $sameCleanReleaseBuild
    protocol = [ordered]@{
        path = $protocolRelativePath
        sha256 = $protocolSha256
        frozenBeforeFinalExecution = $true
    }
    build = $runAb.build
    hardware = $runAb.hardware
    corpus = $runAb.corpus
    rawRuns = @(
        [ordered]@{
            executionOrder = 'AB'
            path = Get-WorkspaceRelativePath -Path $RunAbPath
            sha256 = (Get-FileHash -LiteralPath $RunAbPath -Algorithm SHA256).Hash.ToLowerInvariant()
            schemaVersion = $runAb.schemaVersion
            collectedAtUtc = $runAb.collectedAtUtc
        },
        [ordered]@{
            executionOrder = 'BA'
            path = Get-WorkspaceRelativePath -Path $RunBaPath
            sha256 = (Get-FileHash -LiteralPath $RunBaPath -Algorithm SHA256).Hash.ToLowerInvariant()
            schemaVersion = $runBa.schemaVersion
            collectedAtUtc = $runBa.collectedAtUtc
        }
    )
    metrics = $metrics
    robustness = $robustness
    implementationCost = $implementationCost
    checks = $checks
    completion = [ordered]@{
        criteriaFrozenBeforeFinalExecution = $criteriaFrozenBeforeFinalExecution
        balancedOrdersPassed = $balancedOrdersPassed
        hardwareRecorded = $sameHardware
        corpusRecorded = $sameCorpus
        rawMeasurementsRecorded = $rawMeasurementsPreserved
        failuresRecorded = $robustnessFactsRecorded
        robustnessFactsRecorded = $robustnessFactsRecorded
        implementationCostRecorded = $implementationCostRecorded
        ticketCriterionSatisfied = $ticketCriterionSatisfied
    }
    interpretation = [ordered]@{
        criterionClosed = $ticketCriterionSatisfied
        performanceRankingAllowed = $false
        topologyRecommendationDeferred = $true
        reason = if ($ticketCriterionSatisfied) {
            'The frozen AB/BA comparison protocol passed; the topology decision remains separate.'
        }
        else {
            'At least one frozen comparison gate failed; the recorded facts identify which one.'
        }
    }
}

$outputDirectory = Split-Path -Parent $OutputPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$json = $artifact | ConvertTo-Json -Depth 24
[System.IO.File]::WriteAllText(
    $OutputPath,
    $json + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)
Write-Output "Topology final comparison artifact: $OutputPath"
Write-Output $json

if (-not $ticketCriterionSatisfied) {
    throw 'The final topology comparison did not satisfy every frozen gate.'
}
