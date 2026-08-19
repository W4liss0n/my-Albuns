param([string] $OutputPath)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
. (Join-Path $PSScriptRoot 'Gate-OwnedProcessJob.ps1')
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
$scratchRoot = [System.IO.Path]::GetFullPath(
    (Join-Path `
        $workspaceRoot `
        '.scratch\cargo-target-tests\issue-45-media-cache')
)
$scratchContainer = [System.IO.Path]::GetDirectoryName($scratchRoot)
$scratchContainerExisted = Test-Path -LiteralPath $scratchContainer
$scratchRootExisted = Test-Path -LiteralPath $scratchRoot
$runRoot = $null
$distPath = Join-Path $workspaceRoot 'dist'
$preparedSidecarPath = Join-Path `
    $workspaceRoot `
    'src-tauri\binaries\myalbuns-imaging-x86_64-pc-windows-msvc.exe'
$sharedCargoTarget = Join-Path $workspaceRoot 'target'
$windowsPathTarget = Join-Path $workspaceRoot 'target\windows-path-gate'
$workspaceScratch = Join-Path $workspaceRoot '.scratch'
$windowsPathScratch = Join-Path $workspaceRoot '.scratch\windows-path-gate'
function Assert-Issue45OwnedOutputsAbsent([string[]] $Paths) {
    $existing = @(
        $Paths | Where-Object { Test-Path -LiteralPath $_ }
    )
    if ($existing.Count -ne 0) {
        throw "The issue 45 gate requires its output paths to be absent before the run: $($existing -join ', ')."
    }
}
$ownedOutputPreflightPaths = @(
    $preparedSidecarPath
    $windowsPathTarget
    $distPath
    $sharedCargoTarget
    $scratchContainer
    $windowsPathScratch
)
Assert-Issue45OwnedOutputsAbsent -Paths $ownedOutputPreflightPaths
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
$previousModulePath = $env:PSModulePath
$previousTargetDirectory = $env:CARGO_TARGET_DIR
$gateRunStartedUtc = [DateTime]::UtcNow
$ownedProcessRecords = [System.Collections.Generic.Dictionary[string, object]]::new()
$ownedJobs = [System.Collections.Generic.List[object]]::new()
$preexistingProcessIdentities = [System.Collections.Generic.HashSet[string]]::new()

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

function Register-OwnedGateJobProcesses([object] $Job) {
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
        if ($preexistingProcessIdentities.Contains($identity)) {
            throw "A pre-existing process identity entered an issue 45 owned Job: $identity."
        }
        if (-not $ownedProcessRecords.ContainsKey($identity)) {
            $ownedProcessRecords.Add($identity, [pscustomobject]@{
                processId = $processId
                parentProcessId = [uint32] $process.ParentProcessId
                creationUtc = Get-ProcessCreationUtc -Process $process
                executablePath = [string] $process.ExecutablePath
                commandLine = [string] $process.CommandLine
            })
        }
    }
}

function Get-ActiveOwnedGateProcesses {
    $jobProcessIds = [System.Collections.Generic.HashSet[uint32]]::new()
    foreach ($job in @($ownedJobs.ToArray())) {
        Register-OwnedGateJobProcesses -Job $job
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
    $jobs = @($ownedJobs.ToArray())
    $before = @(Get-ActiveOwnedGateProcesses)
    $listenersBefore = @(Get-OwnedGateListeners -Processes $before)
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
    while ($remainingJobProcessCount -ne 0 -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 50
        $remainingJobProcessCount = @(
            $jobs | ForEach-Object { $_.ProcessIds() }
        ).Count
    }
    foreach ($job in $jobs) {
        $job.Dispose()
    }
    $ownedJobs.Clear()
    $after = @(Get-ActiveOwnedGateProcesses)
    $listenersAfter = @(Get-OwnedGateListeners -Processes $after)
    if ($remainingJobProcessCount -ne 0 -or
            $after.Count -ne 0 -or
            $listenersAfter.Count -ne 0) {
        $identifiers = @($after | ForEach-Object { $_.ProcessId }) -join ', '
        throw "The issue 45 gate could not terminate its owned process tree: $identifiers."
    }
    return [pscustomobject]@{
        stoppedProcessCount = $activeProcessIds.Count
        listenersBefore = $listenersBefore.Count
        processesAfter = $after.Count
        listenersAfter = $listenersAfter.Count
    }
}

function Clear-Issue45GateOutputs {
    $cleanupFailures = [System.Collections.Generic.List[string]]::new()

    try {
        if (Test-Path -LiteralPath $preparedSidecarPath -PathType Leaf) {
            [System.IO.File]::Delete($preparedSidecarPath)
        }
    }
    catch {
        $cleanupFailures.Add("prepared sidecar: $($_.Exception.Message)")
    }

    try {
        if (Test-Path -LiteralPath $windowsPathTarget) {
            Remove-GateScratchDirectory `
                -Path $windowsPathTarget `
                -AllowedParent (Join-Path $workspaceRoot 'target')
        }
    }
    catch {
        $cleanupFailures.Add("Windows path target: $($_.Exception.Message)")
    }

    try {
        if (Test-Path -LiteralPath $distPath) {
            Remove-GateScratchDirectory `
                -Path $distPath `
                -AllowedParent $workspaceRoot
        }
    }
    catch {
        $cleanupFailures.Add("frontend distribution: $($_.Exception.Message)")
    }

    try {
        if (Test-Path -LiteralPath $windowsPathScratch) {
            Remove-GateScratchDirectory `
                -Path $windowsPathScratch `
                -AllowedParent $workspaceScratch
        }
    }
    catch {
        $cleanupFailures.Add("Windows path scratch: $($_.Exception.Message)")
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

    try {
        if (-not $scratchContainerExisted -and
                (Test-Path -LiteralPath $scratchContainer) -and
                @(Get-ChildItem -LiteralPath $scratchContainer -Force).Count -eq 0) {
            [System.IO.Directory]::Delete($scratchContainer)
        }
    }
    catch {
        $cleanupFailures.Add("scratch container: $($_.Exception.Message)")
    }

    if ($cleanupFailures.Count -ne 0) {
        throw "The issue 45 gate could not clean all owned outputs: $($cleanupFailures -join '; ')"
    }
}

try {
foreach ($process in @(Get-CimInstance Win32_Process)) {
    if ([uint32] $process.ProcessId -ne $PID) {
        [void] $preexistingProcessIdentities.Add(
            (Get-GateProcessIdentity -Process $process)
        )
    }
}
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
$gateTarget = [System.IO.Path]::GetFullPath(
    (Join-Path $runRoot 'cargo-target')
)
if (-not [string]::Equals(
        [System.IO.Path]::GetDirectoryName($gateTarget),
        $runRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The issue 45 Cargo target escaped its approved run scratch.'
}
$env:CARGO_TARGET_DIR = $gateTarget

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
$json = $null

function Assert-Issue45SourceUnchanged {
    param(
        [Parameter(Mandatory = $true)]
        [psobject] $Before,

        [Parameter(Mandatory = $true)]
        [string] $WorkspaceRoot,

        [Parameter(Mandatory = $true)]
        [string] $EvidencePath,

        [Parameter(Mandatory = $true)]
        [string] $Stage
    )

    $after = Get-GateSourceSnapshot `
        -WorkspaceRoot $WorkspaceRoot `
        -EvidencePath $EvidencePath
    if (Test-GateSourceSnapshotsDirty -Before $Before -After $after) {
        throw "The issue 45 gate source changed $Stage."
    }
    return $after
}

function Test-PostProofSourceMutationContract([string] $FixtureRoot) {
    New-Item -ItemType Directory -Path $FixtureRoot | Out-Null
    $inputPath = Join-Path $FixtureRoot 'behavior.txt'
    $evidencePath = Join-Path $FixtureRoot 'evidence.json'
    $encoding = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($inputPath, "baseline`n", $encoding)
    & git -C $FixtureRoot init --quiet
    & git -C $FixtureRoot add -- behavior.txt
    & git `
        -C $FixtureRoot `
        -c user.name='MyAlbuns Gate' `
        -c user.email='gate@myalbuns.invalid' `
        commit --quiet -m baseline
    if ($LASTEXITCODE -ne 0) {
        throw 'The post-proof provenance fixture could not create its clean input commit.'
    }

    $before = Get-GateSourceSnapshot `
        -WorkspaceRoot $FixtureRoot `
        -EvidencePath $evidencePath
    [void] [System.IO.File]::ReadAllText($inputPath)
    [System.IO.File]::WriteAllText($inputPath, "mutated after proof`n", $encoding)

    $rejected = $false
    try {
        [void] (Assert-Issue45SourceUnchanged `
            -Before $before `
            -WorkspaceRoot $FixtureRoot `
            -EvidencePath $evidencePath `
            -Stage 'after the post-proof mutation fixture')
    }
    catch {
        if ($_.Exception.Message -ne
                'The issue 45 gate source changed after the post-proof mutation fixture.') {
            throw
        }
        $rejected = $true
    }
    if (-not $rejected) {
        throw 'The issue 45 gate accepted a behavioral input mutation after proof collection.'
    }
    return 1
}

function Get-NormalizedCommandOutput([object[]] $Lines) {
    $text = ($Lines | ForEach-Object { $_.ToString() }) -join "`n"
    return $text -replace "$([char]27)\[[0-9;?]*[ -/]*[@-~]", ''
}

function Start-OwnedGateProcess {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.ProcessStartInfo] $StartInfo,

        [Parameter(Mandatory = $true)]
        [string] $StartSignalPath
    )

    if (Test-Path -LiteralPath $StartSignalPath) {
        throw "The owned process start signal already exists: $StartSignalPath."
    }
    $job = [Issue45OwnedProcessJob]::new()
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $StartInfo
    $started = $false
    try {
        if (-not $process.Start()) {
            throw 'The owned gate process could not be started.'
        }
        $started = $true
        $job.Assign($process)
        $ownedJobs.Add($job)
        Register-OwnedGateJobProcesses -Job $job
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
    $payloadPath = Join-Path $runRoot "$Name-command.json"
    $wrapperPath = Join-Path $runRoot "$Name-command.ps1"
    $startSignalPath = Join-Path $runRoot "$Name-command.assigned"
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
    $startInfo.FileName = $windowsPowerShell
    $startInfo.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$escapedWrapper`" `"$escapedPayload`" `"$escapedStartSignal`""
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $owned = Start-OwnedGateProcess `
        -StartInfo $startInfo `
        -StartSignalPath $startSignalPath
    $process = $owned.process
    $job = $owned.job
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    while (-not $process.HasExited) {
        Register-OwnedGateJobProcesses -Job $job
        Start-Sleep -Milliseconds 50
    }
    $process.WaitForExit()
    Register-OwnedGateJobProcesses -Job $job
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

function Test-OwnedOutputPreflightContracts {
    $sentinelFile = Join-Path $runRoot 'preexisting-output.bin'
    $sentinelDirectory = Join-Path $runRoot 'preexisting-output-directory'
    $sentinelChild = Join-Path $sentinelDirectory 'sentinel.bin'
    $missing = Join-Path $runRoot 'absent-output'
    New-Item -ItemType Directory -Path $sentinelDirectory | Out-Null
    [System.IO.File]::WriteAllBytes($sentinelFile, [byte[]] (1, 3, 5, 7))
    [System.IO.File]::WriteAllBytes($sentinelChild, [byte[]] (2, 4, 6, 8))
    $fileHash = Get-Sha256 -Path $sentinelFile
    $childHash = Get-Sha256 -Path $sentinelChild
    $rejected = $false
    try {
        Assert-Issue45OwnedOutputsAbsent -Paths @(
            $sentinelFile
            $sentinelDirectory
            $missing
        )
    }
    catch {
        if ($_.Exception.Message -notlike 'The issue 45 gate requires its output paths to be absent*') {
            throw
        }
        $rejected = $true
    }
    if (-not $rejected -or
            (Get-Sha256 -Path $sentinelFile) -ne $fileHash -or
            (Get-Sha256 -Path $sentinelChild) -ne $childHash) {
        throw 'The output preflight did not reject and preserve its byte sentinels.'
    }
    Assert-Issue45OwnedOutputsAbsent -Paths @($missing)
    return 4
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

function Measure-VerifiedProof([object] $Requirement) {
    $requiredText = [string] $Requirement.requiredText
    switch ([string] $Requirement.proofKind) {
        'rust-test' {
            $resultPattern = '(?m)^test\s+(?:[A-Za-z0-9_]+::)*' +
                [regex]::Escape($requiredText) +
                '\s+\.\.\.\s+(?<status>ok|ignored|FAILED)\s*$'
            $results = [regex]::Matches(
                [string] $Requirement.sourceText,
                $resultPattern
            )
            if ($results.Count -ne 1 -or
                    $results[0].Groups['status'].Value -ne 'ok') {
                return 0
            }
            return 1
        }
        'frontend-test' {
            $results = @(
                $Requirement.sourceData |
                    Where-Object {
                        [string]::Equals(
                            [string] $_.title,
                            $requiredText,
                            [System.StringComparison]::Ordinal
                        )
                    }
            )
            if ($results.Count -ne 1 -or [string] $results[0].status -ne 'passed') {
                return 0
            }
            return 1
        }
        'exact-line' {
            return @(
                ([string] $Requirement.sourceText) -split "`r?`n" |
                    Where-Object {
                        [string]::Equals(
                            $_.Trim(),
                            $requiredText,
                            [System.StringComparison]::Ordinal
                        )
                    }
            ).Count
        }
        default {
            throw "Unknown issue 45 proof kind '$($Requirement.proofKind)'."
        }
    }
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
        $requiredText = [string] $requirement.requiredText
        $matchCount = Measure-VerifiedProof -Requirement $requirement
        if ($matchCount -ne 1) {
            throw "Criterion '$Name' has no single successful named proof '$requiredText' in '$($requirement.source)'."
        }
        $proofs.Add([ordered]@{
            source = [string] $requirement.source
            name = $requiredText
            matchCount = $matchCount
        })
        $assertionCount += 1
    }
    $passed = $proofs.Count -eq @($Requirements).Count -and
        $assertionCount -eq $proofs.Count -and
        @($proofs | Where-Object { $_.matchCount -ne 1 }).Count -eq 0
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

function Test-ProofParserContracts {
    $passedFrontend = [pscustomobject]@{ title = 'frontend proof'; status = 'passed' }
    $pendingFrontend = [pscustomobject]@{ title = 'frontend proof'; status = 'pending' }
    $cases = @(
        @{
            expected = 1
            requirement = @{
                proofKind = 'rust-test'
                sourceText = 'test module::rust_proof ... ok'
                requiredText = 'rust_proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'rust-test'
                sourceText = 'test module::rust_proof ... ignored'
                requiredText = 'rust_proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'rust-test'
                sourceText = "test a::rust_proof ... ok`ntest b::rust_proof ... ok"
                requiredText = 'rust_proof'
            }
        },
        @{
            expected = 1
            requirement = @{
                proofKind = 'frontend-test'
                sourceData = @($passedFrontend)
                requiredText = 'frontend proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'frontend-test'
                sourceData = @($pendingFrontend)
                requiredText = 'frontend proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'frontend-test'
                sourceData = @($passedFrontend, $passedFrontend)
                requiredText = 'frontend proof'
            }
        },
        @{
            expected = 1
            requirement = @{
                proofKind = 'exact-line'
                sourceText = "other`nexact proof"
                requiredText = 'exact proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'exact-line'
                sourceText = 'prefix exact proof suffix'
                requiredText = 'exact proof'
            }
        }
    )
    foreach ($case in $cases) {
        $actual = Measure-VerifiedProof -Requirement $case.requirement
        if ($actual -ne $case.expected) {
            throw "The fail-closed proof parser accepted or rejected the wrong fixture: expected=$($case.expected), actual=$actual."
        }
    }
    return $cases.Count
}

$expectedImagingRecoveryCheckNames = @(
    'protocol'
    'cache-temporary-cleanup'
    'imaging-sidecar-build'
    'production-recovery-integration'
    'cache-webview-canvas-export-journey'
    'obsolete-cache-cancellation-integration'
    'causal-cache-pause-integration'
    'actual-tauri-webview2-build'
    'actual-tauri-album-canvas-pixi-webview2'
)

$expectedWindowsPathCheckNames = @(
    'path-contract'
    'path-policy'
    'real-mapped-unc'
    'imaging-protocol'
    'imaging-sidecar-build'
    'sidecar-protocol-preflight'
    'desktop-host-build'
    'path-io-thread'
    'real-sidecar-frozen-plan'
    'desktop-long-path-manifest'
    'sidecar-long-path-manifest'
)

function Test-ExactPassedCheckSet(
    [object[]] $Checks,
    [string[]] $ExpectedNames
) {
    $actual = @($Checks)
    if ($actual.Count -ne @($ExpectedNames).Count) {
        return $false
    }
    $expected = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($name in @($ExpectedNames)) {
        if (-not $expected.Add([string] $name)) {
            throw "The expected check set duplicates '$name'."
        }
    }
    $seen = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($check in $actual) {
        if ($null -eq $check `
                -or $null -eq $check.PSObject.Properties['name'] `
                -or $null -eq $check.PSObject.Properties['passed'] `
                -or $check.name -isnot [string] `
                -or $check.passed -isnot [System.Boolean]) {
            return $false
        }
        $name = $check.name
        if ($check.passed -ne $true `
                -or [string]::IsNullOrWhiteSpace($name) `
                -or -not $expected.Contains($name) `
                -or -not $seen.Add($name)) {
            return $false
        }
    }
    return $seen.Count -eq $expected.Count
}

function Test-ExactPassedCheckSetContracts([string[]] $ExpectedNames) {
    $valid = @(
        $ExpectedNames | ForEach-Object {
            [pscustomobject]@{ name = $_; passed = $true }
        }
    )
    if (-not (Test-ExactPassedCheckSet -Checks $valid -ExpectedNames $ExpectedNames)) {
        throw 'The exact check validator rejected its complete passing fixture.'
    }
    $assertionCount = 1
    for ($removed = 0; $removed -lt $valid.Count; $removed++) {
        $fixture = @(
            for ($index = 0; $index -lt $valid.Count; $index++) {
                if ($index -ne $removed) { $valid[$index] }
            }
        )
        if (Test-ExactPassedCheckSet -Checks $fixture -ExpectedNames $ExpectedNames) {
            throw "The check validator accepted a fixture without '$($valid[$removed].name)'."
        }
        $assertionCount += 1
    }
    $duplicate = @(
        for ($index = 0; $index -lt $valid.Count; $index++) {
            if ($index -eq ($valid.Count - 1)) { $valid[0] } else { $valid[$index] }
        }
    )
    if (Test-ExactPassedCheckSet -Checks $duplicate -ExpectedNames $ExpectedNames) {
        throw 'The check validator accepted a duplicate in place of a required check.'
    }
    $assertionCount += 1
    foreach ($invalidPassed in @($false, 'false', 1, $null)) {
        $invalid = @(
            for ($index = 0; $index -lt $valid.Count; $index++) {
                [pscustomobject]@{
                    name = $valid[$index].name
                    passed = if ($index -eq 0) { $invalidPassed } else { $true }
                }
            }
        )
        if (Test-ExactPassedCheckSet -Checks $invalid -ExpectedNames $ExpectedNames) {
            $type = if ($null -eq $invalidPassed) {
                'null'
            }
            else {
                $invalidPassed.GetType().FullName
            }
            throw "The check validator accepted a non-true Boolean value of type '$type'."
        }
        $assertionCount += 1
    }
    $missingPassed = @(
        for ($index = 0; $index -lt $valid.Count; $index++) {
            if ($index -eq 0) {
                [pscustomobject]@{ name = $valid[$index].name }
            }
            else {
                $valid[$index]
            }
        }
    )
    if (Test-ExactPassedCheckSet -Checks $missingPassed -ExpectedNames $ExpectedNames) {
        throw 'The check validator accepted a required check without a passed property.'
    }
    return $assertionCount + 1
}

function Test-ExactFalseBoolean([object] $Value) {
    return $Value -is [System.Boolean] -and $Value -eq $false
}

function ConvertFrom-DesignMatrix([string] $Markdown) {
    $section = [regex]::Match(
        $Markdown,
        '(?ms)^## Matriz do design 0010\s*\r?\n(?<body>.*?)(?=^##\s|\z)'
    )
    if (-not $section.Success) {
        throw 'The issue 45 research has no design 0010 matrix section.'
    }
    $rows = [System.Collections.Generic.List[object]]::new()
    foreach ($line in @($section.Groups['body'].Value -split "`r?`n")) {
        $match = [regex]::Match(
            $line,
            '^\|(?<scenario>[^|]+)\|(?<producer>[^|]+)\|(?<effect>[^|]+)\|(?<proof>[^|]+)\|\s*$'
        )
        if (-not $match.Success) {
            continue
        }
        $scenario = $match.Groups['scenario'].Value.Trim()
        $rawProof = $match.Groups['proof'].Value.Trim()
        $proofMatch = [regex]::Match(
            $rawProof,
            '^`(?<proof>[A-Za-z0-9_-]+)`$'
        )
        if (-not $proofMatch.Success) {
            continue
        }
        $proof = $proofMatch.Groups['proof'].Value
        $rows.Add([pscustomobject]@{
            scenario = $scenario
            producer = $match.Groups['producer'].Value.Trim()
            consumerEffect = $match.Groups['effect'].Value.Trim()
            proof = $proof
            key = "$scenario => $proof"
        })
    }
    return @($rows.ToArray())
}

function Get-NormativeDesignScenarios([string] $Markdown) {
    $blocks = [System.Collections.Generic.List[object]]::new()
    $collecting = $false
    $current = [System.Collections.Generic.List[string]]::new()
    foreach ($line in @($Markdown -split "`r?`n")) {
        $row = [regex]::Match(
            $line,
            '^\|(?<scenario>[^|]+)\|(?<result>[^|]+)\|\s*$'
        )
        if (-not $row.Success) {
            if ($collecting -and $current.Count -ne 0) {
                $blocks.Add(@($current.ToArray()))
            }
            $collecting = $false
            $current.Clear()
            continue
        }
        $scenario = $row.Groups['scenario'].Value.Trim()
        $result = $row.Groups['result'].Value.Trim()
        if ($scenario -match '^-+$' -and $result -match '^-+$') {
            $collecting = $true
            $current.Clear()
            continue
        }
        if ($collecting) {
            $current.Add($scenario)
        }
    }
    if ($collecting -and $current.Count -ne 0) {
        $blocks.Add(@($current.ToArray()))
    }
    $normative = @($blocks | Where-Object { @($_).Count -eq 14 })
    if ($normative.Count -ne 1) {
        throw 'The normative design 0010 must contain exactly one 14-row two-column scenario matrix.'
    }
    return @($normative[0])
}

function Test-DesignMatrixCoverage(
    [object[]] $Rows,
    [object[]] $Expected
) {
    if (@($Rows).Count -ne @($Expected).Count) {
        return $false
    }
    $expectedByScenario = [System.Collections.Generic.Dictionary[string, string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($entry in @($Expected)) {
        if ($expectedByScenario.ContainsKey([string] $entry.scenario)) {
            throw "The expected design matrix duplicates '$($entry.scenario)'."
        }
        $expectedByScenario.Add(
            [string] $entry.scenario,
            [string] $entry.proof
        )
    }
    $seen = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($row in @($Rows)) {
        $scenario = [string] $row.scenario
        if (-not $seen.Add($scenario) -or
                [string]::IsNullOrWhiteSpace([string] $row.producer) -or
                [string]::IsNullOrWhiteSpace([string] $row.consumerEffect) -or
                [string]::Equals(
                    [string] $row.producer,
                    [string] $row.consumerEffect,
                    [System.StringComparison]::Ordinal
                ) -or
                -not $expectedByScenario.ContainsKey($scenario) -or
                -not [string]::Equals(
                    [string] $row.proof,
                    $expectedByScenario[$scenario],
                    [System.StringComparison]::Ordinal
                )) {
            return $false
        }
    }
    return $seen.Count -eq $expectedByScenario.Count
}

function Test-DesignMatrixContracts(
    [object[]] $Rows,
    [object[]] $Expected
) {
    if (-not (Test-DesignMatrixCoverage -Rows $Rows -Expected $Expected)) {
        throw 'The design 0010 matrix is missing, duplicated, extra, or mapped to the wrong proof.'
    }
    $assertionCount = 1
    for ($removed = 0; $removed -lt @($Rows).Count; $removed++) {
        $fixture = @(
            for ($index = 0; $index -lt @($Rows).Count; $index++) {
                if ($index -ne $removed) { $Rows[$index] }
            }
        )
        if (Test-DesignMatrixCoverage -Rows $fixture -Expected $Expected) {
            throw 'The design matrix validator accepted a fixture with one normative row removed.'
        }
        $assertionCount += 1
    }
    return $assertionCount
}

function Invoke-OwnedCleanupProbe {
    $probeScript = Join-Path $runRoot 'owned-cleanup-probe.ps1'
    $probeReady = Join-Path $runRoot 'owned-cleanup-probe.ready'
    $probeStartSignal = Join-Path $runRoot 'owned-cleanup-probe.assigned'
    $sentinelScript = Join-Path $runRoot 'concurrent-independent-sentinel.ps1'
    $sentinelReady = Join-Path $runRoot 'concurrent-independent-sentinel.ready'
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
    $startInfo.FileName = $windowsPowerShell
    $startInfo.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$probeScript`" `"$probeReady`" `"$probeStartSignal`""
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $owned = Start-OwnedGateProcess `
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
            Register-OwnedGateJobProcesses -Job $job
            if ([DateTime]::UtcNow -ge $deadline) {
                throw 'The owned-process cleanup probe did not become ready.'
            }
            Start-Sleep -Milliseconds 50
        }
        Register-OwnedGateJobProcesses -Job $job

        $sentinelStartInfo = [System.Diagnostics.ProcessStartInfo]::new()
        $sentinelStartInfo.FileName = $windowsPowerShell
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
        if ($sentinel.HasExited) {
            throw 'Owned Job cleanup terminated the concurrent independent sentinel.'
        }
        if ($ownedProcessRecords.ContainsKey($sentinelIdentity)) {
            throw 'The concurrent independent sentinel was misclassified as owned.'
        }
        if ($cleanup.stoppedProcessCount -lt 1 `
                -or $cleanup.listenersBefore -lt 1 `
                -or $cleanup.processesAfter -ne 0 `
                -or $cleanup.listenersAfter -ne 0) {
            throw 'The cleanup probe did not terminate and verify its complete owned state.'
        }
        $cleanup | Add-Member `
            -NotePropertyName independentSentinelSurvived `
            -NotePropertyValue $true
        return $cleanup
    }
    finally {
        if ($null -eq $cleanup -and $ownedJobs.Count -ne 0) {
            try { [void] (Stop-OwnedGateProcesses) } catch {}
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

try {
    $outputPreflightAssertionCount = Test-OwnedOutputPreflightContracts
    $preflightCoversSharedCargoTarget =
        $ownedOutputPreflightPaths -contains $sharedCargoTarget
    $preflightCoversOwnedScratch =
        $ownedOutputPreflightPaths -contains $scratchContainer -and
        $ownedOutputPreflightPaths -contains $windowsPathScratch
    $checks.Add([ordered]@{
        name = 'fail-closed-preexisting-output-preflight'
        passed = ($outputPreflightAssertionCount -eq 4 -and
            $ownedOutputPreflightPaths.Count -eq 6 -and
            $preflightCoversSharedCargoTarget -and
            $preflightCoversOwnedScratch)
        assertionCount = $outputPreflightAssertionCount + 3
        requiredOutputPathCount = $ownedOutputPreflightPaths.Count
        sharedCargoTargetRequiredAbsent = $preflightCoversSharedCargoTarget
        ownedScratchRequiredAbsent = $preflightCoversOwnedScratch
    })

    $proofParserAssertionCount = Test-ProofParserContracts
    $checks.Add([ordered]@{
        name = 'fail-closed-named-proof-parser'
        passed = ($proofParserAssertionCount -eq 8)
        assertionCount = $proofParserAssertionCount
    })

    $imagingCheckSetAssertionCount = Test-ExactPassedCheckSetContracts `
        -ExpectedNames $expectedImagingRecoveryCheckNames
    $checks.Add([ordered]@{
        name = 'fail-closed-imaging-recovery-check-set'
        passed = ($imagingCheckSetAssertionCount -eq 16)
        assertionCount = $imagingCheckSetAssertionCount
    })

    $windowsCheckSetAssertionCount = Test-ExactPassedCheckSetContracts `
        -ExpectedNames $expectedWindowsPathCheckNames
    $checks.Add([ordered]@{
        name = 'fail-closed-windows-path-check-set'
        passed = ($windowsCheckSetAssertionCount -eq 18)
        assertionCount = $windowsCheckSetAssertionCount
    })

    $postProofMutationAssertionCount = Test-PostProofSourceMutationContract `
        -FixtureRoot (Join-Path $runRoot 'post-proof-provenance-fixture')
    $checks.Add([ordered]@{
        name = 'fail-closed-post-proof-source-mutation'
        passed = ($postProofMutationAssertionCount -eq 1)
        assertionCount = $postProofMutationAssertionCount
    })

    $cleanupProbe = Invoke-OwnedCleanupProbe
    $checks.Add([ordered]@{
        name = 'owned-process-listener-cleanup-probe'
        passed = ($cleanupProbe.stoppedProcessCount -ge 1 -and
            $cleanupProbe.listenersBefore -ge 1 -and
            $cleanupProbe.processesAfter -eq 0 -and
            $cleanupProbe.listenersAfter -eq 0 -and
            $cleanupProbe.independentSentinelSurvived)
        assertionCount = 6
        stoppedProcessCount = $cleanupProbe.stoppedProcessCount
        observedListenerCount = $cleanupProbe.listenersBefore
        independentSentinelSurvived = $cleanupProbe.independentSentinelSurvived
    })

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

    $frontendResultsPath = Join-Path $runRoot 'frontend-test-results.json'
    $frontendRun = Invoke-RecordedCommand `
        -Name 'frontend-tests' `
        -FilePath $npm `
        -Arguments @(
            'test',
            '--',
            '--reporter=verbose',
            '--reporter=json',
            "--outputFile.json=$frontendResultsPath"
        )
    if (-not (Test-Path -LiteralPath $frontendResultsPath -PathType Leaf)) {
        throw 'The frontend gate did not produce its machine-readable test report.'
    }
    $frontendResults = Get-Content `
        -LiteralPath $frontendResultsPath `
        -Raw `
        -Encoding UTF8 |
        ConvertFrom-Json
    $frontendFileCount = [int] $frontendResults.numPassedTestSuites
    $frontendTestCount = [int] $frontendResults.numPassedTests
    if (-not $frontendResults.success `
            -or $frontendFileCount -lt 1 `
            -or $frontendTestCount -lt 1 `
            -or [int] $frontendResults.numFailedTestSuites -ne 0 `
            -or [int] $frontendResults.numFailedTests -ne 0 `
            -or [int] $frontendResults.numPendingTests -ne 0 `
            -or [int] $frontendResults.numTodoTests -ne 0) {
        throw 'The frontend machine report is empty, failed, pending, or incomplete.'
    }
    $frontendAssertions = @(
        $frontendResults.testResults |
            ForEach-Object { $_.assertionResults }
    )
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
    $imagingChecks = @($imagingEvidence.checks)
    $imagingCheckCount = $imagingChecks.Count
    $imagingFailedCheckCount = @($imagingChecks | Where-Object {
            $_.passed -isnot [System.Boolean] -or $_.passed -ne $true
        }).Count
    if (-not (Test-ExactFalseBoolean $imagingEvidence.sourceInputsDirty) `
            -or $imagingEvidence.gitCommit -ne $sourceBefore.gitCommit `
            -or $imagingFailedCheckCount -ne 0 `
            -or -not (Test-ExactPassedCheckSet `
                -Checks $imagingChecks `
                -ExpectedNames $expectedImagingRecoveryCheckNames)) {
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
    $windowsChecks = @($windowsEvidence.checks)
    $windowsCheckCount = $windowsChecks.Count
    $windowsFailedCheckCount = @($windowsChecks | Where-Object {
            $_.passed -isnot [System.Boolean] -or $_.passed -ne $true
        }).Count
    if (-not (Test-ExactFalseBoolean $windowsEvidence.sourceInputsDirty) `
            -or $windowsEvidence.gitCommit -ne $sourceBefore.gitCommit `
            -or $windowsFailedCheckCount -ne 0 `
            -or -not (Test-ExactPassedCheckSet `
                -Checks $windowsChecks `
                -ExpectedNames $expectedWindowsPathCheckNames)) {
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
    $env:CARGO_TARGET_DIR = $gateTarget

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

    $finalOwnedCleanup = Stop-OwnedGateProcesses
    $remainingProcesses = @(Get-ActiveOwnedGateProcesses)
    $remainingListeners = @(Get-OwnedGateListeners -Processes $remainingProcesses)
    $untrackedWorkspaceProcesses = @(Get-WorkspaceProcesses)
    $claimedPreexistingIdentities = @(
        $ownedProcessRecords.Keys |
            Where-Object { $preexistingProcessIdentities.Contains($_) }
    )
    if ($remainingProcesses.Count -ne 0 `
            -or $remainingListeners.Count -ne 0 `
            -or $untrackedWorkspaceProcesses.Count -ne 0 `
            -or $claimedPreexistingIdentities.Count -ne 0) {
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
            $ownedListenerCountAfter -eq 0 -and
            $claimedPreexistingIdentities.Count -eq 0)
        assertionCount = 4
        ownedProcessCount = $ownedProcessCountAfter
        ownedListenerCount = $ownedListenerCountAfter
        claimedPreexistingProcessIdentityCount = $claimedPreexistingIdentities.Count
        stoppedOwnedProcessCount = $finalOwnedCleanup.stoppedProcessCount
        observedOwnedListenerCountBeforeCleanup = $finalOwnedCleanup.listenersBefore
        exclusiveArtifactLockFailures = 0
    })

    Clear-Issue45GateOutputs

    $sharedCargoTargetUntouched = -not (Test-Path -LiteralPath $sharedCargoTarget)
    $isolatedCargoTargetRemoved = -not (Test-Path -LiteralPath $gateTarget)
    $runScratchRemoved = -not (Test-Path -LiteralPath $runRoot)
    $ownedScratchContainerRemoved = -not (Test-Path -LiteralPath $scratchContainer)
    $windowsPathScratchRemoved = -not (Test-Path -LiteralPath $windowsPathScratch)
    if (-not $sharedCargoTargetUntouched `
            -or -not $isolatedCargoTargetRemoved `
            -or -not $runScratchRemoved `
            -or -not $ownedScratchContainerRemoved `
            -or -not $windowsPathScratchRemoved) {
        throw 'The issue 45 gate touched the shared Cargo target or retained owned scratch.'
    }
    $checks.Add([ordered]@{
        name = 'isolated-cargo-target-cleanup'
        passed = $true
        assertionCount = 5
        sharedCargoTargetUntouched = $sharedCargoTargetUntouched
        isolatedCargoTargetRemoved = $isolatedCargoTargetRemoved
        runScratchRemoved = $runScratchRemoved
        ownedScratchContainerRemoved = $ownedScratchContainerRemoved
        windowsPathScratchRemoved = $windowsPathScratchRemoved
    })

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
    $cachePathApiSource = Get-Content `
        -LiteralPath (Join-Path $workspaceRoot 'crates\myalbuns-paths\src\app_paths.rs') `
        -Raw `
        -Encoding UTF8
    if ($cachePathApiSource -match '\binspect_cache_namespaces\b') {
        throw 'The Cache path API reintroduced uncoordinated bulk namespace inspection.'
    }
    $narrowApiProofText = @(
        $cacheCommandNames
        'no-bulk-cache-namespace-inspection'
    ) -join "`n"
    $researchProofText = Get-Content `
        -LiteralPath (Join-Path $workspaceRoot 'docs\research\0036-integracao-final-de-midias-e-cache.md') `
        -Raw `
        -Encoding UTF8
    $design0010Text = Get-Content `
        -LiteralPath (Join-Path $workspaceRoot 'docs\design\0010-armazenamento-local-e-cache.md') `
        -Raw `
        -Encoding UTF8
    $normativeScenarios = @(
        Get-NormativeDesignScenarios -Markdown $design0010Text
    )
    $expectedMatrixProofs = @(
        'cache_consumes_authoritative_identity_transitions_without_owning_them'
        'a_new_authorized_identity_reserves_an_independent_empty_namespace'
        'cache_consumes_authoritative_identity_transitions_without_owning_them'
        'cache_consumes_authoritative_identity_transitions_without_owning_them'
        'monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes'
        'absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state'
        'absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state'
        'corrupted_or_incompatible_index_is_discarded_and_rebuilt'
        'obsolete_job_that_finishes_does_not_publish_and_discards_its_candidate_generation'
        'reopening_after_host_death_recovers_the_contained_processors_temporary'
        'project_open_during_free_space_is_serialized_by_namespace_reservation'
        'schedules_total_cleanup_while_a_project_is_active_and_runs_it_at_safe_startup'
        'export_plan_rejects_missing_originals_at_the_typed_plan_stage'
        'real-mapped-unc'
    )
    if ($normativeScenarios.Count -ne $expectedMatrixProofs.Count) {
        throw 'The normative design scenario and behavioral proof counts diverged.'
    }
    $expectedDesignMatrix = @(
        for ($index = 0; $index -lt $normativeScenarios.Count; $index++) {
            [pscustomobject]@{
                scenario = $normativeScenarios[$index]
                proof = $expectedMatrixProofs[$index]
            }
        }
    )
    $designMatrixRows = @(
        ConvertFrom-DesignMatrix -Markdown $researchProofText
    )
    $designMatrixAssertionCount = Test-DesignMatrixContracts `
        -Rows $designMatrixRows `
        -Expected $expectedDesignMatrix
    $checks.Add([ordered]@{
        name = 'complete-fail-closed-design-0010-matrix'
        passed = ($designMatrixAssertionCount -eq 15)
        assertionCount = $designMatrixAssertionCount
        normativeRowCount = $designMatrixRows.Count
    })
    $expectedTopLevelCheckNames = @(
        'fail-closed-preexisting-output-preflight'
        'fail-closed-named-proof-parser'
        'fail-closed-imaging-recovery-check-set'
        'fail-closed-windows-path-check-set'
        'fail-closed-post-proof-source-mutation'
        'owned-process-listener-cleanup-probe'
        'clean-debug-sidecar-preparation'
        'rust-typescript-contracts'
        'frontend-tests'
        'frontend-typecheck'
        'rust-tests'
        'rust-fmt-clippy-deny-warnings'
        'real-processor-cache-canvas-recovery'
        'windows-local-unc-mapped-long-paths'
        'release-build-and-nsis-package'
        'owned-process-lock-listener-cleanup'
        'isolated-cargo-target-cleanup'
        'complete-fail-closed-design-0010-matrix'
    )
    $topLevelChecks = @(
        $checks | ForEach-Object { [pscustomobject] $_ }
    )
    if (-not (Test-ExactPassedCheckSet `
            -Checks $topLevelChecks `
            -ExpectedNames $expectedTopLevelCheckNames)) {
        throw 'The issue 45 top-level gate checks are not the exact passing Boolean set.'
    }
    $designMatrixProofText = @(
        $designMatrixRows | ForEach-Object { $_.key }
    ) -join "`n"

    function New-RustProof([string] $Name) {
        return @{
            source = 'rust-tests'
            proofKind = 'rust-test'
            sourceText = $rustRun.output
            requiredText = $Name
        }
    }

    function New-FrontendProof([string] $Name) {
        return @{
            source = 'frontend-tests'
            proofKind = 'frontend-test'
            sourceData = $frontendAssertions
            requiredText = $Name
        }
    }

    function New-ExactProof(
        [string] $Source,
        [string] $Text,
        [string] $Name
    ) {
        return @{
            source = $Source
            proofKind = 'exact-line'
            sourceText = $Text
            requiredText = $Name
        }
    }
    $completeMatrixRequirements = [System.Collections.Generic.List[object]]::new()
    foreach ($row in $designMatrixRows) {
        $completeMatrixRequirements.Add(
            (New-ExactProof `
                -Source 'research-matrix' `
                -Text $designMatrixProofText `
                -Name $row.key)
        )
    }
    foreach ($proofName in @(
            'cache_consumes_authoritative_identity_transitions_without_owning_them'
            'a_new_authorized_identity_reserves_an_independent_empty_namespace'
            'monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes'
            'absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state'
            'corrupted_or_incompatible_index_is_discarded_and_rebuilt'
            'obsolete_job_that_finishes_does_not_publish_and_discards_its_candidate_generation'
            'reopening_after_host_death_recovers_the_contained_processors_temporary'
            'project_open_during_free_space_is_serialized_by_namespace_reservation'
            'schedules_total_cleanup_while_a_project_is_active_and_runs_it_at_safe_startup'
            'export_plan_rejects_missing_originals_at_the_typed_plan_stage'
        )) {
        $completeMatrixRequirements.Add((New-RustProof -Name $proofName))
    }
    $completeMatrixRequirements.Add(
        (New-ExactProof `
            -Source 'windows-paths' `
            -Text $windowsProofText `
            -Name 'real-mapped-unc')
    )
    $criteria = @(
        New-VerifiedCriterion `
            -Name 'authorized-independent-empty-namespace' `
            -Requirements @(
                (New-RustProof -Name 'a_new_authorized_identity_reserves_an_independent_empty_namespace')
                (New-RustProof -Name 'cache_consumes_authoritative_identity_transitions_without_owning_them')
            )
        New-VerifiedCriterion `
            -Name 'authoritative-absent-unavailable-and-visual-context' `
            -Requirements @(
                (New-RustProof -Name 'resolver_monitor_and_runtime_keep_observed_state_outside_media_refs')
                (New-RustProof -Name 'absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state')
                (New-FrontendProof -Name 'keeps the last known preview when linked media becomes unavailable')
                (New-FrontendProof -Name 'keeps the last representation only as visual context when the Original is absent')
                (New-RustProof -Name 'export_plan_rejects_missing_originals_at_the_typed_plan_stage')
            )
        New-VerifiedCriterion `
            -Name 'relink-occurrence-stable-change-and-reappearance' `
            -Requirements @(
                (New-RustProof -Name 'public_relink_command_updates_only_the_selected_occurrence_and_participates_in_history')
                (New-RustProof -Name 'public_relink_flow_reinspects_and_invalidates_only_the_selected_occurrence')
                (New-RustProof -Name 'monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes')
                (New-FrontendProof -Name 'offers public Relink only for an absent occurrence and applies the returned projection')
            )
        New-VerifiedCriterion `
            -Name 'discardable-complete-index-and-candidate-validation' `
            -Requirements @(
                (New-RustProof -Name 'corrupted_or_incompatible_index_is_discarded_and_rebuilt')
                (New-RustProof -Name 'duplicate_media_entries_make_the_discardable_cache_index_non_current')
                (New-RustProof -Name 'cache_engine_publishes_index_last_reuses_and_invalidates_only_the_requested_media')
                (New-RustProof -Name 'failed_validation_discards_the_candidate_and_preserves_the_last_published_generation')
                (New-RustProof -Name 'a_wrong_response_correlation_discards_the_unpublished_candidate_generation')
                (New-RustProof -Name 'repeated_crashes_after_candidate_publication_leave_no_orphan_generation')
            )
        New-VerifiedCriterion `
            -Name 'request-fingerprint-variant-revalidation-and-obsolete-collection' `
            -Requirements @(
                (New-RustProof -Name 'terminal_fingerprint_reopens_the_frozen_path_after_atomic_replacement')
                (New-RustProof -Name 'authoritative_demand_revision_rejects_queued_and_late_obsolete_work')
                (New-RustProof -Name 'obsolete_job_that_finishes_does_not_publish_and_discards_its_candidate_generation')
                (New-RustProof -Name 'failed_validation_discards_the_candidate_and_preserves_the_last_published_generation')
            )
        New-VerifiedCriterion `
            -Name 'processor-restart-once-then-nonblocking-suspension' `
            -Requirements @(
                (New-RustProof -Name 'repeated_processor_crashes_suspend_new_cache_work_after_one_restart')
                (New-RustProof -Name 'repeated_processor_failure_suspends_before_fallible_recovery_cleanup')
                (New-FrontendProof -Name 'shows a non-blocking warning when repeated processor failures suspend Cache')
                (New-FrontendProof -Name 'registers the Cache warning listener before the first preview demand')
            )
        New-VerifiedCriterion `
            -Name 'tracer-44-host-death-and-real-recovery' `
            -Requirements @(
                (New-RustProof -Name 'terminating_the_host_closes_its_job_and_terminates_the_active_processor')
                (New-RustProof -Name 'attach_rejects_a_recycled_pid_identity_without_containing_or_killing_the_observed_process')
                (New-RustProof -Name 'guarded_writer_claim_storage_publishes_reads_and_conditionally_removes_by_handle')
                (New-RustProof -Name 'writer_wait_rejects_namespace_link_replacement_and_preserves_external_claim_files')
                (New-RustProof -Name 'fragmented_handshake_preserves_the_exact_process_instance')
                (New-RustProof -Name 'processor_handshake_reports_the_exact_instance_seen_through_the_spawned_child_handle')
                (New-RustProof -Name 'reopening_after_host_death_recovers_the_contained_processors_temporary')
                (New-RustProof -Name 'free_closed_projects_after_host_death_waits_before_measuring_and_removing')
                (New-RustProof -Name 'clear_all_after_host_death_waits_before_measuring_and_removing')
                (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'protocol')
                (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'cache-temporary-cleanup')
                (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'imaging-sidecar-build')
                (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'production-recovery-integration')
                (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'cache-webview-canvas-export-journey')
                (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'obsolete-cache-cancellation-integration')
                (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'causal-cache-pause-integration')
                (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'actual-tauri-webview2-build')
                (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'actual-tauri-album-canvas-pixi-webview2')
            )
        New-VerifiedCriterion `
            -Name 'local-unc-mapped-and-long-paths' `
            -Requirements @(
                (New-ExactProof -Source 'windows-paths' -Text $windowsProofText -Name 'path-contract')
                (New-ExactProof -Source 'windows-paths' -Text $windowsProofText -Name 'path-policy')
                (New-ExactProof -Source 'windows-paths' -Text $windowsProofText -Name 'real-mapped-unc')
                (New-RustProof -Name 'prepares_the_cache_only_as_directories_below_the_authorized_root')
            )
        New-VerifiedCriterion `
            -Name 'measure-free-reserve-and-safe-total-cleanup' `
            -Requirements @(
                (New-RustProof -Name 'measures_and_frees_only_namespaces_without_an_active_owner')
                (New-RustProof -Name 'schedules_total_cleanup_while_a_project_is_active_and_runs_it_at_safe_startup')
                (New-RustProof -Name 'project_open_during_free_space_is_serialized_by_namespace_reservation')
                (New-RustProof -Name 'active_namespace_measurement_tolerates_real_writer_promotion_and_exclusive_files')
                (New-RustProof -Name 'scheduled_cleanup_keeps_the_runtime_responsive_until_the_exact_writer_exits')
                (New-RustProof -Name 'reopening_after_host_death_recovers_the_contained_processors_temporary')
                (New-RustProof -Name 'free_closed_projects_quiesces_writers_before_measuring_removed_bytes')
                (New-RustProof -Name 'clear_all_quiesces_writers_before_measuring_removed_bytes')
                (New-RustProof -Name 'free_closed_projects_after_host_death_waits_before_measuring_and_removing')
                (New-RustProof -Name 'clear_all_after_host_death_waits_before_measuring_and_removing')
                (New-RustProof -Name 'reserved_namespace_recovery_discards_abandoned_files_and_preserves_indexed_generation')
                (New-RustProof -Name 'active_namespace_with_different_windows_casing_is_never_releasable')
            )
        New-VerifiedCriterion `
            -Name 'narrow-api-without-universal-coordinator' `
            -Requirements @(
                (New-ExactProof -Source 'cache-service-source' -Text $narrowApiProofText -Name 'cache_service_status')
                (New-ExactProof -Source 'cache-service-source' -Text $narrowApiProofText -Name 'free_closed_project_cache')
                (New-ExactProof -Source 'cache-service-source' -Text $narrowApiProofText -Name 'clear_all_cache')
                (New-ExactProof -Source 'cache-service-source' -Text $narrowApiProofText -Name 'no-bulk-cache-namespace-inspection')
                (New-FrontendProof -Name 'maps the Project and media ports to the desktop commands')
                (New-FrontendProof -Name 'initializes the native dialog used by the productive relink command')
                (New-RustProof -Name 'global_cache_service_commands_are_explicitly_allowed_only_to_global_window')
            )
        New-VerifiedCriterion `
            -Name 'complete-design-0010-producer-consumer-matrix' `
            -Requirements @($completeMatrixRequirements.ToArray())
    )
    if ($criteria.Count -ne 11 -or @($criteria | Where-Object { -not $_.passed }).Count -ne 0) {
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
            ownedProcessesStoppedAtFinalCleanup = $finalOwnedCleanup.stoppedProcessCount
            ownedListenersObservedBeforeFinalCleanup = $finalOwnedCleanup.listenersBefore
        }
        checks = @($checks)
        criteria = $criteria
        nestedEvidence = [ordered]@{
            imaging = [ordered]@{
                schemaVersion = $imagingEvidence.schemaVersion
                checks = $imagingCheckCount
                checkNames = @($imagingChecks | ForEach-Object { $_.name })
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
            runScratchRemoved = $runScratchRemoved
            ownedScratchContainerRemoved = $ownedScratchContainerRemoved
            windowsPathScratchRemoved = $windowsPathScratchRemoved
            isolatedCargoTargetRemoved = $isolatedCargoTargetRemoved
            sharedCargoTargetUntouched = $sharedCargoTargetUntouched
            preparedSidecarRemoved = -not (Test-Path -LiteralPath $preparedSidecarPath)
            windowsPathTargetRemoved = -not (Test-Path -LiteralPath $windowsPathTarget)
            distRemoved = -not (Test-Path -LiteralPath $distPath)
            ownedProcesses = $ownedProcessCountAfter
            ownedListeners = $ownedListenerCountAfter
            claimedPreexistingProcessIdentities = $claimedPreexistingIdentities.Count
            stoppedOwnedProcesses = $finalOwnedCleanup.stoppedProcessCount
            observedOwnedListenersBeforeCleanup = $finalOwnedCleanup.listenersBefore
            artifactLocks = 0
        }
    }
    [void] (Assert-Issue45SourceUnchanged `
        -Before $sourceBefore `
        -WorkspaceRoot $workspaceRoot `
        -EvidencePath $OutputPath `
        -Stage 'while assembling the final report')
    $json = $report | ConvertTo-Json -Depth 12
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

if ([string]::IsNullOrWhiteSpace($json)) {
    throw 'The issue 45 gate produced no verified report for publication.'
}
[void] (Assert-Issue45SourceUnchanged `
    -Before $sourceBefore `
    -WorkspaceRoot $workspaceRoot `
    -EvidencePath $OutputPath `
    -Stage 'before terminal evidence publication')
$evidenceExisted = Test-Path -LiteralPath $OutputPath -PathType Leaf
$previousEvidence = if ($evidenceExisted) {
    [System.IO.File]::ReadAllBytes($OutputPath)
}
else {
    $null
}
try {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputPath) |
        Out-Null
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $json + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    [void] (Assert-Issue45SourceUnchanged `
        -Before $sourceBefore `
        -WorkspaceRoot $workspaceRoot `
        -EvidencePath $OutputPath `
        -Stage 'during terminal evidence publication')
}
catch {
    $publicationFailure = $_.Exception
    if ($evidenceExisted) {
        [System.IO.File]::WriteAllBytes($OutputPath, $previousEvidence)
    }
    else {
        [System.IO.File]::Delete($OutputPath)
    }
    throw $publicationFailure
}
Write-Output "Issue 45 Media and Cache report: $OutputPath"
Write-Output $json
