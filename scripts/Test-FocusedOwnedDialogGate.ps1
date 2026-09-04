param(
    [string] $OutputPath,
    [ValidateSet('all', 'external-copy-opening-owner', 'late-graphics-project-dialog')]
    [string] $Scenario = 'all',
    [string] $BuildManifestPath = '.tools\native-gate-build.json',
    [switch] $AllowVisibleWindows
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Native-GatePolicy.ps1')
Assert-NativeGateExecutionAllowed -AllowVisibleWindows:$AllowVisibleWindows

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Native-GateBuild.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
. (Join-Path $PSScriptRoot 'Gate-OwnedProcessJob.ps1')
. (Join-Path $PSScriptRoot 'Gate-ProcessScope.ps1')
Initialize-MyAlbunsToolchain

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'The focused owned-dialog gate must run on Windows.'
}

$workspaceRoot = $script:WorkspaceRoot
if (-not [IO.Path]::IsPathRooted($BuildManifestPath)) {
    $BuildManifestPath = Join-Path $workspaceRoot $BuildManifestPath
}
if (-not (Test-Path -LiteralPath $BuildManifestPath)) {
    & (Join-Path $PSScriptRoot 'Build-NativeGate.ps1') -OutputPath $BuildManifestPath
}
$build = Read-NativeGateBuild -ManifestPath $BuildManifestPath -WorkspaceRoot $workspaceRoot
$applicationPath = $build.application.path
$fixturePath = $build.fixture.path
$scratchParent = [System.IO.Path]::GetFullPath(
    (Join-Path $workspaceRoot '.scratch')
)
New-Item -ItemType Directory -Force -Path $scratchParent | Out-Null
$retainedEvidenceRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $scratchParent 'focused-owned-dialog-evidence')
)
New-Item -ItemType Directory -Force -Path $retainedEvidenceRoot | Out-Null
$null = Resolve-GateRetainedEvidenceRoot `
    -GitRoot $workspaceRoot `
    -RetainedEvidenceRoot $retainedEvidenceRoot
$runRoot = [System.IO.Path]::GetFullPath(
    (Join-Path `
        $scratchParent `
        "focused-owned-dialog-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 8))")
)
if (-not [string]::Equals(
        [System.IO.Path]::GetDirectoryName($runRoot),
        $scratchParent,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The focused owned-dialog scratch root escaped the workspace.'
}
New-Item -ItemType Directory -Force -Path $runRoot | Out-Null

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $stamp = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff')
    $OutputPath = Join-Path `
        $retainedEvidenceRoot `
        "$stamp\report.json"
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $workspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$evidenceDirectory = Split-Path -Parent $OutputPath
New-Item -ItemType Directory -Force -Path $evidenceDirectory | Out-Null
$sourceBefore = Get-GateSourceSnapshot `
    -WorkspaceRoot $workspaceRoot `
    -EvidencePath $OutputPath `
    -RetainedEvidenceRoot $retainedEvidenceRoot
Assert-NativeGateBuildSource -Build $build -Source $sourceBefore
$applicationArtifact = $build.application
$scope = $null
$runRootCleaned = $false

function Copy-NativeScenarioDiagnostics {
    Get-ChildItem -LiteralPath $runRoot -File | Where-Object {
        $_.Name -like 'webdriver-*.log' -or $_.Name -like 'failure-*'
    } | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $evidenceDirectory $_.Name) -Force
    }
}

try {
    $node = (Get-Command node.exe -ErrorAction Stop).Source
    $windowsPowerShell = (Get-Command powershell.exe -ErrorAction Stop).Source
    foreach ($knownFolder in @('Roaming', 'Local', 'Temporary')) {
        New-Item `
            -ItemType Directory `
            -Force `
            -Path (Join-Path $runRoot "process-data\$knownFolder") |
            Out-Null
    }

    $previousProcessDataRoot = $env:MYALBUNS_PROCESS_GATE_DATA_ROOT
    try {
        $env:MYALBUNS_PROCESS_GATE_DATA_ROOT = Join-Path $runRoot 'process-data'
        & $fixturePath $runRoot
        if ($LASTEXITCODE -ne 0) {
            throw "The focused fixture preparation failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        if ($null -eq $previousProcessDataRoot) {
            Remove-Item Env:MYALBUNS_PROCESS_GATE_DATA_ROOT -ErrorAction SilentlyContinue
        }
        else {
            $env:MYALBUNS_PROCESS_GATE_DATA_ROOT = $previousProcessDataRoot
        }
    }

    $driver = & (Join-Path $PSScriptRoot 'Resolve-TauriWebDriver.ps1') |
        Select-Object -Last 1 |
        ConvertFrom-Json
    $scope = New-GateProcessScope `
        -WorkspaceRoot $workspaceRoot `
        -RunRoot $runRoot `
        -WindowsPowerShell $windowsPowerShell
    $focused = Invoke-GateScopedCommand `
        -Scope $scope `
        -Name 'focused-native' `
        -FilePath $node `
        -Arguments @(
            (Join-Path $PSScriptRoot 'Run-FocusedOwnedDialogGate.mjs'),
            $workspaceRoot,
            $runRoot,
            $applicationPath,
            $driver.nativeDriverPath,
            $Scenario
        )
    $gate = @($focused.output -split "`n" | Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }) |
        Select-Object -Last 1 |
        ConvertFrom-Json
    $expectedScenarios = if ($Scenario -eq 'all') {
        @('external-copy-opening-owner', 'late-graphics-project-dialog')
    } else { @($Scenario) }
    $externalCopyInvalid = $expectedScenarios -contains 'external-copy-opening-owner' -and (
        -not $gate.externalCopy.oneVisibleDialog -or
        -not $gate.externalCopy.ownerDisabled -or
        -not $gate.externalCopy.exactPickerOwner -or
        -not $gate.externalCopy.samePendingHostAndRevision -or
        -not $gate.externalCopy.activationDispatched -or
        -not $gate.externalCopy.hostCorrelated -or
        -not $gate.externalCopy.publicTerminalObserved -or
        -not $gate.externalCopy.terminalCleaned
    )
    $graphicsInvalid = $expectedScenarios -contains 'late-graphics-project-dialog' -and (
        -not $gate.graphicsFailure.oneVisibleDialog -or
        -not $gate.graphicsFailure.ownerDisabled -or
        -not $gate.graphicsFailure.workspaceInert -or
        $gate.graphicsFailure.exactAction -cne 'Fechar Projeto' -or
        -not $gate.graphicsFailure.terminalCleaned
    )
    if (($gate.scenarios -join ',') -cne ($expectedScenarios -join ',') -or
        $externalCopyInvalid -or $graphicsInvalid -or -not $gate.cleanupCompleted) {
        throw "The focused native evidence is incomplete: $($gate | ConvertTo-Json -Depth 8 -Compress)"
    }
    $cleanup = Stop-GateProcessScope -Scope $scope
    $scope = $null
    if ($cleanup.processesAfter -ne 0 -or $cleanup.listenersAfter -ne 0) {
        throw 'The focused owned-dialog process scope did not clean up.'
    }

    Copy-Item `
        -LiteralPath (Join-Path $runRoot 'focused-native.log') `
        -Destination (Join-Path $evidenceDirectory 'focused-native.log')

    Copy-NativeScenarioDiagnostics

    Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchParent
    $runRootCleaned = $true

    $sourceAfter = Get-GateSourceSnapshot `
        -WorkspaceRoot $workspaceRoot `
        -EvidencePath $OutputPath `
        -RetainedEvidenceRoot $retainedEvidenceRoot
    $sourceInputsDirty = Test-GateSourceSnapshotsDirty `
        -Before $sourceBefore `
        -After $sourceAfter
    $report = [ordered]@{
        schemaVersion = 1
        gate = 'focused-owned-dialogs'
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        gitCommit = $sourceBefore.gitCommit
        sourceInputsDirty = [bool] $sourceInputsDirty
        applicationArtifact = $applicationArtifact
        scenarios = @($expectedScenarios)
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
            nativeDriverVersion = $driver.nativeDriverVersion
            webView2RuntimeVersion = $driver.webView2RuntimeVersion
        }
        externalCopy = $gate.externalCopy
        graphicsFailure = $gate.graphicsFailure
        cleanup = [ordered]@{
            applicationCompleted = [bool] $gate.cleanupCompleted
            ownedProcessesAfter = [int] $cleanup.processesAfter
            ownedListenersAfter = [int] $cleanup.listenersAfter
        }
        buildManifestSha256 = (Get-FileHash -LiteralPath $BuildManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
        fullProductiveJourneyInvoked = $false
    }
    $json = $report | ConvertTo-Json -Depth 10
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $json + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )

    Write-Output "Focused owned-dialog report: $OutputPath"
    Write-Output $json
}
catch {
    $caughtFailure = $_
    try {
        $failure = [ordered]@{
            schemaVersion = 1
            gate = 'focused-owned-dialogs'
            collectedAtUtc = [DateTime]::UtcNow.ToString('o')
            gitCommit = $sourceBefore.gitCommit
            applicationArtifact = $applicationArtifact
            error = [string] $caughtFailure.Exception.Message
            scriptStack = [string] $caughtFailure.ScriptStackTrace
            fullProductiveJourneyInvoked = $false
        }
        [System.IO.File]::WriteAllText(
            (Join-Path $evidenceDirectory 'failure.json'),
            ($failure | ConvertTo-Json -Depth 8) + [System.Environment]::NewLine,
            [System.Text.UTF8Encoding]::new($false)
        )
        $focusedLog = Join-Path $runRoot 'focused-native.log'
        if (Test-Path -LiteralPath $focusedLog -PathType Leaf) {
            Copy-Item `
                -LiteralPath $focusedLog `
                -Destination (Join-Path $evidenceDirectory 'focused-native.log') `
                -Force
        }
        Copy-NativeScenarioDiagnostics
        $processLogs = Join-Path $runRoot 'process-data\Local\MyAlbuns2\Logs'
        if (Test-Path -LiteralPath $processLogs -PathType Container) {
            Copy-Item `
                -LiteralPath $processLogs `
                -Destination (Join-Path $evidenceDirectory 'process-logs') `
                -Recurse `
                -Force
        }
    }
    catch {
        Write-Warning "The focused gate could not retain every failure diagnostic: $($_.Exception.Message)"
    }
    throw $caughtFailure
}
finally {
    if ($null -ne $scope) {
        [void] (Stop-GateProcessScope -Scope $scope)
    }
    if (-not $runRootCleaned -and (Test-Path -LiteralPath $runRoot)) {
        Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchParent
    }
}
