$script:ProjectOpenGateScenarios = @('normal_close', 'owner_death')

function New-ProjectOpenGateContext {
    param(
        [Parameter(Mandatory = $true)][string] $ProbeParent,
        [Parameter(Mandatory = $true)][string] $RunId
    )

    $roots = [ordered]@{}
    foreach ($scenario in $script:ProjectOpenGateScenarios) {
        $roots[$scenario] = [System.IO.Path]::GetFullPath(
            (Join-Path $ProbeParent "run-$RunId-project-open-$scenario")
        )
    }
    return [pscustomobject]@{
        Scenarios = @($script:ProjectOpenGateScenarios)
        Roots = $roots
        FixtureRoots = @(
            $script:ProjectOpenGateScenarios |
                ForEach-Object { $roots[$_] }
        )
    }
}

function Get-ProjectOpenProbeEnvironmentNames {
    return @(
        'MYALBUNS_PROJECT_OPEN_PROBE_ROOT',
        'MYALBUNS_PROJECT_OPEN_PROBE_FILE',
        'MYALBUNS_PROJECT_OPEN_PROBE_SCENARIO'
    )
}

function Get-ProjectOpenProbeEnvironment {
    param(
        [Parameter(Mandatory = $true)][string] $ProbeRoot,
        [Parameter(Mandatory = $true)][string] $Scenario,
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology
    )

    Assert-ProjectOpenScenarioName -Scenario $Scenario
    if ($Topology -cne 'independent') {
        throw 'The Project opening probe supports only independent hosts.'
    }
    return [ordered]@{
        MYALBUNS_PROJECT_OPEN_PROBE_ROOT = $ProbeRoot
        MYALBUNS_PROJECT_OPEN_PROBE_FILE =
            (Join-Path $ProbeRoot 'Projeto.myalbum')
        MYALBUNS_PROJECT_OPEN_PROBE_SCENARIO = $Scenario
    }
}

function Assert-ProjectOpenScenarioName {
    param([Parameter(Mandatory = $true)][string] $Scenario)

    if ($script:ProjectOpenGateScenarios -cnotcontains $Scenario) {
        throw "Unknown Project opening scenario '$Scenario'."
    }
}

function Read-StrictProjectOpenEvent {
    param([Parameter(Mandatory = $true)][string] $Path)

    $expectedNames = @(
        'schemaVersion',
        'processId',
        'role',
        'scenario',
        'state',
        'operationMode',
        'operationGateState',
        'projectFileLockState'
    )
    $root = Read-StrictJsonObject `
        -Path $Path `
        -SchemaLabel 'The Project opening' `
        -ExpectedNames $expectedNames

    foreach ($name in @('schemaVersion', 'processId')) {
        $value = $root.$name
        if ($value -isnot [int] -and $value -isnot [long]) {
            throw "Project opening field '$name' must be an integer."
        }
        if ($value -lt [int]::MinValue -or $value -gt [int]::MaxValue) {
            throw "Project opening field '$name' must be a 32-bit integer."
        }
    }
    foreach ($name in @(
            'role',
            'scenario',
            'state',
            'operationMode',
            'operationGateState',
            'projectFileLockState'
        )) {
        if ($root.$name -isnot [string]) {
            throw "Project opening field '$name' must be a string."
        }
    }

    return [ordered]@{
        schemaVersion = [int] $root.schemaVersion
        processId = [int] $root.processId
        role = [string] $root.role
        scenario = [string] $root.scenario
        state = [string] $root.state
        operationMode = [string] $root.operationMode
        operationGateState = [string] $root.operationGateState
        projectFileLockState = [string] $root.projectFileLockState
    }
}

function Test-StrictProjectOpenEventParser {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $path = Join-Path $ProbeRoot 'project-open-parser-contract.json'
    $valid = @'
{"schemaVersion":1,"processId":42,"role":"owner","scenario":"owner_death","state":"owner_both_reheld","operationMode":"normal_export","operationGateState":"held","projectFileLockState":"held"}
'@
    [System.IO.File]::WriteAllText(
        $path,
        $valid.Trim(),
        [System.Text.UTF8Encoding]::new($false)
    )
    $event = Read-StrictProjectOpenEvent -Path $path
    if (
        $event.schemaVersion -ne 1 -or
        $event.processId -ne 42 -or
        $event.state -cne 'owner_both_reheld' -or
        $event.projectFileLockState -cne 'held'
    ) {
        throw 'The Project opening parser changed a valid event.'
    }

    $invalidCases = @(
        [ordered]@{
            label = 'duplicate field'
            json = $valid.Replace(
                '"schemaVersion":1',
                '"schemaVersion":1,"schema\u0056ersion":2'
            )
            error = '^The OperationGate event repeats the JSON field'
        },
        [ordered]@{
            label = 'unexpected field'
            json = $valid.Replace(
                '"projectFileLockState":"held"}',
                '"projectFileLockState":"held","unexpected":true}'
            )
            error = '^The Project opening event fields differ from the closed schema'
        },
        [ordered]@{
            label = 'mis-cased field'
            json = $valid.Replace('"operationGateState":', '"OperationGateState":')
            error = '^The Project opening event fields differ from the closed schema'
        }
    )
    foreach ($invalidCase in $invalidCases) {
        [System.IO.File]::WriteAllText(
            $path,
            [string] $invalidCase.json,
            [System.Text.UTF8Encoding]::new($false)
        )
        try {
            [void] (Read-StrictProjectOpenEvent -Path $path)
            throw (
                "The Project opening parser accepted $($invalidCase.label)."
            )
        }
        catch {
            if ($_.Exception.Message -cnotmatch $invalidCase.error) {
                throw
            }
        }
    }

    $stopwatch.Stop()
    $checks.Add([ordered]@{
        name = 'strict-project-open-json-parser'
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}

function Assert-ProjectOpenEvent {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Event,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProcessId,
        [Parameter(Mandatory = $true)]
        [ValidateSet('normal_close', 'owner_death')]
        [string] $ExpectedScenario,
        [Parameter(Mandatory = $true)]
        [ValidateSet(
            'owner_both_held',
            'challenger_both_conflict',
            'owner_gate_released_lock_held',
            'challenger_gate_held_lock_conflict',
            'owner_both_reheld',
            'owner_session_closed',
            'challenger_both_recovered'
        )]
        [string] $ExpectedState
    )

    Assert-ProjectOpenScenarioName -Scenario $ExpectedScenario
    $expected = switch ($ExpectedState) {
        'owner_both_held' {
            [ordered]@{ role = 'owner'; gate = 'held'; lock = 'held' }
        }
        'challenger_both_conflict' {
            [ordered]@{
                role = 'challenger'
                gate = 'conflict'
                lock = 'conflict'
            }
        }
        'owner_gate_released_lock_held' {
            [ordered]@{ role = 'owner'; gate = 'released'; lock = 'held' }
        }
        'challenger_gate_held_lock_conflict' {
            [ordered]@{
                role = 'challenger'
                gate = 'held'
                lock = 'conflict'
            }
        }
        'owner_both_reheld' {
            [ordered]@{ role = 'owner'; gate = 'held'; lock = 'held' }
        }
        'owner_session_closed' {
            [ordered]@{
                role = 'owner'
                gate = 'released'
                lock = 'released'
            }
        }
        'challenger_both_recovered' {
            [ordered]@{
                role = 'challenger'
                gate = 'recovered'
                lock = 'recovered'
            }
        }
    }
    if (
        $ExpectedState -ceq 'owner_session_closed' -and
        $ExpectedScenario -cne 'normal_close'
    ) {
        throw 'Only normal_close may report a cooperative Project session close.'
    }
    if (
        [int] $Event.schemaVersion -ne 1 -or
        [long] $Event.processId -ne $ExpectedProcessId -or
        [string] $Event.role -cne $expected.role -or
        [string] $Event.scenario -cne $ExpectedScenario -or
        [string] $Event.state -cne $ExpectedState -or
        [string] $Event.operationMode -cne 'normal_export' -or
        [string] $Event.operationGateState -cne $expected.gate -or
        [string] $Event.projectFileLockState -cne $expected.lock
    ) {
        $actual = $Event | ConvertTo-Json -Depth 5 -Compress
        throw (
            "Invalid Project opening event for state '$ExpectedState'. " +
            "Received: $actual"
        )
    }

    return [ordered]@{
        schemaVersion = 1
        processId = [int] $Event.processId
        role = [string] $Event.role
        scenario = [string] $Event.scenario
        state = [string] $Event.state
        operationMode = [string] $Event.operationMode
        operationGateState = [string] $Event.operationGateState
        projectFileLockState = [string] $Event.projectFileLockState
    }
}

function New-ProjectOpenFixture {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    $path = Join-Path $ProbeRoot 'Projeto.myalbum'
    $source = @'
{"schemaVersion":3,"projectId":"project-open-probe","projectName":"Projeto do probe de abertura","revision":0,"album":{"sheets":[{"id":"lamina-01","number":1,"role":"initial","widthUm":600000,"heightUm":300000,"frames":[],"overlayMediaId":null},{"id":"lamina-02","number":2,"role":"final","widthUm":600000,"heightUm":300000,"frames":[],"overlayMediaId":null}],"media":[]}}
'@
    $payload = [System.Text.UTF8Encoding]::new($false).GetBytes(
        $source.Trim()
    )
    $stream = [System.IO.File]::Open(
        $path,
        [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::None
    )
    try {
        $stream.Write($payload, 0, $payload.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
    return $path
}

function Get-ProjectOpenFixtureEvidence {
    param([Parameter(Mandatory = $true)][string] $Path)

    $item = Get-Item -LiteralPath $Path
    if (
        -not $item.PSIsContainer -and
        ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0 -and
        $item.Length -gt 0
    ) {
        return [ordered]@{
            name = $item.Name
            bytes = [long] $item.Length
            sha256 = (
                Get-FileHash -LiteralPath $Path -Algorithm SHA256
            ).Hash.ToLowerInvariant()
        }
    }
    throw 'The Project opening fixture is not a non-empty regular file.'
}

function Assert-ProjectOpenFixtureUnchanged {
    param(
        [Parameter(Mandatory = $true)][object] $Before,
        [Parameter(Mandatory = $true)][object] $After
    )

    if (
        [string] $Before.name -cne [string] $After.name -or
        [long] $Before.bytes -ne [long] $After.bytes -or
        [string] $Before.sha256 -cne [string] $After.sha256
    ) {
        throw 'The Project opening probe changed the Project fixture.'
    }
}

function Assert-ProjectOpenOwnerAlive {
    param([Parameter(Mandatory = $true)][System.Diagnostics.Process] $Process)

    if (-not (Assert-OwnedOperationProbeProcess -Process $Process)) {
        throw 'The Project opening owner exited during an intermediate phase.'
    }
}

function Invoke-ProjectOpenScenario {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('normal_close', 'owner_death')]
        [string] $Scenario,
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot
    )

    Assert-ProjectOpenScenarioName -Scenario $Scenario
    $scenarioProcesses =
        [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
    $startedAt = [DateTimeOffset]::UtcNow
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $ownerTerminated = $false
    $ownerAliveAfterSessionClose = $false
    $ownerSessionClosed = $null
    try {
        $projectFile = New-ProjectOpenFixture -ProbeRoot $ProbeRoot
        $fixtureBefore = Get-ProjectOpenFixtureEvidence -Path $projectFile

        $ownerProcess = Start-OperationProbeProcess `
            -Topology 'independent' `
            -ProbeRoot $ProbeRoot `
            -ProjectSlot 'a' `
            -ProjectOpenScenario $Scenario
        $ownerProcessId = $ownerProcess.Id
        $scenarioProcesses.Add($ownerProcess)
        $ownerBothHeld = Assert-ProjectOpenEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-both-held.json') `
                -Processes @($ownerProcess) `
                -ProbeRoot $ProbeRoot `
                -Contract 'project_open') `
            -ExpectedProcessId $ownerProcessId `
            -ExpectedScenario $Scenario `
            -ExpectedState 'owner_both_held'
        Assert-ProjectOpenOwnerAlive -Process $ownerProcess

        $challengerProcess = Start-OperationProbeProcess `
            -Topology 'independent' `
            -ProbeRoot $ProbeRoot `
            -ProjectSlot 'b' `
            -ProjectOpenScenario $Scenario
        $challengerProcessId = $challengerProcess.Id
        $scenarioProcesses.Add($challengerProcess)
        $bothProcesses = @($ownerProcess, $challengerProcess)
        $challengerBothConflict = Assert-ProjectOpenEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-both-conflict.json') `
                -Processes $bothProcesses `
                -ProbeRoot $ProbeRoot `
                -Contract 'project_open') `
            -ExpectedProcessId $challengerProcessId `
            -ExpectedScenario $Scenario `
            -ExpectedState 'challenger_both_conflict'
        Assert-ProjectOpenOwnerAlive -Process $ownerProcess

        $releaseGatePath = Open-ProbeMarker `
            -ProbeRoot $ProbeRoot `
            -Name 'release-owner-gate'
        $ownerGateReleased = Assert-ProjectOpenEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-gate-released-lock-held.json') `
                -Processes $bothProcesses `
                -ProbeRoot $ProbeRoot `
                -Contract 'project_open') `
            -ExpectedProcessId $ownerProcessId `
            -ExpectedScenario $Scenario `
            -ExpectedState 'owner_gate_released_lock_held'
        Assert-ProjectOpenOwnerAlive -Process $ownerProcess

        $challengerSplit = Assert-ProjectOpenEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-gate-held-lock-conflict.json') `
                -Processes $bothProcesses `
                -ProbeRoot $ProbeRoot `
                -Contract 'project_open') `
            -ExpectedProcessId $challengerProcessId `
            -ExpectedScenario $Scenario `
            -ExpectedState 'challenger_gate_held_lock_conflict'
        Assert-ProjectOpenOwnerAlive -Process $ownerProcess

        $ownerBothReheld = Assert-ProjectOpenEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-both-reheld.json') `
                -Processes $bothProcesses `
                -ProbeRoot $ProbeRoot `
                -Contract 'project_open') `
            -ExpectedProcessId $ownerProcessId `
            -ExpectedScenario $Scenario `
            -ExpectedState 'owner_both_reheld'
        Assert-ProjectOpenOwnerAlive -Process $ownerProcess

        if ($Scenario -ceq 'normal_close') {
            $terminalMarkerPath = Open-ProbeMarker `
                -ProbeRoot $ProbeRoot `
                -Name 'close-owner-session'
            $ownerSessionClosed = Assert-ProjectOpenEvent `
                -Event (Wait-ForProbeEvent `
                    -Path (Join-Path $ProbeRoot 'owner-session-closed.json') `
                    -Processes $bothProcesses `
                    -ProbeRoot $ProbeRoot `
                    -Contract 'project_open') `
                -ExpectedProcessId $ownerProcessId `
                -ExpectedScenario $Scenario `
                -ExpectedState 'owner_session_closed'
            Assert-ProjectOpenOwnerAlive -Process $ownerProcess
            $ownerAliveAfterSessionClose = $true
            $recoveryProcesses = $bothProcesses
        }
        else {
            if (Test-Path -LiteralPath (Join-Path $ProbeRoot 'terminate-owner')) {
                throw 'The owner-death scenario requested a cooperative termination.'
            }
            Stop-OwnedOperationProbeProcess `
                -Process $ownerProcess `
                -RequireLiveKill
            [void] $scenarioProcesses.Remove($ownerProcess)
            $ownerTerminated = $true
            $terminalMarkerPath = Open-ProbeMarker `
                -ProbeRoot $ProbeRoot `
                -Name 'owner-terminated'
            $recoveryProcesses = @($challengerProcess)
        }

        $challengerRecovered = Assert-ProjectOpenEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-both-recovered.json') `
                -Processes $recoveryProcesses `
                -ProbeRoot $ProbeRoot `
                -Contract 'project_open') `
            -ExpectedProcessId $challengerProcessId `
            -ExpectedScenario $Scenario `
            -ExpectedState 'challenger_both_recovered'
        if ($Scenario -ceq 'normal_close') {
            Assert-ProjectOpenOwnerAlive -Process $ownerProcess
        }

        $fixtureAfter = Get-ProjectOpenFixtureEvidence -Path $projectFile
        Assert-ProjectOpenFixtureUnchanged `
            -Before $fixtureBefore `
            -After $fixtureAfter

        $stopwatch.Stop()
        return [ordered]@{
            topology = 'independent'
            scenario = $Scenario
            passed = $true
            startedAtUtc = $startedAt.ToString('o')
            elapsedMs = [long] $stopwatch.ElapsedMilliseconds
            ownerProcessId = $ownerProcessId
            challengerProcessId = $challengerProcessId
            ownerAliveDuringIntermediatePhases = $true
            ownerAliveAfterSessionClose = $ownerAliveAfterSessionClose
            ownerTerminationConfirmed = $ownerTerminated
            isolatedGateReleaseCreated = Test-Path `
                -LiteralPath $releaseGatePath `
                -PathType Leaf
            terminalMarkerCreated = Test-Path `
                -LiteralPath $terminalMarkerPath `
                -PathType Leaf
            fixtureBefore = $fixtureBefore
            fixtureAfter = $fixtureAfter
            ownerBothHeld = $ownerBothHeld
            challengerBothConflict = $challengerBothConflict
            ownerGateReleasedLockHeld = $ownerGateReleased
            challengerGateHeldLockConflict = $challengerSplit
            ownerBothReheld = $ownerBothReheld
            ownerSessionClosed = $ownerSessionClosed
            challengerBothRecovered = $challengerRecovered
        }
    }
    finally {
        $stopwatch.Stop()
        foreach ($process in @($scenarioProcesses)) {
            Stop-OwnedOperationProbeProcess -Process $process
        }
    }
}

function Get-ProjectOpenGateRustChecks {
    return @(
        [ordered]@{
            name = 'project-file-lock'
            arguments = @(
                'test',
                '-p',
                'myalbuns-paths',
                'project_file_lock::tests::',
                '--',
                '--nocapture'
            )
        },
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
            name = 'project-open-probe-contract'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'project_open_probe::tests::',
                '--',
                '--nocapture'
            )
        }
    )
}

function Invoke-ProjectOpenGateSuite {
    param(
        [Parameter(Mandatory = $true)][object] $Context,
        [Parameter(Mandatory = $true)][object] $Build,
        [Parameter(Mandatory = $true)][string] $GitCommit,
        [Parameter(Mandatory = $true)][object] $BuildInputState,
        [Parameter(Mandatory = $true)][object] $Checks
    )

    $results = [ordered]@{}
    foreach ($scenario in $Context.Scenarios) {
        $result = Invoke-ProjectOpenScenario `
            -Scenario $scenario `
            -ProbeRoot $Context.Roots[$scenario]
        $results[$scenario] = $result
        $Checks.Add([ordered]@{
            name = "project-open-$scenario"
            passed = $result.passed
            elapsedMs = $result.elapsedMs
        })
    }

    $finalBuildInputState = Get-BuildInputState
    if (
        $finalBuildInputState.fileCount -ne $BuildInputState.fileCount -or
        $finalBuildInputState.digestSha256 -cne
            $BuildInputState.digestSha256 -or
        $finalBuildInputState.dirty -ne $BuildInputState.dirty
    ) {
        throw (
            'Project opening probes changed source inputs after the build; ' +
            'the evidence cannot be tied to one source state.'
        )
    }

    return [ordered]@{
        schemaVersion = 1
        suite = 'operation_gate_project_file_lock'
        collectedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
        gitCommit = $GitCommit
        sourceInputsDirty = $finalBuildInputState.dirty
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture = (
                [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
            )
            powerShellEdition = [string] $PSVersionTable.PSEdition
            powerShellVersion = [string] $PSVersionTable.PSVersion
        }
        build = $Build
        checks = @($Checks)
        results = [ordered]@{
            topology = 'independent_two_host'
            scenarios = $results
        }
        limits = [ordered]@{
            operationGate = $true
            projectFileLock = $true
            mechanismsSeparated = $true
            normalSessionClose = $true
            ownerProcessDeath = $true
            physicalIdentityComparison = $false
            existingSessionFocus = $false
            mappedUncAlias = $false
            completeProjectOpenGuardian = $false
        }
    }
}
