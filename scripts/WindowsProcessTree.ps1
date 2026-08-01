function Get-ProcessTreeIds {
    param([Parameter(Mandatory = $true)][uint32] $RootProcessId)

    $allProcesses = @(Get-CimInstance Win32_Process)
    $treeIds = [System.Collections.Generic.HashSet[uint32]]::new()
    [void] $treeIds.Add($RootProcessId)
    do {
        $added = $false
        foreach ($candidate in $allProcesses) {
            if (
                $treeIds.Contains([uint32] $candidate.ParentProcessId) -and
                $treeIds.Add([uint32] $candidate.ProcessId)
            ) {
                $added = $true
            }
        }
    } while ($added)

    return @($treeIds | Sort-Object)
}

function Stop-StartedProcessTree {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $RootProcess
    )

    $rootProcessId = [uint32] $RootProcess.Id
    $capturedProcesses = @(
        $RootProcess
        Get-ProcessTreeIds -RootProcessId $rootProcessId |
            Where-Object { $_ -ne $rootProcessId } |
            ForEach-Object {
                Get-Process -Id $_ -ErrorAction SilentlyContinue
            }
    )
    foreach (
        $process in @(
            $capturedProcesses |
                Where-Object { $_.Id -ne $rootProcessId }
        )
    ) {
        if (-not $process.HasExited) {
            Stop-Process -InputObject $process -Force -ErrorAction SilentlyContinue
        }
    }
    if (-not $RootProcess.HasExited) {
        Stop-Process -InputObject $RootProcess -Force -ErrorAction SilentlyContinue
    }

    $recapturedProcesses = @(
        Get-ProcessTreeIds -RootProcessId $rootProcessId |
            Where-Object { $_ -ne $rootProcessId } |
            ForEach-Object {
                Get-Process -Id $_ -ErrorAction SilentlyContinue
            }
    )
    foreach ($process in $recapturedProcesses) {
        if (-not $process.HasExited) {
            Stop-Process -InputObject $process -Force -ErrorAction SilentlyContinue
        }
    }
    $capturedProcesses = @($capturedProcesses) + @($recapturedProcesses)

    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        $remaining = @(
            $capturedProcesses |
                Where-Object {
                    $_.Refresh()
                    -not $_.HasExited
                }
        )
        if ($remaining.Count -eq 0) {
            return
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)

    throw "Process tree did not terminate: $(@($remaining.Id) -join ', ')."
}
