param(
    [Parameter(Mandatory)] [string] $OutputPath,
    [Parameter(Mandatory)] [string] $StopPath,
    [int] $IntervalMilliseconds = 1000
)

# Samples system counters ($OutputPath) and per-role process usage
# (roles.csv beside it) once per interval until $StopPath exists. Roles split
# the app by what each process does: the Project host and the launcher of
# myalbuns-desktop, the WebView2 browser, renderer, GPU and other processes,
# and the imaging Processor. Reused .NET performance counters accept the
# English names on a Portuguese Windows and cost ~20 ms per sample; command
# lines are read through CIM only when a new process appears. The sampler
# records its own CPU so its cost is visible in the summary.
$ErrorActionPreference = 'Stop'
$invariant = [Globalization.CultureInfo]::InvariantCulture
$logicalProcessors = [Environment]::ProcessorCount
$processNames = @('myalbuns-desktop', 'msedgewebview2', 'myalbuns-imaging')
$roles = @('app-project', 'app-global', 'webview-browser', 'webview-renderer', 'webview-gpu', 'webview-other', 'imaging', 'sampler')
$system = [ordered] @{
    cpuPercent = [Diagnostics.PerformanceCounter]::new('Processor', '% Processor Time', '_Total')
    availableMB = [Diagnostics.PerformanceCounter]::new('Memory', 'Available MBytes')
    committedBytes = [Diagnostics.PerformanceCounter]::new('Memory', 'Committed Bytes')
    pagesInputPerSec = [Diagnostics.PerformanceCounter]::new('Memory', 'Pages Input/sec')
    diskPercent = [Diagnostics.PerformanceCounter]::new('PhysicalDisk', '% Disk Time', '_Total')
    diskQueue = [Diagnostics.PerformanceCounter]::new('PhysicalDisk', 'Avg. Disk Queue Length', '_Total')
    diskReadBytesPerSec = [Diagnostics.PerformanceCounter]::new('PhysicalDisk', 'Disk Read Bytes/sec', '_Total')
}
foreach ($counter in $system.Values) {
    [void] $counter.NextValue()
}

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class HardwareLabIo {
    [StructLayout(LayoutKind.Sequential)]
    struct IoCounters {
        public ulong ReadOperations, WriteOperations, OtherOperations, ReadBytes, WriteBytes, OtherBytes;
    }
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetProcessIoCounters(IntPtr process, out IoCounters counters);
    public static ulong[] Read(IntPtr process) {
        IoCounters counters;
        return GetProcessIoCounters(process, out counters)
            ? new ulong[] { counters.ReadBytes, counters.WriteBytes }
            : new ulong[] { 0, 0 };
    }
}
'@

function Format-Number([double] $value) {
    $value.ToString('0.0', $invariant)
}

$roleByKey = @{}

function Resolve-Role([string] $name, [string] $commandLine) {
    switch ($name) {
        'myalbuns-imaging' { return 'imaging' }
        'myalbuns-desktop' {
            if ($commandLine -match '--myalbuns-project-host') { return 'app-project' }
            return 'app-global'
        }
        default {
            if ($commandLine -notmatch '--type=(\S+)') { return 'webview-browser' }
            switch ($Matches[1]) {
                'renderer' { return 'webview-renderer' }
                'gpu-process' { return 'webview-gpu' }
                default { return 'webview-other' }
            }
        }
    }
}

# Processor time, private memory and I/O totals per running process, keyed by
# id and start time so a reused id is a new process.
function Read-Processes {
    $snapshot = @{}
    $unknown = $false
    $processes = @(Get-Process -Name $processNames -ErrorAction SilentlyContinue) + @(Get-Process -Id $PID)
    foreach ($process in $processes) {
        try {
            $key = '{0}:{1}' -f $process.Id, $process.StartTime.Ticks
            $io = [HardwareLabIo]::Read($process.Handle)
            $snapshot[$key] = [pscustomobject] @{
                id = $process.Id
                name = $process.ProcessName
                cpuMs = $process.TotalProcessorTime.TotalMilliseconds
                privateBytes = $process.PrivateMemorySize64
                readBytes = [double] $io[0]
                writeBytes = [double] $io[1]
            }
            if ($process.Id -eq $PID) {
                $roleByKey[$key] = 'sampler'
            }
            elseif (-not $roleByKey.ContainsKey($key)) {
                $unknown = $true
            }
        }
        catch {
            # The process ended between the listing and the read.
        }
        finally {
            $process.Dispose()
        }
    }
    if ($unknown) {
        $commandLines = @{}
        Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe' OR Name='myalbuns-desktop.exe' OR Name='myalbuns-imaging.exe'" |
            ForEach-Object { $commandLines[[int] $_.ProcessId] = [string] $_.CommandLine }
        foreach ($key in @($snapshot.Keys)) {
            if (-not $roleByKey.ContainsKey($key)) {
                $entry = $snapshot[$key]
                $roleByKey[$key] = Resolve-Role $entry.name $commandLines[[int] $entry.id]
            }
        }
    }
    $snapshot
}

$writer = [IO.StreamWriter]::new($OutputPath, $false, [Text.UTF8Encoding]::new($false))
$writer.AutoFlush = $true
$writer.WriteLine(
    'timestamp,cpuPercent,availableMB,committedMB,pagesInputPerSec,diskPercent,diskQueue,diskReadMBps,' +
    'appPrivateMB,webviewPrivateMB,imagingPrivateMB,appCpuPercent,webviewCpuPercent,imagingCpuPercent,samplerCpuPercent')
$rolesPath = Join-Path (Split-Path $OutputPath) 'roles.csv'
$roleWriter = [IO.StreamWriter]::new($rolesPath, $false, [Text.UTF8Encoding]::new($false))
$roleWriter.AutoFlush = $true
$roleWriter.WriteLine('timestamp,role,processes,cpuPercent,privateMB,readMB,writeMB')

$previous = Read-Processes
$previousTime = [DateTime]::UtcNow
try {
    while (-not (Test-Path -LiteralPath $StopPath)) {
        Start-Sleep -Milliseconds $IntervalMilliseconds
        $now = [DateTime]::UtcNow
        $elapsedMs = ($now - $previousTime).TotalMilliseconds
        $previousTime = $now
        $current = Read-Processes

        $usage = @{}
        foreach ($role in $roles) {
            $usage[$role] = @{ processes = 0; cpuMs = 0.0; privateBytes = 0.0; readBytes = 0.0; writeBytes = 0.0 }
        }
        foreach ($key in $current.Keys) {
            $entry = $current[$key]
            $roleName = $roleByKey[$key]
            $role = $usage[$(if ($roleName) { $roleName } else { 'webview-other' })]
            # A process that started during the interval counts from zero.
            $before = $previous[$key]
            $role.processes += 1
            $role.cpuMs += $entry.cpuMs - $(if ($before) { $before.cpuMs } else { 0 })
            $role.readBytes += $entry.readBytes - $(if ($before) { $before.readBytes } else { 0 })
            $role.writeBytes += $entry.writeBytes - $(if ($before) { $before.writeBytes } else { 0 })
            $role.privateBytes += $entry.privateBytes
        }
        $previous = $current
        $share = { param($cpuMs) 100 * $cpuMs / ($elapsedMs * $logicalProcessors) }
        $sum = {
            param($prefix, $field)
            ($roles | Where-Object { $_.StartsWith($prefix) } | ForEach-Object { $usage[$_][$field] } | Measure-Object -Sum).Sum
        }
        $stamp = $now.ToString('o')

        $writer.WriteLine((@(
                    $stamp,
                    (Format-Number $system.cpuPercent.NextValue()),
                    (Format-Number $system.availableMB.NextValue()),
                    (Format-Number ($system.committedBytes.NextValue() / 1MB)),
                    (Format-Number $system.pagesInputPerSec.NextValue()),
                    (Format-Number $system.diskPercent.NextValue()),
                    (Format-Number $system.diskQueue.NextValue()),
                    (Format-Number ($system.diskReadBytesPerSec.NextValue() / 1MB)),
                    (Format-Number ((& $sum 'app-' 'privateBytes') / 1MB)),
                    (Format-Number ((& $sum 'webview-' 'privateBytes') / 1MB)),
                    (Format-Number ($usage.imaging.privateBytes / 1MB)),
                    (Format-Number (& $share (& $sum 'app-' 'cpuMs'))),
                    (Format-Number (& $share (& $sum 'webview-' 'cpuMs'))),
                    (Format-Number (& $share $usage.imaging.cpuMs)),
                    (Format-Number (& $share $usage.sampler.cpuMs))
                ) -join ','))
        foreach ($role in $roles) {
            $entry = $usage[$role]
            if ($entry.processes -eq 0) {
                continue
            }
            $roleWriter.WriteLine((@(
                        $stamp,
                        $role,
                        $entry.processes,
                        (Format-Number (& $share $entry.cpuMs)),
                        (Format-Number ($entry.privateBytes / 1MB)),
                        (Format-Number ($entry.readBytes / 1MB)),
                        (Format-Number ($entry.writeBytes / 1MB))
                    ) -join ','))
        }
    }
}
finally {
    $writer.Dispose()
    $roleWriter.Dispose()
    foreach ($counter in $system.Values) {
        $counter.Dispose()
    }
    # Tells the runner the CSVs are closed and safe to copy.
    New-Item -ItemType File -Path "$OutputPath.done" -Force | Out-Null
}
