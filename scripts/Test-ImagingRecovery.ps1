param([string] $OutputPath)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
Initialize-MyAlbunsToolchain

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0004-imaging-recovery.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$sourceSnapshotBefore = Get-GateSourceSnapshot `
    -WorkspaceRoot $script:WorkspaceRoot `
    -EvidencePath $OutputPath

$scratchRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot '.scratch')
)
$evidenceDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path `
        $scratchRoot `
        "ir-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 6))")
)
$evidenceParent = [System.IO.Path]::GetDirectoryName($evidenceDirectory)
if (-not [string]::Equals(
        $evidenceParent,
        $scratchRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The recovery evidence directory escaped the workspace scratch root.'
}
$processorTargetDirectory = Join-Path $evidenceDirectory 'processor-target'

$checks = @(
    [ordered]@{
        name = 'protocol'
        arguments = @(
            'test',
            '-p',
            'myalbuns-imaging-protocol',
            '--test',
            'protocol'
        )
    },
    [ordered]@{
        name = 'cache-temporary-cleanup'
        arguments = @(
            'test',
            '-p',
            'myalbuns-paths',
            '--test',
            'app_paths',
            'discards_only_cache_temporaries_left_by_a_terminated_processor'
        )
    },
    [ordered]@{
        name = 'imaging-sidecar-build'
        arguments = @(
            'build',
            '-p',
            'myalbuns-imaging',
            '--bin',
            'myalbuns-imaging',
            '--target-dir',
            $processorTargetDirectory
        )
    },
    [ordered]@{
        name = 'production-recovery-integration'
        arguments = @(
            'test',
            '-p',
            'myalbuns-desktop',
            '--lib',
            'imaging_recovery_integration::real_processor_recovery_flows_through_production_modules',
            '--',
            '--ignored',
            '--exact',
            '--nocapture'
        )
    },
    [ordered]@{
        name = 'cache-webview-canvas-export-journey'
        arguments = @(
            'test',
            '-p',
            'myalbuns-desktop',
            '--lib',
            'imaging_recovery_integration::real_cache_webview_canvas_reference_matches_background_overlay_export',
            '--',
            '--ignored',
            '--exact',
            '--nocapture',
            '--test-threads=1'
        )
    },
    [ordered]@{
        name = 'obsolete-cache-cancellation-integration'
        arguments = @(
            'test',
            '-p',
            'myalbuns-desktop',
            '--lib',
            'imaging_recovery_integration::real_obsolete_cache_demand_cancels_and_reaps_the_processor',
            '--',
            '--ignored',
            '--exact',
            '--nocapture',
            '--test-threads=1'
        )
    },
    [ordered]@{
        name = 'causal-cache-pause-integration'
        arguments = @(
            'test',
            '-p',
            'myalbuns-desktop',
            '--lib',
            'imaging_recovery_integration::real_cache_is_causally_paused_for_export_and_resumes_after_terminal',
            '--',
            '--ignored',
            '--exact',
            '--nocapture',
            '--test-threads=1'
        )
    }
)

New-Item -ItemType Directory -Force -Path $evidenceDirectory | Out-Null
$evidenceEnvironmentName = 'MYALBUNS_RECOVERY_EVIDENCE_DIR'
$processorEnvironmentName = 'MYALBUNS_REAL_IMAGING_PROCESSOR'
$processDataEnvironmentName = 'MYALBUNS_PROCESS_GATE_DATA_ROOT'
$processorPath = Join-Path $processorTargetDirectory 'debug\myalbuns-imaging.exe'
$processDataRoot = Join-Path $evidenceDirectory 'process-data'
foreach ($knownFolder in @('Roaming', 'Local', 'Temporary')) {
    New-Item `
        -ItemType Directory `
        -Force `
        -Path (Join-Path $processDataRoot $knownFolder) | Out-Null
}
$previousEvidenceDirectory = [System.Environment]::GetEnvironmentVariable(
    $evidenceEnvironmentName,
    [System.EnvironmentVariableTarget]::Process
)
$previousProcessorPath = [System.Environment]::GetEnvironmentVariable(
    $processorEnvironmentName,
    [System.EnvironmentVariableTarget]::Process
)
$previousProcessDataRoot = [System.Environment]::GetEnvironmentVariable(
    $processDataEnvironmentName,
    [System.EnvironmentVariableTarget]::Process
)
[System.Environment]::SetEnvironmentVariable(
    $evidenceEnvironmentName,
    $evidenceDirectory,
    [System.EnvironmentVariableTarget]::Process
)
[System.Environment]::SetEnvironmentVariable(
    $processorEnvironmentName,
    $processorPath,
    [System.EnvironmentVariableTarget]::Process
)
[System.Environment]::SetEnvironmentVariable(
    $processDataEnvironmentName,
    $processDataRoot,
    [System.EnvironmentVariableTarget]::Process
)

$results = [System.Collections.Generic.List[object]]::new()
$cacheEvidence = $null
$exportEvidence = $null
$canvasEvidence = $null
$obsoleteEvidence = $null
$pauseEvidence = $null
$locationWasPushed = $false
try {
    Push-Location $script:WorkspaceRoot
    $locationWasPushed = $true
    foreach ($check in $checks) {
        $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        & $script:CargoExecutable @($check.arguments)
        $exitCode = $LASTEXITCODE
        $stopwatch.Stop()
        if ($exitCode -ne 0) {
            throw "Imaging recovery check '$($check.name)' failed with exit code $exitCode."
        }
        $results.Add([ordered]@{
            name = $check.name
            passed = $true
            elapsedMs = $stopwatch.ElapsedMilliseconds
        })
    }

    $advertisedProtocol = (& $processorPath --protocol-version).Trim()
    if ($LASTEXITCODE -ne 0 -or $advertisedProtocol -notmatch '^\d+$') {
        throw 'The freshly built Imaging sidecar did not advertise a valid protocol version.'
    }
    $expectedProtocol = [int] $advertisedProtocol

    $cacheEvidencePath = Join-Path $evidenceDirectory 'cache.json'
    $exportEvidencePath = Join-Path $evidenceDirectory 'export.json'
    $canvasEvidencePath = Join-Path $evidenceDirectory 'canvas.json'
    $obsoleteEvidencePath = Join-Path $evidenceDirectory 'obsolete.json'
    $pauseEvidencePath = Join-Path $evidenceDirectory 'pause.json'
    if (-not (Test-Path -LiteralPath $cacheEvidencePath -PathType Leaf)) {
        throw 'The real Cache crash test did not produce evidence.'
    }
    if (-not (Test-Path -LiteralPath $exportEvidencePath -PathType Leaf)) {
        throw 'The real Export crash test did not produce evidence.'
    }
    if (-not (Test-Path -LiteralPath $canvasEvidencePath -PathType Leaf)) {
        throw 'The real Cache-WebView-Canvas-Export journey did not produce evidence.'
    }
    if (-not (Test-Path -LiteralPath $obsoleteEvidencePath -PathType Leaf)) {
        throw 'The real obsolete Cache cancellation did not produce evidence.'
    }
    if (-not (Test-Path -LiteralPath $pauseEvidencePath -PathType Leaf)) {
        throw 'The real causal Cache pause test did not produce evidence.'
    }
    $cacheEvidence = Get-Content -LiteralPath $cacheEvidencePath -Raw | ConvertFrom-Json
    $exportEvidence = Get-Content -LiteralPath $exportEvidencePath -Raw | ConvertFrom-Json
    $canvasEvidence = Get-Content -LiteralPath $canvasEvidencePath -Raw | ConvertFrom-Json
    $obsoleteEvidence = Get-Content -LiteralPath $obsoleteEvidencePath -Raw | ConvertFrom-Json
    $pauseEvidence = Get-Content -LiteralPath $pauseEvidencePath -Raw | ConvertFrom-Json

    $tauriBuildStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    & (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1') `
        -Action build `
        -TauriArguments @('--debug', '--no-bundle')
    if ($LASTEXITCODE -ne 0) {
        throw "The actual debug Tauri application build failed with exit code $LASTEXITCODE."
    }
    $tauriBuildStopwatch.Stop()
    $applicationPath = Join-Path `
        $script:WorkspaceRoot `
        'target\debug\myalbuns-desktop.exe'
    if (-not (Test-Path -LiteralPath $applicationPath -PathType Leaf)) {
        throw 'The actual debug Tauri application executable was not produced.'
    }
    $results.Add([ordered]@{
        name = 'actual-tauri-webview2-build'
        passed = $true
        elapsedMs = $tauriBuildStopwatch.ElapsedMilliseconds
    })

    $webDriverSetupOutput = @(& (Join-Path $PSScriptRoot 'Resolve-TauriWebDriver.ps1'))
    $webDriver = $webDriverSetupOutput[-1] | ConvertFrom-Json
    $browserStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $backgroundPreviewPath = Join-Path $evidenceDirectory 'canvas-background-preview.jpg'
    $overlayPreviewPath = Join-Path $evidenceDirectory 'canvas-overlay-preview.png'
    if (-not (Test-Path -LiteralPath $backgroundPreviewPath -PathType Leaf) `
            -or -not (Test-Path -LiteralPath $overlayPreviewPath -PathType Leaf)) {
        throw 'The real Canvas journey did not retain its derived replay evidence.'
    }
    $nodePath = (Get-Command node.exe -ErrorAction Stop).Source
    $canvasGateRunner = Join-Path `
        $script:WorkspaceRoot `
        'scripts\Run-RealCanvasGate.mjs'
    if (-not (Test-Path -LiteralPath $canvasGateRunner -PathType Leaf)) {
        throw 'The real Tauri WebDriver AlbumCanvas runner is absent.'
    }
    $screenshotPath = Join-Path $evidenceDirectory 'actual-album-canvas.png'
    $canvasGateOut = Join-Path $evidenceDirectory 'canvas-gate.out.log'
    $canvasGateError = Join-Path $evidenceDirectory 'canvas-gate.error.log'
    $canvasGateProcess = Start-Process `
        -FilePath $nodePath `
        -ArgumentList @(
            $canvasGateRunner,
            $evidenceDirectory,
            $screenshotPath,
            $applicationPath,
            $webDriver.tauriDriverPath,
            $webDriver.nativeDriverPath
        ) `
        -WorkingDirectory $script:WorkspaceRoot `
        -WindowStyle Hidden `
        -RedirectStandardOutput $canvasGateOut `
        -RedirectStandardError $canvasGateError `
        -Wait `
        -PassThru
    if ($canvasGateProcess.ExitCode -ne 0) {
        $canvasGateFailure = Get-Content `
            -LiteralPath $canvasGateError `
            -Raw `
            -ErrorAction SilentlyContinue
        throw "The real Tauri WebView2 AlbumCanvas gate failed: $canvasGateFailure"
    }
    $canvasGate = Get-Content -LiteralPath $canvasGateOut -Raw | ConvertFrom-Json
    if (-not $canvasGate.actualTauriApp `
            -or -not $canvasGate.actualAlbumCanvas `
            -or -not $canvasGate.actualPixiRuntime `
            -or $canvasGate.originalPathExposedToWebView `
            -or $canvasGate.opaqueResourceCount -lt 2 `
            -or $canvasGate.screenshotScope -ne 'canvas-element' `
            -or $canvasGate.samplePoint -ne 'canvas-center' `
            -or -not (Test-Path -LiteralPath $screenshotPath -PathType Leaf)) {
        throw 'The real Tauri WebView2 AlbumCanvas process did not produce valid evidence.'
    }
    Add-Type -AssemblyName System.Drawing
    $bitmap = [System.Drawing.Bitmap]::FromFile($screenshotPath)
    try {
        if ($bitmap.Width -lt 100 -or $bitmap.Height -lt 100) {
            throw "The productive AlbumCanvas screenshot is unexpectedly small: $($bitmap.Width)x$($bitmap.Height)."
        }
        $expectedCanvasChannels = @($canvasEvidence.canvasReferencePixel)
        $sampleX = [Math]::Floor($bitmap.Width / 2)
        $sampleY = [Math]::Floor($bitmap.Height / 2)
        $actualCanvasPixel = $bitmap.GetPixel($sampleX, $sampleY)
    }
    finally {
        $bitmap.Dispose()
    }
    $actualCanvasChannels = @(
        [int]$actualCanvasPixel.R,
        [int]$actualCanvasPixel.G,
        [int]$actualCanvasPixel.B
    )
    $actualCanvasDelta = @(
        [Math]::Abs($actualCanvasChannels[0] - $expectedCanvasChannels[0]),
        [Math]::Abs($actualCanvasChannels[1] - $expectedCanvasChannels[1]),
        [Math]::Abs($actualCanvasChannels[2] - $expectedCanvasChannels[2])
    )
    $actualCanvasMaxDelta = @($actualCanvasDelta | Measure-Object -Maximum).Maximum
    if ($actualCanvasMaxDelta -gt 12) {
        throw "The actual AlbumCanvas/Pixi pixel diverged: actual=$actualCanvasChannels expected=$expectedCanvasChannels delta=$actualCanvasDelta."
    }
    $canvasEvidence | Add-Member -NotePropertyName actualTauriApp -NotePropertyValue $canvasGate.actualTauriApp
    $canvasEvidence | Add-Member -NotePropertyName actualAlbumCanvas -NotePropertyValue $canvasGate.actualAlbumCanvas
    $canvasEvidence | Add-Member -NotePropertyName actualPixiRuntime -NotePropertyValue $canvasGate.actualPixiRuntime
    $canvasEvidence | Add-Member -NotePropertyName browserProcess -NotePropertyValue $canvasGate.browserProcess
    $canvasEvidence | Add-Member -NotePropertyName tauriDriverVersion -NotePropertyValue $webDriver.tauriDriverVersion
    $canvasEvidence | Add-Member -NotePropertyName nativeDriverVersion -NotePropertyValue $webDriver.nativeDriverVersion
    $canvasEvidence | Add-Member -NotePropertyName webView2RuntimeVersion -NotePropertyValue $webDriver.webView2RuntimeVersion
    $canvasEvidence | Add-Member -NotePropertyName opaqueResourceCount -NotePropertyValue $canvasGate.opaqueResourceCount
    $canvasEvidence | Add-Member -NotePropertyName canvasScreenshotScope -NotePropertyValue $canvasGate.screenshotScope
    $canvasEvidence | Add-Member -NotePropertyName actualCanvasSamplePoint -NotePropertyValue @($sampleX, $sampleY)
    $canvasEvidence | Add-Member -NotePropertyName actualCanvasPixel -NotePropertyValue $actualCanvasChannels
    $canvasEvidence | Add-Member -NotePropertyName actualCanvasDelta -NotePropertyValue $actualCanvasDelta
    $browserStopwatch.Stop()
    $results.Add([ordered]@{
        name = 'actual-tauri-album-canvas-pixi-webview2'
        passed = $true
        elapsedMs = $browserStopwatch.ElapsedMilliseconds
    })

    if ($cacheEvidence.failedProcessId -eq $cacheEvidence.restartedProcessId `
            -or -not $cacheEvidence.temporaryObservedAfterFailure `
            -or $cacheEvidence.removedTemporaryCount -lt 1 `
            -or $cacheEvidence.temporaryExistedAfterCleanup `
            -or -not $cacheEvidence.foreignTemporarySurvivedCleanup `
            -or $cacheEvidence.metadataExistedAfterFailure `
            -or -not $cacheEvidence.metadataExistedAfterRestart `
            -or $cacheEvidence.generatedCountAfterRestart -lt 1) {
        throw 'The observed Cache recovery evidence does not satisfy the gate.'
    }
    if ($exportEvidence.failedProcessId -eq $exportEvidence.retryProcessId `
            -or $exportEvidence.protocolVersion -ne $expectedProtocol `
            -or $exportEvidence.processCountBeforeExplicitRetry -ne 1 `
            -or $exportEvidence.successResponseBeforeExplicitRetry `
            -or -not $exportEvidence.partialPreparationObserved `
            -or $exportEvidence.previousOutputSha256BeforeFailure `
                -ne $exportEvidence.previousOutputSha256AfterFailure `
            -or $exportEvidence.projectSha256BeforeFailure `
                -ne $exportEvidence.projectSha256AfterFailure `
            -or $exportEvidence.finalOutputSha256AfterExplicitRetry `
                -eq $exportEvidence.previousOutputSha256BeforeFailure) {
        throw 'The observed Export recovery evidence does not satisfy the gate.'
    }
    $canvasProcessorIds = @($canvasEvidence.processorIds)
    $canvasUniqueProcessorIds = @($canvasProcessorIds | Sort-Object -Unique)
    $canvasMaxDelta = @($canvasEvidence.channelDelta | Measure-Object -Maximum).Maximum
    if ($canvasProcessorIds.Count -ne 3 `
            -or $canvasUniqueProcessorIds.Count -ne 3 `
            -or $canvasEvidence.equivalentBackgroundDemandCount -ne 2 `
            -or $canvasEvidence.singleFlightProcessorCount -ne 1 `
            -or -not $canvasEvidence.actualAlbumCanvas `
            -or -not $canvasEvidence.actualTauriApp `
            -or -not $canvasEvidence.actualPixiRuntime `
            -or -not $canvasEvidence.backgroundUrlOpaque `
            -or -not $canvasEvidence.overlayUrlOpaque `
            -or $canvasEvidence.originalPathExposedToWebView `
            -or $canvasEvidence.originalBytesExposedToWebView `
            -or $canvasEvidence.finalSourceCount -ne 2 `
            -or $canvasMaxDelta -gt 12) {
        throw 'The real Cache-WebView-Canvas-Export evidence does not satisfy the gate.'
    }
    if ($pauseEvidence.cancelledProcessId -eq $pauseEvidence.resumedProcessId `
            -or -not $pauseEvidence.cancelledProcessReaped `
            -or $pauseEvidence.cancelledStage -ne 'cancelled' `
            -or $pauseEvidence.cacheIndexAfterCancellation `
            -or $pauseEvidence.pauseReason -ne 'paused' `
            -or -not $pauseEvidence.cacheBlockedWhileExportLease `
            -or -not $pauseEvidence.processorExclusiveWhileExportLease `
            -or -not $pauseEvidence.resumedAfterExportTerminal `
            -or -not $pauseEvidence.resumedGenerationPublished) {
        throw 'The observed causal Cache pause evidence does not satisfy the gate.'
    }
    if ($obsoleteEvidence.cancelledProcessId -le 0 `
            -or -not $obsoleteEvidence.cancelledProcessReaped `
            -or $obsoleteEvidence.cancelledStage -ne 'cancelled' `
            -or $obsoleteEvidence.cancellationReason -ne 'obsolete' `
            -or $obsoleteEvidence.singleFlightDemandCount -ne 2 `
            -or $obsoleteEvidence.singleFlightProcessorCount -ne 1 `
            -or -not $obsoleteEvidence.waiterObservedCancellation `
            -or $obsoleteEvidence.resumableAfterCancellation `
            -or $obsoleteEvidence.cacheIndexAfterCancellation) {
        throw 'The observed obsolete Cache cancellation evidence does not satisfy the gate.'
    }
}
finally {
    if ($locationWasPushed) {
        Pop-Location
    }
    [System.Environment]::SetEnvironmentVariable(
        $evidenceEnvironmentName,
        $previousEvidenceDirectory,
        [System.EnvironmentVariableTarget]::Process
    )
    [System.Environment]::SetEnvironmentVariable(
        $processorEnvironmentName,
        $previousProcessorPath,
        [System.EnvironmentVariableTarget]::Process
    )
    [System.Environment]::SetEnvironmentVariable(
        $processDataEnvironmentName,
        $previousProcessDataRoot,
        [System.EnvironmentVariableTarget]::Process
    )
    Remove-GateScratchDirectory `
        -Path $evidenceDirectory `
        -AllowedParent $scratchRoot
}

$sourceSnapshotAfter = Get-GateSourceSnapshot `
    -WorkspaceRoot $script:WorkspaceRoot `
    -EvidencePath $OutputPath
$report = [ordered]@{
    schemaVersion = 1
    collectedAtUtc = [DateTime]::UtcNow.ToString('o')
    gitCommit = $sourceSnapshotBefore.gitCommit
    sourceInputsDirty = Test-GateSourceSnapshotsDirty `
        -Before $sourceSnapshotBefore `
        -After $sourceSnapshotAfter
    platform = [ordered]@{
        operatingSystem = [System.Environment]::OSVersion.VersionString
        architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    }
    checks = @($results)
    evidence = [ordered]@{
        cache = $cacheEvidence
        export = $exportEvidence
        canvas = $canvasEvidence
        obsolete = $obsoleteEvidence
        pause = $pauseEvidence
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
Write-Output "Imaging recovery report: $OutputPath"
Write-Output $json
