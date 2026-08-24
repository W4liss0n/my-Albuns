param(
    [string] $OutputPath,
    [string] $UncRoot,
    [string] $DriveLetter
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
. (Join-Path $PSScriptRoot 'Gate-WindowsProcessArgument.ps1')
Initialize-MyAlbunsToolchain

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'The Issue #14 opening-lock gate must run on Windows.'
}

$workspaceRoot = $script:WorkspaceRoot
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $workspaceRoot `
        'docs\research\artifacts\0040-issue-14-opening-lock.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $workspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$sourceBefore = Get-GateSourceSnapshot `
    -WorkspaceRoot $workspaceRoot `
    -EvidencePath $OutputPath

$scratchParent = [System.IO.Path]::GetFullPath(
    (Join-Path $workspaceRoot '.scratch')
)
New-Item -ItemType Directory -Force -Path $scratchParent | Out-Null
$runRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $scratchParent "issue14-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 8))")
)
if (-not [string]::Equals(
        [System.IO.Path]::GetDirectoryName($runRoot),
        $scratchParent,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The Issue #14 gate scratch root escaped the workspace.'
}

if ([string]::IsNullOrWhiteSpace($UncRoot)) {
    $volumeRoot = [System.IO.Path]::GetPathRoot($runRoot)
    if ($volumeRoot -notmatch '^[A-Za-z]:\\$') {
        throw 'The default UNC fixture requires a drive-letter volume.'
    }
    $volumeLetter = $volumeRoot.Substring(0, 1)
    $relativeRunRoot = $runRoot.Substring($volumeRoot.Length)
    $UncRoot = "\\127.0.0.1\$volumeLetter`$\$relativeRunRoot"
}
$UncRoot = $UncRoot.TrimEnd('\')

if ([string]::IsNullOrWhiteSpace($DriveLetter)) {
    $usedLetters = @(
        [System.IO.DriveInfo]::GetDrives() |
            ForEach-Object { $_.Name.Substring(0, 1).ToUpperInvariant() }
    )
    $DriveLetter = @('R', 'Q', 'P', 'O', 'N', 'M') |
        Where-Object { $usedLetters -notcontains $_ } |
        Select-Object -First 1
}
$DriveLetter = $DriveLetter.TrimEnd(':').ToUpperInvariant()
if ($DriveLetter -notmatch '^[A-Z]$') {
    throw 'DriveLetter must be one unused drive letter.'
}
$mappedDrive = "$DriveLetter`:"

$applicationPath = Join-Path $workspaceRoot 'target\debug\myalbuns-desktop.exe'
$coreGatePath = Join-Path `
    $workspaceRoot `
    'target\debug\examples\issue10_identity_gate.exe'
$dataRoot = Join-Path $runRoot 'data'
$appLocalRoot = Join-Path $dataRoot 'Local\MyAlbuns2'
$leaseRoot = Join-Path $appLocalRoot 'State\ProjectIdentityLeases'
$registryRoot = Join-Path $appLocalRoot 'State\ProjectIdentities'
$logRoot = Join-Path $appLocalRoot 'Logs'
$originalPath = Join-Path $runRoot 'Original.myalbuns'
$copyPath = Join-Path $runRoot 'Cópia independente.myalbuns'
$candidatePath = Join-Path $runRoot 'Candidato indeterminado.myalbuns'
$networkOriginName = 'Origem de rede.myalbuns'
$mappedOriginalPath = "$mappedDrive\Original.myalbuns"
$uncOriginalPath = "$UncRoot\Original.myalbuns"
$mappedNetworkOriginPath = "$mappedDrive\$networkOriginName"
$mappingCreated = $false
$evidence = $null
$checks = [System.Collections.Generic.List[object]]::new()

function Add-Check([string] $Name) {
    $checks.Add([ordered]@{ name = $Name; passed = $true })
}

function Get-FileSha256([string] $Path) {
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digest = $algorithm.ComputeHash([System.IO.File]::ReadAllBytes($Path))
        return [System.BitConverter]::ToString($digest).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
    }
}

function Wait-Until {
    param(
        [Parameter(Mandatory = $true)] [scriptblock] $Condition,
        [Parameter(Mandatory = $true)] [string] $Description,
        [int] $TimeoutMilliseconds = 60000
    )
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    do {
        if (& $Condition) {
            return
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $Description."
}

function Get-ApplicationProcesses {
    if (-not (Test-Path -LiteralPath $applicationPath -PathType Leaf)) {
        return @()
    }
    $expected = [System.IO.Path]::GetFullPath($applicationPath)
    return @(
        Get-CimInstance Win32_Process |
            Where-Object {
                -not [string]::IsNullOrWhiteSpace($_.ExecutablePath) -and
                [string]::Equals(
                    [System.IO.Path]::GetFullPath($_.ExecutablePath),
                    $expected,
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            }
    )
}

function Get-HostProcesses {
    return @(
        Get-ApplicationProcesses |
            Where-Object {
                $_.CommandLine -match '(?:^|\s)--myalbuns-project-host(?:\s|$)'
            }
    )
}

function Get-GlobalProcesses {
    return @(
        Get-ApplicationProcesses |
            Where-Object {
                $_.CommandLine -notmatch '(?:^|\s)--myalbuns-project-host(?:\s|$)'
            }
    )
}

function Get-ExactProcessIdentity([int] $ProcessId) {
    $process = [System.Diagnostics.Process]::GetProcessById($ProcessId)
    try {
        return [ordered]@{
            processId = [uint32] $ProcessId
            creationTime = [uint64] $process.StartTime.ToFileTimeUtc()
        }
    }
    finally {
        $process.Dispose()
    }
}

function Start-Desktop {
    param([string[]] $Projects = @())
    $start = [System.Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $applicationPath
    $start.WorkingDirectory = $workspaceRoot
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.Environment['MYALBUNS_PROCESS_GATE_DATA_ROOT'] = $dataRoot
    $start.Arguments = (@($Projects) | ForEach-Object {
            ConvertTo-WindowsProcessArgument $_
        }) -join ' '
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) {
        throw 'A real MyAlbuns desktop entry did not start.'
    }
    return $process
}

function Wait-DesktopExit {
    param(
        [Parameter(Mandatory = $true)] [System.Diagnostics.Process] $Process,
        [Parameter(Mandatory = $true)] [string] $Description
    )
    if (-not $Process.WaitForExit(60000)) {
        throw "The $Description did not exit after its terminal handoff."
    }
    if ($Process.ExitCode -ne 0) {
        throw "The $Description exited with code $($Process.ExitCode)."
    }
}

function Get-LogRecords {
    $records = [System.Collections.Generic.List[object]]::new()
    if (-not (Test-Path -LiteralPath $logRoot -PathType Container)) {
        return @()
    }
    foreach ($file in Get-ChildItem -LiteralPath $logRoot -Filter '*.jsonl' -File) {
        foreach ($line in Get-Content -LiteralPath $file.FullName) {
            if ([string]::IsNullOrWhiteSpace($line)) {
                continue
            }
            try {
                $records.Add(($line | ConvertFrom-Json))
            }
            catch {
                # A non-blocking logger can expose its last line between writes;
                # the next bounded observation will retry that line.
            }
        }
    }
    return @($records)
}

function Get-EventRecords([string] $Event) {
    return @(Get-LogRecords | Where-Object { $_.event -eq $Event })
}

function Wait-EventCount {
    param(
        [Parameter(Mandatory = $true)] [string] $Event,
        [Parameter(Mandatory = $true)] [int] $Count
    )
    Wait-Until `
        -Description "$Count '$Event' log records" `
        -Condition { @(Get-EventRecords $Event).Count -ge $Count }
}

function Invoke-CoreGate {
    param(
        [Parameter(Mandatory = $true)] [string] $Operation,
        [Parameter(Mandatory = $true)] [string] $Project,
        [Parameter(Mandatory = $true)] [string] $Lease,
        [Parameter(Mandatory = $true)] [string] $Registry
    )
    $output = @(
        & $coreGatePath `
            $Operation `
            --project $Project `
            --lease-root $Lease `
            --registry-root $Registry
    )
    if ($LASTEXITCODE -ne 0 -or $output.Count -eq 0) {
        throw "The public ProjectCore '$Operation' probe failed."
    }
    return $output[-1] | ConvertFrom-Json
}

function Close-ApplicationWindow([int] $ProcessId) {
    return [Issue14WindowControl]::CloseVisibleWindows([uint32] $ProcessId) -gt 0
}

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class Issue14WindowControl
{
    private delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern bool PostMessageW(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    public static int CloseVisibleWindows(uint processId)
    {
        int count = 0;
        EnumWindows(delegate(IntPtr window, IntPtr parameter)
        {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner == processId && IsWindowVisible(window))
            {
                if (PostMessageW(window, 0x0010, IntPtr.Zero, IntPtr.Zero))
                {
                    count++;
                }
            }
            return true;
        }, IntPtr.Zero);
        return count;
    }
}
'@

try {
    New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
    if (@(Get-ApplicationProcesses).Count -ne 0) {
        throw 'A MyAlbuns process from this worktree already exists.'
    }

    & (Join-Path $PSScriptRoot 'Prepare-Sidecar.ps1') -Profile debug
    if ($LASTEXITCODE -ne 0) {
        throw 'The debug image Processor could not be prepared.'
    }
    Push-Location $workspaceRoot
    try {
        & $script:CargoExecutable `
            build `
            -p myalbuns-core `
            --example issue10_identity_gate
        if ($LASTEXITCODE -ne 0) {
            throw 'The public ProjectCore probe executable did not build.'
        }
        & (Join-Path $workspaceRoot 'node_modules\.bin\tauri.cmd') `
            build `
            --debug `
            --no-bundle
        if ($LASTEXITCODE -ne 0) {
            throw 'The debug desktop application did not build.'
        }
    }
    finally {
        Pop-Location
    }
    if (-not (Test-Path -LiteralPath $applicationPath -PathType Leaf) -or
        -not (Test-Path -LiteralPath $coreGatePath -PathType Leaf)) {
        throw 'A required Issue #14 gate executable is missing.'
    }
    Add-Check 'debug-product-and-public-core-build'

    $created = Invoke-CoreGate `
        -Operation 'create' `
        -Project $originalPath `
        -Lease $leaseRoot `
        -Registry $registryRoot
    if ($created.status -ne 'opened') {
        throw 'The original Project fixture was not created.'
    }
    Copy-Item -LiteralPath $originalPath -Destination $copyPath
    $originalBefore = Get-FileSha256 $originalPath

    $preflightName = ".issue14-unc-$([guid]::NewGuid().ToString('N')).tmp"
    $preflightLocal = Join-Path $runRoot $preflightName
    $preflightUnc = Join-Path $UncRoot $preflightName
    $preflightBytes = [byte[]](14, 20, 26)
    [System.IO.File]::WriteAllBytes($preflightLocal, $preflightBytes)
    $observedPreflight = [System.IO.File]::ReadAllBytes($preflightUnc)
    Remove-Item -LiteralPath $preflightLocal -Force
    if ([Convert]::ToBase64String($observedPreflight) -ne
        [Convert]::ToBase64String($preflightBytes)) {
        throw 'The UNC fixture does not resolve to the isolated gate root.'
    }
    $mappingOutput = @(& net.exe use $mappedDrive $UncRoot /persistent:no 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "The real mapped drive could not be created: $($mappingOutput -join ' ')"
    }
    $mappingCreated = $true
    Add-Check 'real-loopback-unc-and-mapped-drive'

    $entryOne = Start-Desktop -Projects @($originalPath, $mappedOriginalPath)
    $entryOneIdentity = Get-ExactProcessIdentity $entryOne.Id
    Start-Sleep -Milliseconds 75
    $entryTwo = Start-Desktop -Projects @($copyPath, $uncOriginalPath)
    $entryTwoIdentity = Get-ExactProcessIdentity $entryTwo.Id
    Wait-DesktopExit -Process $entryOne -Description 'first native entry'
    Wait-DesktopExit -Process $entryTwo -Description 'second native entry'
    Wait-EventCount -Event 'global_activation_forwarded' -Count 1
    Wait-EventCount -Event 'global_activation_batch_completed' -Count 2
    Wait-EventCount -Event 'host_ready' -Count 2
    Wait-EventCount -Event 'existing_project_window_focused' -Count 2
    Wait-Until -Description 'two independent live Project Hosts' -Condition {
        @(Get-HostProcesses).Count -eq 2 -and @(Get-GlobalProcesses).Count -eq 0
    }

    $forwarded = Get-EventRecords 'global_activation_forwarded' | Select-Object -First 1
    if ($forwarded.project_count -ne 2 -or
        @($entryOne.Id, $entryTwo.Id) -notcontains [int] $forwarded.client_process_id) {
        throw 'The later native batch was not correlated to one of the exact desktop entries.'
    }
    $initialBatches = @(Get-EventRecords 'global_activation_batch_completed' | Select-Object -First 2)
    if (($initialBatches | Measure-Object -Property project_count -Sum).Sum -ne 4 -or
        ($initialBatches | Measure-Object -Property opened_count -Sum).Sum -ne 2 -or
        ($initialBatches | Measure-Object -Property focused_count -Sum).Sum -ne 2 -or
        ($initialBatches | Measure-Object -Property failed_count -Sum).Sum -ne 0) {
        throw 'The two native batches did not open two identities and focus two aliases.'
    }
    Add-Check 'single-primary-forwards-second-native-entry'
    Add-Check 'multi-file-batches-open-distinct-and-focus-duplicates'

    $copyDocument = Get-Content -LiteralPath $copyPath -Raw | ConvertFrom-Json
    if ($copyDocument.projectId -eq $created.projectId -or
        (Get-FileSha256 $originalPath) -ne $originalBefore) {
        throw 'The independent physical copy did not receive isolated Identidade authority.'
    }
    $readyRecords = @(Get-EventRecords 'host_ready')
    $originalHostRecord = $readyRecords |
        Where-Object { $_.project_id -eq $created.projectId } |
        Select-Object -Last 1
    $copyHostRecord = $readyRecords |
        Where-Object { $_.project_id -eq $copyDocument.projectId } |
        Select-Object -Last 1
    if ($null -eq $originalHostRecord -or $null -eq $copyHostRecord -or
        $originalHostRecord.process_id -eq $copyHostRecord.process_id) {
        throw 'The two independent Identidades do not own distinct live Hosts.'
    }
    Add-Check 'external-copy-has-independent-identity-lock-and-host'

    $hostIdsBeforeDuplicate = @(
        Get-HostProcesses | ForEach-Object { [int] $_.ProcessId } | Sort-Object
    )
    $focusBeforeDuplicate = @(Get-EventRecords 'existing_project_window_focused').Count
    $duplicateEntry = Start-Desktop -Projects @($uncOriginalPath, $mappedOriginalPath)
    Wait-DesktopExit -Process $duplicateEntry -Description 'duplicate alias entry'
    Wait-EventCount `
        -Event 'existing_project_window_focused' `
        -Count ($focusBeforeDuplicate + 2)
    $hostIdsAfterDuplicate = @(
        Get-HostProcesses | ForEach-Object { [int] $_.ProcessId } | Sort-Object
    )
    if (Compare-Object $hostIdsBeforeDuplicate $hostIdsAfterDuplicate) {
        throw 'UNC or mapped aliases started a competing Project Host.'
    }
    Add-Check 'unc-and-mapped-aliases-reuse-exact-live-session'

    $liveProbe = Invoke-CoreGate `
        -Operation 'open' `
        -Project $mappedOriginalPath `
        -Lease $leaseRoot `
        -Registry $registryRoot
    $originalHostIdentity = Get-ExactProcessIdentity ([int] $originalHostRecord.process_id)
    if ($liveProbe.status -ne 'focusExisting' -or
        [uint32] $liveProbe.ownerProcess.processId -ne $originalHostIdentity.processId -or
        [uint64] $liveProbe.ownerProcess.creationTime -ne $originalHostIdentity.creationTime) {
        throw 'The live owner was not returned as one exact process instance.'
    }
    Add-Check 'live-owner-uses-pid-and-creation-time'

    $bypassLease = Join-Path $runRoot 'bypass\leases'
    $bypassRegistry = Join-Path $runRoot 'bypass\identities'
    $fileLockProbe = Invoke-CoreGate `
        -Operation 'open' `
        -Project $originalPath `
        -Lease $bypassLease `
        -Registry $bypassRegistry
    if ($fileLockProbe.status -ne 'projectInUse') {
        throw 'Bypassing the identity namespace was not stopped by the real Project file lock.'
    }
    Add-Check 'real-project-file-lock-is-final-protection'

    $indeterminateLease = Join-Path $runRoot 'indeterminate\leases'
    $indeterminateRegistry = Join-Path $runRoot 'indeterminate\identities'
    $networkOrigin = Invoke-CoreGate `
        -Operation 'create' `
        -Project $mappedNetworkOriginPath `
        -Lease $indeterminateLease `
        -Registry $indeterminateRegistry
    if ($networkOrigin.status -ne 'opened') {
        throw 'The network-origin fixture was not created.'
    }
    Copy-Item -LiteralPath $mappedNetworkOriginPath -Destination $candidatePath
    $candidateBefore = Get-FileSha256 $candidatePath

    $leasePath = Join-Path $leaseRoot "$($created.projectId).lease"
    $targetPath = Join-Path $leaseRoot "$($created.projectId).target"
    $originalHostPid = [int] $originalHostRecord.process_id
    Stop-Process -Id $originalHostPid -Force -ErrorAction Stop
    Wait-Until -Description 'forced Project Host death' -Condition {
        -not (Get-Process -Id $originalHostPid -ErrorAction SilentlyContinue)
    }
    if (-not (Test-Path -LiteralPath $leasePath -PathType Leaf) -or
        -not (Test-Path -LiteralPath $targetPath -PathType Leaf)) {
        throw 'The forced death did not leave the expected orphan ownership evidence.'
    }
    $orphanTarget = Get-Content -LiteralPath $targetPath -Raw | ConvertFrom-Json
    if ([uint32] $orphanTarget.ownerProcess.processId -ne $originalHostIdentity.processId -or
        [uint64] $orphanTarget.ownerProcess.creationTime -ne $originalHostIdentity.creationTime -or
        (Get-FileSha256 $originalPath) -ne $originalBefore) {
        throw 'Forced release changed the document or lost exact orphan provenance.'
    }
    Add-Check 'forced-death-preserves-document-and-orphan-provenance'

    $recoveryEntry = Start-Desktop -Projects @($originalPath)
    Wait-DesktopExit -Process $recoveryEntry -Description 'orphan recovery entry'
    Wait-Until -Description 'replacement Host after orphan recovery' -Condition {
        @(Get-HostProcesses | Where-Object { [int] $_.ProcessId -ne $originalHostPid }).Count -eq 2
    }
    Wait-EventCount -Event 'host_ready' -Count 3
    $recoveredHostRecord = Get-EventRecords 'host_ready' |
        Where-Object {
            $_.project_id -eq $created.projectId -and
            [int] $_.process_id -ne $originalHostPid
        } |
        Select-Object -Last 1
    if ($null -eq $recoveredHostRecord -or
        (Get-FileSha256 $originalPath) -ne $originalBefore) {
        throw 'The orphan was not recovered into a distinct Host without document mutation.'
    }
    $recoveredIdentity = Get-ExactProcessIdentity ([int] $recoveredHostRecord.process_id)
    $recoveredTarget = Get-Content -LiteralPath $targetPath -Raw | ConvertFrom-Json
    if ([uint32] $recoveredTarget.ownerProcess.processId -ne $recoveredIdentity.processId -or
        [uint64] $recoveredTarget.ownerProcess.creationTime -ne $recoveredIdentity.creationTime) {
        throw 'The recovered lease does not identify its exact live owner.'
    }
    Add-Check 'orphan-recovers-only-through-normal-open-flow'

    if (-not (Close-ApplicationWindow ([int] $recoveredHostRecord.process_id))) {
        throw 'The recovered Host exposed no closeable native window.'
    }
    Wait-Until -Description 'normal recovered Host closure' -Condition {
        -not (Get-Process -Id ([int] $recoveredHostRecord.process_id) -ErrorAction SilentlyContinue)
    }
    Wait-Until -Description 'clean replacement Global entry' -Condition {
        @(Get-GlobalProcesses).Count -eq 1
    }
    $normalReleaseProbe = Invoke-CoreGate `
        -Operation 'open' `
        -Project $originalPath `
        -Lease $leaseRoot `
        -Registry $registryRoot
    if ($normalReleaseProbe.status -ne 'opened') {
        throw 'The normal Host close did not release the real identity and Project file locks.'
    }
    Add-Check 'normal-close-releases-identity-and-file-locks'

    $focusBeforeReopen = @(Get-EventRecords 'global_activation_forwarded').Count
    $reopenEntry = Start-Desktop -Projects @($originalPath)
    Wait-DesktopExit -Process $reopenEntry -Description 'Explorer-style reopen entry'
    Wait-EventCount `
        -Event 'global_activation_forwarded' `
        -Count ($focusBeforeReopen + 1)
    Wait-EventCount -Event 'host_ready' -Count 4
    Wait-Until -Description 'Host reopened through the existing Global' -Condition {
        @(Get-HostProcesses).Count -eq 2 -and @(Get-GlobalProcesses).Count -eq 0
    }
    $normalReopenRecord = Get-EventRecords 'host_ready' |
        Where-Object {
            $_.project_id -eq $created.projectId -and
            [int] $_.process_id -ne [int] $recoveredHostRecord.process_id
        } |
        Select-Object -Last 1
    if ($null -eq $normalReopenRecord -or
        (Get-FileSha256 $originalPath) -ne $originalBefore) {
        throw 'The normally released Project did not reopen in a fresh Host.'
    }
    Add-Check 'later-explorer-entry-is-forwarded-to-existing-global'

    $unmapOutput = @(& net.exe use $mappedDrive /delete /y 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "The mapped drive could not be removed: $($unmapOutput -join ' ')"
    }
    $mappingCreated = $false
    $indeterminate = Invoke-CoreGate `
        -Operation 'open' `
        -Project $candidatePath `
        -Lease $indeterminateLease `
        -Registry $indeterminateRegistry
    if ($indeterminate.status -ne 'identityIndeterminate' -or
        (Get-FileSha256 $candidatePath) -ne $candidateBefore) {
        throw 'Unavailable origin evidence did not fail closed as Indeterminate.'
    }
    Add-Check 'same-different-indeterminate-fails-closed'

    $batchRecords = @(Get-EventRecords 'global_activation_batch_completed')
    $focusRecords = @(Get-EventRecords 'existing_project_window_focused')
    $foregroundDenied = @(Get-EventRecords 'existing_project_window_foreground_denied')
    $evidence = [ordered]@{
        schemaVersion = 1
        gate = 'issue-14-opening-lock'
        collectedAtUtc = [DateTime]::UtcNow.ToString('O')
        gitCommit = $sourceBefore.gitCommit
        sourceInputsDirty = $null
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        }
        checks = @($checks)
        evidence = [ordered]@{
            activation = [ordered]@{
                firstEntry = $entryOneIdentity
                secondEntry = $entryTwoIdentity
                forwardedClientProcessId = [uint32] $forwarded.client_process_id
                initialBatchCount = 2
                totalBatchCount = $batchRecords.Count
                totalFocusedAliases = $focusRecords.Count
                foregroundPolicyFallbacks = $foregroundDenied.Count
            }
            identity = [ordered]@{
                sameAliasOutcome = 'focusExisting'
                differentCopyOutcome = 'newIdentity'
                indeterminateOutcome = $indeterminate.status
                originalProjectId = $created.projectId
                copiedProjectId = $copyDocument.projectId
                originalHostProcessId = [uint32] $originalHostIdentity.processId
                copiedHostProcessId = [uint32] $copyHostRecord.process_id
            }
            lock = [ordered]@{
                liveOwner = $originalHostIdentity
                bypassedIdentityNamespaceOutcome = $fileLockProbe.status
                orphanOwner = $orphanTarget.ownerProcess
                recoveredOwner = $recoveredIdentity
                normalReopenProcessId = [uint32] $normalReopenRecord.process_id
                normalReleaseProbeOutcome = $normalReleaseProbe.status
                forcedDeathPreservedDocument = $true
                normalCloseReleasedLocks = $true
                originalSha256 = $originalBefore
            }
            cleanupCompleted = $false
        }
    }
}
finally {
    if ($mappingCreated) {
        & net.exe use $mappedDrive /delete /y | Out-Null
        $mappingCreated = $false
    }

    for ($pass = 0; $pass -lt 4; $pass++) {
        $running = @(Get-ApplicationProcesses)
        if ($running.Count -eq 0) {
            break
        }
        foreach ($process in $running) {
            [void] (Close-ApplicationWindow ([int] $process.ProcessId))
        }
        Start-Sleep -Seconds 2
    }
    $remaining = @(Get-ApplicationProcesses)
    foreach ($process in $remaining) {
        Stop-Process -Id ([int] $process.ProcessId) -Force -ErrorAction SilentlyContinue
    }
    Wait-GatePathProcessesExit -Path $runRoot
    Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchParent
}

if ($null -eq $evidence) {
    throw 'The Issue #14 gate produced no evidence.'
}
if (@(Get-ApplicationProcesses).Count -ne 0 -or (Test-Path -LiteralPath $runRoot)) {
    throw 'The Issue #14 gate left a product process or scratch directory behind.'
}
$sourceAfter = Get-GateSourceSnapshot `
    -WorkspaceRoot $workspaceRoot `
    -EvidencePath $OutputPath
$evidence.sourceInputsDirty = [bool] (Test-GateSourceSnapshotsDirty `
        -Before $sourceBefore `
        -After $sourceAfter)
$evidence.evidence.cleanupCompleted = $true
$outputParent = [System.IO.Path]::GetDirectoryName($OutputPath)
New-Item -ItemType Directory -Force -Path $outputParent | Out-Null
$json = $evidence | ConvertTo-Json -Depth 10
[System.IO.File]::WriteAllText(
    $OutputPath,
    $json + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)
Write-Output $json
