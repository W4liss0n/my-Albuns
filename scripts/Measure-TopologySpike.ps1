param(
    [switch] $SkipBuild,
    [ValidateRange(10, 120)]
    [int] $WindowTimeoutSeconds = 45,
    [ValidateRange(30, 1800)]
    [int] $CacheTimeoutSeconds = 900,
    [ValidateRange(30, 1800)]
    [int] $PerformanceTimeoutSeconds = 300,
    [string] $OutputPath
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

$targetDirectory = Join-Path $script:WorkspaceRoot '.scratch\topology-spike-target'
$executablePath = Join-Path $targetDirectory 'release\myalbuns-desktop.exe'
$imagingExecutablePath = Join-Path $targetDirectory 'release\myalbuns-imaging.exe'
$executableRelativePath = '.scratch/topology-spike-target/release/myalbuns-desktop.exe'
$imagingExecutableRelativePath = '.scratch/topology-spike-target/release/myalbuns-imaging.exe'
$buildManifestPath = Join-Path $targetDirectory 'topology-build-manifest.json'
$buildInputPathspecs = @(
    'Cargo.toml',
    'Cargo.lock',
    'crates',
    'index.html',
    'package.json',
    'package-lock.json',
    'public',
    'scripts',
    'src',
    'src-tauri',
    'tests',
    'tsconfig.json',
    'tsconfig.node.json',
    'vite.config.ts',
    'vitest.config.ts'
)
$topologyEnvironment = 'MYALBUNS_TOPOLOGY_SPIKE'
$projectSlotEnvironment = 'MYALBUNS_TOPOLOGY_PROJECT'
$corpusManifestEnvironment = 'MYALBUNS_TOPOLOGY_CORPUS_MANIFEST'
$probeGateEnvironment = 'MYALBUNS_TOPOLOGY_PROBE_GATE'
$exportGateEnvironment = 'MYALBUNS_TOPOLOGY_EXPORT_GATE'
$corpusManifestPath = Join-Path $script:WorkspaceRoot '.scratch\topology-corpus\manifest.json'
$probeGateDirectory = Join-Path $script:WorkspaceRoot '.scratch\topology-probe-gates'
$desktopLogDirectory = Join-Path $env:LOCALAPPDATA 'MyAlbuns2\Logs'
$startedProcessIds = [System.Collections.Generic.List[int]]::new()
$probeGatePaths = [System.Collections.Generic.List[string]]::new()

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot 'docs\research\artifacts\0003-topology-interaction-export.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

$reportText = @'
{
  "notMeasured": [
    "recupera\u00e7\u00e3o persistida",
    "complexidade operacional da IPC"
  ],
  "notes": [
    "O Cache foi reconstru\u00eddo a frio com uma representa\u00e7\u00e3o JPEG de at\u00e9 1600 px por Foto.",
    "Pan e Zoom foram medidos separadamente sobre uma textura real do Cache, depois de 24 frames de aquecimento.",
    "As duas Janelas iniciaram o probe pelo mesmo arquivo-gate, somente depois da conclus\u00e3o do Cache frio.",
    "A Exporta\u00e7\u00e3o mediu a primeira L\u00e2mina do \u00c1lbum principal a 300 DPI, lendo e verificando os JPEGs originais.",
    "Cada uso valida o tamanho e o SHA-256 da Foto; o corpus completo foi recalculado depois das duas alternativas.",
    "A mem\u00f3ria inclui o host e todos os processos descendentes observados.",
    "A Exporta\u00e7\u00e3o foi liberada por um segundo gate somente depois dos dois probes de Canvas.",
    "Os dois hosts independentes foram iniciados antes da espera pelas Janelas, usando o mesmo marco inicial da alternativa multiwindow.",
    "A queda s\u00f3 \u00e9 for\u00e7ada depois de validar o caminho do execut\u00e1vel do PID alvo."
  ],
  "summary": {
    "title": "Intera\u00e7\u00f5es e Exporta\u00e7\u00e3o com imagens reais",
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
    "twoWindows": "Duas Janelas identificadas",
    "afterCrash": "Janelas depois da queda for\u00e7ada",
    "cacheReadyTime": "Duas Janelas com Cache pronto",
    "canvasReadyTime": "Dois Canvas com texturas prontos",
    "cacheWallTime": "Dura\u00e7\u00e3o de parede do Cache frio",
    "cachePhotos": "Fotos processadas pelo Cache",
    "cacheThroughput": "Vaz\u00e3o agregada dos originais",
    "cacheSize": "Representa\u00e7\u00f5es reduzidas",
    "panP95": "Pan: pior p95 entre Projetos",
    "panOver33": "Pan: frames acima de 33 ms",
    "zoomP95": "Zoom: pior p95 entre Projetos",
    "zoomOver33": "Zoom: frames acima de 33 ms",
    "exportDuration": "Exporta\u00e7\u00e3o: dura\u00e7\u00e3o",
    "exportDimensions": "Exporta\u00e7\u00e3o: dimens\u00f5es a 300 DPI",
    "exportSources": "Exporta\u00e7\u00e3o: volume dos originais",
    "exportOutput": "Exporta\u00e7\u00e3o: tamanho do PNG",
    "corpus": "Corpus real",
    "corpusAlbums": "\u00c1lbuns",
    "corpusPhotos": "Fotos JPEG",
    "corpusSourceVolume": "Volume dos originais",
    "corpusDigest": "Digest do corpus",
    "corpusIntegrity": "Integridade antes/depois",
    "corpusIntegrityValue": "confirmada por SHA-256",
    "previewPolicy": "Pol\u00edtica da representa\u00e7\u00e3o",
    "previewPolicyValue": "uma pr\u00e9via JPEG por Foto, com aresta m\u00e1xima de {0} px",
    "notApplicable": "n\u00e3o se aplica",
    "otherPreserved": "outra Janela preservada",
    "build": "Build medida",
    "commit": "Commit do c\u00f3digo",
    "builtAt": "Build conclu\u00edda em UTC",
    "profile": "Perfil",
    "workingTreeDirty": "\u00c1rvore de trabalho tinha mudan\u00e7as alheias",
    "buildInputsDirty": "Entradas da build tinham mudan\u00e7as",
    "buildInputCount": "Arquivos de entrada",
    "buildInputDigest": "Digest das entradas",
    "executableHash": "Hash do host",
    "imagingExecutableHash": "Hash do Processador de Imagens",
    "checkoutMatches": "Checkout atual corresponde ao manifesto",
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

function Reset-TopologyCache {
    if (-not (Test-Path -LiteralPath $imagingExecutablePath -PathType Leaf)) {
        throw "Imaging processor not found at $imagingExecutablePath."
    }
    $requestId = "topology-cache-reset-$([DateTime]::UtcNow.Ticks)"
    $command = [ordered]@{
        kind = 'resetCache'
        request = [ordered]@{
            protocolVersion = 7
            requestId = $requestId
            projectIds = @('project-spike-001', 'project-spike-002')
        }
    }
    $responseText = (
        $command |
            ConvertTo-Json -Depth 4 -Compress |
            & $imagingExecutablePath
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'The native imaging processor could not reset the topology Cache.'
    }
    $response = $responseText | ConvertFrom-Json
    if (
        $response.kind -ne 'cacheReset' -or
        $response.requestId -ne $requestId -or
        $response.removedCount -notin @(0, 1, 2)
    ) {
        throw 'The native imaging processor returned an invalid Cache reset response.'
    }
}

function Get-BuildInputState {
    $relativeFiles = @(
        & git `
            -C $script:WorkspaceRoot `
            ls-files `
            --cached `
            --others `
            --exclude-standard `
            -- `
            @buildInputPathspecs
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not enumerate topology build inputs with Git.'
    }

    $inputHashes = @(
        $relativeFiles |
            Sort-Object -Unique |
            ForEach-Object {
                $relativePath = $_
                $fullPath = Join-Path $script:WorkspaceRoot $relativePath
                if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
                    throw "Topology build input no longer exists: $relativePath"
                }
                $hash = (
                    Get-FileHash -LiteralPath $fullPath -Algorithm SHA256
                ).Hash.ToLowerInvariant()
                "$relativePath`0$hash"
            }
    )
    $payload = [System.Text.Encoding]::UTF8.GetBytes(
        $inputHashes -join "`n"
    )
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digest = -join (
            $sha256.ComputeHash($payload) |
                ForEach-Object { $_.ToString('x2') }
        )
    }
    finally {
        $sha256.Dispose()
    }

    $status = @(
        & git `
            -C $script:WorkspaceRoot `
            status `
            --short `
            -- `
            @buildInputPathspecs
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not inspect topology build input status with Git.'
    }

    return [ordered]@{
        fileCount = $inputHashes.Count
        digestSha256 = $digest
        dirty = $status.Count -gt 0
    }
}

function New-TopologyBuildManifest {
    if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
        throw "Topology spike executable not found at $executablePath."
    }
    if (-not (Test-Path -LiteralPath $imagingExecutablePath -PathType Leaf)) {
        throw "Imaging processor not found at $imagingExecutablePath."
    }

    $inputState = Get-BuildInputState
    $workingTreeStatus = @(& git -C $script:WorkspaceRoot status --short)
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not inspect the topology build working tree.'
    }
    $manifest = [ordered]@{
        manifestVersion = 1
        builtAtUtc = [DateTime]::UtcNow.ToString('o')
        gitCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
        workingTreeDirty = $workingTreeStatus.Count -gt 0
        buildInputsDirty = $inputState.dirty
        buildInputFileCount = $inputState.fileCount
        buildInputDigestSha256 = $inputState.digestSha256
        executable = $executableRelativePath
        executableSha256 = (
            Get-FileHash -LiteralPath $executablePath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        imagingExecutable = $imagingExecutableRelativePath
        imagingExecutableSha256 = (
            Get-FileHash -LiteralPath $imagingExecutablePath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        profile = 'release'
    }
    $manifestJson = $manifest | ConvertTo-Json -Depth 4
    [System.IO.File]::WriteAllText(
        $buildManifestPath,
        $manifestJson + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    return $manifest
}

function Read-TopologyBuildManifest {
    if (-not (Test-Path -LiteralPath $buildManifestPath -PathType Leaf)) {
        throw (
            "Topology build manifest not found at $buildManifestPath. " +
            'Run without -SkipBuild first.'
        )
    }
    if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
        throw "Topology spike executable not found at $executablePath."
    }
    if (-not (Test-Path -LiteralPath $imagingExecutablePath -PathType Leaf)) {
        throw "Imaging processor not found at $imagingExecutablePath."
    }

    $manifest = Get-Content `
        -LiteralPath $buildManifestPath `
        -Raw `
        -Encoding utf8 |
            ConvertFrom-Json
    $executableHash = (
        Get-FileHash -LiteralPath $executablePath -Algorithm SHA256
    ).Hash.ToLowerInvariant()
    if ($executableHash -ne $manifest.executableSha256) {
        throw (
            'Topology executable does not match its build manifest. ' +
            'Run without -SkipBuild.'
        )
    }
    $imagingExecutableHash = (
        Get-FileHash -LiteralPath $imagingExecutablePath -Algorithm SHA256
    ).Hash.ToLowerInvariant()
    if ($imagingExecutableHash -ne $manifest.imagingExecutableSha256) {
        throw (
            'Imaging processor does not match its build manifest. ' +
            'Run without -SkipBuild.'
        )
    }
    return $manifest
}

function Start-TopologyProcess {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [ValidateSet('a', 'b')]
        [string] $ProjectSlot,
        [Parameter(Mandatory = $true)]
        [string] $ProbeGatePath,
        [Parameter(Mandatory = $true)]
        [string] $ExportGatePath
    )

    $previousTopology = [System.Environment]::GetEnvironmentVariable(
        $topologyEnvironment,
        [System.EnvironmentVariableTarget]::Process
    )
    $previousProjectSlot = [System.Environment]::GetEnvironmentVariable(
        $projectSlotEnvironment,
        [System.EnvironmentVariableTarget]::Process
    )
    $previousCorpusManifest = [System.Environment]::GetEnvironmentVariable(
        $corpusManifestEnvironment,
        [System.EnvironmentVariableTarget]::Process
    )
    $previousProbeGate = [System.Environment]::GetEnvironmentVariable(
        $probeGateEnvironment,
        [System.EnvironmentVariableTarget]::Process
    )
    $previousExportGate = [System.Environment]::GetEnvironmentVariable(
        $exportGateEnvironment,
        [System.EnvironmentVariableTarget]::Process
    )
    try {
        Set-ProcessEnvironmentValue -Name $topologyEnvironment -Value $Topology
        Set-ProcessEnvironmentValue `
            -Name $corpusManifestEnvironment `
            -Value $corpusManifestPath
        Set-ProcessEnvironmentValue `
            -Name $probeGateEnvironment `
            -Value $ProbeGatePath
        Set-ProcessEnvironmentValue `
            -Name $exportGateEnvironment `
            -Value $ExportGatePath
        if ([string]::IsNullOrWhiteSpace($ProjectSlot)) {
            Set-ProcessEnvironmentValue -Name $projectSlotEnvironment -Value $null
        }
        else {
            Set-ProcessEnvironmentValue -Name $projectSlotEnvironment -Value $ProjectSlot
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
        Set-ProcessEnvironmentValue `
            -Name $projectSlotEnvironment `
            -Value $previousProjectSlot
        Set-ProcessEnvironmentValue `
            -Name $corpusManifestEnvironment `
            -Value $previousCorpusManifest
        Set-ProcessEnvironmentValue `
            -Name $probeGateEnvironment `
            -Value $previousProbeGate
        Set-ProcessEnvironmentValue `
            -Name $exportGateEnvironment `
            -Value $previousExportGate
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

function Get-DesktopLogEventsSince {
    param([Parameter(Mandatory = $true)][DateTimeOffset] $Since)

    if (-not (Test-Path -LiteralPath $desktopLogDirectory -PathType Container)) {
        return @()
    }
    $events = [System.Collections.Generic.List[object]]::new()
    foreach ($logFile in Get-ChildItem `
        -LiteralPath $desktopLogDirectory `
        -File `
        -Filter 'myalbuns-desktop.*.jsonl') {
        $stream = $null
        $reader = $null
        try {
            $stream = [System.IO.File]::Open(
                $logFile.FullName,
                [System.IO.FileMode]::Open,
                [System.IO.FileAccess]::Read,
                [System.IO.FileShare]::ReadWrite
            )
            $reader = [System.IO.StreamReader]::new(
                $stream,
                [System.Text.Encoding]::UTF8
            )
            while (-not $reader.EndOfStream) {
                $line = $reader.ReadLine()
                if ([string]::IsNullOrWhiteSpace($line)) {
                    continue
                }
                try {
                    $event = $line | ConvertFrom-Json
                    if (
                        $null -ne $event.timestamp -and
                        [DateTimeOffset]::Parse($event.timestamp) -ge $Since
                    ) {
                        $events.Add($event)
                    }
                }
                catch {
                    # A writer may still be completing the last JSONL record.
                }
            }
        }
        finally {
            if ($null -ne $reader) {
                $reader.Dispose()
            }
            elseif ($null -ne $stream) {
                $stream.Dispose()
            }
        }
    }
    return @($events)
}

function Wait-ForMediaCache {
    param(
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProjectCount,
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $StartedAt,
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Stopwatch] $TopologyStopwatch
    )

    $operationPrefixes = @(
        $RootProcessIds | ForEach-Object { "cache-$($_)-" }
    )
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($CacheTimeoutSeconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        foreach ($processId in $RootProcessIds) {
            if ($null -eq (Get-Process -Id $processId -ErrorAction SilentlyContinue)) {
                throw "Topology host $processId exited before its media Cache was ready."
            }
        }
        $cacheEvents = @(Get-DesktopLogEventsSince -Since $StartedAt)
        $failedEvents = @(
            $cacheEvents |
                Where-Object {
                    $candidate = $_
                    $candidate.event -eq 'media_cache_failed' -and
                    $null -ne $candidate.operation_id -and
                    @(
                        $operationPrefixes |
                            Where-Object {
                                $prefix = $_
                                $eventOperation = [string]$candidate.operation_id
                                $eventOperation.StartsWith($prefix)
                            }
                    ).Count -gt 0
                }
        )
        if ($failedEvents.Count -gt 0) {
            $failure = $failedEvents |
                Sort-Object timestamp |
                Select-Object -First 1
            throw (
                "Media Cache failed for project $($failure.project_id) " +
                "at stage $($failure.stage)."
            )
        }
        $completionEvents = @(
            $cacheEvents |
                Where-Object {
                    $candidate = $_
                    $candidate.event -eq 'media_cache_completed' -and
                    $null -ne $candidate.operation_id -and
                    @(
                        $operationPrefixes |
                            Where-Object {
                                $prefix = $_
                                $eventOperation = [string]$candidate.operation_id
                                $eventOperation.StartsWith($prefix)
                            }
                    ).Count -gt 0
                }
        )
        $projectEvents = @(
            $completionEvents |
                Group-Object project_id |
                ForEach-Object {
                    $_.Group |
                        Sort-Object timestamp |
                        Select-Object -First 1
                }
        )
        if ($projectEvents.Count -eq $ExpectedProjectCount) {
            $completedTimes = @(
                $projectEvents |
                    ForEach-Object { [DateTimeOffset]::Parse($_.timestamp) }
            )
            $startedTimes = @(
                $projectEvents |
                    ForEach-Object {
                        ([DateTimeOffset]::Parse($_.timestamp)).AddMilliseconds(
                            -[double]$_.elapsed_ms
                        )
                    }
            )
            $cacheWallTimeMs = [long](
                (($completedTimes | Sort-Object | Select-Object -Last 1) -
                    ($startedTimes | Sort-Object | Select-Object -First 1)
                ).TotalMilliseconds
            )
            $totalSourceBytes = [long](
                ($projectEvents | Measure-Object source_bytes -Sum).Sum
            )
            return [ordered]@{
                readyElapsedMs = $TopologyStopwatch.ElapsedMilliseconds
                cacheWallTimeMs = $cacheWallTimeMs
                projectCount = $projectEvents.Count
                photoCount = [long](
                    ($projectEvents | Measure-Object generated_count -Sum).Sum +
                    ($projectEvents | Measure-Object reused_count -Sum).Sum
                )
                generatedCount = [long](
                    ($projectEvents | Measure-Object generated_count -Sum).Sum
                )
                reusedCount = [long](
                    ($projectEvents | Measure-Object reused_count -Sum).Sum
                )
                sourceBytes = $totalSourceBytes
                previewBytes = [long](
                    ($projectEvents | Measure-Object preview_bytes -Sum).Sum
                )
                sourceBytesPerSecond = if ($cacheWallTimeMs -gt 0) {
                    [long]($totalSourceBytes / ($cacheWallTimeMs / 1000.0))
                }
                else {
                    $null
                }
                projects = @(
                    $projectEvents |
                        Sort-Object project_id |
                        ForEach-Object {
                            [ordered]@{
                                projectId = $_.project_id
                                generatedCount = [long]$_.generated_count
                                reusedCount = [long]$_.reused_count
                                sourceBytes = [long]$_.source_bytes
                                previewBytes = [long]$_.preview_bytes
                                elapsedMs = [long]$_.elapsed_ms
                            }
                        }
                )
            }
        }
        Start-Sleep -Milliseconds 250
    }
    throw (
        "Expected Cache completion for $ExpectedProjectCount projects within " +
        "$CacheTimeoutSeconds seconds."
    )
}

function Open-TopologyProbeGate {
    param([Parameter(Mandatory = $true)][string] $Path)

    $gateRoot = [System.IO.Path]::GetFullPath($probeGateDirectory) +
        [System.IO.Path]::DirectorySeparatorChar
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith(
        $gateRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw 'The topology probe gate is outside its dedicated directory.'
    }
    if (Test-Path -LiteralPath $fullPath) {
        throw "The topology probe gate already exists: $fullPath"
    }
    New-Item -ItemType Directory -Force -Path $probeGateDirectory | Out-Null
    [System.IO.File]::WriteAllText(
        $fullPath,
        "ready$([System.Environment]::NewLine)",
        [System.Text.UTF8Encoding]::new($false)
    )
}

function Convert-CanvasTiming {
    [OutputType([pscustomobject])]
    param(
        [Parameter(Mandatory = $true)] $Event,
        [Parameter(Mandatory = $true)]
        [ValidateSet('pan', 'zoom')]
        [string] $Prefix
    )

    $sampleCount = "${Prefix}_sample_count"
    $duration = "${Prefix}_duration_ms"
    $firstFrame = "${Prefix}_first_frame_latency_ms"
    $mean = "${Prefix}_mean_frame_ms"
    $p50 = "${Prefix}_p50_frame_ms"
    $p95 = "${Prefix}_p95_frame_ms"
    $p99 = "${Prefix}_p99_frame_ms"
    $maximum = "${Prefix}_max_frame_ms"
    $over16 = "${Prefix}_frames_over16_ms"
    $over33 = "${Prefix}_frames_over33_ms"
    return [pscustomobject][ordered]@{
        sampleCount = [long] $Event.$sampleCount
        durationMs = [double] $Event.$duration
        firstFrameLatencyMs = [double] $Event.$firstFrame
        meanFrameMs = [double] $Event.$mean
        p50FrameMs = [double] $Event.$p50
        p95FrameMs = [double] $Event.$p95
        p99FrameMs = [double] $Event.$p99
        maxFrameMs = [double] $Event.$maximum
        framesOver16Ms = [long] $Event.$over16
        framesOver33Ms = [long] $Event.$over33
    }
}

function Measure-CanvasTimingAggregate {
    param([Parameter(Mandatory = $true)][object[]] $Timings)

    $sampleCount = [long] (($Timings | Measure-Object sampleCount -Sum).Sum)
    $weightedMean = if ($sampleCount -gt 0) {
        (
            $Timings |
                ForEach-Object { $_.meanFrameMs * $_.sampleCount } |
                Measure-Object -Sum
        ).Sum / $sampleCount
    }
    else {
        $null
    }
    return [ordered]@{
        sampleCount = $sampleCount
        weightedMeanFrameMs = $weightedMean
        worstProjectP95FrameMs = [double](
            ($Timings | Measure-Object p95FrameMs -Maximum).Maximum
        )
        worstProjectP99FrameMs = [double](
            ($Timings | Measure-Object p99FrameMs -Maximum).Maximum
        )
        worstProjectMaxFrameMs = [double](
            ($Timings | Measure-Object maxFrameMs -Maximum).Maximum
        )
        framesOver16Ms = [long](
            ($Timings | Measure-Object framesOver16Ms -Sum).Sum
        )
        framesOver33Ms = [long](
            ($Timings | Measure-Object framesOver33Ms -Sum).Sum
        )
    }
}

function Assert-ComparableCanvasTargets {
    param(
        [Parameter(Mandatory = $true)] $Independent,
        [Parameter(Mandatory = $true)] $Multiwindow
    )

    $independentFrames = @{}
    foreach ($project in $Independent.canvas.projects) {
        $independentFrames[[string]$project.projectId] = [string]$project.frameId
    }
    foreach ($project in $Multiwindow.canvas.projects) {
        $projectId = [string]$project.projectId
        $independentFrame = $independentFrames[$projectId]
        if (
            [string]::IsNullOrWhiteSpace($independentFrame) -or
            $independentFrame -ne [string]$project.frameId
        ) {
            throw (
                "Canvas target mismatch for project ${projectId}: " +
                "independent=${independentFrame}, " +
                "multiwindow=$($project.frameId)."
            )
        }
    }
}

function Wait-ForTopologyBenchmark {
    param(
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProjectCount,
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $StartedAt,
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Stopwatch] $TopologyStopwatch,
        [Parameter(Mandatory = $true)]
        [string] $ExportGatePath
    )

    $exportGateOpened = $false
    $exportPrefixes = @(
        $RootProcessIds | ForEach-Object { "export-$($_)-" }
    )
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($PerformanceTimeoutSeconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        foreach ($processId in $RootProcessIds) {
            if ($null -eq (Get-Process -Id $processId -ErrorAction SilentlyContinue)) {
                throw "Topology host $processId exited during the interaction benchmark."
            }
        }

        $events = @(Get-DesktopLogEventsSince -Since $StartedAt)
        $benchmarkFailure = @(
            $events |
                Where-Object {
                    $_.event -eq 'topology_benchmark_failed' -and
                    [int]$_.process_id -in $RootProcessIds
                } |
                Sort-Object timestamp |
                Select-Object -First 1
        )
        if ($benchmarkFailure.Count -gt 0) {
            throw (
                "Canvas benchmark failed for project " +
                "$($benchmarkFailure[0].project_id): " +
                "$($benchmarkFailure[0].reason)."
            )
        }
        $exportFailure = @(
            $events |
                Where-Object {
                    $candidate = $_
                    $candidate.event -eq 'export_failed' -and
                    $null -ne $candidate.operation_id -and
                    @(
                        $exportPrefixes |
                            Where-Object {
                                ([string]$candidate.operation_id).StartsWith($_)
                            }
                    ).Count -gt 0
                } |
                Sort-Object timestamp |
                Select-Object -First 1
        )
        if ($exportFailure.Count -gt 0) {
            throw (
                "Export benchmark failed at stage " +
                "$($exportFailure[0].stage)."
            )
        }

        $canvasReadyEvents = @(
            $events |
                Where-Object {
                    $_.event -eq 'canvas_benchmark_ready' -and
                    $_.topology -eq $Topology -and
                    [int]$_.process_id -in $RootProcessIds
                } |
                Group-Object project_id |
                ForEach-Object {
                    $_.Group |
                        Sort-Object timestamp |
                        Select-Object -First 1
                }
        )
        $canvasEvents = @(
            $events |
                Where-Object {
                    $_.event -eq 'canvas_benchmark_completed' -and
                    $_.topology -eq $Topology -and
                    [int]$_.process_id -in $RootProcessIds
                } |
                Group-Object project_id |
                ForEach-Object {
                    $_.Group |
                        Sort-Object timestamp |
                        Select-Object -First 1
                }
        )
        $exportEvent = @(
            $events |
                Where-Object {
                    $_.event -eq 'export_completed' -and
                    $_.project_id -eq 'project-spike-001' -and
                    [int]$_.process_id -in $RootProcessIds
                } |
                Sort-Object timestamp |
                Select-Object -First 1
        )
        if (
            -not $exportGateOpened -and
            $canvasReadyEvents.Count -eq $ExpectedProjectCount -and
            $canvasEvents.Count -eq $ExpectedProjectCount
        ) {
            Open-TopologyProbeGate -Path $ExportGatePath
            $exportGateOpened = $true
        }
        if (
            $canvasReadyEvents.Count -eq $ExpectedProjectCount -and
            $canvasEvents.Count -eq $ExpectedProjectCount -and
            $exportEvent.Count -eq 1
        ) {
            if (@($canvasEvents | Where-Object { -not $_.texture_backed }).Count -gt 0) {
                throw 'At least one Canvas probe did not use a real Cache texture.'
            }
            if (
                [int]$exportEvent[0].dpi -ne 300 -or
                [long]$exportEvent[0].source_count -lt 1 -or
                [long]$exportEvent[0].source_bytes -lt 1 -or
                [long]$exportEvent[0].output_bytes -lt 1 -or
                [string]$exportEvent[0].output_sha256 -notmatch '^[0-9a-f]{64}$'
            ) {
                throw 'The real-image Export benchmark returned invalid evidence.'
            }

            $readyByProject = @{}
            foreach ($readyEvent in $canvasReadyEvents) {
                $readyByProject[[string]$readyEvent.project_id] = $readyEvent
            }
            $projects = @(
                $canvasEvents |
                    Sort-Object project_id |
                    ForEach-Object {
                        $readyEvent = $readyByProject[[string]$_.project_id]
                        if ($null -eq $readyEvent) {
                            throw "Canvas readiness is missing for project $($_.project_id)."
                        }
                        [ordered]@{
                            projectId = $_.project_id
                            processId = [int]$_.process_id
                            windowLabel = $_.window_label
                            readyElapsedMs = [long][math]::Round(
                                (
                                    [DateTimeOffset]::Parse($readyEvent.timestamp) -
                                    $StartedAt
                                ).TotalMilliseconds
                            )
                            frameId = $_.frame_id
                            textureBacked = [bool]$_.texture_backed
                            pan = Convert-CanvasTiming -Event $_ -Prefix pan
                            zoom = Convert-CanvasTiming -Event $_ -Prefix zoom
                        }
                    }
            )
            $panTimings = @($projects | ForEach-Object { $_.pan })
            $zoomTimings = @($projects | ForEach-Object { $_.zoom })
            return [ordered]@{
                completedElapsedMs = $TopologyStopwatch.ElapsedMilliseconds
                canvas = [ordered]@{
                    projectCount = $projects.Count
                    allProjectsReadyElapsedMs = [long](
                        (
                            $projects |
                                ForEach-Object { [long]$_.readyElapsedMs } |
                                Measure-Object -Maximum
                        ).Maximum
                    )
                    warmupFramesPerProject = 24
                    projects = $projects
                    aggregate = [ordered]@{
                        pan = Measure-CanvasTimingAggregate -Timings $panTimings
                        zoom = Measure-CanvasTimingAggregate -Timings $zoomTimings
                    }
                }
                export = [ordered]@{
                    projectId = $exportEvent[0].project_id
                    processId = [int]$exportEvent[0].process_id
                    windowLabel = $exportEvent[0].window_label
                    elapsedMs = [long]$exportEvent[0].elapsed_ms
                    widthPx = [long]$exportEvent[0].width_px
                    heightPx = [long]$exportEvent[0].height_px
                    dpi = [long]$exportEvent[0].dpi
                    sourceCount = [long]$exportEvent[0].source_count
                    sourceBytes = [long]$exportEvent[0].source_bytes
                    outputBytes = [long]$exportEvent[0].output_bytes
                    outputSha256 = [string]$exportEvent[0].output_sha256
                }
            }
        }
        Start-Sleep -Milliseconds 100
    }
    throw (
        "Expected Canvas and Export evidence for $ExpectedProjectCount " +
        "projects within $PerformanceTimeoutSeconds seconds."
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
    $checkoutMatches = if (
        $Report.build.currentBuildInputsMatchManifest
    ) {
        $yes
    }
    else {
        $no
    }
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
        'document: technical-research'
        'ticket: 01-plataforma-e-arquitetura'
        "date: $collectedDate"
        "updated: $collectedDate"
        '---'
        ''
        "# $($summary.title)"
        ''
        "$($summary.collected): ``$($Report.collectedAtUtc)``."
        "[$($summary.raw)]($([System.IO.Path]::GetFileName($OutputPath)))."
        ''
        "| $($summary.measure) | $($summary.independent) | $($summary.multiwindow) |"
        '|---|---:|---:|'
        "| $($summary.hosts) | $($independent.processes.hostProcessCount) | $($multiwindow.processes.hostProcessCount) |"
        "| $($summary.windows) | $($independent.ready.windows.Count) | $($multiwindow.ready.windows.Count) |"
        "| $($summary.processes) | $($independent.processes.processTreeCount) | $($multiwindow.processes.processTreeCount) |"
        "| $($summary.workingSet) | $(Format-Mebibytes $independent.processes.workingSetBytes) MiB | $(Format-Mebibytes $multiwindow.processes.workingSetBytes) MiB |"
        "| $($summary.privateMemory) | $(Format-Mebibytes $independent.processes.privateMemoryBytes) MiB | $(Format-Mebibytes $multiwindow.processes.privateMemoryBytes) MiB |"
        "| $($summary.gpuMemory) | $(Format-Mebibytes $independent.processes.gpuMemory.sharedBytes) MiB | $(Format-Mebibytes $multiwindow.processes.gpuMemory.sharedBytes) MiB |"
        "| $($summary.twoWindows) | $($independent.ready.elapsedMs) ms | $($multiwindow.ready.elapsedMs) ms |"
        "| $($summary.cacheReadyTime) | $($independent.cache.readyElapsedMs) ms | $($multiwindow.cache.readyElapsedMs) ms |"
        "| $($summary.canvasReadyTime) | $($independent.interaction.canvas.allProjectsReadyElapsedMs) ms | $($multiwindow.interaction.canvas.allProjectsReadyElapsedMs) ms |"
        "| $($summary.cacheWallTime) | $($independent.cache.cacheWallTimeMs) ms | $($multiwindow.cache.cacheWallTimeMs) ms |"
        "| $($summary.cachePhotos) | $($independent.cache.photoCount) | $($multiwindow.cache.photoCount) |"
        "| $($summary.cacheThroughput) | $(Format-Mebibytes $independent.cache.sourceBytesPerSecond) MiB/s | $(Format-Mebibytes $multiwindow.cache.sourceBytesPerSecond) MiB/s |"
        "| $($summary.cacheSize) | $(Format-Mebibytes $independent.cache.previewBytes) MiB | $(Format-Mebibytes $multiwindow.cache.previewBytes) MiB |"
        "| $($summary.panP95) | $($independent.interaction.canvas.aggregate.pan.worstProjectP95FrameMs) ms | $($multiwindow.interaction.canvas.aggregate.pan.worstProjectP95FrameMs) ms |"
        "| $($summary.panOver33) | $($independent.interaction.canvas.aggregate.pan.framesOver33Ms) | $($multiwindow.interaction.canvas.aggregate.pan.framesOver33Ms) |"
        "| $($summary.zoomP95) | $($independent.interaction.canvas.aggregate.zoom.worstProjectP95FrameMs) ms | $($multiwindow.interaction.canvas.aggregate.zoom.worstProjectP95FrameMs) ms |"
        "| $($summary.zoomOver33) | $($independent.interaction.canvas.aggregate.zoom.framesOver33Ms) | $($multiwindow.interaction.canvas.aggregate.zoom.framesOver33Ms) |"
        "| $($summary.exportDuration) | $($independent.interaction.export.elapsedMs) ms | $($multiwindow.interaction.export.elapsedMs) ms |"
        "| $($summary.exportDimensions) | $($independent.interaction.export.widthPx) x $($independent.interaction.export.heightPx) px | $($multiwindow.interaction.export.widthPx) x $($multiwindow.interaction.export.heightPx) px |"
        "| $($summary.exportSources) | $(Format-Mebibytes $independent.interaction.export.sourceBytes) MiB | $(Format-Mebibytes $multiwindow.interaction.export.sourceBytes) MiB |"
        "| $($summary.exportOutput) | $(Format-Mebibytes $independent.interaction.export.outputBytes) MiB | $(Format-Mebibytes $multiwindow.interaction.export.outputBytes) MiB |"
        "| $($summary.afterCrash) | $independentAfterCrash | $multiwindowAfterCrash |"
        ''
        "## $($summary.corpus)"
        ''
        "- $($summary.corpusAlbums): $($Report.corpus.albumCount)"
        "- $($summary.corpusPhotos): $($Report.corpus.photoCount)"
        "- $($summary.corpusSourceVolume): $(Format-Mebibytes $Report.corpus.sourceBytes) MiB"
        "- $($summary.corpusDigest): ``$($Report.corpus.corpusSha256)``"
        "- $($summary.corpusIntegrity): $($summary.corpusIntegrityValue)"
        "- $($summary.previewPolicy): $($summary.previewPolicyValue -f $Report.corpus.previewPolicy.maximumEdgePx)"
        ''
        "## $($summary.build)"
        ''
        "- $($summary.commit): ``$($Report.build.gitCommit)``"
        "- $($summary.builtAt): ``$($Report.build.builtAtUtc)``"
        "- $($summary.profile): ``$($Report.build.profile)``"
        "- $($summary.workingTreeDirty): $workingTreeDirty"
        "- $($summary.buildInputsDirty): $buildInputsDirty"
        "- $($summary.buildInputCount): $($Report.build.buildInputFileCount)"
        "- $($summary.buildInputDigest): ``$($Report.build.buildInputDigestSha256)``"
        "- $($summary.executableHash): ``$($Report.build.executableSha256)``"
        "- $($summary.imagingExecutableHash): ``$($Report.build.imagingExecutableSha256)``"
        "- $($summary.checkoutMatches): $checkoutMatches"
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
    & (Join-Path $PSScriptRoot 'Prepare-TopologyCorpus.ps1')
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $corpusManifest = Get-Content `
        -LiteralPath $corpusManifestPath `
        -Raw `
        -Encoding utf8 |
            ConvertFrom-Json

    if (-not $SkipBuild) {
        Set-ProcessEnvironmentValue -Name 'CARGO_TARGET_DIR' -Value $targetDirectory
        & (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1') build --no-bundle
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
        $buildManifest = New-TopologyBuildManifest
    }
    else {
        $buildManifest = Read-TopologyBuildManifest
    }

    $runId = "$PID-$([DateTime]::UtcNow.Ticks)"
    $independentProbeGate = Join-Path `
        $probeGateDirectory `
        "independent-$runId.ready"
    $multiwindowProbeGate = Join-Path `
        $probeGateDirectory `
        "multiwindow-$runId.ready"
    $independentExportGate = Join-Path `
        $probeGateDirectory `
        "independent-export-$runId.ready"
    $multiwindowExportGate = Join-Path `
        $probeGateDirectory `
        "multiwindow-export-$runId.ready"
    $probeGatePaths.Add($independentProbeGate)
    $probeGatePaths.Add($multiwindowProbeGate)
    $probeGatePaths.Add($independentExportGate)
    $probeGatePaths.Add($multiwindowExportGate)

    Reset-TopologyCache
    $independentStartedAt = [DateTimeOffset]::UtcNow
    $independentStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $independentA = Start-TopologyProcess `
        -Topology independent `
        -ProjectSlot a `
        -ProbeGatePath $independentProbeGate `
        -ExportGatePath $independentExportGate
    $independentB = Start-TopologyProcess `
        -Topology independent `
        -ProjectSlot b `
        -ProbeGatePath $independentProbeGate `
        -ExportGatePath $independentExportGate
    $independentReady = Wait-ForTopologyWindows `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedCount 2 `
        -ExpectedTitleMarker '[Topologia A]' `
        -Stopwatch $independentStopwatch
    $independentCache = Wait-ForMediaCache `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedProjectCount 2 `
        -StartedAt $independentStartedAt `
        -TopologyStopwatch $independentStopwatch
    Open-TopologyProbeGate -Path $independentProbeGate
    $independentInteraction = Wait-ForTopologyBenchmark `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedProjectCount 2 `
        -Topology independent `
        -StartedAt $independentStartedAt `
        -TopologyStopwatch $independentStopwatch `
        -ExportGatePath $independentExportGate
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
    Reset-TopologyCache
    $multiwindowStartedAt = [DateTimeOffset]::UtcNow
    $multiwindowStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $multiwindow = Start-TopologyProcess `
        -Topology multiwindow `
        -ProbeGatePath $multiwindowProbeGate `
        -ExportGatePath $multiwindowExportGate
    $multiwindowReady = Wait-ForTopologyWindows `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedCount 2 `
        -ExpectedTitleMarker '[Topologia B]' `
        -Stopwatch $multiwindowStopwatch
    $multiwindowCache = Wait-ForMediaCache `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedProjectCount 2 `
        -StartedAt $multiwindowStartedAt `
        -TopologyStopwatch $multiwindowStopwatch
    Open-TopologyProbeGate -Path $multiwindowProbeGate
    $multiwindowInteraction = Wait-ForTopologyBenchmark `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedProjectCount 2 `
        -Topology multiwindow `
        -StartedAt $multiwindowStartedAt `
        -TopologyStopwatch $multiwindowStopwatch `
        -ExportGatePath $multiwindowExportGate
    Assert-ComparableCanvasTargets `
        -Independent $independentInteraction `
        -Multiwindow $multiwindowInteraction
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

    & (Join-Path $PSScriptRoot 'Prepare-TopologyCorpus.ps1')
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $corpusManifestAfterRuns = Get-Content `
        -LiteralPath $corpusManifestPath `
        -Raw `
        -Encoding utf8 |
            ConvertFrom-Json
    if (
        $corpusManifestAfterRuns.corpusSha256 -ne $corpusManifest.corpusSha256 -or
        $corpusManifestAfterRuns.totalFiles -ne $corpusManifest.totalFiles -or
        $corpusManifestAfterRuns.totalBytes -ne $corpusManifest.totalBytes
    ) {
        throw 'The real-image corpus changed while the topology spike was running.'
    }

    $currentInputState = Get-BuildInputState
    $report = [ordered]@{
        schemaVersion = 7
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        hardware = Get-HardwareInventory
        corpus = [ordered]@{
            schemaVersion = $corpusManifest.schemaVersion
            albumCount = $corpusManifest.albums.Count
            photoCount = $corpusManifest.totalFiles
            sourceBytes = $corpusManifest.totalBytes
            corpusSha256 = $corpusManifest.corpusSha256
            integrity = [ordered]@{
                verified = $true
                beforeSha256 = $corpusManifest.corpusSha256
                afterSha256 = $corpusManifestAfterRuns.corpusSha256
            }
            previewPolicy = [ordered]@{
                representationsPerPhoto = 1
                format = 'jpeg'
                maximumEdgePx = 1600
            }
        }
        build = [ordered]@{
            manifestVersion = $buildManifest.manifestVersion
            builtAtUtc = $buildManifest.builtAtUtc
            gitCommit = $buildManifest.gitCommit
            workingTreeDirty = $buildManifest.workingTreeDirty
            buildInputsDirty = $buildManifest.buildInputsDirty
            buildInputFileCount = $buildManifest.buildInputFileCount
            buildInputDigestSha256 = $buildManifest.buildInputDigestSha256
            executable = $buildManifest.executable
            executableSha256 = $buildManifest.executableSha256
            imagingExecutable = $buildManifest.imagingExecutable
            imagingExecutableSha256 = $buildManifest.imagingExecutableSha256
            profile = $buildManifest.profile
            currentBuildInputsMatchManifest = (
                $currentInputState.digestSha256 -eq
                    $buildManifest.buildInputDigestSha256
            )
        }
        alternatives = [ordered]@{
            independentHosts = [ordered]@{
                ready = $independentReady
                cache = $independentCache
                interaction = $independentInteraction
                processes = $independentMetrics
                forcedFailure = $independentFailureIsolation
            }
            multiwindowHost = [ordered]@{
                ready = $multiwindowReady
                cache = $multiwindowCache
                interaction = $multiwindowInteraction
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
    $gateRoot = [System.IO.Path]::GetFullPath($probeGateDirectory) +
        [System.IO.Path]::DirectorySeparatorChar
    foreach ($gatePath in $probeGatePaths) {
        $fullGatePath = [System.IO.Path]::GetFullPath($gatePath)
        if (
            $fullGatePath.StartsWith(
                $gateRoot,
                [System.StringComparison]::OrdinalIgnoreCase
            ) -and
            (Test-Path -LiteralPath $fullGatePath -PathType Leaf)
        ) {
            Remove-Item -LiteralPath $fullGatePath -Force
        }
    }
    Set-ProcessEnvironmentValue -Name 'CARGO_TARGET_DIR' -Value $previousCargoTarget
}
