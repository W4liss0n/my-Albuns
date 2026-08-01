param(
    [string] $OutputPath,
    [ValidateRange(1, 8)]
    [int] $CargoBuildJobs = 1
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
. (Join-Path $PSScriptRoot 'Evidence-BuildInputs.ps1')
. (Join-Path $PSScriptRoot 'WindowsProcessTree.ps1')

if ($env:OS -ne 'Windows_NT') {
    throw 'The global shell gate must run on Windows.'
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0017-global-shell-candidate.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$artifactDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'docs\research\artifacts')
)
if (
    [System.IO.Path]::GetDirectoryName($OutputPath) -cne
        $artifactDirectory -or
    [System.IO.Path]::GetExtension($OutputPath) -cne '.json'
) {
    throw 'Global shell evidence must be a JSON file in docs\research\artifacts.'
}

$targetDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'target\global-shell-gate')
)
$executablePath = Join-Path $targetDirectory 'release\myalbuns-desktop.exe'
$runRoot = Join-Path $targetDirectory 'gate-runs'
$checks = [System.Collections.Generic.List[object]]::new()
$startedProcesses = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()

function Add-Check {
    param(
        [Parameter(Mandatory = $true)][string] $Name,
        [Parameter(Mandatory = $true)][long] $ElapsedMs
    )

    $script:checks.Add([ordered]@{
        name = $Name
        passed = $true
        elapsedMs = $ElapsedMs
    })
}

function Invoke-CheckedCommand {
    param(
        [Parameter(Mandatory = $true)][string] $Name,
        [Parameter(Mandatory = $true)][string] $Executable,
        [Parameter(Mandatory = $true)][string[]] $Arguments
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    & $Executable @Arguments
    $exitCode = $LASTEXITCODE
    $stopwatch.Stop()
    if ($exitCode -ne 0) {
        throw "Global shell check '$Name' failed with exit code $exitCode."
    }
    Add-Check -Name $Name -ElapsedMs $stopwatch.ElapsedMilliseconds
}

function Get-AvailableLoopbackPort {
    $listener = [System.Net.Sockets.TcpListener]::new(
        [System.Net.IPAddress]::Loopback,
        0
    )
    try {
        $listener.Start()
        return ([System.Net.IPEndPoint] $listener.LocalEndpoint).Port
    }
    finally {
        $listener.Stop()
    }
}

function New-GlobalProcessStartInfo {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)][int] $Port,
        [Parameter(Mandatory = $true)][string] $RunId,
        [Parameter(Mandatory = $true)][string] $LogDirectory
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new($script:executablePath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.Environment['MYALBUNS_PROCESS_ROLE'] = 'global'
    $startInfo.Environment['MYALBUNS_GLOBAL_SPIKE_ENDPOINT'] =
        "127.0.0.1:$Port"
    $startInfo.Environment['MYALBUNS_TOPOLOGY_RUN_ID'] = $RunId
    $startInfo.Environment['MYALBUNS_TOPOLOGY_SPIKE'] = $Topology
    $startInfo.Environment['MYALBUNS_LOG_DIR'] = $LogDirectory
    $startInfo.Environment['MYALBUNS_GLOBAL_SPIKE_WELCOME_VISIBLE'] = '1'
    return $startInfo
}

function Wait-ForWelcomeScreen {
    param(
        [Parameter(Mandatory = $true)][System.Diagnostics.Process] $Process,
        [Parameter(Mandatory = $true)][string] $LogDirectory,
        [int] $TimeoutSeconds = 30
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $frontendReady = $false
    do {
        Start-Sleep -Milliseconds 200
        $Process.Refresh()
        if ($Process.HasExited) {
            throw "Global shell exited before readiness with code $($Process.ExitCode)."
        }
        if (-not $frontendReady) {
            foreach (
                $logFile in @(
                    Get-ChildItem `
                        -LiteralPath $LogDirectory `
                        -Filter 'myalbuns-global*.jsonl' `
                        -File `
                        -ErrorAction SilentlyContinue
                )
            ) {
                if (
                    Select-String `
                        -LiteralPath $logFile.FullName `
                        -SimpleMatch '"event":"welcome_screen_ready"' `
                        -Quiet
                ) {
                    $frontendReady = $true
                    break
                }
            }
        }
        $Process.Refresh()
        if (
            $frontendReady -and
            $Process.MainWindowHandle -ne 0 -and
            $Process.MainWindowTitle -ceq 'MyAlbuns'
        ) {
            return
        }
    } while ([DateTime]::UtcNow -lt $deadline)

    throw 'The Welcome Screen did not expose both frontend readiness and its window before timeout.'
}

function Invoke-GlobalStatusProbe {
    param(
        [Parameter(Mandatory = $true)][int] $Port,
        [Parameter(Mandatory = $true)][string] $RunId,
        [Parameter(Mandatory = $true)][string] $Topology,
        [Parameter(Mandatory = $true)][uint32] $ExpectedProcessId
    )

    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $connect = $client.ConnectAsync('127.0.0.1', $Port)
        if (-not $connect.Wait([TimeSpan]::FromSeconds(5))) {
            throw 'The global status endpoint did not accept a connection.'
        }
        $stream = $client.GetStream()
        $stream.ReadTimeout = 5000
        $stream.WriteTimeout = 5000
        $probeId = "$Topology-global-shell-gate"
        $request = [ordered]@{
            kind = 'status'
            runId = $RunId
            probeId = $probeId
        } | ConvertTo-Json -Compress
        $bytes = [System.Text.Encoding]::UTF8.GetBytes(
            $request + [System.Environment]::NewLine
        )
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush()
        $reader = [System.IO.StreamReader]::new(
            $stream,
            [System.Text.Encoding]::UTF8,
            $false,
            1024,
            $true
        )
        try {
            $response = $reader.ReadLine() | ConvertFrom-Json
        }
        finally {
            $reader.Dispose()
        }
    }
    finally {
        $client.Dispose()
    }

    if (
        $response.kind -cne 'status' -or
        [uint32] $response.processId -ne $ExpectedProcessId -or
        $response.runId -cne $RunId -or
        $response.topology -cne $Topology -or
        $response.probeId -cne $probeId
    ) {
        throw 'The global status response did not match the running Welcome Screen.'
    }
    return $response
}

function Get-ProcessTreeSnapshot {
    param([Parameter(Mandatory = $true)][uint32] $RootProcessId)

    $observed = [System.Collections.Generic.List[object]]::new()
    foreach ($processId in @(Get-ProcessTreeIds -RootProcessId $RootProcessId)) {
        $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
        if ($null -eq $process) {
            continue
        }
        $process.Refresh()
        $observed.Add([ordered]@{
            processId = [uint32] $process.Id
            name = $process.ProcessName
            workingSetBytes = [long] $process.WorkingSet64
            privateMemoryBytes = [long] $process.PrivateMemorySize64
            handleCount = [int] $process.HandleCount
            threadCount = [int] $process.Threads.Count
        })
    }
    if (@($observed).Count -eq 0) {
        throw 'The global process tree disappeared before measurement.'
    }

    return [ordered]@{
        processCount = @($observed).Count
        workingSetBytes = [long] (
            ($observed | ForEach-Object { $_.workingSetBytes } | Measure-Object -Sum).Sum
        )
        privateMemoryBytes = [long] (
            ($observed | ForEach-Object { $_.privateMemoryBytes } | Measure-Object -Sum).Sum
        )
        handleCount = [long] (
            ($observed | ForEach-Object { $_.handleCount } | Measure-Object -Sum).Sum
        )
        threadCount = [long] (
            ($observed | ForEach-Object { $_.threadCount } | Measure-Object -Sum).Sum
        )
        processes = @($observed)
    }
}

function Invoke-GlobalShellRun {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('independent', 'multiwindow')]
        [string] $Topology
    )

    $port = Get-AvailableLoopbackPort
    $runId = "global-shell-$Topology-$([DateTime]::UtcNow.Ticks)"
    $runDirectory = Join-Path $script:runRoot $runId
    $logDirectory = Join-Path $runDirectory 'logs'
    [System.IO.Directory]::CreateDirectory($logDirectory) | Out-Null
    $startInfo = New-GlobalProcessStartInfo `
        -Topology $Topology `
        -Port $port `
        -RunId $runId `
        -LogDirectory $logDirectory
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $owner = [System.Diagnostics.Process]::Start($startInfo)
    $script:startedProcesses.Add($owner)
    try {
        Wait-ForWelcomeScreen -Process $owner -LogDirectory $logDirectory
        $readyMs = $stopwatch.ElapsedMilliseconds
        $status = Invoke-GlobalStatusProbe `
            -Port $port `
            -RunId $runId `
            -Topology $Topology `
            -ExpectedProcessId ([uint32] $owner.Id)

        $duplicate = [System.Diagnostics.Process]::Start($startInfo)
        $script:startedProcesses.Add($duplicate)
        if (-not $duplicate.WaitForExit(10000)) {
            throw 'A duplicate global process did not terminate after singleton rejection.'
        }
        if ($duplicate.ExitCode -ne 73) {
            throw "The duplicate global process exited with $($duplicate.ExitCode), not 73."
        }
        $owner.Refresh()
        if ($owner.HasExited) {
            throw 'The singleton rejection displaced the owning global process.'
        }

        Start-Sleep -Milliseconds 750
        $processTree = Get-ProcessTreeSnapshot -RootProcessId ([uint32] $owner.Id)
        $unexpectedImagingProcesses = @(
            $processTree.processes |
                Where-Object { $_.name -like 'myalbuns-imaging*' }
        )
        if ($unexpectedImagingProcesses.Count -gt 0) {
            throw 'The idle global shell started an Imaging Processor.'
        }
        $owner.Refresh()
        return [ordered]@{
            topology = $Topology
            passed = $true
            readyMs = [long] $readyMs
            processId = [uint32] $owner.Id
            windowTitle = $owner.MainWindowTitle
            welcomeScreenVisible = $true
            status = $status
            singleton = [ordered]@{
                rejectedExitCode = [int] $duplicate.ExitCode
                ownerPreserved = $true
            }
            projectWorkload = [ordered]@{
                imagingProcessCount = 0
            }
            processTree = $processTree
        }
    }
    finally {
        $stopwatch.Stop()
        Stop-StartedProcessTree -RootProcess $owner
    }
}

function Get-GlobalBundleEvidence {
    $globalHtmlPath = Join-Path $script:WorkspaceRoot 'dist\global.html'
    if (-not (Test-Path -LiteralPath $globalHtmlPath -PathType Leaf)) {
        throw 'The production build did not emit global.html.'
    }
    $html = Get-Content -LiteralPath $globalHtmlPath -Raw
    $references = @(
        [regex]::Matches($html, '(?:src|href)="(?<path>/[^"]+)"') |
            ForEach-Object { $_.Groups['path'].Value }
    )
    if ($references.Count -eq 0) {
        throw 'global.html has no production assets.'
    }
    if (@($references | Where-Object { $_ -match '/assets/project-' }).Count -gt 0) {
        throw 'The global entry loads a Project editor asset.'
    }
    $assets = @(
        foreach ($reference in $references) {
            $relativePath = $reference.TrimStart('/').Replace(
                '/',
                [System.IO.Path]::DirectorySeparatorChar
            )
            $fullPath = Join-Path $script:WorkspaceRoot "dist\$relativePath"
            if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
                throw "Missing global production asset: $reference"
            }
            [ordered]@{
                path = $reference
                bytes = [long] (Get-Item -LiteralPath $fullPath).Length
                sha256 = (
                    Get-FileHash -LiteralPath $fullPath -Algorithm SHA256
                ).Hash.ToLowerInvariant()
            }
        }
    )
    return [ordered]@{
        htmlBytes = [long] (Get-Item -LiteralPath $globalHtmlPath).Length
        directAssetBytes = [long] (
            ($assets | ForEach-Object { $_.bytes } | Measure-Object -Sum).Sum
        )
        assets = $assets
        projectAssetReferences = 0
    }
}

$tokens = $null
$parserErrors = $null
[void] [System.Management.Automation.Language.Parser]::ParseFile(
    $PSCommandPath,
    [ref] $tokens,
    [ref] $parserErrors
)
if (@($parserErrors).Count -gt 0) {
    $messages = @($parserErrors | ForEach-Object { $_.Message }) -join ' | '
    throw "The global shell runner has parser errors: $messages"
}
Add-Check -Name 'powershell-runner-ast' -ElapsedMs 0

$initialBuildInputs = Get-BuildInputState
if ($initialBuildInputs.dirty) {
    throw 'Global shell evidence requires clean source and build inputs.'
}

$previousCargoTarget = [System.Environment]::GetEnvironmentVariable(
    'CARGO_TARGET_DIR',
    [System.EnvironmentVariableTarget]::Process
)
$previousCargoBuildJobs = [System.Environment]::GetEnvironmentVariable(
    'CARGO_BUILD_JOBS',
    [System.EnvironmentVariableTarget]::Process
)
$locationWasPushed = $false

try {
    [System.Environment]::SetEnvironmentVariable(
        'CARGO_TARGET_DIR',
        $targetDirectory,
        [System.EnvironmentVariableTarget]::Process
    )
    [System.Environment]::SetEnvironmentVariable(
        'CARGO_BUILD_JOBS',
        [string] $CargoBuildJobs,
        [System.EnvironmentVariableTarget]::Process
    )
    Push-Location $script:WorkspaceRoot
    $locationWasPushed = $true

    Invoke-CheckedCommand `
        -Name 'frontend-global-shell-contract' `
        -Executable 'npx' `
        -Arguments @(
            'vitest', 'run',
            'src/global/GlobalShell.test.tsx',
            'src/platform/tauriBoundary.test.ts'
        )
    Invoke-CheckedCommand `
        -Name 'rust-global-shell-contract' `
        -Executable $script:CargoExecutable `
        -Arguments @(
            'test', '-p', 'myalbuns-desktop', '--lib', 'global'
        )

    $buildWatch = [System.Diagnostics.Stopwatch]::StartNew()
    & (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1') build --no-bundle
    $buildExitCode = $LASTEXITCODE
    $buildWatch.Stop()
    if ($buildExitCode -ne 0) {
        throw "The global shell release build failed with code $buildExitCode."
    }
    Add-Check `
        -Name 'tauri-release-with-global-entry' `
        -ElapsedMs $buildWatch.ElapsedMilliseconds
    if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
        throw 'The global shell release executable is missing.'
    }

    $bundle = Get-GlobalBundleEvidence
    Add-Check -Name 'global-html-excludes-project-entry' -ElapsedMs 0

    $independentWatch = [System.Diagnostics.Stopwatch]::StartNew()
    $independent = Invoke-GlobalShellRun -Topology 'independent'
    $independentWatch.Stop()
    Add-Check `
        -Name 'independent-global-shell-runtime' `
        -ElapsedMs $independentWatch.ElapsedMilliseconds

    $multiwindowWatch = [System.Diagnostics.Stopwatch]::StartNew()
    $multiwindow = Invoke-GlobalShellRun -Topology 'multiwindow'
    $multiwindowWatch.Stop()
    Add-Check `
        -Name 'multiwindow-global-shell-runtime' `
        -ElapsedMs $multiwindowWatch.ElapsedMilliseconds
}
finally {
    foreach ($process in $startedProcesses) {
        if ($null -ne $process) {
            Stop-StartedProcessTree -RootProcess $process
        }
    }
    if ($locationWasPushed) {
        Pop-Location
    }
    [System.Environment]::SetEnvironmentVariable(
        'CARGO_TARGET_DIR',
        $previousCargoTarget,
        [System.EnvironmentVariableTarget]::Process
    )
    [System.Environment]::SetEnvironmentVariable(
        'CARGO_BUILD_JOBS',
        $previousCargoBuildJobs,
        [System.EnvironmentVariableTarget]::Process
    )
}

$finalBuildInputs = Get-BuildInputState
if (
    $finalBuildInputs.dirty -or
    $finalBuildInputs.digestSha256 -cne $initialBuildInputs.digestSha256
) {
    throw 'Source or build inputs changed while the global shell gate was running.'
}

$gitCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) {
    throw 'Could not resolve the global shell evidence commit.'
}
$workingTreeStatus = @(& git -C $script:WorkspaceRoot status --short)
if ($LASTEXITCODE -ne 0) {
    throw 'Could not inspect the final working tree.'
}
$computerSystem = Get-CimInstance Win32_ComputerSystem
$operatingSystem = Get-CimInstance Win32_OperatingSystem
$processors = @(Get-CimInstance Win32_Processor)
$videoControllers = @(Get-CimInstance Win32_VideoController)

$report = [ordered]@{
    schemaVersion = 1
    suite = 'global_shell_candidate'
    collectedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
    gitCommit = $gitCommit
    sourceInputsDirty = $false
    build = [ordered]@{
        profile = 'release'
        inputFileCount = $initialBuildInputs.fileCount
        inputDigestSha256 = $initialBuildInputs.digestSha256
        executable = 'target/global-shell-gate/release/myalbuns-desktop.exe'
        executableSha256 = (
            Get-FileHash -LiteralPath $executablePath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        globalBundle = $bundle
        workingTreeDirty = $workingTreeStatus.Count -gt 0
    }
    hardware = [ordered]@{
        operatingSystem = $operatingSystem.Caption
        operatingSystemVersion = $operatingSystem.Version
        processors = @($processors | ForEach-Object { $_.Name })
        physicalMemoryBytes = [long] $computerSystem.TotalPhysicalMemory
        videoControllers = @(
            $videoControllers | ForEach-Object {
                [ordered]@{
                    name = $_.Name
                    driverVersion = $_.DriverVersion
                }
            }
        )
    }
    checks = @($checks)
    results = [ordered]@{
        passed = [bool] ($independent.passed -and $multiwindow.passed)
        ranking = $null
        recommendation = $null
        independent = $independent
        multiwindow = $multiwindow
    }
    interpretation = [ordered]@{
        candidateOnly = $true
        numericBudgetApplied = $false
        reason = (
            'This gate materializes the real Welcome entry in both topologies, observes no ' +
            'Imaging Processor and checks the source boundary against editor dependencies. ' +
            'Final comparison budgets remain reserved for the terminal spike gate.'
        )
    }
}

$json = $report | ConvertTo-Json -Depth 14
[System.IO.File]::WriteAllText(
    $OutputPath,
    $json + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)

Write-Host "Global shell gate passed. Evidence: $OutputPath"
