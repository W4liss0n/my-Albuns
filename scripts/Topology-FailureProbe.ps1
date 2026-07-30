function Get-AvailableTopologyEndpoint {
    $reservation = [System.Net.Sockets.TcpListener]::new(
        [System.Net.IPAddress]::Loopback,
        0
    )
    try {
        $reservation.Start()
        return [System.Net.IPEndPoint] $reservation.LocalEndpoint
    }
    finally {
        $reservation.Stop()
    }
}

function Start-TopologyGlobalProcess {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [System.Net.IPEndPoint] $Endpoint,
        [switch] $DoNotTrack
    )

    $environmentNames = @(
        $processRoleEnvironment,
        $topologyEnvironment,
        $topologyRunIdEnvironment,
        $globalEndpointEnvironment,
        $projectSlotEnvironment
    )
    $previousValues = @{}
    foreach ($name in $environmentNames) {
        $previousValues[$name] = [System.Environment]::GetEnvironmentVariable(
            $name,
            [System.EnvironmentVariableTarget]::Process
        )
    }

    try {
        Set-ProcessEnvironmentValue `
            -Name $processRoleEnvironment `
            -Value 'global'
        Set-ProcessEnvironmentValue -Name $topologyEnvironment -Value $Topology
        Set-ProcessEnvironmentValue -Name $topologyRunIdEnvironment -Value $RunId
        Set-ProcessEnvironmentValue `
            -Name $globalEndpointEnvironment `
            -Value $Endpoint.ToString()
        Set-ProcessEnvironmentValue -Name $projectSlotEnvironment -Value $null

        $process = Start-Process `
            -FilePath $executablePath `
            -WorkingDirectory $script:WorkspaceRoot `
            -PassThru
        if (-not $DoNotTrack) {
            $startedProcessIds.Add($process.Id)
        }
        return $process
    }
    finally {
        foreach ($name in $environmentNames) {
            Set-ProcessEnvironmentValue `
                -Name $name `
                -Value $previousValues[$name]
        }
    }
}

function Invoke-TopologyGlobalStatus {
    param(
        [Parameter(Mandatory = $true)]
        [System.Net.IPEndPoint] $Endpoint,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [string] $ProbeId,
        [ValidateRange(100, 10000)]
        [int] $TimeoutMilliseconds = 2000
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $client = [System.Net.Sockets.TcpClient]::new()
    $stream = $null
    $writer = $null
    $reader = $null
    try {
        $connect = $client.ConnectAsync($Endpoint.Address, $Endpoint.Port)
        if (-not $connect.Wait($TimeoutMilliseconds)) {
            throw "Timed out connecting to global endpoint $Endpoint."
        }
        [void] $connect.GetAwaiter().GetResult()
        $stream = $client.GetStream()
        $stream.ReadTimeout = $TimeoutMilliseconds
        $stream.WriteTimeout = $TimeoutMilliseconds
        $encoding = [System.Text.UTF8Encoding]::new($false)
        $writer = [System.IO.StreamWriter]::new(
            $stream,
            $encoding,
            1024,
            $true
        )
        $writer.NewLine = "`n"
        $reader = [System.IO.StreamReader]::new(
            $stream,
            $encoding,
            $false,
            1024,
            $true
        )
        $request = [ordered]@{
            kind = 'status'
            runId = $RunId
            probeId = $ProbeId
        } | ConvertTo-Json -Compress
        $writer.WriteLine($request)
        $writer.Flush()
        $responseLine = $reader.ReadLine()
        if ([string]::IsNullOrWhiteSpace($responseLine)) {
            throw 'The global process closed the status channel without a response.'
        }
        $response = $responseLine | ConvertFrom-Json
        if ($response.kind -eq 'error') {
            throw "Global status rejected the request with code '$($response.code)'."
        }
        if (
            $response.kind -ne 'status' -or
            $response.runId -ne $RunId -or
            $response.probeId -ne $ProbeId -or
            $response.topology -notin @('independent', 'multiwindow') -or
            [long]$response.processId -le 0
        ) {
            throw 'The global status response is invalid or not correlated.'
        }
        $stopwatch.Stop()
        return [ordered]@{
            available = $true
            processId = [int] $response.processId
            runId = [string] $response.runId
            topology = [string] $response.topology
            probeId = [string] $response.probeId
            roundTripMs = [double] $stopwatch.Elapsed.TotalMilliseconds
        }
    }
    finally {
        $stopwatch.Stop()
        if ($null -ne $reader) {
            $reader.Dispose()
        }
        if ($null -ne $writer) {
            $writer.Dispose()
        }
        if ($null -ne $stream) {
            $stream.Dispose()
        }
        $client.Dispose()
    }
}

function Wait-ForTopologyGlobalStatus {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process,
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [System.Net.IPEndPoint] $Endpoint,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [string] $ProbeId
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($WindowTimeoutSeconds)
    $lastReason = $null
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if ($Process.HasExited) {
            throw (
                "Global process $($Process.Id) exited with code " +
                "$($Process.ExitCode) before status became available."
            )
        }
        try {
            $status = Invoke-TopologyGlobalStatus `
                -Endpoint $Endpoint `
                -RunId $RunId `
                -ProbeId $ProbeId
            if ($status.processId -ne $Process.Id) {
                throw (
                    "Global endpoint belongs to PID $($status.processId), " +
                    "not the launched PID $($Process.Id)."
                )
            }
            if ($status.topology -ne $Topology) {
                throw (
                    "Global endpoint reported topology '$($status.topology)', " +
                    "not '$Topology'."
                )
            }
            return $status
        }
        catch {
            $lastReason = $_.Exception.Message
            Start-Sleep -Milliseconds 100
        }
    }
    throw (
        "Global process $($Process.Id) did not expose its correlated status " +
        "within $WindowTimeoutSeconds seconds. Last error: $lastReason"
    )
}

function Confirm-TopologyGlobalUnavailable {
    param(
        [Parameter(Mandatory = $true)]
        [System.Net.IPEndPoint] $Endpoint,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [string] $ProbeId
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $unexpected = Invoke-TopologyGlobalStatus `
            -Endpoint $Endpoint `
            -RunId $RunId `
            -ProbeId $ProbeId `
            -TimeoutMilliseconds 500
        throw (
            "Global operations unexpectedly remained available from PID " +
            "$($unexpected.processId)."
        )
    }
    catch {
        if ($_.Exception.Message.StartsWith(
            'Global operations unexpectedly remained available'
        )) {
            throw
        }
        $stopwatch.Stop()
        return [ordered]@{
            available = $false
            probeId = $ProbeId
            observedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
            elapsedMs = [double] $stopwatch.Elapsed.TotalMilliseconds
            failureType = $_.Exception.GetType().Name
        }
    }
}

function Confirm-TopologyGlobalSingleton {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [System.Net.IPEndPoint] $Endpoint,
        [Parameter(Mandatory = $true)]
        [int] $OwnerProcessId,
        [Parameter(Mandatory = $true)]
        [string] $ProbeId
    )

    $before = Invoke-TopologyGlobalStatus `
        -Endpoint $Endpoint `
        -RunId $RunId `
        -ProbeId "$ProbeId-before"
    if (
        $before.processId -ne $OwnerProcessId -or
        $before.topology -ne $Topology
    ) {
        throw 'The singleton endpoint changed owner before the duplicate launch.'
    }
    $duplicate = Start-TopologyGlobalProcess `
        -Topology $Topology `
        -RunId $RunId `
        -Endpoint $Endpoint `
        -DoNotTrack
    if (-not $duplicate.WaitForExit(10000)) {
        Invoke-TopologyProcessCrash `
            -ProcessId $duplicate.Id `
            -Role 'unexpected_global_duplicate' | Out-Null
        throw 'The duplicate global process did not reject itself.'
    }
    if ($duplicate.ExitCode -ne 73) {
        throw (
            "The duplicate global process exited with $($duplicate.ExitCode); " +
            'expected the stable singleton rejection code 73.'
        )
    }
    $after = Invoke-TopologyGlobalStatus `
        -Endpoint $Endpoint `
        -RunId $RunId `
        -ProbeId "$ProbeId-after"
    if (
        $after.processId -ne $OwnerProcessId -or
        $after.topology -ne $Topology
    ) {
        throw 'The duplicate launch displaced the original global process.'
    }
    return [ordered]@{
        ownerProcessId = $OwnerProcessId
        rejectedProcessId = $duplicate.Id
        rejectedExitCode = $duplicate.ExitCode
        ownerPreserved = $true
        probeId = $ProbeId
    }
}

function Invoke-TopologyProcessCrash {
    param(
        [Parameter(Mandatory = $true)]
        [int] $ProcessId,
        [Parameter(Mandatory = $true)]
        [string] $Role
    )

    Assert-OwnedTopologyProcess -ProcessId $ProcessId
    $process = Get-Process -Id $ProcessId -ErrorAction Stop
    $observedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    if (-not $process.HasExited) {
        $process.Kill()
    }
    $exitObserved = $process.WaitForExit(30000)
    $stopwatch.Stop()
    if (-not $exitObserved -or -not $process.HasExited) {
        throw "The validated $Role process $ProcessId survived forced termination."
    }
    [void] $startedProcessIds.Remove($ProcessId)
    return [ordered]@{
        role = $Role
        processId = $ProcessId
        executable = $executableRelativePath
        executableValidated = $true
        requestedAtUtc = $observedAtUtc
        exitObserved = $true
        exitObservationMs = [long] $stopwatch.ElapsedMilliseconds
    }
}

function Set-TopologyFaultGate {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [string] $ProbeId,
        [Parameter(Mandatory = $true)]
        [bool] $ExpectedGlobalAvailable
    )

    $gateRoot = [System.IO.Path]::GetFullPath($probeGateDirectory) +
        [System.IO.Path]::DirectorySeparatorChar
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith(
        $gateRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw 'The topology fault gate is outside its dedicated directory.'
    }
    New-Item -ItemType Directory -Force -Path $probeGateDirectory | Out-Null
    $temporaryPath = "$fullPath.$([Guid]::NewGuid().ToString('N')).tmp"
    try {
        $json = [ordered]@{
            runId = $RunId
            probeId = $ProbeId
            expectedGlobalAvailable = $ExpectedGlobalAvailable
        } | ConvertTo-Json -Compress
        [System.IO.File]::WriteAllText(
            $temporaryPath,
            $json + [System.Environment]::NewLine,
            [System.Text.UTF8Encoding]::new($false)
        )
        if ([System.IO.File]::Exists($fullPath)) {
            [System.IO.File]::Replace(
                $temporaryPath,
                $fullPath,
                $null
            )
        }
        else {
            [System.IO.File]::Move($temporaryPath, $fullPath)
        }
    }
    finally {
        if (Test-Path -LiteralPath $temporaryPath -PathType Leaf) {
            Remove-Item -LiteralPath $temporaryPath -Force
        }
    }
}

function Remove-TopologyFaultGate {
    param([Parameter(Mandatory = $true)][string] $Path)

    $gateRoot = [System.IO.Path]::GetFullPath($probeGateDirectory) +
        [System.IO.Path]::DirectorySeparatorChar
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith(
        $gateRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw 'The topology fault gate is outside its dedicated directory.'
    }
    if (Test-Path -LiteralPath $fullPath -PathType Leaf) {
        Remove-Item -LiteralPath $fullPath -Force
    }
}

function Wait-ForTopologyFaultProbe {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [string] $ProbeId,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProjectCount,
        [Parameter(Mandatory = $true)]
        [bool] $ExpectedGlobalAvailable,
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $StartedAt,
        [Parameter(Mandatory = $true)]
        [string] $OutputRoot
    )

    $fullOutputRoot = [System.IO.Path]::GetFullPath($OutputRoot)
    $outputPrefix = $fullOutputRoot +
        [System.IO.Path]::DirectorySeparatorChar
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(
        $PerformanceTimeoutSeconds
    )
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        foreach ($processId in $RootProcessIds) {
            if ($null -eq (
                Get-Process -Id $processId -ErrorAction SilentlyContinue
            )) {
                throw (
                    "Topology host $processId exited while probe " +
                    "'$ProbeId' was running."
                )
            }
        }

        $events = @(Get-DesktopLogEventsSince -Since $StartedAt)
        $failures = @(
            $events |
                Where-Object {
                    $_.event -eq 'topology_fault_probe_failed' -and
                    $_.run_id -eq $RunId -and
                    $_.topology -eq $Topology -and
                    $_.probe_id -eq $ProbeId
                }
        )
        if ($failures.Count -gt 0) {
            $failure = $failures |
                Sort-Object timestamp |
                Select-Object -First 1
            throw (
                "Topology fault probe '$ProbeId' failed for project " +
                "$($failure.project_id): $($failure.reason)"
            )
        }

        $completions = @(
            $events |
                Where-Object {
                    $_.event -eq 'topology_fault_probe_completed' -and
                    $_.run_id -eq $RunId -and
                    $_.topology -eq $Topology -and
                    $_.probe_id -eq $ProbeId
                }
        )
        $groups = @($completions | Group-Object project_id)
        if ($groups.Count -eq $ExpectedProjectCount) {
            $duplicate = @($groups | Where-Object { $_.Count -ne 1 })
            if ($duplicate.Count -gt 0) {
                throw (
                    "Probe '$ProbeId' emitted duplicate completion events " +
                    "for: $($duplicate.Name -join ', ')."
                )
            }

            $evidence = [System.Collections.Generic.List[object]]::new()
            $sources = @{}
            foreach ($event in $completions | Sort-Object project_id) {
                if (
                    [int]$event.process_id -notin $RootProcessIds -or
                    $event.process_role -ne 'desktop_host' -or
                    [string]::IsNullOrWhiteSpace(
                        [string]$event.window_label
                    ) -or
                    [string]::IsNullOrWhiteSpace(
                        [string]$event.project_id
                    )
                ) {
                    throw "Probe '$ProbeId' emitted an invalid host identity."
                }

                $previousRevision = [long] $event.previous_revision
                $persistedRevision = [long] $event.persisted_revision
                $reopenedRevision = [long] $event.reopened_revision
                if (
                    $previousRevision + 1 -ne $persistedRevision -or
                    $reopenedRevision -ne $persistedRevision -or
                    $event.dirty -isnot [bool] -or
                    $event.dirty
                ) {
                    throw (
                        "Probe '$ProbeId' did not prove one persisted, " +
                        'reopened and clean revision.'
                    )
                }

                if (
                    $event.global_available -isnot [bool] -or
                    [bool]$event.global_available -ne
                        $ExpectedGlobalAvailable
                ) {
                    throw (
                        "Probe '$ProbeId' observed an unexpected global " +
                        'availability state.'
                    )
                }
                $globalProcessId = $null
                if (
                    $event.PSObject.Properties.Name -contains
                        'global_process_id' -and
                    $null -ne $event.global_process_id
                ) {
                    $globalProcessId = [int] $event.global_process_id
                }
                if (
                    $ExpectedGlobalAvailable -and
                    ($null -eq $globalProcessId -or $globalProcessId -le 0)
                ) {
                    throw (
                        "Probe '$ProbeId' did not identify the available " +
                        'global process.'
                    )
                }
                if (
                    -not $ExpectedGlobalAvailable -and
                    $null -ne $globalProcessId
                ) {
                    throw (
                        "Probe '$ProbeId' reported a global PID while " +
                        'global operations were unavailable.'
                    )
                }
                $roundTripMs = [double] $event.global_round_trip_ms
                if (
                    [double]::IsNaN($roundTripMs) -or
                    [double]::IsInfinity($roundTripMs) -or
                    $roundTripMs -lt 0
                ) {
                    throw "Probe '$ProbeId' emitted an invalid round-trip time."
                }

                $fileName = [string] $event.persisted_file_name
                if (
                    [string]::IsNullOrWhiteSpace($fileName) -or
                    [System.IO.Path]::GetFileName($fileName) -ne $fileName
                ) {
                    throw "Probe '$ProbeId' emitted an unsafe artifact name."
                }
                $sourcePath = [System.IO.Path]::GetFullPath(
                    (Join-Path $fullOutputRoot $fileName)
                )
                if (-not $sourcePath.StartsWith(
                    $outputPrefix,
                    [System.StringComparison]::OrdinalIgnoreCase
                )) {
                    throw "Probe '$ProbeId' artifact escaped its output root."
                }
                if (-not (
                    Test-Path -LiteralPath $sourcePath -PathType Leaf
                )) {
                    throw "Probe '$ProbeId' artifact was not published."
                }
                $item = Get-Item -LiteralPath $sourcePath
                $sha256 = (
                    Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256
                ).Hash.ToLowerInvariant()
                if (
                    [long]$event.persisted_bytes -ne $item.Length -or
                    [string]$event.persisted_sha256 -ne $sha256
                ) {
                    throw (
                        "Probe '$ProbeId' artifact size or SHA-256 does not " +
                        'match its completion event.'
                    )
                }
                $document = Get-Content `
                    -LiteralPath $sourcePath `
                    -Raw `
                    -Encoding utf8 |
                        ConvertFrom-Json
                if (
                    $document.projectId -ne $event.project_id -or
                    [long]$document.revision -ne $persistedRevision
                ) {
                    throw (
                        "Probe '$ProbeId' artifact does not reopen the " +
                        'reported project revision.'
                    )
                }

                $sources[[string]$event.project_id] = $sourcePath
                $evidence.Add([ordered]@{
                    projectId = [string] $event.project_id
                    windowLabel = [string] $event.window_label
                    processId = [int] $event.process_id
                    previousRevision = $previousRevision
                    persistedRevision = $persistedRevision
                    dirty = $false
                    persistedBytes = [long] $item.Length
                    persistedSha256 = $sha256
                    persistedFileName = $fileName
                    reopenedRevision = $reopenedRevision
                    globalAvailable = [bool] $event.global_available
                    globalProcessId = $globalProcessId
                    globalRoundTripMs = $roundTripMs
                })
            }
            return [ordered]@{
                probeId = $ProbeId
                expectedCompletions = $ExpectedProjectCount
                observedCompletions = $completions.Count
                duplicateCompletions = 0
                missingCompletions = 0
                evidence = @($evidence)
                sources = $sources
            }
        }
        if ($groups.Count -gt $ExpectedProjectCount) {
            throw (
                "Probe '$ProbeId' completed for more projects than expected."
            )
        }
        Start-Sleep -Milliseconds 100
    }
    throw (
        "Expected $ExpectedProjectCount correlated completions for probe " +
        "'$ProbeId' within $PerformanceTimeoutSeconds seconds."
    )
}

function Wait-ForTopologyProjectReopen {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [object[]] $ExpectedProjects,
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $StartedAt
    )

    $expectedByProject = @{}
    foreach ($project in $ExpectedProjects) {
        $expectedByProject[[string]$project.projectId] =
            [long]$project.persistedRevision
    }
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($WindowTimeoutSeconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        foreach ($processId in $RootProcessIds) {
            if ($null -eq (
                Get-Process -Id $processId -ErrorAction SilentlyContinue
            )) {
                throw "Restarted topology host $processId exited before reopen."
            }
        }
        $events = @(
            Get-DesktopLogEventsSince -Since $StartedAt |
                Where-Object {
                    $_.event -eq 'topology_project_reopened' -and
                    $_.run_id -eq $RunId -and
                    $_.topology -eq $Topology -and
                    [int]$_.process_id -in $RootProcessIds -and
                    $expectedByProject.ContainsKey([string]$_.project_id)
                }
        )
        $groups = @($events | Group-Object project_id)
        if ($groups.Count -eq $ExpectedProjects.Count) {
            if (@($groups | Where-Object { $_.Count -ne 1 }).Count -gt 0) {
                throw 'A restarted Project emitted duplicate reopen evidence.'
            }
            $evidence = @(
                $events |
                    Sort-Object project_id |
                    ForEach-Object {
                        $expectedRevision =
                            $expectedByProject[[string]$_.project_id]
                        if (
                            [long]$_.revision -ne $expectedRevision -or
                            [string]::IsNullOrWhiteSpace(
                                [string]$_.window_label
                            )
                        ) {
                            throw (
                                "Restarted project $($_.project_id) opened " +
                                'an unexpected revision.'
                            )
                        }
                        [ordered]@{
                            projectId = [string] $_.project_id
                            windowLabel = [string] $_.window_label
                            processId = [int] $_.process_id
                            revision = [long] $_.revision
                        }
                    }
            )
            return [ordered]@{
                expectedProjects = $ExpectedProjects.Count
                observedProjects = $evidence.Count
                projects = $evidence
            }
        }
        Start-Sleep -Milliseconds 100
    }
    throw (
        "Expected $($ExpectedProjects.Count) reopened Projects within " +
        "$WindowTimeoutSeconds seconds."
    )
}

function Measure-TopologyFailureLogQuality {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $StartedAt,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedHostStreamCount,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedCompletionCount,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedReopenCount
    )

    $globalEvents = @(
        Get-GlobalLogEventsSince -Since $StartedAt |
            Where-Object {
                $_.run_id -eq $RunId -and
                $_.topology -eq $Topology
            }
    )
    $globalStarts = @(
        $globalEvents | Where-Object { $_.event -eq 'start' }
    )
    $singletonRejections = @(
        $globalEvents |
            Where-Object { $_.event -eq 'singleton_rejected' }
    )
    $globalStatuses = @(
        $globalEvents | Where-Object { $_.event -eq 'status' }
    )
    if (
        @($globalStarts | Group-Object process_id).Count -ne 2 -or
        $globalStarts.Count -ne 2 -or
        $singletonRejections.Count -ne 2 -or
        $globalStatuses.Count -lt 4
    ) {
        throw 'Global process logs are incomplete for the failure gate.'
    }
    $missingGlobalFields = @(
        $globalEvents |
            Where-Object {
                $_.process_role -ne 'global_shell' -or
                [long]$_.process_id -le 0 -or
                [string]::IsNullOrWhiteSpace([string]$_.run_id) -or
                [string]::IsNullOrWhiteSpace([string]$_.topology) -or
                [string]::IsNullOrWhiteSpace([string]$_.endpoint) -or
                [string]::IsNullOrWhiteSpace([string]$_.event) -or
                (
                    $_.event -eq 'status' -and
                    [string]::IsNullOrWhiteSpace([string]$_.probe_id)
                )
            }
    )
    if ($missingGlobalFields.Count -gt 0) {
        throw 'Global process logs are missing required correlation fields.'
    }

    $desktopEvents = @(
        Get-DesktopLogEventsSince -Since $StartedAt |
            Where-Object {
                $_.run_id -eq $RunId -and
                $_.topology -eq $Topology -and
                $_.event -in @(
                    'topology_fault_probe_completed',
                    'topology_fault_probe_failed',
                    'topology_project_reopened'
                )
            }
    )
    $hostStreams = @(
        $desktopEvents |
            Where-Object { $null -ne $_.process_id } |
            Group-Object process_id
    )
    if ($hostStreams.Count -ne $ExpectedHostStreamCount) {
        throw (
            "Expected $ExpectedHostStreamCount correlated host log streams; " +
            "observed $($hostStreams.Count)."
        )
    }
    $missingDesktopFields = @(
        $desktopEvents |
            Where-Object {
                $_.process_role -ne 'desktop_host' -or
                [long]$_.process_id -le 0 -or
                [string]::IsNullOrWhiteSpace([string]$_.run_id) -or
                [string]::IsNullOrWhiteSpace([string]$_.topology) -or
                [string]::IsNullOrWhiteSpace([string]$_.window_label) -or
                [string]::IsNullOrWhiteSpace([string]$_.project_id) -or
                [string]::IsNullOrWhiteSpace([string]$_.event)
            }
    )
    if ($missingDesktopFields.Count -gt 0) {
        throw 'Desktop host logs are missing required correlation fields.'
    }
    $completionEvents = @(
        $desktopEvents |
            Where-Object { $_.event -eq 'topology_fault_probe_completed' }
    )
    $failureEvents = @(
        $desktopEvents |
            Where-Object { $_.event -eq 'topology_fault_probe_failed' }
    )
    $reopenEvents = @(
        $desktopEvents |
            Where-Object { $_.event -eq 'topology_project_reopened' }
    )
    if (
        $completionEvents.Count -ne $ExpectedCompletionCount -or
        $failureEvents.Count -ne 0 -or
        $reopenEvents.Count -ne $ExpectedReopenCount
    ) {
        throw (
            'Desktop host logs do not contain the expected continuity and ' +
            'reopen event counts.'
        )
    }

    return [ordered]@{
        global = [ordered]@{
            streamCount = @(
                $globalEvents | Group-Object process_id
            ).Count
            startEvents = $globalStarts.Count
            singletonRejectionEvents = $singletonRejections.Count
            statusEvents = $globalStatuses.Count
            missingRequiredFields = 0
        }
        projectHosts = [ordered]@{
            streamCount = $hostStreams.Count
            continuityCompletionEvents = $completionEvents.Count
            continuityFailureEvents = $failureEvents.Count
            reopenEvents = $reopenEvents.Count
            missingRequiredFields = 0
        }
        forcedTerminationObservation = 'runner'
    }
}

function Wait-ForTopologyFailureLogQuality {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $StartedAt,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedHostStreamCount,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedCompletionCount,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedReopenCount
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($WindowTimeoutSeconds)
    $lastReason = $null
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        try {
            return Measure-TopologyFailureLogQuality `
                -Topology $Topology `
                -RunId $RunId `
                -StartedAt $StartedAt `
                -ExpectedHostStreamCount $ExpectedHostStreamCount `
                -ExpectedCompletionCount $ExpectedCompletionCount `
                -ExpectedReopenCount $ExpectedReopenCount
        }
        catch {
            $lastReason = $_.Exception.Message
            Start-Sleep -Milliseconds 100
        }
    }
    throw (
        "Failure logs for '$Topology' did not become complete within " +
        "$WindowTimeoutSeconds seconds. Last error: $lastReason"
    )
}

function Assert-TopologyWindowCount {
    param(
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedCount,
        [Parameter(Mandatory = $true)]
        [string] $Context
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($WindowTimeoutSeconds)
    $observed = @()
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        $observed = @(
            [MyAlbunsWindowProbe]::VisibleWindowsFor($RootProcessIds)
        )
        if ($observed.Count -eq $ExpectedCount) {
            return [ordered]@{
                expectedCount = $ExpectedCount
                observedCount = $observed.Count
                windows = @(
                    $observed |
                        Sort-Object ProcessId, Handle |
                        ForEach-Object {
                            [ordered]@{
                                processId = $_.ProcessId
                                handle = $_.Handle
                                title = $_.Title
                            }
                        }
                )
            }
        }
        Start-Sleep -Milliseconds 100
    }
    throw (
        "$Context expected $ExpectedCount visible Project windows; " +
        "observed $($observed.Count)."
    )
}

function Assert-TopologyExecutableProcessSet {
    param(
        [Parameter(Mandatory = $true)]
        [int[]] $ExpectedProcessIds,
        [Parameter(Mandatory = $true)]
        [string] $Context
    )

    $expected = @($ExpectedProcessIds | Sort-Object -Unique)
    $executable = [System.IO.Path]::GetFullPath($executablePath)
    $observed = @(
        Get-CimInstance Win32_Process |
            Where-Object {
                $null -ne $_.ExecutablePath -and
                [string]::Equals(
                    [System.IO.Path]::GetFullPath($_.ExecutablePath),
                    $executable,
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            } |
            ForEach-Object { [int] $_.ProcessId } |
            Sort-Object -Unique
    )
    if (
        (Compare-Object `
            -ReferenceObject $expected `
            -DifferenceObject $observed).Count -ne 0
    ) {
        throw (
            "$Context expected executable PIDs $($expected -join ', '); " +
            "observed $($observed -join ', ')."
        )
    }
    return [ordered]@{
        executable = $executableRelativePath
        expectedProcessIds = $expected
        observedProcessIds = $observed
        unexpectedProcessIds = @()
    }
}

function Invoke-TopologyContinuityProbe {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [string] $ProbeId,
        [Parameter(Mandatory = $true)]
        [bool] $ExpectedGlobalAvailable,
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProjectCount,
        [Parameter(Mandatory = $true)]
        [string] $GatePath,
        [Parameter(Mandatory = $true)]
        [string] $OutputRoot,
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $TopologyStartedAt
    )

    Remove-TopologyFaultGate -Path $GatePath
    $probeStartedAt = [DateTimeOffset]::UtcNow
    Set-TopologyFaultGate `
        -Path $GatePath `
        -RunId $RunId `
        -ProbeId $ProbeId `
        -ExpectedGlobalAvailable $ExpectedGlobalAvailable
    try {
        $result = Wait-ForTopologyFaultProbe `
            -Topology $Topology `
            -RunId $RunId `
            -ProbeId $ProbeId `
            -ExpectedProjectCount $ExpectedProjectCount `
            -ExpectedGlobalAvailable $ExpectedGlobalAvailable `
            -RootProcessIds $RootProcessIds `
            -StartedAt $TopologyStartedAt `
            -OutputRoot $OutputRoot
        $result['probeStartedAtUtc'] = $probeStartedAt.ToString('o')
        return $result
    }
    finally {
        Remove-TopologyFaultGate -Path $GatePath
    }
}

function Get-TopologyProbeProject {
    param(
        [Parameter(Mandatory = $true)]
        $Probe,
        [Parameter(Mandatory = $true)]
        [string] $ProjectId
    )

    $matches = @(
        $Probe.evidence |
            Where-Object { $_.projectId -eq $ProjectId }
    )
    if ($matches.Count -ne 1) {
        throw (
            "Probe '$($Probe.probeId)' did not produce exactly one result " +
            "for Project '$ProjectId'."
        )
    }
    return $matches[0]
}

function Get-TopologyProbeSource {
    param(
        [Parameter(Mandatory = $true)]
        $Probe,
        [Parameter(Mandatory = $true)]
        [string] $ProjectId
    )

    if (-not $Probe.sources.ContainsKey($ProjectId)) {
        throw (
            "Probe '$($Probe.probeId)' did not publish a source for " +
            "Project '$ProjectId'."
        )
    }
    return [string] $Probe.sources[$ProjectId]
}

function Select-TopologyProbeReport {
    param([Parameter(Mandatory = $true)] $Probe)

    return [ordered]@{
        probeId = [string] $Probe.probeId
        probeStartedAtUtc = [string] $Probe.probeStartedAtUtc
        expectedCompletions = [int] $Probe.expectedCompletions
        observedCompletions = [int] $Probe.observedCompletions
        duplicateCompletions = [int] $Probe.duplicateCompletions
        missingCompletions = [int] $Probe.missingCompletions
        evidence = @($Probe.evidence)
    }
}

function Assert-TopologyProbeGlobalOwner {
    param(
        [Parameter(Mandatory = $true)]
        $Probe,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProcessId
    )

    $mismatches = @(
        $Probe.evidence |
            Where-Object {
                $null -eq $_.globalProcessId -or
                [int]$_.globalProcessId -ne $ExpectedProcessId
            }
    )
    if ($mismatches.Count -gt 0) {
        throw (
            "Probe '$($Probe.probeId)' did not correlate every Project " +
            "with global PID $ExpectedProcessId."
        )
    }
}

function New-TopologyIpcEvidence {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [int] $SuccessfulProjectProbeCount
    )

    $hostLinkCount = if ($Topology -eq 'independent') { 2 } else { 1 }
    return [ordered]@{
        score = $null
        scoreReason = 'No synthetic complexity score was invented.'
        globalListenerEndpointCount = 1
        projectHostToGlobalLinkCount = $hostLinkCount
        linksInterruptedByGlobalCrash = $hostLinkCount
        successfulProjectContinuityProbes = $SuccessfulProjectProbeCount
        minimumProjectHostCommandsPerProjectProbe = 4
        minimumProjectHostCommands = (
            $SuccessfulProjectProbeCount * 4
        )
        minimumCorrelatedInteractionsPerProjectProbe = 5
        minimumCorrelatedInteractions = (
            $SuccessfulProjectProbeCount * 5
        )
        interactionFamilies = @(
            [ordered]@{
                order = 1
                boundary = 'frontend_to_project_host'
                operation = 'topology_fault_probe_config'
                transport = 'tauri_command'
                minimumCallsPerProjectProbe = 1
            },
            [ordered]@{
                order = 2
                boundary = 'frontend_to_project_host'
                operation = 'project_state'
                transport = 'tauri_command'
                minimumCallsPerProjectProbe = 1
            },
            [ordered]@{
                order = 3
                boundary = 'frontend_to_project_host'
                operation = 'apply_project_intent'
                transport = 'tauri_command'
                minimumCallsPerProjectProbe = 1
            },
            [ordered]@{
                order = 4
                boundary = 'frontend_to_project_host'
                operation = 'persist_topology_fault_probe'
                transport = 'tauri_command'
                minimumCallsPerProjectProbe = 1
            },
            [ordered]@{
                order = 5
                boundary = 'project_host_to_global_shell'
                operation = 'status'
                transport = 'typed_loopback_json_line_spike'
                minimumCallsPerProjectProbe = 1
            }
        )
        note = (
            'The four Project Host commands are counted separately from the ' +
            'typed status request to the global process. Polling beyond the ' +
            'first config read is intentionally not estimated.'
        )
    }
}

function Read-ValidatedImagingRecoveryArtifact {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path,
        [Parameter(Mandatory = $true)]
        [string] $TopologyBuildCommit
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $expectedPath = [System.IO.Path]::GetFullPath(
        (Join-Path `
            $script:WorkspaceRoot `
            'docs\research\artifacts\0004-imaging-recovery.json')
    )
    if (-not [string]::Equals(
        $fullPath,
        $expectedPath,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw 'The Imaging recovery evidence must be artifact 0004.'
    }
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        throw (
            'Imaging recovery artifact 0004 is missing. Run ' +
            'npm run spike:imaging-recovery first.'
        )
    }
    $artifact = Get-Content `
        -LiteralPath $fullPath `
        -Raw `
        -Encoding utf8 |
            ConvertFrom-Json
    $expectedChecks = @(
        'protocol',
        'cache-temporary-cleanup',
        'imaging-sidecar-build',
        'production-recovery-integration'
    )
    $checks = @($artifact.checks)
    if (
        [int]$artifact.schemaVersion -ne 1 -or
        $checks.Count -ne $expectedChecks.Count -or
        @(
            $expectedChecks |
                Where-Object {
                    $name = $_
                    @(
                        $checks |
                            Where-Object {
                                $_.name -eq $name -and
                                $_.passed -eq $true
                            }
                    ).Count -ne 1
                }
        ).Count -gt 0
    ) {
        throw 'Imaging recovery artifact 0004 has incomplete checks.'
    }

    $cache = $artifact.evidence.cache
    $export = $artifact.evidence.export
    $cacheRecovered = (
        [long]$cache.failedProcessId -gt 0 -and
        [long]$cache.restartedProcessId -gt 0 -and
        $cache.failedProcessId -ne $cache.restartedProcessId -and
        $cache.temporaryObservedAfterFailure -eq $true -and
        [long]$cache.removedTemporaryCount -ge 1 -and
        $cache.temporaryExistedAfterCleanup -eq $false -and
        $cache.foreignTemporarySurvivedCleanup -eq $true -and
        $cache.metadataExistedAfterFailure -eq $false -and
        $cache.metadataExistedAfterRestart -eq $true -and
        [long]$cache.generatedCountAfterRestart -ge 1
    )
    $exportFailedSafely = (
        [long]$export.failedProcessId -gt 0 -and
        [long]$export.retryProcessId -gt 0 -and
        $export.failedProcessId -ne $export.retryProcessId -and
        $export.sourcePolicy -eq 'linkedOriginals' -and
        [long]$export.processCountBeforeExplicitRetry -eq 1 -and
        $export.successResponseBeforeExplicitRetry -eq $false -and
        $export.partialPreparationObserved -eq $true -and
        $export.previousOutputSha256BeforeFailure -eq
            $export.previousOutputSha256AfterFailure -and
        $export.projectSha256BeforeFailure -eq
            $export.projectSha256AfterFailure -and
        $export.finalOutputSha256AfterExplicitRetry -ne
            $export.previousOutputSha256BeforeFailure
    )
    if (-not $cacheRecovered -or -not $exportFailedSafely) {
        throw 'Imaging recovery artifact 0004 does not satisfy its gate.'
    }
    if ([string]$artifact.gitCommit -ne $TopologyBuildCommit) {
        throw (
            'Imaging recovery artifact 0004 does not match the topology ' +
            'build commit. Run npm run spike:imaging-recovery after the ' +
            'implementation commit and before npm run spike:topology.'
        )
    }
    if ([bool]$artifact.sourceInputsDirty) {
        throw (
            'Imaging recovery artifact 0004 was collected with dirty source ' +
            'inputs. Commit the implementation and run ' +
            'npm run spike:imaging-recovery again.'
        )
    }

    return [ordered]@{
        validated = $true
        artifact = 'docs/research/artifacts/0004-imaging-recovery.json'
        artifactSha256 = (
            Get-FileHash -LiteralPath $fullPath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        artifactSchemaVersion = [int] $artifact.schemaVersion
        collectedAtUtc = [string] $artifact.collectedAtUtc
        gitCommit = [string] $artifact.gitCommit
        sourceInputsDirty = [bool] $artifact.sourceInputsDirty
        sameGitCommitAsTopologyBuild = $true
        passedChecks = $expectedChecks
        cacheRecoveredAfterOneExplicitRestart = $cacheRecovered
        exportFailedSafelyUntilExplicitRetry = $exportFailedSafely
    }
}

function Remove-TopologyFailureScratchRoot {
    param([Parameter(Mandatory = $true)][string] $Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        return
    }
    $scratchRoot = [System.IO.Path]::GetFullPath(
        (Join-Path $script:WorkspaceRoot '.scratch')
    )
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $parent = [System.IO.Path]::GetDirectoryName($fullPath)
    if (
        -not [string]::Equals(
            $parent,
            $scratchRoot,
            [System.StringComparison]::OrdinalIgnoreCase
        ) -or
        -not [System.IO.Path]::GetFileName($fullPath).StartsWith(
            'topology-failure-',
            [System.StringComparison]::Ordinal
        )
    ) {
        throw 'Refusing to remove an unverified topology failure directory.'
    }
    Remove-Item -LiteralPath $fullPath -Recurse -Force
}
