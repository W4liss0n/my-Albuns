param(
    [string] $OutputPath,
    [string] $ArtifactDirectory
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
Initialize-MyAlbunsToolchain

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'The productive journey gate must run on Windows.'
}

$workspaceRoot = $script:WorkspaceRoot
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $workspaceRoot `
        'docs\research\artifacts\0023-productive-journey.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $workspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

$runnerMutex = [System.Threading.Mutex]::new(
    $false,
    'Local\MyAlbuns.ProductiveJourneyGate.v1'
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
    throw 'Another productive journey gate is already running.'
}

$scratchRoot = Join-Path $workspaceRoot '.scratch'
New-Item -ItemType Directory -Force -Path $scratchRoot | Out-Null
$artifactRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $scratchRoot 'productive-journey-evidence')
)
$retainedArtifactDirectory = $null
if (-not [string]::IsNullOrWhiteSpace($ArtifactDirectory)) {
    if (-not [System.IO.Path]::IsPathRooted($ArtifactDirectory)) {
        $ArtifactDirectory = Join-Path $workspaceRoot $ArtifactDirectory
    }
    $retainedArtifactDirectory = [System.IO.Path]::GetFullPath(
        $ArtifactDirectory
    )
    if (-not [string]::Equals(
            [System.IO.Path]::GetDirectoryName($retainedArtifactDirectory),
            $artifactRoot,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'Retained productive-journey evidence must be one direct child of .scratch/productive-journey-evidence.'
    }
    if (Test-Path -LiteralPath $retainedArtifactDirectory) {
        throw "The retained productive-journey evidence directory already exists: $retainedArtifactDirectory"
    }
}
$runRoot = Join-Path `
    $scratchRoot `
    "j8-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 6))"
$runRoot = [System.IO.Path]::GetFullPath($runRoot)
if (-not [string]::Equals(
        [System.IO.Path]::GetDirectoryName($runRoot),
        [System.IO.Path]::GetFullPath($scratchRoot),
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The productive journey scratch root escaped the workspace.'
}

$sourceBefore = Get-GateSourceSnapshot `
    -WorkspaceRoot $workspaceRoot `
    -EvidencePath $OutputPath
$runRootCleaned = $false
$gate = $null
$driver = $null
$sampleCount = 0
$nonWhiteCount = 0
$retainedArtifactEvidence = $null
$frontendStructureTestLog = Join-Path $runRoot 'frontend-structure-tests.log'
$publicProjectCoreTestLog = Join-Path $runRoot 'public-project-core-journey.log'

function Get-ExactExecutableProcesses([string] $ExecutablePath) {
    $expected = [System.IO.Path]::GetFullPath($ExecutablePath)
    return @(
        Get-CimInstance Win32_Process |
            Where-Object {
                -not [string]::IsNullOrWhiteSpace($_.ExecutablePath) -and
                [string]::Equals(
                    [System.IO.Path]::GetFullPath($_.ExecutablePath),
                    $expected,
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            }
    )
}

function Get-Sha256([string] $Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        return [System.BitConverter]::ToString(
            $algorithm.ComputeHash($stream)
        ).Replace('-', '')
    }
    finally {
        $algorithm.Dispose()
        $stream.Dispose()
    }
}

function Get-ContainedRelativePath(
    [string] $ParentPath,
    [string] $ChildPath
) {
    $parent = [System.IO.Path]::GetFullPath($ParentPath)
    $child = [System.IO.Path]::GetFullPath($ChildPath)
    $parentPrefix = $parent.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $child.StartsWith(
            $parentPrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw "A relative evidence path escaped its parent: $child"
    }
    return $child.Substring($parentPrefix.Length)
}

function Copy-RetainedArtifact(
    [string] $SourcePath,
    [string] $RelativePath
) {
    $source = [System.IO.Path]::GetFullPath($SourcePath)
    $runPrefix = $runRoot.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $source.StartsWith(
            $runPrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw "A retained artifact escaped the productive-journey run root: $source"
    }
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "A retained productive-journey artifact is missing: $source"
    }

    $destination = [System.IO.Path]::GetFullPath(
        (Join-Path $retainedArtifactDirectory $RelativePath)
    )
    $artifactPrefix = $retainedArtifactDirectory.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $destination.StartsWith(
            $artifactPrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw "A retained artifact destination escaped its evidence directory: $destination"
    }
    New-Item -ItemType Directory -Force -Path (
        [System.IO.Path]::GetDirectoryName($destination)
    ) | Out-Null
    Copy-Item -LiteralPath $source -Destination $destination

    return [ordered]@{
        path = $RelativePath.Replace('\', '/')
        byteCount = [int64] (Get-Item -LiteralPath $destination).Length
        sha256 = (Get-Sha256 $destination).ToLowerInvariant()
    }
}

try {
    New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
    $node = (Get-Command node.exe -ErrorAction Stop).Source
    $npm = (Get-Command npm.cmd -ErrorAction Stop).Source
    $frontendStructureTestFiles = @(
        (Join-Path $workspaceRoot 'src\components\ProjectWorkspace.test.tsx'),
        (Join-Path $workspaceRoot 'src\components\SheetContextMenu.test.tsx'),
        (Join-Path $workspaceRoot 'src\components\useProjectCommandShortcuts.test.tsx'),
        (Join-Path $workspaceRoot 'src\components\sheetReorderSession.test.ts'),
        (Join-Path $workspaceRoot 'src\components\SheetBarReorderOverlay.test.tsx'),
        (Join-Path $workspaceRoot 'src\components\InspectorPanelStructure.test.tsx')
    )
    $priorErrorActionPreference = $ErrorActionPreference
    try {
        # Windows PowerShell surfaces redirected native stderr as non-terminating
        # ErrorRecord values. Preserve those values in the log without letting
        # npm's informational stderr bypass the native exit-code contract.
        $ErrorActionPreference = 'Continue'
        $frontendStructureTestOutput = @(
            & $npm test -- @frontendStructureTestFiles --reporter=verbose 2>&1
        )
        $frontendStructureTestExitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $priorErrorActionPreference
    }
    $frontendStructureTestText = @(
        $frontendStructureTestOutput | ForEach-Object { $_.ToString() }
    ) -join [System.Environment]::NewLine
    [System.IO.File]::WriteAllText(
        $frontendStructureTestLog,
        $frontendStructureTestText + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output $frontendStructureTestText
    if ($frontendStructureTestExitCode -ne 0) {
        throw "The public workspace structure tests failed with exit code $frontendStructureTestExitCode."
    }
    & $node --test `
        (Join-Path $PSScriptRoot 'Test-ProductiveJourneyObservations.mjs') `
        (Join-Path $PSScriptRoot 'Test-GateWebDriver.mjs')
    if ($LASTEXITCODE -ne 0) {
        throw "The productive journey observation tests failed with exit code $LASTEXITCODE."
    }
    $priorErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $publicProjectCoreTestOutput = @(
            & $script:CargoExecutable `
                test `
                -p myalbuns-core `
                --test productive_project_core_journey `
                -- `
                --exact `
                2>&1
        )
        $publicProjectCoreTestExitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $priorErrorActionPreference
    }
    $publicProjectCoreTestText = @(
        $publicProjectCoreTestOutput | ForEach-Object { $_.ToString() }
    ) -join [System.Environment]::NewLine
    [System.IO.File]::WriteAllText(
        $publicProjectCoreTestLog,
        $publicProjectCoreTestText + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output $publicProjectCoreTestText
    if ($publicProjectCoreTestExitCode -ne 0) {
        throw "The public ProjectCore journey failed with exit code $publicProjectCoreTestExitCode."
    }

    & (Join-Path $PSScriptRoot 'Prepare-Sidecar.ps1') -Profile debug
    if ($LASTEXITCODE -ne 0) {
        throw "The debug Processor build failed with exit code $LASTEXITCODE."
    }
    $tauri = Join-Path $workspaceRoot 'node_modules\.bin\tauri.cmd'
    & $tauri build --debug --no-bundle
    if ($LASTEXITCODE -ne 0) {
        throw "The debug Tauri build failed with exit code $LASTEXITCODE."
    }

    $applicationPath = Join-Path $workspaceRoot 'target\debug\myalbuns-desktop.exe'
    $preparedSidecarPath = Join-Path `
        $workspaceRoot `
        'src-tauri\binaries\myalbuns-imaging-x86_64-pc-windows-msvc.exe'
    $runtimeSidecarPath = Join-Path $workspaceRoot 'target\debug\myalbuns-imaging.exe'
    if (-not (Test-Path -LiteralPath $runtimeSidecarPath -PathType Leaf) -or
        (Get-Sha256 $preparedSidecarPath) -ne
            (Get-Sha256 $runtimeSidecarPath)) {
        throw 'The debug Tauri runtime does not own the prepared debug Processor.'
    }
    $driver = & (Join-Path $PSScriptRoot 'Resolve-TauriWebDriver.ps1') |
        Select-Object -Last 1 |
        ConvertFrom-Json
    if ((Get-ExactExecutableProcesses $applicationPath).Count -ne 0) {
        throw 'A productive desktop process already exists for this worktree.'
    }
    if ((Get-ExactExecutableProcesses $driver.nativeDriverPath).Count -ne 0) {
        throw 'A native WebDriver process already exists for this worktree.'
    }

    $gateOutput = & $node `
        (Join-Path $PSScriptRoot 'Run-ProductiveJourneyGate.mjs') `
        $workspaceRoot `
        $runRoot `
        $applicationPath `
        $driver.nativeDriverPath
    if ($LASTEXITCODE -ne 0) {
        throw "The productive journey failed with exit code $LASTEXITCODE."
    }
    $gate = $gateOutput | Select-Object -Last 1 | ConvertFrom-Json
    $expectedRecoveryChoices = @(
        'Reabrir e recuperar',
        "Abrir $([char]0x00FA)ltima vers$([char]0x00E3)o salva",
        "Agora n$([char]0x00E3)o"
    )
    $contractViolations = @()
    if (
        -not $gate.cancelledCreationBeforeCore -or
        -not $gate.cancelledExportBeforePipeline -or
        $gate.createAuthorization -ne 'createOnly' -or
        $gate.exportedSheetNumber -ne 2 -or
        $gate.selectedSheetId -notmatch '^[0-9a-f-]{36}$' -or
        $gate.selectedSheetActiveSides -ne 'both' -or
        $gate.firstSheetDimensions.width -ne 720 -or
        $gate.firstSheetDimensions.height -ne 360 -or
        $gate.selectedSheetDimensions.width -ne 1440 -or
        $gate.selectedSheetDimensions.height -ne 360 -or
        $gate.expectedBackgroundRgb -ne '#204060' -or
        $gate.exportedDpi -ne 360 -or
        $gate.schemaVersion -ne 3 -or
        $gate.savedRevision -ne 3 -or
        $gate.savedDpi -ne 300 -or
        $gate.photoFrameCount -ne 1 -or
        -not $gate.persistedPhotoLinkOnly -or
        -not $gate.reimportedExistingPhotoWithoutRevision
    ) {
        $contractViolations += 'base'
    }
    if (
        $gate.sessionRecovery.schemaVersion -ne 1 -or
        $gate.sessionRecovery.baseSavedRevision -ne 3 -or
        $gate.sessionRecovery.creativeRevision -ne 4 -or
        $gate.sessionRecovery.recoveredDpi -ne 360 -or
        $gate.sessionRecovery.promptChoices.Count -ne 3 -or
        $gate.sessionRecovery.promptChoices[0] -cne $expectedRecoveryChoices[0] -or
        $gate.sessionRecovery.promptChoices[1] -cne $expectedRecoveryChoices[1] -or
        $gate.sessionRecovery.promptChoices[2] -cne $expectedRecoveryChoices[2] -or
        -not $gate.sessionRecovery.opaqueProjectKey -or
        -not $gate.sessionRecovery.completedActionCheckpointed -or
        -not $gate.sessionRecovery.midGesturePreservedPreviousCheckpoint -or
        -not $gate.sessionRecovery.projectFileUnchangedThroughRecovery -or
        -not $gate.sessionRecovery.checkpointPreservedAfterRecovery -or
        -not $gate.sessionRecovery.recoveredUnsaved -or
        -not $gate.sessionRecovery.recoveredHistoryEmpty -or
        -not $gate.sessionRecovery.postRecoveryActionsCheckpointed -or
        -not $gate.sessionRecovery.checkpointPreservedByCancelledSaveAs -or
        -not $gate.sessionRecovery.checkpointFinishedBySuccessfulSaveAs -or
        -not $gate.sessionRecovery.lockReleasedToDistinctHost
    ) {
        $contractViolations += 'sessionRecovery'
    }
    if (
        -not $gate.saveAs.cancelledBeforeCore -or
        $gate.saveAs.createAuthorization -ne 'createOnly' -or
        $gate.saveAs.originalProjectId -notmatch '^[0-9a-f-]{36}$' -or
        $gate.saveAs.copiedProjectId -notmatch '^[0-9a-f-]{36}$' -or
        $gate.saveAs.originalProjectId -eq $gate.saveAs.copiedProjectId -or
        $gate.saveAs.savedAsRevision -ne 6 -or
        -not $gate.saveAs.contentPreserved -or
        -not $gate.saveAs.originalByteIdentical -or
        -not $gate.saveAs.historyPreserved -or
        -not $gate.saveAs.originalHistoryEmpty -or
        -not $gate.saveAs.simultaneouslyOpen -or
        -not $gate.saveAs.isolatedIndependentSaves -or
        $gate.saveAs.originalSavedRevision -ne 4 -or
        $gate.saveAs.originalSavedDpi -ne 320 -or
        $gate.saveAs.copySavedRevision -ne 7 -or
        $gate.saveAs.copySavedDpi -ne 420 -or
        -not $gate.saveAs.previousRecoveryFinished -or
        -not $gate.saveAs.cacheStagedEmpty -or
        -not $gate.saveAs.localAuthorityTransitioned -or
        -not $gate.saveAs.webviewNamespaceTransitioned -or
        -not $gate.saveAs.replacementWebviewReady -or
        -not $gate.saveAs.globalInspectorPreferencePreserved -or
        -not $gate.saveAs.projectLocalSelectionReset -or
        -not $gate.saveAs.nativeTitleUpdated
    ) {
        $contractViolations += 'saveAs'
    }
    $physicalAlbumStructure = $gate.physicalAlbumStructure
    $physicalBeforeOrder = @($physicalAlbumStructure.before.order)
    $physicalAfterAddOrder = @($physicalAlbumStructure.afterAdd.order)
    $physicalAfterReorderOrder = @($physicalAlbumStructure.afterReorder.order)
    $physicalAfterDeleteOrder = @($physicalAlbumStructure.afterDelete.order)
    $physicalProjectCoreEvents = @($physicalAlbumStructure.projectCoreEvents)
    if (
        $physicalAlbumStructure.reorderSurface -cne 'grid' -or
        $physicalAlbumStructure.dragTransport -cne 'w3c-pointer-actions' -or
        $physicalAlbumStructure.addedSheetId -notmatch '^[0-9a-f-]{36}$' -or
        $physicalAlbumStructure.before.count -ne 3 -or
        $physicalBeforeOrder.Count -ne 3 -or
        $physicalAlbumStructure.before.focusedSheetId -cne $physicalBeforeOrder[1] -or
        $physicalAlbumStructure.afterAdd.count -ne 4 -or
        $physicalAfterAddOrder.Count -ne 4 -or
        $physicalAlbumStructure.afterAdd.focusedSheetId -cne $physicalAlbumStructure.addedSheetId -or
        $physicalAfterAddOrder[0] -cne $physicalBeforeOrder[0] -or
        $physicalAfterAddOrder[1] -cne $physicalBeforeOrder[1] -or
        $physicalAfterAddOrder[2] -cne $physicalAlbumStructure.addedSheetId -or
        $physicalAfterAddOrder[3] -cne $physicalBeforeOrder[2] -or
        $physicalAlbumStructure.afterReorder.count -ne 4 -or
        $physicalAfterReorderOrder.Count -ne 4 -or
        $physicalAlbumStructure.afterReorder.focusedSheetId -cne $physicalAlbumStructure.addedSheetId -or
        $physicalAfterReorderOrder[0] -cne $physicalBeforeOrder[0] -or
        $physicalAfterReorderOrder[1] -cne $physicalAlbumStructure.addedSheetId -or
        $physicalAfterReorderOrder[2] -cne $physicalBeforeOrder[1] -or
        $physicalAfterReorderOrder[3] -cne $physicalBeforeOrder[2] -or
        $physicalAlbumStructure.afterDelete.count -ne 3 -or
        $physicalAfterDeleteOrder.Count -ne 3 -or
        $physicalAlbumStructure.afterDelete.focusedSheetId -cne $physicalAlbumStructure.before.focusedSheetId -or
        ($physicalAfterDeleteOrder -join ',') -cne ($physicalBeforeOrder -join ',') -or
        -not $physicalAlbumStructure.restoredOriginalOrder -or
        $physicalProjectCoreEvents.Count -ne 3 -or
        $physicalProjectCoreEvents[0].event -cne 'project_intent_applied' -or
        $physicalProjectCoreEvents[0].intent -cne 'add_sheet' -or
        $physicalProjectCoreEvents[1].event -cne 'project_intent_applied' -or
        $physicalProjectCoreEvents[1].intent -cne 'reorder_sheet' -or
        $physicalProjectCoreEvents[2].event -cne 'project_intent_applied' -or
        $physicalProjectCoreEvents[2].intent -cne 'delete_sheet' -or
        $physicalProjectCoreEvents[0].processId -ne $gate.processIds.host -or
        $physicalProjectCoreEvents[1].processId -ne $gate.processIds.host -or
        $physicalProjectCoreEvents[2].processId -ne $gate.processIds.host -or
        [int64] $physicalProjectCoreEvents[1].revision -ne ([int64] $physicalProjectCoreEvents[0].revision + 1) -or
        [int64] $physicalProjectCoreEvents[2].revision -ne ([int64] $physicalProjectCoreEvents[1].revision + 1)
    ) {
        $contractViolations += 'physicalAlbumStructure'
    }
    if (
        -not $gate.originalUnchanged -or
        -not $gate.missingOriginalBlocked -or
        -not $gate.missingOriginalActionable -or
        -not $gate.residentCanvasPreviewBeforeMissingOriginal -or
        $gate.previewArtifactCountBeforePurge -le 0 -or
        $gate.cacheEntryCountBeforeExport -ne 0 -or
        $gate.cacheByteCountBeforeExport -ne 0 -or
        $gate.cacheEntryCountAfterExport -ne 0 -or
        $gate.cacheByteCountAfterExport -ne 0 -or
        -not $gate.cacheCouldNotProduceFalseSuccess -or
        $gate.jpeg.width -ne 1440 -or
        $gate.jpeg.height -ne 360 -or
        $gate.jpeg.byteCount -le 0 -or
        $gate.jpeg.sha256 -notmatch '^[0-9a-f]{64}$' -or
        -not $gate.exportedAfterReopen -or
        $gate.canvasPhotoSample.cssWidth -le 0 -or
        $gate.canvasPhotoSample.cssHeight -le 0 -or
        $gate.sourcePathExposedToWebView
    ) {
        $contractViolations += 'output'
    }
    if (
        $gate.correlations.bootstraps -ne 4 -or
        $gate.correlations.imagingAttempts -ne 2 -or
        $gate.processIds.firstHost -eq $gate.processIds.host -or
        -not $gate.reopenedInIndependentHost -or
        -not $gate.reopenedHistoryEmpty -or
        $gate.terminalCounts.globalHandoffs -ne 4 -or
        $gate.terminalCounts.hostReady -ne 4 -or
        $gate.terminalCounts.imagingStopped -ne 2
    ) {
        $contractViolations += 'processes'
    }
    if ($contractViolations.Count -ne 0) {
        $observed = $gate | ConvertTo-Json -Depth 8 -Compress
        $violationSummary = $contractViolations -join ', '
        throw "The productive journey result violated contract groups ($violationSummary): $observed"
    }
    Add-Type -AssemblyName System.Drawing
    $jpegPath = Join-Path $runRoot 'Jornada produtiva_002.jpg'
    $jpegBitmap = [System.Drawing.Bitmap]::FromFile($jpegPath)
    try {
        $sampleX = 2
        $sampleY = [Math]::Floor($jpegBitmap.Height / 2)
        $backgroundSample = $jpegBitmap.GetPixel($sampleX, $sampleY)
        # Match the Canvas sample while staying clear of the editor-only
        # center spine, which is intentionally absent from the exported JPEG.
        $photoExportSampleX = [Math]::Floor($jpegBitmap.Width * 0.45)
        $photoExportSampleY = [Math]::Floor($jpegBitmap.Height / 2)
        $photoExportSample = $jpegBitmap.GetPixel(
            $photoExportSampleX,
            $photoExportSampleY
        )
    }
    finally {
        $jpegBitmap.Dispose()
    }
    $backgroundChannelDeltas = @(
        [Math]::Abs([int] $backgroundSample.R - 0x20),
        [Math]::Abs([int] $backgroundSample.G - 0x40),
        [Math]::Abs([int] $backgroundSample.B - 0x60)
    )
    $backgroundMaxChannelDelta = [int] (
        $backgroundChannelDeltas | Measure-Object -Maximum
    ).Maximum
    if ($backgroundMaxChannelDelta -gt 8) {
        throw "The JPEG did not preserve the saved Background personalization: $($backgroundSample.R),$($backgroundSample.G),$($backgroundSample.B)."
    }
    $photoPath = Join-Path $runRoot 'Foto da jornada.jpg'
    $photoBitmap = [System.Drawing.Bitmap]::FromFile($photoPath)
    try {
        $photoOriginalSample = $photoBitmap.GetPixel(
            [Math]::Floor($photoBitmap.Width / 2),
            [Math]::Floor($photoBitmap.Height / 2)
        )
    }
    finally {
        $photoBitmap.Dispose()
    }
    $originalJpegMaxChannelDelta = [int] (@(
        [Math]::Abs([int] $photoOriginalSample.R - [int] $photoExportSample.R),
        [Math]::Abs([int] $photoOriginalSample.G - [int] $photoExportSample.G),
        [Math]::Abs([int] $photoOriginalSample.B - [int] $photoExportSample.B)
    ) | Measure-Object -Maximum).Maximum
    $photoTolerance = 32
    if ($originalJpegMaxChannelDelta -gt $photoTolerance) {
        throw "The JPEG Photo sample exceeded the explicit $photoTolerance-channel tolerance from the Original."
    }
    $jpegEvidence = [ordered]@{
        width = [int] $gate.jpeg.width
        height = [int] $gate.jpeg.height
        byteCount = [int64] $gate.jpeg.byteCount
        sha256 = $gate.jpeg.sha256
        backgroundSample = [ordered]@{
            x = [int] $sampleX
            y = [int] $sampleY
            red = [int] $backgroundSample.R
            green = [int] $backgroundSample.G
            blue = [int] $backgroundSample.B
            maxChannelDelta = $backgroundMaxChannelDelta
        }
        photoSample = [ordered]@{
            x = [int] $photoExportSampleX
            y = [int] $photoExportSampleY
            red = [int] $photoExportSample.R
            green = [int] $photoExportSample.G
            blue = [int] $photoExportSample.B
            originalMaxChannelDelta = $originalJpegMaxChannelDelta
            tolerance = $photoTolerance
        }
    }
    $processIds = @(
        [int] $gate.processIds.global,
        [int] $gate.processIds.host,
        [int] $gate.processIds.imaging
    )
    if (@($processIds | Sort-Object -Unique).Count -ne 3) {
        throw 'Global, Host and Processor were not three distinct processes.'
    }

    $bitmap = [System.Drawing.Bitmap]::FromFile($gate.screenshotPath)
    try {
        $canvasPhotoX = [Math]::Min(
            $bitmap.Width - 1,
            [Math]::Max(
                0,
                [Math]::Round(
                    [double] $gate.canvasPhotoSample.x /
                    [double] $gate.canvasPhotoSample.cssWidth *
                    $bitmap.Width
                )
            )
        )
        $canvasPhotoY = [Math]::Min(
            $bitmap.Height - 1,
            [Math]::Max(
                0,
                [Math]::Round(
                    [double] $gate.canvasPhotoSample.y /
                    [double] $gate.canvasPhotoSample.cssHeight *
                    $bitmap.Height
                )
            )
        )
        $canvasPhotoSample = $bitmap.GetPixel($canvasPhotoX, $canvasPhotoY)
        $stepX = [Math]::Max(1, [Math]::Floor($bitmap.Width / 40))
        $stepY = [Math]::Max(1, [Math]::Floor($bitmap.Height / 30))
        for ($x = 0; $x -lt $bitmap.Width; $x += $stepX) {
            for ($y = 0; $y -lt $bitmap.Height; $y += $stepY) {
                $pixel = $bitmap.GetPixel($x, $y)
                $sampleCount++
                if ($pixel.R -lt 245 -or $pixel.G -lt 245 -or $pixel.B -lt 245) {
                    $nonWhiteCount++
                }
            }
        }
    }
    finally {
        $bitmap.Dispose()
    }
    if ($nonWhiteCount -lt 10) {
        throw "The productive Canvas screenshot is blank ($nonWhiteCount/$sampleCount)."
    }
    $canvasJpegMaxChannelDelta = [int] (@(
        [Math]::Abs([int] $canvasPhotoSample.R - [int] $photoExportSample.R),
        [Math]::Abs([int] $canvasPhotoSample.G - [int] $photoExportSample.G),
        [Math]::Abs([int] $canvasPhotoSample.B - [int] $photoExportSample.B)
    ) | Measure-Object -Maximum).Maximum
    if ($canvasJpegMaxChannelDelta -gt $photoTolerance) {
        throw "Canvas and JPEG exceeded the explicit $photoTolerance-channel Photo tolerance."
    }

    if ((Get-ExactExecutableProcesses $applicationPath).Count -ne 0) {
        throw 'The productive journey left a desktop process alive.'
    }
    if ((Get-ExactExecutableProcesses $driver.nativeDriverPath).Count -ne 0) {
        throw 'The productive journey left a native WebDriver alive.'
    }
    Wait-GatePathProcessesExit -Path $runRoot

    if ($null -ne $retainedArtifactDirectory) {
        New-Item -ItemType Directory -Force -Path $artifactRoot | Out-Null
        New-Item -ItemType Directory -Path $retainedArtifactDirectory |
            Out-Null
        $retainedFiles = @(
            Copy-RetainedArtifact `
                -SourcePath $gate.screenshotPath `
                -RelativePath 'project-canvas.png'
            Copy-RetainedArtifact `
                -SourcePath $jpegPath `
                -RelativePath 'Jornada produtiva_002.jpg'
            Copy-RetainedArtifact `
                -SourcePath $photoPath `
                -RelativePath 'Foto da jornada.jpg'
            Copy-RetainedArtifact `
                -SourcePath $frontendStructureTestLog `
                -RelativePath 'frontend-structure-tests.log'
            Copy-RetainedArtifact `
                -SourcePath $publicProjectCoreTestLog `
                -RelativePath 'public-project-core-journey.log'
        )
        $processDataRoot = Join-Path $runRoot 'process-data'
        if (-not (Test-Path -LiteralPath $processDataRoot -PathType Container)) {
            throw 'The productive journey did not retain its correlated process-data directory.'
        }
        $processLogs = @(
            Get-ChildItem `
                -LiteralPath $processDataRoot `
                -Recurse `
                -File `
                -Filter '*.jsonl'
        )
        if ($processLogs.Count -eq 0) {
            throw 'The productive journey did not emit correlated process logs.'
        }
        foreach ($processLog in $processLogs) {
            $relativeLogPath = Get-ContainedRelativePath `
                -ParentPath $runRoot `
                -ChildPath $processLog.FullName
            $retainedFiles += Copy-RetainedArtifact `
                -SourcePath $processLog.FullName `
                -RelativePath $relativeLogPath
        }
        foreach ($projectFile in @(
                Get-ChildItem -LiteralPath $runRoot -Recurse -File |
                    Where-Object {
                        $_.Extension -in @('.myalbum', '.myalbuns')
                    }
            )) {
            $relativeProjectPath = Get-ContainedRelativePath `
                -ParentPath $runRoot `
                -ChildPath $projectFile.FullName
            $retainedFiles += Copy-RetainedArtifact `
                -SourcePath $projectFile.FullName `
                -RelativePath $relativeProjectPath
        }
        $artifactManifest = [ordered]@{
            schemaVersion = 1
            gate = 'productive-end-to-end-journey'
            gitCommit = $sourceBefore.gitCommit
            collectedAtUtc = [DateTime]::UtcNow.ToString('o')
            files = $retainedFiles
        }
        $artifactManifestPath = Join-Path `
            $retainedArtifactDirectory `
            'artifact-manifest.json'
        [System.IO.File]::WriteAllText(
            $artifactManifestPath,
            ($artifactManifest | ConvertTo-Json -Depth 6) +
                [System.Environment]::NewLine,
            [System.Text.UTF8Encoding]::new($false)
        )
        $retainedArtifactEvidence = [ordered]@{
            directory = (Get-ContainedRelativePath `
                    -ParentPath $workspaceRoot `
                    -ChildPath $retainedArtifactDirectory
                ).Replace('\', '/')
            manifest = 'artifact-manifest.json'
            manifestSha256 = (Get-Sha256 $artifactManifestPath).ToLowerInvariant()
            fileCount = $retainedFiles.Count
        }
    }

    Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchRoot
    $runRootCleaned = $true
    $sourceAfter = Get-GateSourceSnapshot `
        -WorkspaceRoot $workspaceRoot `
        -EvidencePath $OutputPath
    $sourceInputsDirty = Test-GateSourceSnapshotsDirty `
        -Before $sourceBefore `
        -After $sourceAfter

    $windowsWebView2CheckNames = @(
        'cancel-before-project-core',
        'create-only-causal-handoff',
        'project-core-save-history',
        'crash-recovery-distinct-host',
        'interrupted-gesture-keeps-previous-checkpoint',
        'recovered-project-unsaved-empty-history',
        'native-save-as-cancel-before-core',
        'save-as-copy-content-and-history',
        'save-as-webview-cache-recovery-transition',
        'simultaneous-original-copy-isolated-saves',
        'native-jpeg-import-reselect-external-link-only',
        'double-click-frame-selection-canvas',
        'cancel-before-export-pipeline',
        'distinguishable-sheet-two-jpeg-export',
        'canvas-jpeg-photo-fidelity',
        'real-application-empty-cache-original-read',
        'missing-original-actionable-failure',
        'saved-project-unchanged-by-export',
        'physical-album-structure-ui-project-core',
        'independent-host-reopen-empty-history',
        'correlated-process-terminals-cleanup'
    )
    $frontendWorkspaceCheckNames = @(
        'queued-history-structural-authority',
        'pending-structure-cancels-reorder-preview-drop',
        'delete-shortcut-context',
        'context-menu-viewport-clamp',
        'sheet-reorder-bar-grid',
        'sheet-reorder-autoscroll-policy'
    )
    $retainedFrontendLog = if ($null -ne $retainedArtifactEvidence) {
        'frontend-structure-tests.log'
    }
    else {
        'gate stdout (ephemeral; use -ArtifactDirectory to retain)'
    }
    $retainedProjectCoreLog = if ($null -ne $retainedArtifactEvidence) {
        'public-project-core-journey.log'
    }
    else {
        'gate stdout (ephemeral; use -ArtifactDirectory to retain)'
    }
    $windowsWebView2Evidence = if ($null -ne $retainedArtifactEvidence) {
        'productive-runner observations and retained raw artifacts'
    }
    else {
        'productive-runner observations (raw artifacts not retained)'
    }
    $checks = @(
        foreach ($checkName in $windowsWebView2CheckNames) {
            [ordered]@{
                name = $checkName
                passed = $true
                proofLayer = 'windows-webview2'
                evidence = $windowsWebView2Evidence
            }
        }
        [ordered]@{
            name = 'public-project-core-journey'
            passed = $true
            proofLayer = 'rust-public-api'
            evidence = $retainedProjectCoreLog
        }
        foreach ($checkName in $frontendWorkspaceCheckNames) {
            [ordered]@{
                name = $checkName
                passed = $true
                proofLayer = 'public-workspace-vitest'
                evidence = $retainedFrontendLog
            }
        }
    )

    $report = [ordered]@{
        schemaVersion = 1
        gate = 'productive-end-to-end-journey'
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        gitCommit = $sourceBefore.gitCommit
        sourceInputsDirty = [bool] $sourceInputsDirty
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
            nativeDriverVersion = $driver.nativeDriverVersion
            webView2RuntimeVersion = $driver.webView2RuntimeVersion
        }
        checks = $checks
        coverageLimits = @(
            [ordered]@{
                property = 'queued-history-structural-authority-in-windows-webview2'
                status = 'not-claimed'
                provenBy = 'public-workspace-vitest'
                reason = 'Deterministic delayed History completion is exercised at the public ProjectWorkspace boundary without a product-only timing hook.'
            },
            [ordered]@{
                property = 'sheet-reorder-autoscroll-visual-distance'
                status = 'not-claimed'
                provenBy = 'public-workspace-vitest'
                reason = 'Progressive frame scheduling, edge distance and stop terminals are deterministic component behavior, not a stable screenshot property.'
            },
            [ordered]@{
                property = 'context-menu-all-corners-static-visual'
                status = 'not-claimed'
                provenBy = 'public-workspace-vitest'
                reason = 'All four viewport clamp geometries are asserted from the public floating-menu component; the visual suite demonstrates the current menu but does not claim every corner.'
            }
        )
        evidence = [ordered]@{
            cancelledCreationBeforeCore = [bool] $gate.cancelledCreationBeforeCore
            cancelledExportBeforePipeline = [bool] $gate.cancelledExportBeforePipeline
            createAuthorization = $gate.createAuthorization
            exportedSheetNumber = [int] $gate.exportedSheetNumber
            selectedSheetId = $gate.selectedSheetId
            selectedSheetActiveSides = $gate.selectedSheetActiveSides
            firstSheetDimensions = $gate.firstSheetDimensions
            selectedSheetDimensions = $gate.selectedSheetDimensions
            expectedBackgroundRgb = $gate.expectedBackgroundRgb
            exportedDpi = [int] $gate.exportedDpi
            savedRevision = [int] $gate.savedRevision
            savedDpi = [int] $gate.savedDpi
            projectSchemaVersion = [int] $gate.schemaVersion
            photoFrameCount = [int] $gate.photoFrameCount
            persistedPhotoLinkOnly = [bool] $gate.persistedPhotoLinkOnly
            reimportedExistingPhotoWithoutRevision = [bool] $gate.reimportedExistingPhotoWithoutRevision
            physicalAlbumStructure = $gate.physicalAlbumStructure
            sessionRecovery = $gate.sessionRecovery
            saveAs = $gate.saveAs
            originalUnchanged = [bool] $gate.originalUnchanged
            missingOriginalBlocked = [bool] $gate.missingOriginalBlocked
            missingOriginalActionable = [bool] $gate.missingOriginalActionable
            residentCanvasPreviewBeforeMissingOriginal = [bool] $gate.residentCanvasPreviewBeforeMissingOriginal
            previewArtifactCountBeforePurge = [int] $gate.previewArtifactCountBeforePurge
            cacheEntryCountBeforeExport = [int] $gate.cacheEntryCountBeforeExport
            cacheByteCountBeforeExport = [int64] $gate.cacheByteCountBeforeExport
            cacheEntryCountAfterExport = [int] $gate.cacheEntryCountAfterExport
            cacheByteCountAfterExport = [int64] $gate.cacheByteCountAfterExport
            cacheCouldNotProduceFalseSuccess = [bool] $gate.cacheCouldNotProduceFalseSuccess
            jpeg = $jpegEvidence
            processIds = $gate.processIds
            correlations = $gate.correlations
            exportedAfterReopen = [bool] $gate.exportedAfterReopen
            reopenedInIndependentHost = [bool] $gate.reopenedInIndependentHost
            reopenedHistoryEmpty = [bool] $gate.reopenedHistoryEmpty
            sourcePathExposedToWebView = [bool] $gate.sourcePathExposedToWebView
            terminalCounts = $gate.terminalCounts
            canvasNonWhiteSamples = $nonWhiteCount
            canvasSampleCount = $sampleCount
            canvasPhotoSample = [ordered]@{
                x = [int] $canvasPhotoX
                y = [int] $canvasPhotoY
                red = [int] $canvasPhotoSample.R
                green = [int] $canvasPhotoSample.G
                blue = [int] $canvasPhotoSample.B
                jpegMaxChannelDelta = $canvasJpegMaxChannelDelta
                tolerance = $photoTolerance
            }
            retainedArtifacts = $retainedArtifactEvidence
            cleanupCompleted = $true
        }
    }
    $json = $report | ConvertTo-Json -Depth 8
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputPath) |
        Out-Null
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $json + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "Productive journey report: $OutputPath"
    Write-Output $json
}
finally {
    if (-not $runRootCleaned -and (Test-Path -LiteralPath $runRoot)) {
        Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchRoot
    }
    if ($runnerMutexHeld) {
        $runnerMutex.ReleaseMutex()
    }
    $runnerMutex.Dispose()
}
