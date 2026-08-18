param([string] $OutputPath)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
Initialize-MyAlbunsToolchain

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'The issue 45 Media and Cache gate must run on Windows.'
}

$workspaceRoot = $script:WorkspaceRoot
$canonicalOutputPath = [System.IO.Path]::GetFullPath(
    (Join-Path `
        $workspaceRoot `
        'docs\research\artifacts\0036-issue-45-media-cache-integration.json')
)
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = $canonicalOutputPath
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $workspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
if (-not [string]::Equals(
        $OutputPath,
        $canonicalOutputPath,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The issue 45 gate writes only its canonical versioned evidence artifact.'
}

$fixedPoint = 'f6518d63b2c75656a58b6769e87abc318a913e23'
$runnerMutex = [System.Threading.Mutex]::new(
    $false,
    'Local\MyAlbuns.Issue45MediaCacheGate.v1'
)
$runnerMutexHeld = $false
try {
    $runnerMutexHeld = $runnerMutex.WaitOne(0)
}
catch [System.Threading.AbandonedMutexException] {
    $runnerMutexHeld = $true
}
if (-not $runnerMutexHeld) {
    $runnerMutex.Dispose()
    throw 'Another issue 45 Media and Cache evidence runner is active.'
}

$scratchRoot = [System.IO.Path]::GetFullPath(
    (Join-Path `
        $workspaceRoot `
        '.scratch\cargo-target-tests\issue-45-media-cache')
)
$scratchRootExisted = Test-Path -LiteralPath $scratchRoot
$runRoot = $null
$distPath = Join-Path $workspaceRoot 'dist'
$distExistedBefore = Test-Path -LiteralPath $distPath
$preparedSidecarPath = Join-Path `
    $workspaceRoot `
    'src-tauri\binaries\myalbuns-imaging-x86_64-pc-windows-msvc.exe'
$preparedSidecarExistedBefore = Test-Path -LiteralPath $preparedSidecarPath
$windowsPathTarget = Join-Path $workspaceRoot 'target\windows-path-gate'
$windowsPathTargetExistedBefore = Test-Path -LiteralPath $windowsPathTarget
$previousModulePath = $env:PSModulePath
$previousTargetDirectory = $env:CARGO_TARGET_DIR
$gateRunStartedUtc = [DateTime]::UtcNow
$ownedProcessRecords = [System.Collections.Generic.Dictionary[string, object]]::new()
$ownedParentProcessIds = [System.Collections.Generic.HashSet[uint32]]::new()

function Get-ProcessCreationUtc([object] $Process) {
    if ($Process.CreationDate -is [DateTime]) {
        return ([DateTime] $Process.CreationDate).ToUniversalTime()
    }
    return [System.Management.ManagementDateTimeConverter]::ToDateTime(
        [string] $Process.CreationDate
    ).ToUniversalTime()
}

function Get-GateProcessIdentity([object] $Process) {
    $created = Get-ProcessCreationUtc -Process $Process
    return "$([uint32] $Process.ProcessId)|$($created.Ticks)"
}

function Test-WorkspaceProcess([object] $Process) {
    $workspacePrefix = $workspaceRoot.TrimEnd('\', '/') + '\'
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

function Get-WorkspaceProcesses {
    return @(
        Get-CimInstance Win32_Process |
            Where-Object {
                $_.ProcessId -ne $PID -and (Test-WorkspaceProcess -Process $_)
            }
    )
}

function Register-OwnedGateProcesses {
    param(
        [Parameter(Mandatory = $true)]
        [uint32] $RootProcessId,

        [Parameter(Mandatory = $true)]
        [DateTime] $CommandStartedUtc
    )

    $processes = @(Get-CimInstance Win32_Process)
    $knownParents = [System.Collections.Generic.HashSet[uint32]]::new()
    [void] $knownParents.Add($RootProcessId)
    foreach ($known in $ownedParentProcessIds) {
        [void] $knownParents.Add($known)
    }
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($process in $processes) {
            $processId = [uint32] $process.ProcessId
            if ($processId -eq $PID) {
                continue
            }
            $createdUtc = Get-ProcessCreationUtc -Process $process
            $isRoot = $processId -eq $RootProcessId
            $hasOwnedParent = $knownParents.Contains([uint32] $process.ParentProcessId)
            $isNewWorkspaceProcess = $createdUtc -ge $CommandStartedUtc -and
                (Test-WorkspaceProcess -Process $process)
            if (-not ($isRoot -or $hasOwnedParent -or $isNewWorkspaceProcess)) {
                continue
            }
            $identity = Get-GateProcessIdentity -Process $process
            if (-not $ownedProcessRecords.ContainsKey($identity)) {
                $ownedProcessRecords.Add($identity, [pscustomobject]@{
                    processId = $processId
                    parentProcessId = [uint32] $process.ParentProcessId
                    creationUtc = $createdUtc
                    executablePath = [string] $process.ExecutablePath
                    commandLine = [string] $process.CommandLine
                })
                $changed = $true
            }
            [void] $knownParents.Add($processId)
            [void] $ownedParentProcessIds.Add($processId)
        }
    }
}

function Get-ActiveOwnedGateProcesses {
    $active = [System.Collections.Generic.List[object]]::new()
    foreach ($process in @(Get-CimInstance Win32_Process)) {
        $identity = Get-GateProcessIdentity -Process $process
        if ($ownedProcessRecords.ContainsKey($identity)) {
            $active.Add($process)
        }
    }
    return @($active.ToArray())
}

function Get-OwnedGateListeners([object[]] $Processes) {
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

function Stop-OwnedGateProcesses {
    $before = @(Get-ActiveOwnedGateProcesses)
    $listenersBefore = @(Get-OwnedGateListeners -Processes $before)
    foreach ($process in ($before | Sort-Object ProcessId -Descending)) {
        Stop-Process -Id ([int] $process.ProcessId) -Force -ErrorAction SilentlyContinue
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    $after = @(Get-ActiveOwnedGateProcesses)
    while ($after.Count -ne 0 -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 50
        $after = @(Get-ActiveOwnedGateProcesses)
    }
    $listenersAfter = @(Get-OwnedGateListeners -Processes $after)
    if ($after.Count -ne 0 -or $listenersAfter.Count -ne 0) {
        $identifiers = @($after | ForEach-Object { $_.ProcessId }) -join ', '
        throw "The issue 45 gate could not terminate its owned process tree: $identifiers."
    }
    return [pscustomobject]@{
        stoppedProcessCount = $before.Count
        listenersBefore = $listenersBefore.Count
        processesAfter = $after.Count
        listenersAfter = $listenersAfter.Count
    }
}

function Clear-Issue45GateOutputs {
    $cleanupFailures = [System.Collections.Generic.List[string]]::new()

    try {
        if (-not $preparedSidecarExistedBefore -and
                (Test-Path -LiteralPath $preparedSidecarPath -PathType Leaf)) {
            [System.IO.File]::Delete($preparedSidecarPath)
        }
    }
    catch {
        $cleanupFailures.Add("prepared sidecar: $($_.Exception.Message)")
    }

    try {
        if (-not $windowsPathTargetExistedBefore -and
                (Test-Path -LiteralPath $windowsPathTarget)) {
            Remove-GateScratchDirectory `
                -Path $windowsPathTarget `
                -AllowedParent (Join-Path $workspaceRoot 'target')
        }
    }
    catch {
        $cleanupFailures.Add("Windows path target: $($_.Exception.Message)")
    }

    try {
        if (-not $distExistedBefore -and (Test-Path -LiteralPath $distPath)) {
            Remove-GateScratchDirectory `
                -Path $distPath `
                -AllowedParent $workspaceRoot
        }
    }
    catch {
        $cleanupFailures.Add("frontend distribution: $($_.Exception.Message)")
    }

    try {
        if (-not [string]::IsNullOrWhiteSpace($runRoot) -and
                (Test-Path -LiteralPath $runRoot)) {
            Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchRoot
        }
    }
    catch {
        $cleanupFailures.Add("run scratch: $($_.Exception.Message)")
    }

    try {
        if (-not $scratchRootExisted -and
                (Test-Path -LiteralPath $scratchRoot) -and
                @(Get-ChildItem -LiteralPath $scratchRoot -Force).Count -eq 0) {
            [System.IO.Directory]::Delete($scratchRoot)
        }
    }
    catch {
        $cleanupFailures.Add("scratch root: $($_.Exception.Message)")
    }

    if ($cleanupFailures.Count -ne 0) {
        throw "The issue 45 gate could not clean all owned outputs: $($cleanupFailures -join '; ')"
    }
}

try {
New-Item -ItemType Directory -Force -Path $scratchRoot | Out-Null
& git -C $workspaceRoot check-ignore --quiet -- $scratchRoot
if ($LASTEXITCODE -ne 0) {
    throw 'The issue 45 gate scratch root must be excluded from source provenance.'
}
$runRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $scratchRoot "run-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 8))")
)
if (-not [string]::Equals(
        [System.IO.Path]::GetDirectoryName($runRoot),
        $scratchRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The issue 45 gate scratch directory escaped its approved root.'
}
New-Item -ItemType Directory -Force -Path $runRoot | Out-Null

$preexistingProcesses = @(Get-WorkspaceProcesses)
if ($preexistingProcesses.Count -ne 0) {
    $identifiers = @($preexistingProcesses | ForEach-Object { $_.ProcessId }) -join ', '
    throw "The issue 45 gate requires zero pre-existing worktree processes: $identifiers."
}

$sourceBefore = Get-GateSourceSnapshot `
    -WorkspaceRoot $workspaceRoot `
    -EvidencePath $OutputPath
if ($sourceBefore.sourceInputsDirty) {
    throw 'The issue 45 gate requires a clean behavioral input commit.'
}
$mergeBase = (& git -C $workspaceRoot merge-base HEAD $fixedPoint).Trim()
if ($LASTEXITCODE -ne 0 -or $mergeBase -ne $fixedPoint) {
    throw 'The issue 45 gate input is not based on the required fixed point.'
}

$windowsPowerShell = Join-Path `
    $env:SystemRoot `
    'System32\WindowsPowerShell\v1.0\powershell.exe'
$npm = (Get-Command npm.cmd -ErrorAction Stop).Source
$standardModulePath = Join-Path `
    $env:SystemRoot `
    'System32\WindowsPowerShell\v1.0\Modules'
if ($standardModulePath -notin @($env:PSModulePath -split ';')) {
    $env:PSModulePath = "$standardModulePath;$env:PSModulePath"
}
$checks = [System.Collections.Generic.List[object]]::new()

function Get-NormalizedCommandOutput([object[]] $Lines) {
    $text = ($Lines | ForEach-Object { $_.ToString() }) -join "`n"
    return $text -replace "$([char]27)\[[0-9;?]*[ -/]*[@-~]", ''
}

function Invoke-RecordedCommand {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [string] $FilePath,

        [Parameter(Mandatory = $true)]
        [string[]] $Arguments
    )

    Write-Host "START $Name"
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $commandStartedUtc = [DateTime]::UtcNow
    $payloadPath = Join-Path $runRoot "$Name-command.json"
    $wrapperPath = Join-Path $runRoot "$Name-command.ps1"
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
param([Parameter(Mandatory = $true)][string] $PayloadPath)
$ErrorActionPreference = 'Stop'
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
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $windowsPowerShell
    $startInfo.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$escapedWrapper`" `"$escapedPayload`""
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "Gate command '$Name' could not be started."
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    while (-not $process.HasExited) {
        Register-OwnedGateProcesses `
            -RootProcessId ([uint32] $process.Id) `
            -CommandStartedUtc $commandStartedUtc
        Start-Sleep -Milliseconds 50
    }
    $process.WaitForExit()
    Register-OwnedGateProcesses `
        -RootProcessId ([uint32] $process.Id) `
        -CommandStartedUtc $commandStartedUtc
    $exitCode = $process.ExitCode
    $rawOutput = @(
        $stdoutTask.GetAwaiter().GetResult()
        $stderrTask.GetAwaiter().GetResult()
    )
    $process.Dispose()
    $stopwatch.Stop()
    $output = Get-NormalizedCommandOutput -Lines $rawOutput
    $logPath = Join-Path $runRoot "$Name.log"
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

function Get-Sha256([string] $Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
        $stream.Dispose()
    }
}

function Get-ReleaseArtifact([string] $Name, [string] $Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "The release artifact '$Name' was not produced."
    }
    $file = Get-Item -LiteralPath $Path
    return [ordered]@{
        name = $Name
        bytes = [long] $file.Length
        sha256 = Get-Sha256 -Path $file.FullName
    }
}

function Test-ExclusiveRead([string] $Path) {
    $stream = [System.IO.File]::Open(
        $Path,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::None
    )
    $stream.Dispose()
}

function New-VerifiedCriterion {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [object[]] $Requirements
    )

    $proofs = [System.Collections.Generic.List[object]]::new()
    $assertionCount = 0
    foreach ($requirement in @($Requirements)) {
        $sourceText = [string] $requirement.sourceText
        $requiredText = [string] $requirement.requiredText
        $matchCount = [regex]::Matches(
            $sourceText,
            [regex]::Escape($requiredText),
            [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
        ).Count
        if ($matchCount -lt 1) {
            throw "Criterion '$Name' has no named proof '$requiredText' in '$($requirement.source)'."
        }
        $proofs.Add([ordered]@{
            source = [string] $requirement.source
            name = $requiredText
            matchCount = $matchCount
        })
        $assertionCount += $matchCount
    }
    $passed = $proofs.Count -eq @($Requirements).Count -and $assertionCount -ge $proofs.Count
    if (-not $passed) {
        throw "Criterion '$Name' did not retain every required named proof."
    }
    return [ordered]@{
        name = $Name
        passed = [bool] $passed
        assertionCount = $assertionCount
        proofs = @($proofs.ToArray())
    }
}

function Invoke-OwnedCleanupProbe {
    $probeScript = Join-Path $runRoot 'owned-cleanup-probe.ps1'
    $probeReady = Join-Path $runRoot 'owned-cleanup-probe.ready'
    $probeSource = @'
param([Parameter(Mandatory = $true)][string] $ReadyPath)
$listener = [System.Net.Sockets.TcpListener]::new(
    [System.Net.IPAddress]::Loopback,
    0
)
$listener.Start()
$port = ([System.Net.IPEndPoint] $listener.LocalEndpoint).Port
[System.IO.File]::WriteAllText($ReadyPath, [string] $port)
while ($true) { Start-Sleep -Seconds 1 }
'@
    [System.IO.File]::WriteAllText(
        $probeScript,
        $probeSource + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $windowsPowerShell
    $startInfo.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$probeScript`" `"$probeReady`""
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw 'The owned-process cleanup probe could not start.'
    }
    $startedUtc = [DateTime]::UtcNow
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while (-not (Test-Path -LiteralPath $probeReady -PathType Leaf)) {
        if ($process.HasExited) {
            throw 'The owned-process cleanup probe exited before listening.'
        }
        Register-OwnedGateProcesses `
            -RootProcessId ([uint32] $process.Id) `
            -CommandStartedUtc $startedUtc
        if ([DateTime]::UtcNow -ge $deadline) {
            throw 'The owned-process cleanup probe did not become ready.'
        }
        Start-Sleep -Milliseconds 50
    }
    Register-OwnedGateProcesses `
        -RootProcessId ([uint32] $process.Id) `
        -CommandStartedUtc $startedUtc
    $active = @(Get-ActiveOwnedGateProcesses)
    $listeners = @(Get-OwnedGateListeners -Processes $active)
    while ($listeners.Count -eq 0 -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 50
        $active = @(Get-ActiveOwnedGateProcesses)
        $listeners = @(Get-OwnedGateListeners -Processes $active)
    }
    if ($active.Count -lt 1 -or $listeners.Count -lt 1) {
        throw 'The cleanup probe did not causally observe its process and listener.'
    }
    $cleanup = Stop-OwnedGateProcesses
    $process.WaitForExit()
    $process.Dispose()
    if ($cleanup.stoppedProcessCount -lt 1 `
            -or $cleanup.listenersBefore -lt 1 `
            -or $cleanup.processesAfter -ne 0 `
            -or $cleanup.listenersAfter -ne 0) {
        throw 'The cleanup probe did not terminate and verify its complete owned state.'
    }
    return $cleanup
}

try {
    $cleanupProbe = Invoke-OwnedCleanupProbe
    $checks.Add([ordered]@{
        name = 'owned-process-listener-cleanup-probe'
        passed = ($cleanupProbe.stoppedProcessCount -ge 1 -and
            $cleanupProbe.listenersBefore -ge 1 -and
            $cleanupProbe.processesAfter -eq 0 -and
            $cleanupProbe.listenersAfter -eq 0)
        assertionCount = 4
        stoppedProcessCount = $cleanupProbe.stoppedProcessCount
        observedListenerCount = $cleanupProbe.listenersBefore
    })

    $bootstrapTarget = Join-Path $runRoot 'bootstrap-target'
    $env:CARGO_TARGET_DIR = $bootstrapTarget
    $sidecarPreparationRun = Invoke-RecordedCommand `
        -Name 'debug-sidecar-preparation' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Prepare-Sidecar.ps1'),
            '-Profile',
            'debug'
        )
    $env:CARGO_TARGET_DIR = $previousTargetDirectory
    if (-not (Test-Path -LiteralPath $preparedSidecarPath -PathType Leaf)) {
        throw 'The clean evidence run did not prepare the required debug sidecar.'
    }
    Test-ExclusiveRead -Path $preparedSidecarPath
    $checks.Add([ordered]@{
        name = 'clean-debug-sidecar-preparation'
        passed = $true
        assertionCount = 2
        elapsedMs = $sidecarPreparationRun.elapsedMs
    })

    $contractRun = Invoke-RecordedCommand `
        -Name 'contracts' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-Contracts.ps1')
        )
    $contractCount = @(
        Get-ChildItem -LiteralPath (Join-Path $workspaceRoot 'src\domain\generated') -File
        Get-ChildItem -LiteralPath (Join-Path $workspaceRoot 'src\platform\generated') -File
    ).Count
    if ($contractCount -lt 1) {
        throw 'The contract gate produced an empty binding count.'
    }
    $checks.Add([ordered]@{
        name = 'rust-typescript-contracts'
        passed = $true
        assertionCount = $contractCount
        elapsedMs = $contractRun.elapsedMs
    })

    $frontendRun = Invoke-RecordedCommand `
        -Name 'frontend-tests' `
        -FilePath $npm `
        -Arguments @('test', '--', '--reporter=verbose')
    $frontendFilesMatch = [regex]::Match(
        $frontendRun.output,
        'Test Files\s+(\d+) passed'
    )
    $frontendTestsMatch = [regex]::Match(
        $frontendRun.output,
        'Tests\s+(\d+) passed'
    )
    if (-not $frontendFilesMatch.Success -or -not $frontendTestsMatch.Success) {
        throw 'The frontend gate did not report non-empty passing counts.'
    }
    $frontendFileCount = [int] $frontendFilesMatch.Groups[1].Value
    $frontendTestCount = [int] $frontendTestsMatch.Groups[1].Value
    if ($frontendFileCount -lt 1 -or $frontendTestCount -lt 1) {
        throw 'The frontend gate reported an empty passing count.'
    }
    $checks.Add([ordered]@{
        name = 'frontend-tests'
        passed = $true
        assertionCount = $frontendTestCount
        fileCount = $frontendFileCount
        elapsedMs = $frontendRun.elapsedMs
    })

    $typecheckRun = Invoke-RecordedCommand `
        -Name 'frontend-typecheck' `
        -FilePath $npm `
        -Arguments @('run', 'typecheck')
    $checks.Add([ordered]@{
        name = 'frontend-typecheck'
        passed = $true
        assertionCount = 1
        elapsedMs = $typecheckRun.elapsedMs
    })

    $rustRun = Invoke-RecordedCommand `
        -Name 'rust-tests' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-Rust.ps1')
        )
    $rustMatches = [regex]::Matches(
        $rustRun.output,
        'test result: ok\.\s+(\d+) passed;'
    )
    $rustTestCount = 0
    foreach ($match in $rustMatches) {
        $rustTestCount += [int] $match.Groups[1].Value
    }
    if ($rustTestCount -lt 1) {
        throw 'The Rust gate did not report a non-empty passing count.'
    }
    $checks.Add([ordered]@{
        name = 'rust-tests'
        passed = $true
        assertionCount = $rustTestCount
        suiteResultCount = $rustMatches.Count
        elapsedMs = $rustRun.elapsedMs
    })

    $qualityRun = Invoke-RecordedCommand `
        -Name 'rust-quality' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-RustQuality.ps1')
        )
    $checks.Add([ordered]@{
        name = 'rust-fmt-clippy-deny-warnings'
        passed = $true
        assertionCount = 3
        elapsedMs = $qualityRun.elapsedMs
    })

    $imagingEvidencePath = Join-Path $runRoot 'imaging-recovery.json'
    $imagingRun = Invoke-RecordedCommand `
        -Name 'imaging-recovery' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-ImagingRecovery.ps1'),
            '-OutputPath',
            $imagingEvidencePath
        )
    $imagingEvidence = Get-Content -LiteralPath $imagingEvidencePath -Raw |
        ConvertFrom-Json
    $imagingCheckCount = @($imagingEvidence.checks).Count
    $imagingFailedCheckCount = @(
        $imagingEvidence.checks | Where-Object { -not $_.passed }
    ).Count
    if ($imagingEvidence.sourceInputsDirty `
            -or $imagingEvidence.gitCommit -ne $sourceBefore.gitCommit `
            -or $imagingCheckCount -lt 1 `
            -or $imagingFailedCheckCount -ne 0) {
        throw "The real Processor/Cache/Canvas recovery evidence is not authoritative: sourceInputsDirty=$($imagingEvidence.sourceInputsDirty), gitCommit=$($imagingEvidence.gitCommit), expectedCommit=$($sourceBefore.gitCommit), checks=$imagingCheckCount, failed=$imagingFailedCheckCount."
    }
    $checks.Add([ordered]@{
        name = 'real-processor-cache-canvas-recovery'
        passed = $true
        assertionCount = $imagingCheckCount
        elapsedMs = $imagingRun.elapsedMs
    })

    $windowsEvidencePath = Join-Path $runRoot 'windows-paths.json'
    $windowsRun = Invoke-RecordedCommand `
        -Name 'windows-paths' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Test-WindowsPathGate.ps1'),
            '-OutputPath',
            $windowsEvidencePath
        )
    $windowsEvidence = Get-Content -LiteralPath $windowsEvidencePath -Raw |
        ConvertFrom-Json
    $windowsCheckCount = @($windowsEvidence.checks).Count
    $windowsFailedCheckCount = @(
        $windowsEvidence.checks | Where-Object { -not $_.passed }
    ).Count
    if ($windowsEvidence.sourceInputsDirty `
            -or $windowsEvidence.gitCommit -ne $sourceBefore.gitCommit `
            -or $windowsCheckCount -lt 1 `
            -or $windowsFailedCheckCount -ne 0) {
        throw "The Windows local/UNC/mapped/long-path evidence is not authoritative: sourceInputsDirty=$($windowsEvidence.sourceInputsDirty), gitCommit=$($windowsEvidence.gitCommit), expectedCommit=$($sourceBefore.gitCommit), checks=$windowsCheckCount, failed=$windowsFailedCheckCount."
    }
    $checks.Add([ordered]@{
        name = 'windows-local-unc-mapped-long-paths'
        passed = $true
        assertionCount = $windowsCheckCount
        elapsedMs = $windowsRun.elapsedMs
    })

    $releaseTarget = Join-Path $runRoot 'release-target'
    $env:CARGO_TARGET_DIR = $releaseTarget
    $releaseRun = Invoke-RecordedCommand `
        -Name 'release-nsis-bundle' `
        -FilePath $windowsPowerShell `
        -Arguments @(
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1'),
            '-Action',
            'build'
        )
    $env:CARGO_TARGET_DIR = $previousTargetDirectory

    $installerCandidates = @(
        Get-ChildItem `
            -LiteralPath (Join-Path $releaseTarget 'release\bundle\nsis') `
            -Filter '*setup.exe' `
            -File
    )
    if ($installerCandidates.Count -ne 1) {
        throw "The NSIS gate expected one installer and found $($installerCandidates.Count)."
    }
    $builtSidecarPath = Join-Path `
        $releaseTarget `
        'sidecar-build\release\myalbuns-imaging.exe'
    $releaseArtifacts = @(
        Get-ReleaseArtifact `
            -Name 'desktop-release' `
            -Path (Join-Path $releaseTarget 'release\myalbuns-desktop.exe')
        Get-ReleaseArtifact `
            -Name 'imaging-release' `
            -Path $builtSidecarPath
        Get-ReleaseArtifact `
            -Name 'prepared-sidecar' `
            -Path $preparedSidecarPath
        Get-ReleaseArtifact `
            -Name 'nsis-installer' `
            -Path $installerCandidates[0].FullName
    )
    if ($releaseArtifacts[1].sha256 -ne $releaseArtifacts[2].sha256) {
        throw 'The sidecar prepared for packaging does not match the release Processor.'
    }
    foreach ($path in @(
            (Join-Path $releaseTarget 'release\myalbuns-desktop.exe'),
            $builtSidecarPath,
            $preparedSidecarPath,
            $installerCandidates[0].FullName
        )) {
        Test-ExclusiveRead -Path $path
    }
    $checks.Add([ordered]@{
        name = 'release-build-and-nsis-package'
        passed = $true
        assertionCount = $releaseArtifacts.Count
        elapsedMs = $releaseRun.elapsedMs
    })

    $remainingProcesses = @(Get-ActiveOwnedGateProcesses)
    $remainingListeners = @(Get-OwnedGateListeners -Processes $remainingProcesses)
    $untrackedWorkspaceProcesses = @(Get-WorkspaceProcesses)
    if ($remainingProcesses.Count -ne 0 `
            -or $remainingListeners.Count -ne 0 `
            -or $untrackedWorkspaceProcesses.Count -ne 0) {
        $identifiers = @(
            $remainingProcesses
            $untrackedWorkspaceProcesses
        ) | ForEach-Object { $_.ProcessId } | Sort-Object -Unique
        $identifiers = @($identifiers) -join ', '
        throw "The issue 45 gate left worktree processes alive: $identifiers."
    }
    $ownedProcessCountAfter = $remainingProcesses.Count
    $ownedListenerCountAfter = $remainingListeners.Count
    $checks.Add([ordered]@{
        name = 'owned-process-lock-listener-cleanup'
        passed = ($ownedProcessCountAfter -eq 0 -and
            $ownedListenerCountAfter -eq 0)
        assertionCount = 3
        ownedProcessCount = $ownedProcessCountAfter
        ownedListenerCount = $ownedListenerCountAfter
        exclusiveArtifactLockFailures = 0
    })

    Clear-Issue45GateOutputs

    $sourceAfter = Get-GateSourceSnapshot `
        -WorkspaceRoot $workspaceRoot `
        -EvidencePath $OutputPath
    $sourceInputsDirty = Test-GateSourceSnapshotsDirty `
        -Before $sourceBefore `
        -After $sourceAfter
    if ($sourceInputsDirty) {
        throw 'The issue 45 gate source changed during evidence collection.'
    }

    $imagingProofText = @($imagingEvidence.checks | ForEach-Object { $_.name }) -join "`n"
    $windowsProofText = @($windowsEvidence.checks | ForEach-Object { $_.name }) -join "`n"
    $cacheServiceSource = Get-Content `
        -LiteralPath (Join-Path $workspaceRoot 'src-tauri\src\cache_service.rs') `
        -Raw `
        -Encoding UTF8
    $cacheCommandMatches = [regex]::Matches(
        $cacheServiceSource,
        '(?ms)^#\[tauri::command\]\s+pub\(crate\) async fn (cache_service_status|free_closed_project_cache|clear_all_cache)\b'
    )
    $cacheCommandNames = @(
        $cacheCommandMatches | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
    )
    if ($cacheCommandMatches.Count -ne 3 -or $cacheCommandNames.Count -ne 3) {
        throw 'The issue 16 Cache service surface is no longer exactly three narrow commands.'
    }
    $narrowApiProofText = $cacheCommandNames -join "`n"
    $researchProofText = Get-Content `
        -LiteralPath (Join-Path $workspaceRoot 'docs\research\0036-integracao-final-de-midias-e-cache.md') `
        -Raw `
        -Encoding UTF8
    $criteria = @(
        New-VerifiedCriterion `
            -Name 'authorized-independent-empty-namespace' `
            -Requirements @(
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'a_new_authorized_identity_reserves_an_independent_empty_namespace' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'cache_consumes_authoritative_identity_transitions_without_owning_them' }
            )
        New-VerifiedCriterion `
            -Name 'authoritative-absent-unavailable-and-visual-context' `
            -Requirements @(
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'resolver_monitor_and_runtime_keep_observed_state_outside_media_refs' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state' },
                @{ source = 'frontend-tests'; sourceText = $frontendRun.output; requiredText = 'keeps the last known preview when linked media becomes unavailable' },
                @{ source = 'frontend-tests'; sourceText = $frontendRun.output; requiredText = 'keeps the last representation only as visual context when the Original is absent' }
            )
        New-VerifiedCriterion `
            -Name 'relink-occurrence-stable-change-and-reappearance' `
            -Requirements @(
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'public_relink_command_updates_only_the_selected_occurrence_and_participates_in_history' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'public_relink_flow_reinspects_and_invalidates_only_the_selected_occurrence' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes' },
                @{ source = 'frontend-tests'; sourceText = $frontendRun.output; requiredText = 'offers public Relink only for an absent occurrence and applies the returned projection' }
            )
        New-VerifiedCriterion `
            -Name 'incompatible-corrupt-invalid-cache-rebuild' `
            -Requirements @(
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'corrupted_or_incompatible_index_is_discarded_and_rebuilt' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'failed_validation_discards_the_candidate_and_preserves_the_last_published_generation' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'a_wrong_response_correlation_discards_the_unpublished_candidate_generation' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'repeated_crashes_after_candidate_publication_leave_no_orphan_generation' }
            )
        New-VerifiedCriterion `
            -Name 'processor-restart-once-then-nonblocking-suspension' `
            -Requirements @(
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'repeated_processor_crashes_suspend_new_cache_work_after_one_restart' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'repeated_processor_failure_suspends_before_fallible_recovery_cleanup' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'terminating_the_host_closes_its_job_and_terminates_the_active_processor' },
                @{ source = 'frontend-tests'; sourceText = $frontendRun.output; requiredText = 'shows a non-blocking warning when repeated processor failures suspend Cache' },
                @{ source = 'imaging-recovery'; sourceText = $imagingProofText; requiredText = 'production-recovery-integration' }
            )
        New-VerifiedCriterion `
            -Name 'local-unc-mapped-and-long-paths' `
            -Requirements @(
                @{ source = 'windows-paths'; sourceText = $windowsProofText; requiredText = 'path-contract' },
                @{ source = 'windows-paths'; sourceText = $windowsProofText; requiredText = 'path-policy' },
                @{ source = 'windows-paths'; sourceText = $windowsProofText; requiredText = 'real-mapped-unc' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'prepares_the_cache_only_as_directories_below_the_authorized_root' }
            )
        New-VerifiedCriterion `
            -Name 'measure-free-reserve-and-safe-total-cleanup' `
            -Requirements @(
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'measures_and_frees_only_namespaces_without_an_active_owner' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'schedules_total_cleanup_while_a_project_is_active_and_runs_it_at_safe_startup' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'namespace_reservation_survives_process_boundaries_and_owner_termination' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'reopening_after_host_death_recovers_the_contained_processors_temporary' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'reserved_namespace_recovery_discards_abandoned_files_and_preserves_indexed_generation' }
            )
        New-VerifiedCriterion `
            -Name 'narrow-api-and-design-0010-ownership-matrix' `
            -Requirements @(
                @{ source = 'cache-service-source'; sourceText = $narrowApiProofText; requiredText = 'cache_service_status' },
                @{ source = 'cache-service-source'; sourceText = $narrowApiProofText; requiredText = 'free_closed_project_cache' },
                @{ source = 'cache-service-source'; sourceText = $narrowApiProofText; requiredText = 'clear_all_cache' },
                @{ source = 'rust-tests'; sourceText = $rustRun.output; requiredText = 'cache_consumes_authoritative_identity_transitions_without_owning_them' },
                @{ source = 'frontend-tests'; sourceText = $frontendRun.output; requiredText = 'maps the Project and media ports to the desktop commands' },
                @{ source = 'research-matrix'; sourceText = $researchProofText; requiredText = 'Religação de uma ocorrência' }
            )
    )
    if ($criteria.Count -ne 8 -or @($criteria | Where-Object { -not $_.passed }).Count -ne 0) {
        throw 'The issue 45 criteria matrix is incomplete or contains an unproved criterion.'
    }

    $report = [ordered]@{
        schemaVersion = 1
        gate = 'issue-45-media-cache-final-integration'
        issue = 45
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        fixedPoint = $fixedPoint
        gitCommit = $sourceBefore.gitCommit
        sourceInputsDirty = $false
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        }
        counts = [ordered]@{
            topLevelChecks = $checks.Count
            contractBindings = $contractCount
            frontendFiles = $frontendFileCount
            frontendTests = $frontendTestCount
            rustTests = $rustTestCount
            rustSuiteResults = $rustMatches.Count
            rustQualityCommands = 3
            imagingRecoveryChecks = $imagingCheckCount
            windowsPathChecks = $windowsCheckCount
            releaseArtifacts = $releaseArtifacts.Count
            ownedProcessesAfter = $ownedProcessCountAfter
            ownedListenersAfter = $ownedListenerCountAfter
        }
        checks = @($checks)
        criteria = $criteria
        nestedEvidence = [ordered]@{
            imaging = [ordered]@{
                schemaVersion = $imagingEvidence.schemaVersion
                checks = $imagingCheckCount
                cache = $imagingEvidence.evidence.cache
                canvas = $imagingEvidence.evidence.canvas
                pause = $imagingEvidence.evidence.pause
                obsolete = $imagingEvidence.evidence.obsolete
            }
            windowsPaths = [ordered]@{
                schemaVersion = $windowsEvidence.schemaVersion
                checks = $windowsCheckCount
                paths = $windowsEvidence.evidence.paths
                sidecar = $windowsEvidence.evidence.sidecar
                longPathAware = $windowsEvidence.evidence.longPathAware
            }
        }
        releaseArtifacts = $releaseArtifacts
        cleanup = [ordered]@{
            runScratchRemoved = $true
            newlyCreatedWindowsPathTargetRemoved = -not $windowsPathTargetExistedBefore
            newlyCreatedDistRemoved = -not $distExistedBefore
            ownedProcesses = $ownedProcessCountAfter
            ownedListeners = $ownedListenerCountAfter
            artifactLocks = 0
        }
    }
    $json = $report | ConvertTo-Json -Depth 12
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputPath) |
        Out-Null
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $json + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "Issue 45 Media and Cache report: $OutputPath"
    Write-Output $json
}
finally {
    $env:PSModulePath = $previousModulePath
    $env:CARGO_TARGET_DIR = $previousTargetDirectory
}
}
finally {
    $ownedCleanupFailure = $null
    try {
        $terminalCleanup = Stop-OwnedGateProcesses
        if ($terminalCleanup.processesAfter -ne 0 -or
                $terminalCleanup.listenersAfter -ne 0) {
            throw 'Owned process or listener state remained after terminal cleanup.'
        }
    }
    catch {
        $ownedCleanupFailure = $_.Exception.Message
    }
    try {
        Clear-Issue45GateOutputs
    }
    finally {
        if ($runnerMutexHeld) {
            $runnerMutex.ReleaseMutex()
        }
        $runnerMutex.Dispose()
    }
    if ($null -ne $ownedCleanupFailure) {
        throw "The issue 45 gate failed closed during terminal process cleanup: $ownedCleanupFailure"
    }
}
