function Get-GateProcessCreationUtc([object] $Process) {
    if ($Process.CreationDate -is [DateTime]) {
        return ([DateTime] $Process.CreationDate).ToUniversalTime()
    }
    return [System.Management.ManagementDateTimeConverter]::ToDateTime(
        [string] $Process.CreationDate
    ).ToUniversalTime()
}

function Get-GateProcessIdentity([object] $Process) {
    $created = Get-GateProcessCreationUtc -Process $Process
    return "$([uint32] $Process.ProcessId)|$($created.Ticks)"
}

function New-GateProcessScope {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $WorkspaceRoot,

        [Parameter(Mandatory = $true)]
        [string] $RunRoot,

        [Parameter(Mandatory = $true)]
        [string] $WindowsPowerShell
    )

    $preexistingProcessIdentities =
        [System.Collections.Generic.HashSet[string]]::new()
    foreach ($process in @(Get-CimInstance Win32_Process)) {
        if ([uint32] $process.ProcessId -ne $PID) {
            [void] $preexistingProcessIdentities.Add(
                (Get-GateProcessIdentity -Process $process)
            )
        }
    }
    return [pscustomobject]@{
        WorkspaceRoot = [System.IO.Path]::GetFullPath($WorkspaceRoot)
        RunRoot = [System.IO.Path]::GetFullPath($RunRoot)
        WindowsPowerShell = [System.IO.Path]::GetFullPath($WindowsPowerShell)
        OwnedProcessRecords =
            [System.Collections.Generic.Dictionary[string, object]]::new()
        OwnedJobs = [System.Collections.Generic.List[object]]::new()
        PreexistingProcessIdentities = $preexistingProcessIdentities
    }
}

function Test-GateWorkspaceProcess {
    param(
        [Parameter(Mandatory = $true)]
        [psobject] $Scope,

        [Parameter(Mandatory = $true)]
        [object] $Process
    )

    $workspacePrefix = $Scope.WorkspaceRoot.TrimEnd('\', '/') + '\'
    $portablePrefix = $workspacePrefix.Replace('\', '/')
    return (
        (-not [string]::IsNullOrWhiteSpace($Process.ExecutablePath) -and (
            $Process.ExecutablePath.StartsWith(
                $workspacePrefix,
                [System.StringComparison]::OrdinalIgnoreCase
            ) -or
            $Process.ExecutablePath.Replace('\', '/').StartsWith(
                $portablePrefix,
                [System.StringComparison]::OrdinalIgnoreCase
            )
        )) -or
        (-not [string]::IsNullOrWhiteSpace($Process.CommandLine) -and (
            $Process.CommandLine.IndexOf(
                $workspacePrefix,
                [System.StringComparison]::OrdinalIgnoreCase
            ) -ge 0 -or
            $Process.CommandLine.Replace('\', '/').IndexOf(
                $portablePrefix,
                [System.StringComparison]::OrdinalIgnoreCase
            ) -ge 0
        ))
    )
}

function Get-GateWorkspaceProcesses([psobject] $Scope) {
    return @(
        Get-CimInstance Win32_Process |
            Where-Object {
                $_.ProcessId -ne $PID -and
                (Test-GateWorkspaceProcess -Scope $Scope -Process $_)
            }
    )
}

function Register-GateJobProcesses {
    param(
        [Parameter(Mandatory = $true)]
        [psobject] $Scope,

        [Parameter(Mandatory = $true)]
        [object] $Job
    )

    $jobProcessIds = [System.Collections.Generic.HashSet[uint32]]::new()
    foreach ($processId in @($Job.ProcessIds())) {
        [void] $jobProcessIds.Add([uint32] $processId)
    }
    if ($jobProcessIds.Count -eq 0) {
        return
    }
    foreach ($process in @(Get-CimInstance Win32_Process)) {
        $processId = [uint32] $process.ProcessId
        if (-not $jobProcessIds.Contains($processId)) {
            continue
        }
        $identity = Get-GateProcessIdentity -Process $process
        if ($Scope.PreexistingProcessIdentities.Contains($identity)) {
            throw "A pre-existing process identity entered a gate-owned Job: $identity."
        }
        if (-not $Scope.OwnedProcessRecords.ContainsKey($identity)) {
            $Scope.OwnedProcessRecords.Add($identity, [pscustomobject]@{
                processId = $processId
                parentProcessId = [uint32] $process.ParentProcessId
                creationUtc = Get-GateProcessCreationUtc -Process $process
                executablePath = [string] $process.ExecutablePath
                commandLine = [string] $process.CommandLine
            })
        }
    }
}

function Get-ActiveGateProcesses([psobject] $Scope) {
    $jobProcessIds = [System.Collections.Generic.HashSet[uint32]]::new()
    foreach ($job in @($Scope.OwnedJobs.ToArray())) {
        Register-GateJobProcesses -Scope $Scope -Job $job
        foreach ($processId in @($job.ProcessIds())) {
            [void] $jobProcessIds.Add([uint32] $processId)
        }
    }
    $active = [System.Collections.Generic.List[object]]::new()
    foreach ($process in @(Get-CimInstance Win32_Process)) {
        if (-not $jobProcessIds.Contains([uint32] $process.ProcessId)) {
            continue
        }
        $identity = Get-GateProcessIdentity -Process $process
        if ($Scope.OwnedProcessRecords.ContainsKey($identity)) {
            $active.Add($process)
        }
    }
    return @($active.ToArray())
}

function Get-GateProcessListeners([object[]] $Processes) {
    $processIds = @($Processes | ForEach-Object { [uint32] $_.ProcessId })
    if ($processIds.Count -eq 0) {
        return @()
    }
    $listeners = [System.Collections.Generic.List[object]]::new()
    foreach ($listener in @(Get-NetTCPConnection -State Listen -ErrorAction Stop)) {
        if ([uint32] $listener.OwningProcess -in $processIds) {
            $listeners.Add([pscustomobject]@{
                protocol = 'tcp'
                localAddress = [string] $listener.LocalAddress
                localPort = [uint16] $listener.LocalPort
                owningProcess = [uint32] $listener.OwningProcess
            })
        }
    }
    foreach ($listener in @(Get-NetUDPEndpoint -ErrorAction Stop)) {
        if ([uint32] $listener.OwningProcess -in $processIds) {
            $listeners.Add([pscustomobject]@{
                protocol = 'udp'
                localAddress = [string] $listener.LocalAddress
                localPort = [uint16] $listener.LocalPort
                owningProcess = [uint32] $listener.OwningProcess
            })
        }
    }
    return @($listeners.ToArray())
}

function Stop-GateProcessScope([psobject] $Scope) {
    if ($null -eq $Scope) {
        return [pscustomobject]@{
            stoppedProcessCount = 0
            listenersBefore = 0
            processesAfter = 0
            listenersAfter = 0
        }
    }
    $jobs = @($Scope.OwnedJobs.ToArray())
    $before = @(Get-ActiveGateProcesses -Scope $Scope)
    $listenersBefore = @(Get-GateProcessListeners -Processes $before)
    $activeProcessIds = [System.Collections.Generic.HashSet[uint32]]::new()
    foreach ($job in $jobs) {
        foreach ($processId in @($job.ProcessIds())) {
            [void] $activeProcessIds.Add([uint32] $processId)
        }
        if (@($job.ProcessIds()).Count -ne 0) {
            $job.Terminate()
        }
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    $remainingJobProcessCount = @(
        $jobs | ForEach-Object { $_.ProcessIds() }
    ).Count
    while ($remainingJobProcessCount -ne 0 -and
            [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 50
        $remainingJobProcessCount = @(
            $jobs | ForEach-Object { $_.ProcessIds() }
        ).Count
    }
    foreach ($job in $jobs) {
        $job.Dispose()
    }
    $Scope.OwnedJobs.Clear()
    $after = @(Get-ActiveGateProcesses -Scope $Scope)
    $listenersAfter = @(Get-GateProcessListeners -Processes $after)
    if ($remainingJobProcessCount -ne 0 -or
            $after.Count -ne 0 -or
            $listenersAfter.Count -ne 0) {
        $identifiers = @($after | ForEach-Object { $_.ProcessId }) -join ', '
        throw "The gate could not terminate its owned process tree: $identifiers."
    }
    return [pscustomobject]@{
        stoppedProcessCount = $activeProcessIds.Count
        listenersBefore = $listenersBefore.Count
        processesAfter = $after.Count
        listenersAfter = $listenersAfter.Count
    }
}

function Start-GateScopedProcess {
    param(
        [Parameter(Mandatory = $true)]
        [psobject] $Scope,

        [Parameter(Mandatory = $true)]
        [System.Diagnostics.ProcessStartInfo] $StartInfo,

        [Parameter(Mandatory = $true)]
        [string] $StartSignalPath
    )

    if (Test-Path -LiteralPath $StartSignalPath) {
        throw "The owned process start signal already exists: $StartSignalPath."
    }
    $job = [GateOwnedProcessJob]::new()
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $StartInfo
    $started = $false
    try {
        if (-not $process.Start()) {
            throw 'The owned gate process could not be started.'
        }
        $started = $true
        $job.Assign($process)
        $Scope.OwnedJobs.Add($job)
        Register-GateJobProcesses -Scope $Scope -Job $job
        [System.IO.File]::WriteAllText(
            $StartSignalPath,
            'assigned',
            [System.Text.UTF8Encoding]::new($false)
        )
        return [pscustomobject]@{
            process = $process
            job = $job
        }
    }
    catch {
        if ($started -and -not $process.HasExited) {
            try { $process.Kill() } catch {}
            try { $process.WaitForExit() } catch {}
        }
        $process.Dispose()
        $job.Dispose()
        throw
    }
}

function Get-NormalizedGateCommandOutput([object[]] $Lines) {
    $text = ($Lines | ForEach-Object { $_.ToString() }) -join "`n"
    return $text -replace "$([char]27)\[[0-9;?]*[ -/]*[@-~]", ''
}

function Invoke-GateScopedCommand {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [psobject] $Scope,

        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [string] $FilePath,

        [Parameter(Mandatory = $true)]
        [string[]] $Arguments
    )

    Write-Host "START $Name"
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $payloadPath = Join-Path $Scope.RunRoot "$Name-command.json"
    $wrapperPath = Join-Path $Scope.RunRoot "$Name-command.ps1"
    $startSignalPath = Join-Path $Scope.RunRoot "$Name-command.assigned"
    $payload = [ordered]@{
        filePath = $FilePath
        arguments = @($Arguments)
    } | ConvertTo-Json -Depth 4
    [System.IO.File]::WriteAllText(
        $payloadPath,
        $payload + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    $wrapper = @'
param(
    [Parameter(Mandatory = $true)][string] $PayloadPath,
    [Parameter(Mandatory = $true)][string] $StartSignalPath
)
$ErrorActionPreference = 'Stop'
while (-not (Test-Path -LiteralPath $StartSignalPath -PathType Leaf)) {
    Start-Sleep -Milliseconds 10
}
$payload = Get-Content -LiteralPath $PayloadPath -Raw -Encoding UTF8 | ConvertFrom-Json
& ([string] $payload.filePath) @($payload.arguments)
if ($null -eq $LASTEXITCODE) { exit 0 }
exit $LASTEXITCODE
'@
    [System.IO.File]::WriteAllText(
        $wrapperPath,
        $wrapper + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    $escapedWrapper = $wrapperPath.Replace('"', '\"')
    $escapedPayload = $payloadPath.Replace('"', '\"')
    $escapedStartSignal = $startSignalPath.Replace('"', '\"')
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $Scope.WindowsPowerShell
    $startInfo.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$escapedWrapper`" `"$escapedPayload`" `"$escapedStartSignal`""
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $owned = Start-GateScopedProcess `
        -Scope $Scope `
        -StartInfo $startInfo `
        -StartSignalPath $startSignalPath
    $process = $owned.process
    $job = $owned.job
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    while (-not $process.HasExited) {
        Register-GateJobProcesses -Scope $Scope -Job $job
        Start-Sleep -Milliseconds 50
    }
    $process.WaitForExit()
    Register-GateJobProcesses -Scope $Scope -Job $job
    $exitCode = $process.ExitCode
    $rawOutput = @(
        $stdoutTask.GetAwaiter().GetResult()
        $stderrTask.GetAwaiter().GetResult()
    )
    $process.Dispose()
    $stopwatch.Stop()
    $output = Get-NormalizedGateCommandOutput -Lines $rawOutput
    $logPath = Join-Path $Scope.RunRoot "$Name.log"
    [System.IO.File]::WriteAllText(
        $logPath,
        $output + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    if ($exitCode -ne 0) {
        $tail = @($output -split "`n" | Select-Object -Last 80) -join "`n"
        throw "Gate command '$Name' failed with exit code $exitCode.`n$tail"
    }
    Write-Host "PASS $Name ($($stopwatch.ElapsedMilliseconds) ms)"
    return [pscustomobject]@{
        output = $output
        elapsedMs = $stopwatch.ElapsedMilliseconds
    }
}

function Invoke-GateProcessCleanupProbe([psobject] $Scope) {
    $probeScript = Join-Path $Scope.RunRoot 'owned-cleanup-probe.ps1'
    $probeReady = Join-Path $Scope.RunRoot 'owned-cleanup-probe.ready'
    $probeStartSignal = Join-Path $Scope.RunRoot 'owned-cleanup-probe.assigned'
    $sentinelScript = Join-Path $Scope.RunRoot 'concurrent-independent-sentinel.ps1'
    $sentinelReady = Join-Path $Scope.RunRoot 'concurrent-independent-sentinel.ready'
    $probeSource = @'
param(
    [Parameter(Mandatory = $true)][string] $ReadyPath,
    [Parameter(Mandatory = $true)][string] $StartSignalPath
)
while (-not (Test-Path -LiteralPath $StartSignalPath -PathType Leaf)) {
    Start-Sleep -Milliseconds 10
}
$listener = [System.Net.Sockets.TcpListener]::new(
    [System.Net.IPAddress]::Loopback,
    0
)
$listener.Start()
$port = ([System.Net.IPEndPoint] $listener.LocalEndpoint).Port
[System.IO.File]::WriteAllText($ReadyPath, [string] $port)
while ($true) { Start-Sleep -Seconds 1 }
'@
    $sentinelSource = @'
param([Parameter(Mandatory = $true)][string] $ReadyPath)
[System.IO.File]::WriteAllText($ReadyPath, 'alive')
while ($true) { Start-Sleep -Seconds 1 }
'@
    [System.IO.File]::WriteAllText(
        $probeScript,
        $probeSource + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    [System.IO.File]::WriteAllText(
        $sentinelScript,
        $sentinelSource + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $Scope.WindowsPowerShell
    $startInfo.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$probeScript`" `"$probeReady`" `"$probeStartSignal`""
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $owned = Start-GateScopedProcess `
        -Scope $Scope `
        -StartInfo $startInfo `
        -StartSignalPath $probeStartSignal
    $process = $owned.process
    $job = $owned.job
    $sentinel = $null
    $cleanup = $null
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        while (-not (Test-Path -LiteralPath $probeReady -PathType Leaf)) {
            if ($process.HasExited) {
                throw 'The owned-process cleanup probe exited before listening.'
            }
            Register-GateJobProcesses -Scope $Scope -Job $job
            if ([DateTime]::UtcNow -ge $deadline) {
                throw 'The owned-process cleanup probe did not become ready.'
            }
            Start-Sleep -Milliseconds 50
        }
        Register-GateJobProcesses -Scope $Scope -Job $job

        $sentinelStartInfo = [System.Diagnostics.ProcessStartInfo]::new()
        $sentinelStartInfo.FileName = $Scope.WindowsPowerShell
        $sentinelStartInfo.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$sentinelScript`" `"$sentinelReady`""
        $sentinelStartInfo.UseShellExecute = $false
        $sentinelStartInfo.CreateNoWindow = $true
        $sentinel = [System.Diagnostics.Process]::new()
        $sentinel.StartInfo = $sentinelStartInfo
        if (-not $sentinel.Start()) {
            throw 'The concurrent independent sentinel could not start.'
        }
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        while (-not (Test-Path -LiteralPath $sentinelReady -PathType Leaf)) {
            if ($sentinel.HasExited) {
                throw 'The concurrent independent sentinel exited before readiness.'
            }
            if ([DateTime]::UtcNow -ge $deadline) {
                throw 'The concurrent independent sentinel did not become ready.'
            }
            Start-Sleep -Milliseconds 50
        }
        $sentinelCim = Get-CimInstance `
            -ClassName Win32_Process `
            -Filter "ProcessId = $($sentinel.Id)"
        if ($null -eq $sentinelCim) {
            throw 'The concurrent independent sentinel identity was not observable.'
        }
        $sentinelIdentity = Get-GateProcessIdentity -Process $sentinelCim

        $active = @(Get-ActiveGateProcesses -Scope $Scope)
        $listeners = @(Get-GateProcessListeners -Processes $active)
        while ($listeners.Count -eq 0 -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 50
            $active = @(Get-ActiveGateProcesses -Scope $Scope)
            $listeners = @(Get-GateProcessListeners -Processes $active)
        }
        if ($active.Count -lt 1 -or $listeners.Count -lt 1) {
            throw 'The cleanup probe did not causally observe its process and listener.'
        }
        $cleanup = Stop-GateProcessScope -Scope $Scope
        $process.WaitForExit()
        if ($sentinel.HasExited) {
            throw 'Owned Job cleanup terminated the concurrent independent sentinel.'
        }
        if ($Scope.OwnedProcessRecords.ContainsKey($sentinelIdentity)) {
            throw 'The concurrent independent sentinel was misclassified as owned.'
        }
        if ($cleanup.stoppedProcessCount -lt 1 -or
                $cleanup.listenersBefore -lt 1 -or
                $cleanup.processesAfter -ne 0 -or
                $cleanup.listenersAfter -ne 0) {
            throw 'The cleanup probe did not terminate and verify its complete owned state.'
        }
        $cleanup | Add-Member `
            -NotePropertyName independentSentinelSurvived `
            -NotePropertyValue $true
        return $cleanup
    }
    finally {
        if ($null -eq $cleanup -and $Scope.OwnedJobs.Count -ne 0) {
            try { [void] (Stop-GateProcessScope -Scope $Scope) } catch {}
        }
        if ($null -ne $process) {
            if (-not $process.HasExited) {
                try { $process.Kill() } catch {}
                try { $process.WaitForExit() } catch {}
            }
            $process.Dispose()
        }
        if ($null -ne $sentinel) {
            if (-not $sentinel.HasExited) {
                try { $sentinel.Kill() } catch {}
                try { $sentinel.WaitForExit() } catch {}
            }
            $sentinel.Dispose()
        }
    }
}
