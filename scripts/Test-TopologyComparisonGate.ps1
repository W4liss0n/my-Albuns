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

function Assert-TopologyAlternativeContract {
    param(
        [Parameter(Mandatory = $true)] $Alternative,
        [Parameter(Mandatory = $true)][string] $Field,
        [Parameter(Mandatory = $true)][int] $ExpectedHostProcessCount,
        [Parameter(Mandatory = $true)][int] $ExpectedHostGlobalLinks,
        [Parameter(Mandatory = $true)][int] $ExpectedRemainingWindows,
        [Parameter(Mandatory = $true)][int] $ExpectedReopenedProjects
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
            [int]$Alternative.forcedFailure.logs.projectHosts.continuityFailureEvents -eq 0 -and
            [int]$Alternative.forcedFailure.logs.projectHosts.missingRequiredFields -eq 0
        ) `
        -Message "Final topology evidence contains incomplete or failed host logs: $Field"
    Assert-JsonNumber `
        -Value $Alternative.forcedFailure.projectHost.remainingWindowCount `
        -Field "$Field.forcedFailure.projectHost.remainingWindowCount"
    Assert-JsonNumber `
        -Value $Alternative.forcedFailure.projectHost.explicitRestart.reopen.observedProjects `
        -Field "$Field.forcedFailure.projectHost.explicitRestart.reopen.observedProjects" `
        -Minimum 1
    Assert-JsonBoolean `
        -Value $Alternative.forcedFailure.projectHost.noAutomaticRestartObserved `
        -Field "$Field.forcedFailure.projectHost.noAutomaticRestartObserved"
    Assert-Condition `
        -Condition $Alternative.forcedFailure.projectHost.noAutomaticRestartObserved `
        -Message "Final topology evidence observed an automatic host restart: $Field"
    Assert-Condition `
        -Condition (
            [int]$Alternative.processes.hostProcessCount -eq $ExpectedHostProcessCount -and
            [int]$Alternative.forcedFailure.ipc.projectHostToGlobalLinkCount -eq
                $ExpectedHostGlobalLinks -and
            [int]$Alternative.forcedFailure.projectHost.remainingWindowCount -eq
                $ExpectedRemainingWindows -and
            [int]$Alternative.forcedFailure.projectHost.explicitRestart.reopen.observedProjects -eq
                $ExpectedReopenedProjects
        ) `
        -Message "Final topology evidence changed the expected process topology: $Field"
}

function Assert-TopologyRunContract {
    param(
        [Parameter(Mandatory = $true)] $Report,
        [Parameter(Mandatory = $true)][ValidateSet('AB', 'BA')][string] $ExpectedOrder
    )

    Assert-Condition `
        -Condition ($Report.schemaVersion -eq 11) `
        -Message 'Final topology runs must use measurement schema 11.'
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
        -ExpectedHostGlobalLinks 2 `
        -ExpectedRemainingWindows 1 `
        -ExpectedReopenedProjects 1
    Assert-TopologyAlternativeContract `
        -Alternative $Report.alternatives.multiwindowHost `
        -Field "$ExpectedOrder.alternatives.multiwindowHost" `
        -ExpectedHostProcessCount 1 `
        -ExpectedHostGlobalLinks 1 `
        -ExpectedRemainingWindows 0 `
        -ExpectedReopenedProjects 2
    Assert-JsonBoolean -Value $Report.failureGate.passed -Field "$ExpectedOrder.failureGate.passed"
    Assert-JsonBoolean `
        -Value $Report.failureGate.imagingProcessor.validated `
        -Field "$ExpectedOrder.failureGate.imagingProcessor.validated"
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
    Assert-Condition `
        -Condition (
            $Report.failureGate.passed -and
            $Report.failureGate.imagingProcessor.validated -and
            -not $Report.failureGate.imagingProcessor.sourceInputsDirty -and
            $Report.failureGate.imagingProcessor.sameGitCommitAsTopologyBuild -and
            $Report.failureGate.imagingProcessor.cacheRecoveredAfterOneExplicitRestart -and
            $Report.failureGate.imagingProcessor.exportFailedSafelyUntilExplicitRetry
        ) `
        -Message "Final topology report failed its resilience gate: $ExpectedOrder"
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
$resilienceGatesPassed = $runAb.failureGate.passed -and $runBa.failureGate.passed
Assert-Condition `
    -Condition $resilienceGatesPassed `
    -Message 'At least one final topology run failed its resilience gate.'

$runPairs = @(
    [ordered]@{ order = 'AB'; report = $runAb },
    [ordered]@{ order = 'BA'; report = $runBa }
)
$allP95WithinTarget = $true
$sameCanvasTargets = $true
$exportHashes = [System.Collections.Generic.List[string]]::new()
$frameTargetSignatures = [System.Collections.Generic.List[string]]::new()
$navigationTargetSignatures = [System.Collections.Generic.List[string]]::new()
foreach ($runPair in $runPairs) {
    foreach ($alternativeName in @('independentHosts', 'multiwindowHost')) {
        $alternative = $runPair.report.alternatives.$alternativeName
        Assert-Condition `
            -Condition (
                $alternative.ready.windows.Count -eq 2 -and
                $alternative.cache.projectCount -eq 2 -and
                $alternative.interaction.canvas.projects.Count -eq 2
            ) `
            -Message "The $($runPair.order) $alternativeName run lost a Project or Window."
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
    Assert-Condition `
        -Condition (
            $independent.interaction.export.outputSha256 -ceq
                $multiwindow.interaction.export.outputSha256 -and
            $independent.interaction.export.widthPx -eq
                $multiwindow.interaction.export.widthPx -and
            $independent.interaction.export.heightPx -eq
                $multiwindow.interaction.export.heightPx
        ) `
        -Message "The $($runPair.order) run produced different exports for A and B."
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
    Assert-Condition `
        -Condition $targetsMatch `
        -Message "The $($runPair.order) run compared different Canvas targets."
}
$sameTargetsAcrossOrders = (
    @($frameTargetSignatures | Select-Object -Unique).Count -eq 1 -and
    @($navigationTargetSignatures | Select-Object -Unique).Count -eq 1
)
$sameCanvasTargets = $sameCanvasTargets -and $sameTargetsAcrossOrders
Assert-Condition `
    -Condition $sameTargetsAcrossOrders `
    -Message 'The AB and BA runs compared different Canvas or navigation targets.'
$identicalExports = @($exportHashes | Select-Object -Unique).Count -eq 1
Assert-Condition `
    -Condition $identicalExports `
    -Message 'The final AB and BA runs did not produce one deterministic Export.'

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
    zoomWorstProjectP95 = New-FinalMetric -Unit 'ms' -Better lower -Selector {
        param($alternative) $alternative.interaction.canvas.aggregate.zoom.worstProjectP95FrameMs
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

function Select-ImplementationCostFacts {
    param(
        [Parameter(Mandatory = $true)] $AbAlternative,
        [Parameter(Mandatory = $true)] $BaAlternative
    )

    return [ordered]@{
        hostProcessCount = @(
            [int]$AbAlternative.processes.hostProcessCount,
            [int]$BaAlternative.processes.hostProcessCount
        )
        processTreeCount = @(
            [int]$AbAlternative.processes.processTreeCount,
            [int]$BaAlternative.processes.processTreeCount
        )
        projectHostToGlobalLinkCount = @(
            [int]$AbAlternative.forcedFailure.ipc.projectHostToGlobalLinkCount,
            [int]$BaAlternative.forcedFailure.ipc.projectHostToGlobalLinkCount
        )
        logStreamCount = @(
            [int]$AbAlternative.forcedFailure.logs.projectHosts.streamCount,
            [int]$BaAlternative.forcedFailure.logs.projectHosts.streamCount
        )
        windowsRemainingAfterHostCrash = @(
            [int]$AbAlternative.forcedFailure.projectHost.remainingWindowCount,
            [int]$BaAlternative.forcedFailure.projectHost.remainingWindowCount
        )
        projectsReopenedAfterHostRestart = @(
            [int]$AbAlternative.forcedFailure.projectHost.explicitRestart.reopen.observedProjects,
            [int]$BaAlternative.forcedFailure.projectHost.explicitRestart.reopen.observedProjects
        )
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
    foreach ($field in @(
        'hostProcessCount',
        'processTreeCount',
        'projectHostToGlobalLinkCount',
        'logStreamCount',
        'windowsRemainingAfterHostCrash',
        'projectsReopenedAfterHostRestart'
    )) {
        $implementationCostRecorded = (
            $implementationCostRecorded -and
            @($cost.$field).Count -eq 2
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
    [ordered]@{ name = 'resilience-gates'; passed = $resilienceGatesPassed },
    [ordered]@{ name = 'pan-zoom-p95-at-most-33ms'; passed = $allP95WithinTarget },
    [ordered]@{ name = 'raw-measurements-preserved'; passed = $rawMeasurementsPreserved },
    [ordered]@{ name = 'implementation-cost-recorded'; passed = $implementationCostRecorded }
)
$ticketCriterionSatisfied = @($checks | Where-Object { -not $_.passed }).Count -eq 0

$artifact = [ordered]@{
    schemaVersion = 1
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
    implementationCost = $implementationCost
    checks = $checks
    completion = [ordered]@{
        criteriaFrozenBeforeFinalExecution = $criteriaFrozenBeforeFinalExecution
        balancedOrdersPassed = $balancedOrdersPassed
        hardwareRecorded = $sameHardware
        corpusRecorded = $sameCorpus
        rawMeasurementsRecorded = $rawMeasurementsPreserved
        failuresRecorded = $resilienceGatesPassed
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
            'At least one topology missed the frozen Pan/Zoom responsiveness target.'
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
