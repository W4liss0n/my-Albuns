param(
    [switch] $SkipBuild,
    [ValidateRange(10, 120)]
    [int] $WindowTimeoutSeconds = 45,
    [string] $OutputPath
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

$targetDirectory = Join-Path $script:WorkspaceRoot '.scratch\topology-spike-target'
$executablePath = Join-Path $targetDirectory 'debug\myalbuns-desktop.exe'
$executableRelativePath = '.scratch/topology-spike-target/debug/myalbuns-desktop.exe'
$topologyEnvironment = 'MYALBUNS_TOPOLOGY_SPIKE'
$projectEnvironment = 'MYALBUNS_TOPOLOGY_PROJECT'
$startedProcessIds = [System.Collections.Generic.List[int]]::new()

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot 'docs\research\artifacts\0001-topology-spike-baseline.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

$reportText = @'
{
  "notMeasured": [
    "lat\u00eancia de Pan/Zoom",
    "vaz\u00e3o do Cache",
    "dura\u00e7\u00e3o da Exporta\u00e7\u00e3o",
    "recupera\u00e7\u00e3o persistida",
    "complexidade operacional da IPC"
  ],
  "notes": [
    "Baseline preliminar do esqueleto de topologia; isto n\u00e3o encerra o spike.",
    "A mem\u00f3ria inclui o host e todos os processos descendentes observados.",
    "A queda s\u00f3 \u00e9 for\u00e7ada depois de validar o caminho do execut\u00e1vel do PID alvo.",
    "Os hosts independentes s\u00e3o iniciados em sequ\u00eancia depois que uma tentativa simult\u00e2nea deixou intermitentemente um host sem Janela vis\u00edvel."
  ],
  "summary": {
    "title": "Baseline preliminar das topologias",
    "collected": "Coletado em UTC",
    "raw": "JSON bruto",
    "measure": "Medida",
    "independent": "A \u2014 hosts independentes",
    "multiwindow": "B \u2014 host multiwindow",
    "hosts": "Hosts do Projeto",
    "windows": "Janelas do Projeto",
    "processes": "Processos na \u00e1rvore",
    "workingSet": "Working set agregado",
    "privateMemory": "Mem\u00f3ria privada agregada",
    "gpuMemory": "Mem\u00f3ria gr\u00e1fica compartilhada",
    "firstHost": "Primeiro host de A identificado",
    "twoWindows": "Duas Janelas identificadas",
    "afterCrash": "Janelas depois da queda for\u00e7ada",
    "notApplicable": "n\u00e3o se aplica",
    "otherPreserved": "outra Janela preservada",
    "build": "Build medida",
    "commit": "Commit do c\u00f3digo",
    "profile": "Perfil",
    "workingTreeDirty": "\u00c1rvore de trabalho tinha mudan\u00e7as alheias",
    "buildInputsDirty": "Entradas da build tinham mudan\u00e7as",
    "yes": "sim",
    "no": "n\u00e3o",
    "environment": "Ambiente registrado",
    "operatingSystem": "Sistema",
    "cpu": "Processador",
    "physicalMemory": "Mem\u00f3ria f\u00edsica",
    "notMeasured": "Campos ainda n\u00e3o medidos",
    "notes": "Observa\u00e7\u00f5es"
  }
}
'@ | ConvertFrom-Json

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public sealed class MyAlbunsWindowInfo
{
    public long Handle { get; set; }
    public int ProcessId { get; set; }
    public string Title { get; set; }
}

public static class MyAlbunsWindowProbe
{
    private delegate bool EnumWindowsCallback(IntPtr handle, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsCallback callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr handle);

    [DllImport("user32.dll")]
    private static extern int GetWindowTextLength(IntPtr handle);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr handle, StringBuilder title, int maximumCount);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr handle, out uint processId);

    public static MyAlbunsWindowInfo[] VisibleWindowsFor(int[] processIds)
    {
        var expected = new HashSet<int>(processIds);
        var windows = new List<MyAlbunsWindowInfo>();
        EnumWindows(delegate(IntPtr handle, IntPtr parameter)
        {
            uint processId;
            GetWindowThreadProcessId(handle, out processId);
            if (!expected.Contains((int)processId) || !IsWindowVisible(handle))
            {
                return true;
            }

            var length = GetWindowTextLength(handle);
            if (length == 0)
            {
                return true;
            }

            var title = new StringBuilder(length + 1);
            GetWindowText(handle, title, title.Capacity);
            windows.Add(new MyAlbunsWindowInfo
            {
                Handle = handle.ToInt64(),
                ProcessId = (int)processId,
                Title = title.ToString()
            });
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }
}
'@

function Set-ProcessEnvironmentValue {
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

function Start-TopologyProcess {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [ValidateSet('a', 'b')]
        [string] $Project
    )

    $previousTopology = [System.Environment]::GetEnvironmentVariable(
        $topologyEnvironment,
        [System.EnvironmentVariableTarget]::Process
    )
    $previousProject = [System.Environment]::GetEnvironmentVariable(
        $projectEnvironment,
        [System.EnvironmentVariableTarget]::Process
    )
    try {
        Set-ProcessEnvironmentValue -Name $topologyEnvironment -Value $Topology
        if ([string]::IsNullOrWhiteSpace($Project)) {
            Set-ProcessEnvironmentValue -Name $projectEnvironment -Value $null
        }
        else {
            Set-ProcessEnvironmentValue -Name $projectEnvironment -Value $Project
        }

        $process = Start-Process `
            -FilePath $executablePath `
            -WorkingDirectory $script:WorkspaceRoot `
            -PassThru
        $startedProcessIds.Add($process.Id)
        return $process
    }
    finally {
        Set-ProcessEnvironmentValue -Name $topologyEnvironment -Value $previousTopology
        Set-ProcessEnvironmentValue -Name $projectEnvironment -Value $previousProject
    }
}

function Assert-OwnedTopologyProcess {
    param([Parameter(Mandatory = $true)][int] $ProcessId)

    $process = Get-CimInstance Win32_Process -Filter "ProcessId = $ProcessId"
    if ($null -eq $process) {
        throw "Topology process $ProcessId is no longer running."
    }
    if (-not [string]::Equals(
        [System.IO.Path]::GetFullPath($process.ExecutablePath),
        [System.IO.Path]::GetFullPath($executablePath),
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw "Process $ProcessId does not belong to the topology spike executable."
    }
}

function Stop-OwnedTopologyProcess {
    param([Parameter(Mandatory = $true)][int] $ProcessId)

    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        return
    }
    Assert-OwnedTopologyProcess -ProcessId $ProcessId
    Stop-Process -Id $ProcessId -Force
    Wait-Process -Id $ProcessId -Timeout 10 -ErrorAction SilentlyContinue
}

function Wait-ForTopologyWindows {
    param(
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedCount,
        [Parameter(Mandatory = $true)]
        [string] $ExpectedTitleMarker,
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Stopwatch] $Stopwatch
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($WindowTimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        foreach ($processId in $RootProcessIds) {
            if ($null -eq (Get-Process -Id $processId -ErrorAction SilentlyContinue)) {
                throw "Topology host $processId exited before its windows were ready."
            }
        }

        $windows = [MyAlbunsWindowProbe]::VisibleWindowsFor($RootProcessIds)
        $unexpectedTitles = @(
            $windows | Where-Object {
                -not ($_.Title.Contains($ExpectedTitleMarker))
            }
        )
        if ($windows.Count -eq $ExpectedCount -and $unexpectedTitles.Count -eq 0) {
            return [ordered]@{
                elapsedMs = $Stopwatch.ElapsedMilliseconds
                windows = @($windows | Sort-Object ProcessId, Handle | ForEach-Object {
                    [ordered]@{
                        processId = $_.ProcessId
                        title = $_.Title
                    }
                })
            }
        }
        Start-Sleep -Milliseconds 100
    }

    $observedWindows = @(
        [MyAlbunsWindowProbe]::VisibleWindowsFor($RootProcessIds) |
            ForEach-Object { "$($_.ProcessId): $($_.Title)" }
    )
    throw (
        "Expected $ExpectedCount visible topology windows with marker " +
        "'$ExpectedTitleMarker' within $WindowTimeoutSeconds seconds. " +
        "Observed: $($observedWindows -join '; ')"
    )
}

function Get-ProcessTreeIds {
    param([Parameter(Mandatory = $true)][int[]] $RootProcessIds)

    $allProcesses = @(Get-CimInstance Win32_Process)
    $known = [System.Collections.Generic.HashSet[int]]::new()
    $queue = [System.Collections.Generic.Queue[int]]::new()
    foreach ($rootProcessId in $RootProcessIds) {
        [void] $known.Add($rootProcessId)
        $queue.Enqueue($rootProcessId)
    }

    while ($queue.Count -gt 0) {
        $parentId = $queue.Dequeue()
        foreach ($child in $allProcesses | Where-Object { $_.ParentProcessId -eq $parentId }) {
            if ($known.Add([int] $child.ProcessId)) {
                $queue.Enqueue([int] $child.ProcessId)
            }
        }
    }
    return @($known | Sort-Object)
}

function Get-GpuMemory {
    param([Parameter(Mandatory = $true)][int[]] $ProcessIds)

    try {
        $samples = (Get-Counter -Counter @(
            '\GPU Process Memory(*)\Dedicated Usage',
            '\GPU Process Memory(*)\Shared Usage'
        ) -ErrorAction Stop).CounterSamples
        $dedicated = 0.0
        $shared = 0.0
        foreach ($sample in $samples) {
            if ($sample.InstanceName -notmatch 'pid_(\d+)_') {
                continue
            }
            if ([int] $matches[1] -notin $ProcessIds) {
                continue
            }
            if ($sample.Path -like '*Dedicated Usage') {
                $dedicated += $sample.CookedValue
            }
            elseif ($sample.Path -like '*Shared Usage') {
                $shared += $sample.CookedValue
            }
        }
        return [ordered]@{
            available = $true
            dedicatedBytes = [long] $dedicated
            sharedBytes = [long] $shared
        }
    }
    catch {
        return [ordered]@{
            available = $false
            reason = $_.Exception.Message
        }
    }
}

function Measure-TopologyProcesses {
    param([Parameter(Mandatory = $true)][int[]] $RootProcessIds)

    $treeProcessIds = @(Get-ProcessTreeIds -RootProcessIds $RootProcessIds)
    $processes = @($treeProcessIds | ForEach-Object {
        Get-Process -Id $_ -ErrorAction SilentlyContinue
    })
    return [ordered]@{
        hostProcessCount = $RootProcessIds.Count
        processTreeCount = $processes.Count
        workingSetBytes = [long] (($processes | Measure-Object WorkingSet64 -Sum).Sum)
        privateMemoryBytes = [long] (($processes | Measure-Object PrivateMemorySize64 -Sum).Sum)
        handleCount = [long] (($processes | Measure-Object HandleCount -Sum).Sum)
        threadCount = [long] (($processes | ForEach-Object { $_.Threads.Count } | Measure-Object -Sum).Sum)
        processTree = @($processes | Sort-Object Id | ForEach-Object {
            [ordered]@{
                processId = $_.Id
                name = $_.ProcessName
                workingSetBytes = $_.WorkingSet64
                privateMemoryBytes = $_.PrivateMemorySize64
            }
        })
        gpuMemory = Get-GpuMemory -ProcessIds $treeProcessIds
    }
}

function Get-HardwareInventory {
    $operatingSystem = Get-CimInstance Win32_OperatingSystem
    $computer = Get-CimInstance Win32_ComputerSystem
    return [ordered]@{
        operatingSystem = [ordered]@{
            caption = $operatingSystem.Caption
            version = $operatingSystem.Version
            buildNumber = $operatingSystem.BuildNumber
        }
        cpu = @(
            Get-CimInstance Win32_Processor |
                Select-Object -ExpandProperty Name
        )
        totalPhysicalMemoryBytes = [long] $computer.TotalPhysicalMemory
        gpu = @(
            Get-CimInstance Win32_VideoController | ForEach-Object {
                [ordered]@{
                    name = $_.Name
                    adapterRamBytes = [long] $_.AdapterRAM
                    driverVersion = $_.DriverVersion
                }
            }
        )
    }
}

function Format-Mebibytes {
    param([Parameter(Mandatory = $true)][long] $Bytes)

    return ($Bytes / 1MB).ToString(
        'N1',
        [System.Globalization.CultureInfo]::GetCultureInfo('pt-BR')
    )
}

function Write-TopologyMarkdownSummary {
    param(
        [Parameter(Mandatory = $true)]
        $Report,
        [Parameter(Mandatory = $true)]
        $Text,
        [Parameter(Mandatory = $true)]
        [string] $SummaryPath
    )

    $summary = $Text.summary
    $independent = $Report.alternatives.independentHosts
    $multiwindow = $Report.alternatives.multiwindowHost
    $collectedDate = ([DateTime] $Report.collectedAtUtc).ToString('yyyy-MM-dd')
    $yes = $summary.yes
    $no = $summary.no
    $workingTreeDirty = if ($Report.build.workingTreeDirty) { $yes } else { $no }
    $buildInputsDirty = if ($Report.build.buildInputsDirty) { $yes } else { $no }
    $independentAfterCrash = if (
        $independent.forcedFailure.otherHostSurvived
    ) {
        "$($independent.forcedFailure.remainingWindowCount) ($($summary.otherPreserved))"
    }
    else {
        "$($independent.forcedFailure.remainingWindowCount)"
    }
    $multiwindowAfterCrash = "$($multiwindow.forcedFailure.remainingWindowCount)"

    $markdown = @(
        '---'
        'status: current'
        'document: generated-research-artifact'
        'ticket: 01-plataforma-e-arquitetura'
        "date: $collectedDate"
        "updated: $collectedDate"
        '---'
        ''
        "# $($summary.title)"
        ''
        "$($summary.collected): ``$($Report.collectedAtUtc)``."
        "[$($summary.raw)](0001-topology-spike-baseline.json)."
        ''
        "| $($summary.measure) | $($summary.independent) | $($summary.multiwindow) |"
        '|---|---:|---:|'
        "| $($summary.hosts) | $($independent.processes.hostProcessCount) | $($multiwindow.processes.hostProcessCount) |"
        "| $($summary.windows) | $($independent.ready.windows.Count) | $($multiwindow.ready.windows.Count) |"
        "| $($summary.processes) | $($independent.processes.processTreeCount) | $($multiwindow.processes.processTreeCount) |"
        "| $($summary.workingSet) | $(Format-Mebibytes $independent.processes.workingSetBytes) MiB | $(Format-Mebibytes $multiwindow.processes.workingSetBytes) MiB |"
        "| $($summary.privateMemory) | $(Format-Mebibytes $independent.processes.privateMemoryBytes) MiB | $(Format-Mebibytes $multiwindow.processes.privateMemoryBytes) MiB |"
        "| $($summary.gpuMemory) | $(Format-Mebibytes $independent.processes.gpuMemory.sharedBytes) MiB | $(Format-Mebibytes $multiwindow.processes.gpuMemory.sharedBytes) MiB |"
        "| $($summary.firstHost) | $($independent.ready.firstHostElapsedMs) ms | $($summary.notApplicable) |"
        "| $($summary.twoWindows) | $($independent.ready.elapsedMs) ms | $($multiwindow.ready.elapsedMs) ms |"
        "| $($summary.afterCrash) | $independentAfterCrash | $multiwindowAfterCrash |"
        ''
        "## $($summary.build)"
        ''
        "- $($summary.commit): ``$($Report.build.gitCommit)``"
        "- $($summary.profile): ``$($Report.build.profile)``"
        "- $($summary.workingTreeDirty): $workingTreeDirty"
        "- $($summary.buildInputsDirty): $buildInputsDirty"
        ''
        "## $($summary.environment)"
        ''
        "- $($summary.operatingSystem): $($Report.hardware.operatingSystem.caption) ``$($Report.hardware.operatingSystem.version)``"
        "- $($summary.cpu): $($Report.hardware.cpu -join '; ')"
        "- $($summary.physicalMemory): $(Format-Mebibytes $Report.hardware.totalPhysicalMemoryBytes) MiB"
        ''
        "## $($summary.notMeasured)"
        ''
    )
    $markdown += @($Report.notMeasured | ForEach-Object { "- $_" })
    $markdown += @(
        ''
        "## $($summary.notes)"
        ''
    )
    $markdown += @($Report.notes | ForEach-Object { "- $_" })

    [System.IO.File]::WriteAllText(
        $SummaryPath,
        ($markdown -join [System.Environment]::NewLine) +
            [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
}

$previousCargoTarget = [System.Environment]::GetEnvironmentVariable(
    'CARGO_TARGET_DIR',
    [System.EnvironmentVariableTarget]::Process
)

try {
    if (-not $SkipBuild) {
        Set-ProcessEnvironmentValue -Name 'CARGO_TARGET_DIR' -Value $targetDirectory
        & (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1') build --debug --no-bundle
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }
    if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
        throw "Topology spike executable not found at $executablePath."
    }

    $independentStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $independentA = Start-TopologyProcess -Topology independent -Project a
    $independentFirstReady = Wait-ForTopologyWindows `
        -RootProcessIds @($independentA.Id) `
        -ExpectedCount 1 `
        -ExpectedTitleMarker '[Topologia A]' `
        -Stopwatch $independentStopwatch
    $independentB = Start-TopologyProcess -Topology independent -Project b
    $independentReady = Wait-ForTopologyWindows `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedCount 2 `
        -ExpectedTitleMarker '[Topologia A]' `
        -Stopwatch $independentStopwatch
    $independentReady['firstHostElapsedMs'] = $independentFirstReady.elapsedMs
    Start-Sleep -Milliseconds 750
    $independentMetrics = Measure-TopologyProcesses `
        -RootProcessIds @($independentA.Id, $independentB.Id)

    Stop-OwnedTopologyProcess -ProcessId $independentA.Id
    Start-Sleep -Milliseconds 750
    $independentFailureIsolation = [ordered]@{
        forcedHostProcessId = $independentA.Id
        otherHostSurvived = $null -ne (
            Get-Process -Id $independentB.Id -ErrorAction SilentlyContinue
        )
        remainingWindowCount = [MyAlbunsWindowProbe]::VisibleWindowsFor(
            @($independentB.Id)
        ).Count
    }
    Stop-OwnedTopologyProcess -ProcessId $independentB.Id

    Start-Sleep -Milliseconds 750
    $multiwindowStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $multiwindow = Start-TopologyProcess -Topology multiwindow
    $multiwindowReady = Wait-ForTopologyWindows `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedCount 2 `
        -ExpectedTitleMarker '[Topologia B]' `
        -Stopwatch $multiwindowStopwatch
    Start-Sleep -Milliseconds 750
    $multiwindowMetrics = Measure-TopologyProcesses `
        -RootProcessIds @($multiwindow.Id)

    Stop-OwnedTopologyProcess -ProcessId $multiwindow.Id
    Start-Sleep -Milliseconds 750
    $multiwindowFailureIsolation = [ordered]@{
        forcedHostProcessId = $multiwindow.Id
        hostSurvived = $null -ne (
            Get-Process -Id $multiwindow.Id -ErrorAction SilentlyContinue
        )
        remainingWindowCount = [MyAlbunsWindowProbe]::VisibleWindowsFor(
            @($multiwindow.Id)
        ).Count
    }

    $gitCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
    $workingTreeStatus = @(& git -C $script:WorkspaceRoot status --short)
    $buildInputStatus = @(
        & git -C $script:WorkspaceRoot status --short -- `
            Cargo.toml `
            Cargo.lock `
            crates `
            package.json `
            package-lock.json `
            scripts `
            src `
            src-tauri
    )
    $report = [ordered]@{
        schemaVersion = 2
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        hardware = Get-HardwareInventory
        build = [ordered]@{
            gitCommit = $gitCommit
            workingTreeDirty = $workingTreeStatus.Count -gt 0
            buildInputsDirty = $buildInputStatus.Count -gt 0
            executable = $executableRelativePath
            profile = 'debug'
        }
        alternatives = [ordered]@{
            independentHosts = [ordered]@{
                ready = $independentReady
                processes = $independentMetrics
                forcedFailure = $independentFailureIsolation
            }
            multiwindowHost = [ordered]@{
                ready = $multiwindowReady
                processes = $multiwindowMetrics
                forcedFailure = $multiwindowFailureIsolation
            }
        }
        notMeasured = @($reportText.notMeasured)
        notes = @($reportText.notes)
    }

    $outputDirectory = Split-Path -Parent $OutputPath
    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
    $json = $report | ConvertTo-Json -Depth 10
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $json + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    $summaryPath = [System.IO.Path]::ChangeExtension($OutputPath, '.md')
    Write-TopologyMarkdownSummary `
        -Report $report `
        -Text $reportText `
        -SummaryPath $summaryPath
    Write-Output "Topology spike report: $OutputPath"
    Write-Output "Topology spike summary: $summaryPath"
    Write-Output $json
}
finally {
    foreach ($processId in $startedProcessIds) {
        try {
            Stop-OwnedTopologyProcess -ProcessId $processId
        }
        catch {
            Write-Warning $_.Exception.Message
        }
    }
    Set-ProcessEnvironmentValue -Name 'CARGO_TARGET_DIR' -Value $previousCargoTarget
}
