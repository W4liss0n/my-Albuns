function Test-ProcessStartIdentity {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process,
        [Parameter(Mandatory = $true)]
        [DateTime] $ExpectedStartTimeUtc
    )

    try {
        $actualStartTimeUtc = $Process.StartTime.ToUniversalTime()
    }
    catch {
        return $false
    }

    return $actualStartTimeUtc.Ticks -eq $ExpectedStartTimeUtc.Ticks
}

function Get-ProcessTreeIds {
    param([Parameter(Mandatory = $true)][uint32] $RootProcessId)

    $allProcesses = @(Get-CimInstance Win32_Process)
    $rootCandidates = @(
        $allProcesses |
            Where-Object { [uint32] $_.ProcessId -eq $RootProcessId }
    )
    if ($rootCandidates.Count -ne 1) {
        return @()
    }

    $entriesById = @{}
    $entriesById[$RootProcessId] = [pscustomobject]@{
        ProcessId = $RootProcessId
        CreationUtc = (
            [DateTime] $rootCandidates[0].CreationDate
        ).ToUniversalTime()
    }
    do {
        $added = $false
        foreach ($candidate in $allProcesses) {
            $candidateId = [uint32] $candidate.ProcessId
            $parentId = [uint32] $candidate.ParentProcessId
            if (
                $entriesById.ContainsKey($candidateId) -or
                -not $entriesById.ContainsKey($parentId)
            ) {
                continue
            }

            $creationUtc = ([DateTime] $candidate.CreationDate).ToUniversalTime()
            if ($creationUtc -lt $entriesById[$parentId].CreationUtc) {
                continue
            }
            $entriesById[$candidateId] = [pscustomobject]@{
                ProcessId = $candidateId
                CreationUtc = $creationUtc
            }
            $added = $true
        }
    } while ($added)

    return @($entriesById.Keys | Sort-Object)
}

function New-ProcessIdentityEntry {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process,
        [Parameter(Mandatory = $true)]
        [uint32] $ParentProcessId,
        [Parameter(Mandatory = $true)]
        [DateTime] $CreationUtc,
        [Parameter(Mandatory = $true)]
        [int] $Depth
    )

    $startTimeUtc = $Process.StartTime.ToUniversalTime()
    $Process.Refresh()
    $hasExited = $Process.HasExited
    return [pscustomobject]@{
        ProcessId = [uint32] $Process.Id
        ParentProcessId = $ParentProcessId
        CreationUtc = $CreationUtc
        StartTimeUtc = $startTimeUtc
        ExitTimeUtc = if ($hasExited) {
            $Process.ExitTime.ToUniversalTime()
        }
        else {
            $null
        }
        Depth = $Depth
        Process = $Process
        IsAlive = -not $hasExited
    }
}

function Update-ProcessIdentityEntry {
    param([Parameter(Mandatory = $true)][object] $Entry)

    try {
        $Entry.Process.Refresh()
        if ($Entry.Process.HasExited) {
            $Entry.IsAlive = $false
            if ($null -eq $Entry.ExitTimeUtc) {
                $Entry.ExitTimeUtc = $Entry.Process.ExitTime.ToUniversalTime()
            }
            return
        }
        $Entry.IsAlive = Test-ProcessStartIdentity `
            -Process $Entry.Process `
            -ExpectedStartTimeUtc $Entry.StartTimeUtc
    }
    catch {
        $Entry.IsAlive = $false
    }
}

function Add-ValidatedProcessDescendants {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable] $EntriesById
    )

    foreach ($entry in @($EntriesById.Values)) {
        Update-ProcessIdentityEntry -Entry $entry
    }
    $allProcesses = @(Get-CimInstance Win32_Process)
    $addedCount = 0
    do {
        $added = $false
        foreach ($candidate in $allProcesses) {
            $candidateId = [uint32] $candidate.ProcessId
            $parentId = [uint32] $candidate.ParentProcessId
            if (
                $EntriesById.ContainsKey($candidateId) -or
                -not $EntriesById.ContainsKey($parentId)
            ) {
                continue
            }

            $parent = $EntriesById[$parentId]
            $creationUtc = ([DateTime] $candidate.CreationDate).ToUniversalTime()
            if (
                $creationUtc -lt $parent.CreationUtc -or
                (
                    -not $parent.IsAlive -and
                    (
                        $null -eq $parent.ExitTimeUtc -or
                        $creationUtc -gt $parent.ExitTimeUtc
                    )
                )
            ) {
                continue
            }
            if (
                $parent.IsAlive -and
                -not (
                    Test-ProcessStartIdentity `
                        -Process $parent.Process `
                        -ExpectedStartTimeUtc $parent.StartTimeUtc
                )
            ) {
                continue
            }

            $process = Get-Process `
                -Id $candidateId `
                -ErrorAction SilentlyContinue
            if ($null -eq $process) {
                continue
            }
            try {
                $entry = New-ProcessIdentityEntry `
                    -Process $process `
                    -ParentProcessId $parentId `
                    -CreationUtc $creationUtc `
                    -Depth ($parent.Depth + 1)
            }
            catch {
                continue
            }

            $confirmed = Get-CimInstance `
                Win32_Process `
                -Filter "ProcessId = $candidateId" `
                -ErrorAction SilentlyContinue
            if (
                $null -eq $confirmed -or
                [uint32] $confirmed.ParentProcessId -ne $parentId -or
                (
                    ([DateTime] $confirmed.CreationDate).ToUniversalTime().Ticks -ne
                    $creationUtc.Ticks
                ) -or
                -not (
                    Test-ProcessStartIdentity `
                        -Process $entry.Process `
                        -ExpectedStartTimeUtc $entry.StartTimeUtc
                )
            ) {
                continue
            }

            $EntriesById[$candidateId] = $entry
            $addedCount++
            $added = $true
        }
    } while ($added)

    return $addedCount
}

function Stop-ValidatedProcessEntry {
    param([Parameter(Mandatory = $true)][object] $Entry)

    Update-ProcessIdentityEntry -Entry $Entry
    if (
        -not $Entry.IsAlive -or
        -not (
            Test-ProcessStartIdentity `
                -Process $Entry.Process `
                -ExpectedStartTimeUtc $Entry.StartTimeUtc
        )
    ) {
        return
    }

    Stop-Process `
        -InputObject $Entry.Process `
        -Force `
        -ErrorAction SilentlyContinue
    [void] $Entry.Process.WaitForExit(1000)
    Update-ProcessIdentityEntry -Entry $Entry
}

function Stop-StartedProcessTree {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $RootProcess
    )

    $rootStartTimeUtc = $RootProcess.StartTime.ToUniversalTime()
    $RootProcess.Refresh()
    $rootHasExited = $RootProcess.HasExited
    $rootEntry = New-ProcessIdentityEntry `
        -Process $RootProcess `
        -ParentProcessId 0 `
        -CreationUtc $rootStartTimeUtc `
        -Depth 0
    $entriesById = @{
        ([uint32] $RootProcess.Id) = $rootEntry
    }
    [void] (Add-ValidatedProcessDescendants -EntriesById $entriesById)

    if (-not $rootHasExited) {
        Stop-ValidatedProcessEntry -Entry $rootEntry
    }

    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $stablePasses = 0
    do {
        [void] (Add-ValidatedProcessDescendants -EntriesById $entriesById)
        foreach (
            $entry in @(
                $entriesById.Values |
                    Where-Object {
                        $_.ProcessId -ne $rootEntry.ProcessId -and
                        $_.IsAlive
                    } |
                    Sort-Object Depth -Descending
            )
        ) {
            Stop-ValidatedProcessEntry -Entry $entry
        }

        $addedAfterStop = Add-ValidatedProcessDescendants `
            -EntriesById $entriesById
        $remaining = @(
            $entriesById.Values |
                Where-Object { $_.IsAlive }
        )
        if ($remaining.Count -eq 0 -and $addedAfterStop -eq 0) {
            $stablePasses++
            if ($stablePasses -ge 2) {
                return
            }
        }
        else {
            $stablePasses = 0
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)

    throw "Process tree did not terminate: $(@($remaining.ProcessId) -join ', ')."
}
