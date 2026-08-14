param(
    [string] $OutputPath
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
Initialize-MyAlbunsToolchain

$workspaceRoot = $script:WorkspaceRoot
$runRoot = Join-Path `
    $workspaceRoot `
    ".scratch\dev-lifecycle-$PID-$([DateTime]::UtcNow.Ticks)"
$processDataRoot = Join-Path $runRoot 'process-data'
$projectPath = Join-Path $runRoot 'Projeto lifecycle.myalbuns'
$screenshotPath = Join-Path $runRoot 'project-ui.png'
New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
$evidencePath = if ($OutputPath) {
    [System.IO.Path]::GetFullPath($OutputPath)
}
else {
    Join-Path $runRoot 'dev-lifecycle-evidence.json'
}
$sourceBefore = Get-GateSourceSnapshot `
    -WorkspaceRoot $workspaceRoot `
    -EvidencePath $evidencePath
$runRootCleaned = $false

try {
    $node = (Get-Command node.exe -ErrorAction Stop).Source
    $processInstanceTest = Join-Path `
        $PSScriptRoot `
        'Test-DevLifecycleProcessInstances.mjs'
    $gateObservationTest = Join-Path `
        $PSScriptRoot `
        'Test-DevLifecycleGateObservations.mjs'
    & $node --test $processInstanceTest $gateObservationTest
    if ($LASTEXITCODE -ne 0) {
        throw "The development lifecycle regressions failed with exit code $LASTEXITCODE."
    }

    $fixturePath = Join-Path `
        $workspaceRoot `
        'crates\myalbuns-core\tests\fixtures\project_document_v2_migration_expected.myalbuns'
    $fixture = Get-Content -LiteralPath $fixturePath -Raw -Encoding UTF8 |
        ConvertFrom-Json
    $fixture.projectId = [guid]::NewGuid().ToString()
    $fixture.revision = 0
    $fixtureJson = $fixture | ConvertTo-Json -Depth 32
    [System.IO.File]::WriteAllText(
        $projectPath,
        $fixtureJson,
        (New-Object System.Text.UTF8Encoding($false))
    )

    & (Join-Path $PSScriptRoot 'Prepare-DevLauncher.ps1')
    if ($LASTEXITCODE -ne 0) {
        throw "The development supervisor build failed with exit code $LASTEXITCODE."
    }
    $applicationPath = Join-Path $workspaceRoot 'target\debug\myalbuns-dev.exe'

    $driver = & (Join-Path $PSScriptRoot 'Resolve-TauriWebDriver.ps1') |
        ConvertFrom-Json
    $gateScript = Join-Path $PSScriptRoot 'Run-DevLifecycleGate.mjs'
    $gateOutput = & $node `
        $gateScript `
        $workspaceRoot `
        $projectPath `
        $processDataRoot `
        $screenshotPath `
        $applicationPath `
        $driver.nativeDriverPath
    if ($LASTEXITCODE -ne 0) {
        throw "The real development lifecycle gate failed with exit code $LASTEXITCODE."
    }
    $gate = $gateOutput | Select-Object -Last 1 | ConvertFrom-Json
    if (
        -not $gate.bootstrapFailureCleanupTerminalObserved -or
        -not $gate.containmentFailureTerminalObserved
    ) {
        throw 'The shared failure-phase runner did not preserve its typed cleanup and containment terminals.'
    }

    Add-Type -AssemblyName System.Drawing
    $bitmap = [System.Drawing.Bitmap]::FromFile($screenshotPath)
    try {
        $sampleCount = 0
        $nonWhiteCount = 0
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
        throw "The Project WebView screenshot is blank ($nonWhiteCount/$sampleCount non-white samples)."
    }

    Remove-GateScratchDirectory `
        -Path $runRoot `
        -AllowedParent (Join-Path $workspaceRoot '.scratch')
    $runRootCleaned = $true
    $sourceAfter = Get-GateSourceSnapshot `
        -WorkspaceRoot $workspaceRoot `
        -EvidencePath $evidencePath
    $sourceInputsDirty = Test-GateSourceSnapshotsDirty `
        -Before $sourceBefore `
        -After $sourceAfter

    $report = [ordered]@{
        schemaVersion = 1
        gate = 'development-lifecycle'
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        gitCommit = $sourceBefore.gitCommit
        sourceInputsDirty = [bool] $sourceInputsDirty
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        }
        supervisorPid = $gate.supervisorPid
        globalPid = $gate.globalPid
        hostPid = $gate.hostPid
        vitePid = $gate.vitePid
        webdriverMode = $gate.webdriverMode
        globalExitedAfterUiReady = [bool] $gate.globalExitedAfterUiReady
        hostSurvivedGlobal = [bool] $gate.hostSurvivedGlobal
        viteSurvivedGlobal = [bool] $gate.viteSurvivedGlobal
        projectUiRendered = [bool] $gate.projectUiRendered
        nonWhiteSamples = $nonWhiteCount
        sampleCount = $sampleCount
        cleanupCompleted = [bool] $gate.cleanupCompleted
        cleanupLogged = [bool] $gate.cleanupLogged
        normalTreeProcessCount = [int] $gate.normalTreeProcessCount
        normalHostTreeProcessCount = [int] $gate.normalHostTreeProcessCount
        abruptCleanupCompleted = [bool] $gate.abruptCleanupCompleted
        abruptTreeProcessCount = [int] $gate.abruptTreeProcessCount
        ctrlCCleanupCompleted = [bool] $gate.ctrlCCleanupCompleted
        ctrlCTreeProcessCount = [int] $gate.ctrlCTreeProcessCount
        ctrlCHostTreeProcessCount = [int] $gate.ctrlCHostTreeProcessCount
        bootstrapFailureCleanupCompleted = [bool] $gate.bootstrapFailureCleanupCompleted
        bootstrapFailureCleanupTerminalObserved = [bool] $gate.bootstrapFailureCleanupTerminalObserved
        bootstrapFailureTreeProcessCount = [int] $gate.bootstrapFailureTreeProcessCount
        containmentFailureCleanupCompleted = [bool] $gate.containmentFailureCleanupCompleted
        containmentFailureTerminalObserved = [bool] $gate.containmentFailureTerminalObserved
        containmentFailureTreeProcessCount = [int] $gate.containmentFailureTreeProcessCount
        frontendFailureCleanupCompleted = [bool] $gate.frontendFailureCleanupCompleted
        nativeDriverVersion = $driver.nativeDriverVersion
        webView2RuntimeVersion = $driver.webView2RuntimeVersion
        checks = @(
            [ordered]@{ name = 'causal-ui-ready-handoff'; passed = $true },
            [ordered]@{ name = 'project-webview-rendered'; passed = $true },
            [ordered]@{ name = 'normal-tree-cleanup'; passed = $true },
            [ordered]@{ name = 'abrupt-tree-cleanup'; passed = $true },
            [ordered]@{ name = 'ctrl-c-tree-cleanup'; passed = $true },
            [ordered]@{ name = 'bootstrap-failure-cleanup'; passed = $true },
            [ordered]@{ name = 'frontend-failure-cleanup'; passed = $true }
        )
    }
    $json = $report | ConvertTo-Json -Depth 8
    if ($OutputPath) {
        New-Item -ItemType Directory -Force -Path (
            Split-Path -Parent $evidencePath
        ) | Out-Null
        Set-Content -LiteralPath $evidencePath -Value $json -Encoding UTF8
    }
    Write-Output $json
}
finally {
    if (-not $runRootCleaned -and (Test-Path -LiteralPath $runRoot)) {
        Remove-GateScratchDirectory `
            -Path $runRoot `
            -AllowedParent (Join-Path $workspaceRoot '.scratch')
    }
}
