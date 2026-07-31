param(
    [string] $OutputPath,
    [ValidateRange(10, 300)]
    [int] $ProbeTimeoutSeconds = 90
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
. (Join-Path $PSScriptRoot 'Evidence-BuildInputs.ps1')

if ($env:OS -ne 'Windows_NT') {
    throw 'The OperationGate gate must run on Windows.'
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0009-operation-gate-lease.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

$probeParent = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot '.scratch\operation-gate-probe')
)
$runId = "$PID-$([DateTime]::UtcNow.Ticks)"
$independentRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $probeParent "run-$runId-independent")
)
$multiwindowRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $probeParent "run-$runId-multiwindow")
)
$fixtureRoots = @($independentRoot, $multiwindowRoot)
foreach ($fixtureRoot in $fixtureRoots) {
    if (-not $fixtureRoot.StartsWith(
            $probeParent + [System.IO.Path]::DirectorySeparatorChar,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'An OperationGate probe root escaped the workspace scratch directory.'
    }
    New-Item -ItemType Directory -Force -Path $fixtureRoot | Out-Null
}

$targetDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'target\operation-gate')
)
$executablePath = Join-Path $targetDirectory 'release\myalbuns-desktop.exe'
$executableRelativePath = 'target/operation-gate/release/myalbuns-desktop.exe'
$startedProcesses =
    [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
$checks = [System.Collections.Generic.List[object]]::new()
$previousCargoTarget = [System.Environment]::GetEnvironmentVariable(
    'CARGO_TARGET_DIR',
    [System.EnvironmentVariableTarget]::Process
)
$locationWasPushed = $false
$report = $null

function Set-OperationProbeEnvironmentValue {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,
        [AllowNull()]
        [string] $Value
    )

    [System.Environment]::SetEnvironmentVariable(
        $Name,
        $Value,
        [System.EnvironmentVariableTarget]::Process
    )
}

function Invoke-RustCheck {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,
        [Parameter(Mandatory = $true)]
        [string[]] $Arguments
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $commandOutput = @(& $script:CargoExecutable @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    $stopwatch.Stop()
    foreach ($line in $commandOutput) {
        Write-Host $line
    }
    if ($exitCode -ne 0) {
        throw "OperationGate check '$Name' failed with exit code $exitCode."
    }
    $transcript = $commandOutput -join [System.Environment]::NewLine
    if ($transcript -notmatch '(?m)^running [1-9][0-9]* tests?') {
        throw "OperationGate check '$Name' did not execute any test."
    }
    if (
        $transcript -notmatch
            '(?m)^test result: ok\. [1-9][0-9]* passed; 0 failed;'
    ) {
        throw "OperationGate check '$Name' did not pass any non-ignored test."
    }
    $checks.Add([ordered]@{
        name = $Name
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}

function Start-OperationProbeProcess {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot,
        [ValidateSet('a', 'b')]
        [string] $ProjectSlot
    )

    $environment = [ordered]@{
        MYALBUNS_PROCESS_ROLE = $null
        MYALBUNS_TOPOLOGY_SPIKE = $Topology
        MYALBUNS_TOPOLOGY_PROJECT = $ProjectSlot
        MYALBUNS_TOPOLOGY_RUN_ID = $null
        MYALBUNS_TOPOLOGY_CORPUS_MANIFEST = $null
        MYALBUNS_TOPOLOGY_PROBE_GATE = $null
        MYALBUNS_TOPOLOGY_EXPORT_GATE = $null
        MYALBUNS_TOPOLOGY_FAULT_GATE = $null
        MYALBUNS_TOPOLOGY_FAULT_OUTPUT_ROOT = $null
        MYALBUNS_TOPOLOGY_PROJECT_A_SOURCE = $null
        MYALBUNS_TOPOLOGY_PROJECT_B_SOURCE = $null
        MYALBUNS_GLOBAL_SPIKE_ENDPOINT = $null
        MYALBUNS_OPERATION_GATE_PROBE_ROOT = $ProbeRoot
    }
    if ($Topology -eq 'multiwindow') {
        $environment.MYALBUNS_TOPOLOGY_PROJECT = $null
    }

    $previous = @{}
    try {
        foreach ($entry in $environment.GetEnumerator()) {
            $previous[$entry.Key] =
                [System.Environment]::GetEnvironmentVariable(
                    $entry.Key,
                    [System.EnvironmentVariableTarget]::Process
                )
            Set-OperationProbeEnvironmentValue `
                -Name $entry.Key `
                -Value $entry.Value
        }

        $process = Start-Process `
            -FilePath $executablePath `
            -WorkingDirectory $script:WorkspaceRoot `
            -PassThru `
            -WindowStyle Hidden
        $startedProcesses.Add($process)
        return $process
    }
    finally {
        foreach ($entry in $previous.GetEnumerator()) {
            Set-OperationProbeEnvironmentValue `
                -Name $entry.Key `
                -Value $entry.Value
        }
    }
}

function Assert-OwnedOperationProbeProcess {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process
    )

    if ($Process.HasExited) {
        return $false
    }
    $processPath = $Process.MainModule.FileName
    if ([string]::IsNullOrWhiteSpace($processPath) -or -not [string]::Equals(
            [System.IO.Path]::GetFullPath($processPath),
            [System.IO.Path]::GetFullPath($executablePath),
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw (
            "Process $($Process.Id) does not belong to the " +
            'OperationGate probe executable.'
        )
    }
    return $true
}

function Stop-OwnedOperationProbeProcess {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process
    )

    $confirmedStopped = $false
    try {
        if ($Process.HasExited) {
            $confirmedStopped = $true
            return
        }
        if (Assert-OwnedOperationProbeProcess -Process $Process) {
            $Process.Kill()
            if (-not $Process.WaitForExit(10000)) {
                throw (
                    "OperationGate probe process $($Process.Id) " +
                    'did not terminate.'
                )
            }
            $confirmedStopped = $true
        }
    }
    finally {
        if (-not $confirmedStopped) {
            try {
                $confirmedStopped = $Process.HasExited
            }
            catch {
                $confirmedStopped = $false
            }
        }
        if ($confirmedStopped) {
            [void] $startedProcesses.Remove($Process)
            $Process.Dispose()
        }
    }
}

function Get-ProbeFailureDiagnostic {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    $failures = @(
        Get-ChildItem `
            -LiteralPath $ProbeRoot `
            -Filter 'failure-*.json' `
            -File `
            -ErrorAction SilentlyContinue |
            Sort-Object Name |
            ForEach-Object {
                try {
                    Get-Content -LiteralPath $_.FullName -Raw -Encoding utf8
                }
                catch {
                    "Unreadable failure file '$($_.Name)': $($_.Exception.Message)"
                }
            }
    )
    if ($failures.Count -eq 0) {
        return 'No typed probe failure file was produced.'
    }
    return $failures -join ' | '
}

function Read-StrictProbeEvent {
    param([Parameter(Mandatory = $true)][string] $Path)

    $json = [System.IO.File]::ReadAllText(
        $Path,
        [System.Text.Encoding]::UTF8
    )
    $document = [System.Text.Json.JsonDocument]::Parse($json)
    try {
        $root = $document.RootElement
        if ($root.ValueKind -ne [System.Text.Json.JsonValueKind]::Object) {
            throw 'The OperationGate event root must be a JSON object.'
        }
        $expectedNames = @(
            'schemaVersion',
            'processId',
            'topology',
            'windowLabel',
            'state',
            'operationMode'
        )
        $actualNames = @(
            $root.EnumerateObject() |
                ForEach-Object { $_.Name }
        )
        $missingNames = @(
            $expectedNames |
                Where-Object { $actualNames -cnotcontains $_ }
        )
        $unexpectedNames = @(
            $actualNames |
                Where-Object { $expectedNames -cnotcontains $_ }
        )
        if ($missingNames.Count -gt 0 -or $unexpectedNames.Count -gt 0) {
            throw (
                'The OperationGate event fields differ from the closed schema. ' +
                "Missing: $($missingNames -join ', '); " +
                "unexpected: $($unexpectedNames -join ', ')."
            )
        }

        $schemaElement = $root.GetProperty('schemaVersion')
        $processElement = $root.GetProperty('processId')
        foreach ($entry in @(
                [ordered]@{
                    name = 'schemaVersion'
                    value = $schemaElement
                },
                [ordered]@{
                    name = 'processId'
                    value = $processElement
                }
            )) {
            if (
                $entry.value.ValueKind -ne
                    [System.Text.Json.JsonValueKind]::Number
            ) {
                throw "OperationGate field '$($entry.name)' must be a number."
            }
        }
        [int] $schemaVersion = 0
        [int] $processId = 0
        if (-not $schemaElement.TryGetInt32([ref] $schemaVersion) -or
            -not $processElement.TryGetInt32([ref] $processId)) {
            throw 'OperationGate numeric fields must be 32-bit integers.'
        }

        $stringValues = [ordered]@{}
        foreach ($name in @(
                'topology',
                'windowLabel',
                'state',
                'operationMode'
            )) {
            $element = $root.GetProperty($name)
            if (
                $element.ValueKind -ne
                    [System.Text.Json.JsonValueKind]::String
            ) {
                throw "OperationGate field '$name' must be a string."
            }
            $stringValues[$name] = $element.GetString()
        }

        return [ordered]@{
            schemaVersion = $schemaVersion
            processId = $processId
            topology = $stringValues.topology
            windowLabel = $stringValues.windowLabel
            state = $stringValues.state
            operationMode = $stringValues.operationMode
        }
    }
    finally {
        $document.Dispose()
    }
}

function Wait-ForProbeEvent {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path,
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process[]] $Processes,
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($ProbeTimeoutSeconds)
    $lastReadError = $null
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            try {
                return Read-StrictProbeEvent -Path $Path
            }
            catch {
                $lastReadError = $_.Exception.Message
            }
        }
        foreach ($process in $Processes) {
            if ($process.HasExited) {
                $diagnostic = Get-ProbeFailureDiagnostic -ProbeRoot $ProbeRoot
                throw (
                    "OperationGate probe process $($process.Id) exited with " +
                    "code $($process.ExitCode) before '$Path' was produced. " +
                    "Probe diagnostic: $diagnostic"
                )
            }
        }
        Start-Sleep -Milliseconds 50
    }

    $diagnostic = Get-ProbeFailureDiagnostic -ProbeRoot $ProbeRoot
    throw (
        "Timed out waiting for OperationGate event '$Path'. " +
        "Last JSON read error: $lastReadError. Probe diagnostic: $diagnostic"
    )
}

function Assert-ProbeEvent {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Event,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProcessId,
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $ExpectedTopology,
        [Parameter(Mandatory = $true)]
        [ValidateSet('main', 'project-b')]
        [string] $ExpectedWindowLabel,
        [Parameter(Mandatory = $true)]
        [ValidateSet(
            'owner_ready',
            'challenger_conflict',
            'owner_released',
            'challenger_success'
        )]
        [string] $ExpectedState
    )

    if (
        [int] $Event.schemaVersion -ne 1 -or
        [long] $Event.processId -ne $ExpectedProcessId -or
        [string] $Event.topology -ne $ExpectedTopology -or
        [string] $Event.windowLabel -ne $ExpectedWindowLabel -or
        [string] $Event.state -ne $ExpectedState -or
        [string] $Event.operationMode -ne 'normal_export'
    ) {
        $actual = $Event | ConvertTo-Json -Depth 5 -Compress
        throw (
            "Invalid OperationGate event for state '$ExpectedState'. " +
            "Received: $actual"
        )
    }

    return [ordered]@{
        schemaVersion = 1
        processId = [int] $Event.processId
        topology = [string] $Event.topology
        windowLabel = [string] $Event.windowLabel
        state = [string] $Event.state
        operationMode = [string] $Event.operationMode
    }
}

function Open-OwnerReleaseGate {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    $path = Join-Path $ProbeRoot 'release-owner'
    $stream = [System.IO.File]::Open(
        $path,
        [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::Read
    )
    try {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes("release`n")
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
    return $path
}

function Invoke-OperationTopologyProbe {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot
    )

    $scenarioProcesses =
        [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
    $startedAt = [DateTimeOffset]::UtcNow
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        if ($Topology -eq 'independent') {
            $ownerProcess = Start-OperationProbeProcess `
                -Topology $Topology `
                -ProbeRoot $ProbeRoot `
                -ProjectSlot 'a'
            $scenarioProcesses.Add($ownerProcess)
            $ownerReadyRaw = Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-ready.json') `
                -Processes @($ownerProcess) `
                -ProbeRoot $ProbeRoot

            $challengerProcess = Start-OperationProbeProcess `
                -Topology $Topology `
                -ProbeRoot $ProbeRoot `
                -ProjectSlot 'b'
            $scenarioProcesses.Add($challengerProcess)
            $processes = @($ownerProcess, $challengerProcess)
            $ownerWindow = 'main'
            $challengerWindow = 'main'
        }
        else {
            $ownerProcess = Start-OperationProbeProcess `
                -Topology $Topology `
                -ProbeRoot $ProbeRoot
            $scenarioProcesses.Add($ownerProcess)
            $challengerProcess = $ownerProcess
            $processes = @($ownerProcess)
            $ownerWindow = 'main'
            $challengerWindow = 'project-b'
            $ownerReadyRaw = Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-ready.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot
        }

        $ownerReady = Assert-ProbeEvent `
            -Event $ownerReadyRaw `
            -ExpectedProcessId $ownerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $ownerWindow `
            -ExpectedState 'owner_ready'
        $challengerConflict = Assert-ProbeEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-conflict.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot) `
            -ExpectedProcessId $challengerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $challengerWindow `
            -ExpectedState 'challenger_conflict'

        $releasePath = Open-OwnerReleaseGate -ProbeRoot $ProbeRoot
        $ownerReleased = Assert-ProbeEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-released.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot) `
            -ExpectedProcessId $ownerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $ownerWindow `
            -ExpectedState 'owner_released'
        $challengerSuccess = Assert-ProbeEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-success.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot) `
            -ExpectedProcessId $challengerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $challengerWindow `
            -ExpectedState 'challenger_success'

        $stopwatch.Stop()
        return [ordered]@{
            topology = $Topology
            passed = $true
            startedAtUtc = $startedAt.ToString('o')
            elapsedMs = [long] $stopwatch.ElapsedMilliseconds
            processCount = $scenarioProcesses.Count
            processIds = @(
                $scenarioProcesses |
                    ForEach-Object { $_.Id }
            )
            releaseGateCreated = Test-Path -LiteralPath $releasePath -PathType Leaf
            ownerReady = $ownerReady
            challengerConflict = $challengerConflict
            ownerReleased = $ownerReleased
            challengerSuccess = $challengerSuccess
        }
    }
    finally {
        $stopwatch.Stop()
        foreach ($process in @($scenarioProcesses)) {
            Stop-OwnedOperationProbeProcess -Process $process
        }
    }
}

try {
    Set-OperationProbeEnvironmentValue `
        -Name 'CARGO_TARGET_DIR' `
        -Value $targetDirectory
    Push-Location $script:WorkspaceRoot
    $locationWasPushed = $true
    $initialBuildInputState = Get-BuildInputState

    $rustChecks = @(
        [ordered]@{
            name = 'operation-gate'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'operation_gate::tests::',
                '--',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'operation-lease'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'operation_lease::tests::',
                '--',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'operation-gate-probe-contract'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'operation_gate_probe::tests::',
                '--',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'cache-pause'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'cache_engine::tests::pause_waits_for_active_work_and_blocks_new_work_until_release',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'processor-reservation'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'imaging_processor::tests::processor_reservation_serializes_callers_and_is_released_with_its_guard',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'real-owner-process-death'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'operation_gate::tests::independent_hosts_share_one_grant_and_recover_after_owner_process_termination',
                '--',
                '--ignored',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'export-success-and-progress'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'export_pipeline::tests::a_verified_preparation_is_published_only_after_the_response_is_validated',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'export-failure'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'export_pipeline::tests::a_processor_crash_is_not_retried_and_preserves_the_previous_output',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'export-cancellation-before-start'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'export_pipeline::tests::cancellation_before_execution_creates_no_preparation_or_processor_work',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'export-cancellation-in-flight'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'export_pipeline::tests::cancellation_during_processing_discards_preparation_and_preserves_previous_output',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'processor-progress-stream'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'imaging_processor::tests::host_reports_chunked_progress_before_decoding_the_final_response',
                '--',
                '--exact',
                '--nocapture'
            )
        }
    )
    foreach ($check in $rustChecks) {
        Invoke-RustCheck `
            -Name $check.name `
            -Arguments @($check.arguments)
    }

    $buildStartedAt = [DateTimeOffset]::UtcNow
    $buildStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    & (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1') build --no-bundle
    $buildExitCode = $LASTEXITCODE
    $buildStopwatch.Stop()
    if ($buildExitCode -ne 0) {
        throw "The real desktop build failed with exit code $buildExitCode."
    }
    if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
        throw "The real desktop executable was not produced at '$executablePath'."
    }
    $checks.Add([ordered]@{
        name = 'real-desktop-build'
        passed = $true
        elapsedMs = [long] $buildStopwatch.ElapsedMilliseconds
    })

    $buildInputState = Get-BuildInputState
    if (
        $buildInputState.fileCount -ne $initialBuildInputState.fileCount -or
        $buildInputState.digestSha256 -ne
            $initialBuildInputState.digestSha256 -or
        $buildInputState.dirty -ne $initialBuildInputState.dirty
    ) {
        throw (
            'OperationGate source inputs changed during tests or build; ' +
            'the executable cannot be tied to one source state.'
        )
    }
    $workingTreeStatus = @(
        & git -C $script:WorkspaceRoot status --short
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not inspect the OperationGate working tree.'
    }
    $gitCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $gitCommit -notmatch '^[0-9a-f]{40}$') {
        throw 'Could not identify the OperationGate source commit.'
    }
    $build = [ordered]@{
        builtAtUtc = $buildStartedAt.ToString('o')
        elapsedMs = [long] $buildStopwatch.ElapsedMilliseconds
        profile = 'release'
        executable = $executableRelativePath
        executableBytes = [long] (
            Get-Item -LiteralPath $executablePath
        ).Length
        executableSha256 = (
            Get-FileHash -LiteralPath $executablePath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        buildInputFileCount = $buildInputState.fileCount
        buildInputDigestSha256 = $buildInputState.digestSha256
        buildInputsDirty = $buildInputState.dirty
        workingTreeDirty = $workingTreeStatus.Count -gt 0
    }

    $independent = Invoke-OperationTopologyProbe `
        -Topology 'independent' `
        -ProbeRoot $independentRoot
    $multiwindow = Invoke-OperationTopologyProbe `
        -Topology 'multiwindow' `
        -ProbeRoot $multiwindowRoot
    $checks.Add([ordered]@{
        name = 'independent-two-host-operation-gate'
        passed = $independent.passed
        elapsedMs = $independent.elapsedMs
    })
    $checks.Add([ordered]@{
        name = 'multiwindow-two-window-operation-gate'
        passed = $multiwindow.passed
        elapsedMs = $multiwindow.elapsedMs
    })

    $report = [ordered]@{
        schemaVersion = 1
        collectedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
        gitCommit = $gitCommit
        sourceInputsDirty = $buildInputState.dirty
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture = (
                [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
            )
        }
        build = $build
        checks = @($checks)
        results = [ordered]@{
            independent = $independent
            multiwindow = $multiwindow
        }
        limits = [ordered]@{
            batchRunner = $false
            multiOutputPromotion = $false
            projectOpenGuardian = $false
            exportCancellationEntryPoint = $false
            progressWindow = $false
        }
    }
}
finally {
    $cleanupErrors = [System.Collections.Generic.List[string]]::new()
    foreach ($process in @($startedProcesses)) {
        try {
            Stop-OwnedOperationProbeProcess -Process $process
        }
        catch {
            $cleanupErrors.Add($_.Exception.Message)
        }
    }
    if ($locationWasPushed) {
        try {
            Pop-Location
        }
        catch {
            $cleanupErrors.Add($_.Exception.Message)
        }
    }
    try {
        Set-OperationProbeEnvironmentValue `
            -Name 'CARGO_TARGET_DIR' `
            -Value $previousCargoTarget
    }
    catch {
        $cleanupErrors.Add($_.Exception.Message)
    }

    if ($cleanupErrors.Count -eq 0) {
        foreach ($fixtureRoot in $fixtureRoots) {
            try {
                if (Test-Path -LiteralPath $fixtureRoot) {
                    $verifiedRoot = [System.IO.Path]::GetFullPath($fixtureRoot)
                    if (-not $verifiedRoot.StartsWith(
                            $probeParent + [System.IO.Path]::DirectorySeparatorChar,
                            [System.StringComparison]::OrdinalIgnoreCase
                        )) {
                        throw 'Refusing to remove an unverified OperationGate probe root.'
                    }
                    Remove-Item -LiteralPath $verifiedRoot -Recurse -Force
                }
            }
            catch {
                $cleanupErrors.Add($_.Exception.Message)
            }
        }
    }
    if ($cleanupErrors.Count -gt 0) {
        throw (
            'OperationGate cleanup failed: ' +
            ($cleanupErrors -join ' | ')
        )
    }
}

if ($null -eq $report) {
    throw 'The OperationGate gate did not produce a report.'
}

$outputDirectory = Split-Path -Parent $OutputPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$json = $report | ConvertTo-Json -Depth 10
[System.IO.File]::WriteAllText(
    $OutputPath,
    $json + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)
Write-Output "OperationGate report: $OutputPath"
Write-Output $json
