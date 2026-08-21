param([string] $OutputPath)

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

try {
    New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
    $node = (Get-Command node.exe -ErrorAction Stop).Source
    & $node --test `
        (Join-Path $PSScriptRoot 'Test-ProductiveJourneyObservations.mjs') `
        (Join-Path $PSScriptRoot 'Test-GateWebDriver.mjs')
    if ($LASTEXITCODE -ne 0) {
        throw "The productive journey observation tests failed with exit code $LASTEXITCODE."
    }
    & $script:CargoExecutable `
        test `
        -p myalbuns-core `
        --test productive_project_core_journey `
        -- `
        --exact
    if ($LASTEXITCODE -ne 0) {
        throw "The public ProjectCore journey failed with exit code $LASTEXITCODE."
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
        (Get-FileHash -Algorithm SHA256 -LiteralPath $preparedSidecarPath).Hash -ne
            (Get-FileHash -Algorithm SHA256 -LiteralPath $runtimeSidecarPath).Hash) {
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
    if (
        -not $gate.cancelledCreationBeforeCore -or
        -not $gate.cancelledExportBeforePipeline -or
        $gate.createAuthorization -ne 'createOnly' -or
        $gate.exportedSheetNumber -ne 2 -or
        $gate.selectedSheetId -notmatch '^[0-9a-f-]{36}$' -or
        $gate.selectedSheetActiveSides -ne 'both' -or
        $gate.firstSheetDimensions.width -ne 360 -or
        $gate.firstSheetDimensions.height -ne 360 -or
        $gate.selectedSheetDimensions.width -ne 720 -or
        $gate.selectedSheetDimensions.height -ne 360 -or
        $gate.expectedBackgroundRgb -ne '#204060' -or
        $gate.exportedDpi -ne 360 -or
        $gate.schemaVersion -ne 3 -or
        $gate.savedRevision -ne 3 -or
        $gate.savedDpi -ne 300 -or
        $gate.photoFrameCount -ne 1 -or
        -not $gate.persistedPhotoLinkOnly -or
        -not $gate.reimportedExistingPhotoWithoutRevision -or
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
        $gate.jpeg.width -ne 720 -or
        $gate.jpeg.height -ne 360 -or
        $gate.jpeg.byteCount -le 0 -or
        $gate.jpeg.sha256 -notmatch '^[0-9a-f]{64}$' -or
        $gate.correlations.bootstraps -ne 2 -or
        $gate.correlations.imagingAttempts -ne 1 -or
        -not $gate.exportedAfterReopen -or
        $gate.processIds.firstHost -eq $gate.processIds.host -or
        -not $gate.reopenedInIndependentHost -or
        -not $gate.reopenedHistoryEmpty -or
        $gate.canvasPhotoSample.cssWidth -le 0 -or
        $gate.canvasPhotoSample.cssHeight -le 0 -or
        $gate.sourcePathExposedToWebView -or
        $gate.terminalCounts.globalHandoffs -ne 2 -or
        $gate.terminalCounts.hostReady -ne 2 -or
        $gate.terminalCounts.imagingStopped -ne 1
    ) {
        throw 'The productive journey result did not satisfy its public contract.'
    }
    Add-Type -AssemblyName System.Drawing
    $jpegPath = Join-Path $runRoot 'Jornada produtiva_002.jpg'
    $jpegBitmap = [System.Drawing.Bitmap]::FromFile($jpegPath)
    try {
        $sampleX = 2
        $sampleY = [Math]::Floor($jpegBitmap.Height / 2)
        $backgroundSample = $jpegBitmap.GetPixel($sampleX, $sampleY)
        $photoExportSampleX = [Math]::Floor($jpegBitmap.Width / 2)
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

    Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchRoot
    $runRootCleaned = $true
    $sourceAfter = Get-GateSourceSnapshot `
        -WorkspaceRoot $workspaceRoot `
        -EvidencePath $OutputPath
    $sourceInputsDirty = Test-GateSourceSnapshotsDirty `
        -Before $sourceBefore `
        -After $sourceAfter

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
        checks = @(
            [ordered]@{ name = 'cancel-before-project-core'; passed = $true },
            [ordered]@{ name = 'create-only-causal-handoff'; passed = $true },
            [ordered]@{ name = 'project-core-save-history'; passed = $true },
            [ordered]@{ name = 'native-jpeg-import-reselect-external-link-only'; passed = $true },
            [ordered]@{ name = 'double-click-frame-selection-canvas'; passed = $true },
            [ordered]@{ name = 'cancel-before-export-pipeline'; passed = $true },
            [ordered]@{ name = 'distinguishable-sheet-two-jpeg-export'; passed = $true },
            [ordered]@{ name = 'canvas-jpeg-photo-fidelity'; passed = $true },
            [ordered]@{ name = 'real-application-empty-cache-original-read'; passed = $true },
            [ordered]@{ name = 'missing-original-actionable-failure'; passed = $true },
            [ordered]@{ name = 'saved-project-unchanged-by-export'; passed = $true },
            [ordered]@{ name = 'independent-host-reopen-empty-history'; passed = $true },
            [ordered]@{ name = 'correlated-process-terminals-cleanup'; passed = $true }
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
