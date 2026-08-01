param(
    [string] $OutputPath,
    [ValidateRange(1, 8)]
    [int] $CargoBuildJobs = 2,
    [ValidateRange(30, 600)]
    [int] $InteractionTimeoutSeconds = 300
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
. (Join-Path $PSScriptRoot 'Evidence-BuildInputs.ps1')
. (Join-Path $PSScriptRoot 'WindowsProcessTree.ps1')

if ($env:OS -ne 'Windows_NT') {
    throw 'The Windows distribution gate must run on Windows.'
}
if (
    [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne
        [System.Runtime.InteropServices.Architecture]::X64
) {
    throw 'The Windows distribution gate requires an x64 operating system.'
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0018-windows-distribution-gate.json'
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
    throw 'Windows distribution evidence must be a JSON file in docs\research\artifacts.'
}

$targetDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'target\windows-distribution-gate')
)
$applicationPath = Join-Path $targetDirectory 'release\myalbuns-desktop.exe'
$builtSidecarPath = Join-Path $targetDirectory 'release\myalbuns-imaging.exe'
$packagedSidecarPath = Join-Path `
    $script:WorkspaceRoot `
    'src-tauri\binaries\myalbuns-imaging-x86_64-pc-windows-msvc.exe'
$bundleDirectory = Join-Path $targetDirectory 'release\bundle\nsis'
$runRoot = Join-Path $targetDirectory 'gate-runs'
$checks = [System.Collections.Generic.List[object]]::new()
$startedProcess = $null

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
        throw "Windows distribution check '$Name' failed with exit code $exitCode."
    }
    Add-Check -Name $Name -ElapsedMs $stopwatch.ElapsedMilliseconds
}

function Get-WorkspaceRelativePath {
    param([Parameter(Mandatory = $true)][string] $Path)

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $prefix = $script:WorkspaceRoot.TrimEnd('\') + '\'
    if (-not $fullPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Evidence path is outside the workspace: $fullPath"
    }
    return $fullPath.Substring($prefix.Length).Replace('\', '/')
}

function Get-FileEvidence {
    param([Parameter(Mandatory = $true)][string] $Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Expected distribution artifact is missing: $Path"
    }
    return [ordered]@{
        path = Get-WorkspaceRelativePath -Path $Path
        bytes = [long] (Get-Item -LiteralPath $Path).Length
        sha256 = (
            Get-FileHash -LiteralPath $Path -Algorithm SHA256
        ).Hash.ToLowerInvariant()
    }
}

function Get-PeEvidence {
    param([Parameter(Mandatory = $true)][string] $Path)

    $stream = [System.IO.File]::Open(
        $Path,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::Read
    )
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        if ($reader.ReadUInt16() -ne 0x5A4D) {
            throw "Artifact is not an MZ executable: $Path"
        }
        $stream.Position = 0x3C
        $peOffset = $reader.ReadInt32()
        if ($peOffset -lt 0x40 -or $peOffset -gt ($stream.Length - 26)) {
            throw "Artifact has an invalid PE offset: $Path"
        }
        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne 0x00004550) {
            throw "Artifact is missing the PE signature: $Path"
        }
        $machine = $reader.ReadUInt16()
        $stream.Position = $peOffset + 24
        $optionalHeaderMagic = $reader.ReadUInt16()
    }
    finally {
        $reader.Dispose()
        $stream.Dispose()
    }

    if ($machine -ne 0x8664 -or $optionalHeaderMagic -ne 0x020B) {
        throw (
            "Artifact is not an AMD64 PE32+ executable: $Path " +
            "(machine=0x{0:x4}, optional=0x{1:x4})." -f
                $machine, $optionalHeaderMagic
        )
    }
    return [ordered]@{
        machine = ('0x{0:x4}' -f $machine)
        architecture = 'x64'
        optionalHeaderMagic = ('0x{0:x4}' -f $optionalHeaderMagic)
        format = 'PE32+'
    }
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

function Get-GlobalLogEntries {
    param(
        [Parameter(Mandatory = $true)][string] $LogDirectory,
        [switch] $AllowTrailingPartial
    )

    $entries = [System.Collections.Generic.List[object]]::new()
    foreach (
        $logFile in @(
            Get-ChildItem `
                -LiteralPath $LogDirectory `
                -Filter 'myalbuns-global*.jsonl' `
                -File `
                -ErrorAction SilentlyContinue
        )
    ) {
        $lines = @(Get-Content -LiteralPath $logFile.FullName -Encoding UTF8)
        for ($index = 0; $index -lt $lines.Count; $index += 1) {
            $line = $lines[$index]
            if ([string]::IsNullOrWhiteSpace($line)) {
                continue
            }
            try {
                $entries.Add(($line | ConvertFrom-Json -ErrorAction Stop))
            }
            catch {
                if ($AllowTrailingPartial -and $index -eq ($lines.Count - 1)) {
                    continue
                }
                throw "Global runtime log contains invalid JSON: $($logFile.FullName)."
            }
        }
    }
    return @($entries)
}

function Get-WebView2Evidence {
    param([Parameter(Mandatory = $true)][uint32] $RootProcessId)

    $processes = [System.Collections.Generic.List[object]]::new()
    foreach ($processId in @(Get-ProcessTreeIds -RootProcessId $RootProcessId)) {
        $process = Get-CimInstance `
            Win32_Process `
            -Filter "ProcessId = $processId" `
            -ErrorAction SilentlyContinue
        if ($null -eq $process -or $process.Name -cne 'msedgewebview2.exe') {
            continue
        }
        if (
            [string]::IsNullOrWhiteSpace($process.ExecutablePath) -or
            -not (Test-Path -LiteralPath $process.ExecutablePath -PathType Leaf)
        ) {
            throw 'A WebView2 descendant did not expose an existing executable path.'
        }
        $resolvedPath = [System.IO.Path]::GetFullPath($process.ExecutablePath)
        if (
            $resolvedPath.StartsWith(
                $script:WorkspaceRoot.TrimEnd('\') + '\',
                [System.StringComparison]::OrdinalIgnoreCase
            )
        ) {
            throw 'The observed WebView2 executable came from the application workspace.'
        }
        $version = (Get-Item -LiteralPath $resolvedPath).VersionInfo.ProductVersion
        $parsedVersion = $null
        if (
            [string]::IsNullOrWhiteSpace($version) -or
            -not [System.Version]::TryParse($version, [ref] $parsedVersion)
        ) {
            throw "The observed WebView2 executable has no valid version: $resolvedPath"
        }
        $processes.Add([ordered]@{
            processId = [uint32] $process.ProcessId
            executablePath = $resolvedPath
            productVersion = [string] $version
        })
    }
    if (@($processes).Count -eq 0) {
        throw 'The global runtime did not expose a WebView2 process.'
    }
    return [ordered]@{
        observedProcessCount = @($processes).Count
        processes = @($processes)
        evergreenEvidence = @(
            'downloadBootstrapper_config',
            'system_runtime_process'
        )
    }
}

function Wait-ForGlobalRuntime {
    param(
        [Parameter(Mandatory = $true)][System.Diagnostics.Process] $Process,
        [Parameter(Mandatory = $true)][string] $LogDirectory,
        [int] $TimeoutSeconds = 45
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        Start-Sleep -Milliseconds 200
        $Process.Refresh()
        if ($Process.HasExited) {
            throw "Global runtime exited before readiness with code $($Process.ExitCode)."
        }
        $events = @(
            Get-GlobalLogEntries `
                -LogDirectory $LogDirectory `
                -AllowTrailingPartial |
                ForEach-Object { $_.event }
        )
        $webView2Processes = @(
            Get-ProcessTreeIds -RootProcessId ([uint32] $Process.Id) |
                ForEach-Object {
                    Get-Process -Id $_ -ErrorAction SilentlyContinue
                } |
                Where-Object { $_.ProcessName -ceq 'msedgewebview2' }
        )
        if (
            $events -ccontains 'welcome_screen_ready' -and
            $Process.MainWindowHandle -ne 0 -and
            $Process.MainWindowTitle -ceq 'MyAlbuns' -and
            $webView2Processes.Count -gt 0
        ) {
            $stopwatch.Stop()
            return [long] $stopwatch.ElapsedMilliseconds
        }
    } while ([DateTime]::UtcNow -lt $deadline)

    throw 'The global runtime did not expose Welcome, its window and WebView2 before timeout.'
}

function Get-ComputerUseReceipt {
    param(
        [Parameter(Mandatory = $true)][string] $ReceiptPath,
        [Parameter(Mandatory = $true)][uint32] $ExpectedProcessId
    )

    if (-not (Test-Path -LiteralPath $ReceiptPath -PathType Leaf)) {
        return $null
    }
    $receipt = Get-Content -LiteralPath $ReceiptPath -Encoding UTF8 -Raw |
        ConvertFrom-Json
    if (
        $receipt.schemaVersion -ne 1 -or
        $receipt.driver -cne 'computer_use' -or
        $receipt.action -cne 'open_project_then_cancel' -or
        [uint32] $receipt.owner.processId -ne $ExpectedProcessId -or
        $receipt.owner.title -cne 'MyAlbuns' -or
        [string]::IsNullOrWhiteSpace($receipt.dialog.app) -or
        [int64] $receipt.dialog.id -le 0 -or
        $receipt.dialog.title -cne 'Abrir Projeto' -or
        [string]::IsNullOrWhiteSpace($receipt.observedAtUtc)
    ) {
        throw 'The Computer Use receipt does not identify the expected native dialog.'
    }
    $observedAt = [DateTimeOffset]::MinValue
    if (-not [DateTimeOffset]::TryParse($receipt.observedAtUtc, [ref] $observedAt)) {
        throw 'The Computer Use receipt has an invalid observation timestamp.'
    }
    return $receipt
}

function Wait-ForNativeDialogCancellation {
    param(
        [Parameter(Mandatory = $true)][System.Diagnostics.Process] $Process,
        [Parameter(Mandatory = $true)][string] $LogDirectory,
        [Parameter(Mandatory = $true)][string] $ReceiptPath,
        [Parameter(Mandatory = $true)][int] $TimeoutSeconds
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $receipt = $null
    do {
        Start-Sleep -Milliseconds 200
        $Process.Refresh()
        if ($Process.HasExited) {
            throw "Global runtime exited during native dialog validation with code $($Process.ExitCode)."
        }
        if ($null -eq $receipt) {
            $receipt = Get-ComputerUseReceipt `
                -ReceiptPath $ReceiptPath `
                -ExpectedProcessId ([uint32] $Process.Id)
        }
        $matchingEntry = @(
            Get-GlobalLogEntries `
                -LogDirectory $LogDirectory `
                -AllowTrailingPartial |
                Where-Object {
                    $_.event -in @(
                        'project_file_selection_cancelled',
                        'project_file_selected',
                        'project_file_selection_failed'
                    )
                } |
                Select-Object -Last 1
        )
        if ($matchingEntry.Count -gt 0) {
            $stopwatch.Stop()
            if ($matchingEntry[0].event -cne 'project_file_selection_cancelled') {
                throw "Unexpected native dialog outcome: $($matchingEntry[0].event)."
            }
            if ($null -eq $receipt) {
                throw 'The dialog was cancelled without an external Computer Use receipt.'
            }
            return [ordered]@{
                elapsedMs = [long] $stopwatch.ElapsedMilliseconds
                entry = $matchingEntry[0]
                receipt = $receipt
            }
        }
    } while ([DateTime]::UtcNow -lt $deadline)

    throw 'Computer Use did not open and cancel the native dialog before timeout.'
}

function Get-CleanEnvironmentEvidence {
    $featureNames = @(
        'Containers-DisposableClientVM',
        'Microsoft-Hyper-V-All',
        'VirtualMachinePlatform',
        'HypervisorPlatform'
    )
    $features = [System.Collections.Generic.List[object]]::new()
    $querySucceeded = $true
    try {
        $installedFeatures = @(Get-CimInstance Win32_OptionalFeature)
        foreach ($name in $featureNames) {
            $feature = @(
                $installedFeatures |
                    Where-Object { $_.Name -ceq $name } |
                    Select-Object -First 1
            )
            $features.Add([ordered]@{
                name = $name
                installState = if ($feature.Count -eq 1) {
                    [int] $feature[0].InstallState
                }
                else {
                    $null
                }
            })
        }
    }
    catch {
        $querySucceeded = $false
    }
    $sandboxExecutable = Join-Path $env:SystemRoot 'System32\WindowsSandbox.exe'
    $sandboxFeature = @(
        $features |
            Where-Object { $_.name -ceq 'Containers-DisposableClientVM' }
    )
    $sandboxAvailable = (
        (Test-Path -LiteralPath $sandboxExecutable -PathType Leaf) -and
        $sandboxFeature.Count -eq 1 -and
        $sandboxFeature[0].installState -eq 1
    )
    $computerSystem = Get-CimInstance Win32_ComputerSystem

    return [ordered]@{
        required = $true
        scope = 'current_automation_context'
        available = [bool] $sandboxAvailable
        provider = if ($sandboxAvailable) { 'windows_sandbox' } else { $null }
        optionalFeatureQuerySucceeded = $querySucceeded
        optionalFeatures = @($features)
        windowsSandbox = [ordered]@{
            executablePresent = [bool] (
                Test-Path -LiteralPath $sandboxExecutable -PathType Leaf
            )
            hypervisorPresent = [bool] $computerSystem.HypervisorPresent
        }
        installerExecuted = $false
        e2ePassed = $false
        reasonCode = if ($sandboxAvailable) {
            'disposable_environment_not_exercised'
        }
        else {
            'no_disposable_windows_environment'
        }
    }
}

$tokens = $null
$parserErrors = $null
foreach ($scriptPath in @($PSCommandPath, (Join-Path $PSScriptRoot 'WindowsProcessTree.ps1'))) {
    [void] [System.Management.Automation.Language.Parser]::ParseFile(
        $scriptPath,
        [ref] $tokens,
        [ref] $parserErrors
    )
    if (@($parserErrors).Count -gt 0) {
        $messages = @($parserErrors | ForEach-Object { $_.Message }) -join ' | '
        throw "Windows distribution runner has parser errors: $messages"
    }
}
Add-Check -Name 'powershell-runner-ast' -ElapsedMs 0

$initialBuildInputs = Get-BuildInputState
if ($initialBuildInputs.dirty) {
    throw 'Windows distribution evidence requires clean source and build inputs.'
}
Add-Check -Name 'clean-build-inputs' -ElapsedMs 0

$tauriConfig = Get-Content `
    -LiteralPath (Join-Path $script:WorkspaceRoot 'src-tauri\tauri.conf.json') `
    -Raw |
    ConvertFrom-Json
$windowsBundle = $tauriConfig.bundle.windows
if (
    @($tauriConfig.bundle.targets).Count -ne 1 -or
    $tauriConfig.bundle.targets[0] -cne 'nsis' -or
    $windowsBundle.webviewInstallMode.type -cne 'downloadBootstrapper' -or
    $windowsBundle.webviewInstallMode.silent -ne $true -or
    $windowsBundle.nsis.installMode -cne 'currentUser'
) {
    throw 'The Windows bundle is not the expected current-user NSIS with Evergreen bootstrapper.'
}
Add-Check -Name 'evergreen-current-user-config' -ElapsedMs 0

$previousCargoTarget = [System.Environment]::GetEnvironmentVariable(
    'CARGO_TARGET_DIR',
    [System.EnvironmentVariableTarget]::Process
)
$previousCargoBuildJobs = [System.Environment]::GetEnvironmentVariable(
    'CARGO_BUILD_JOBS',
    [System.EnvironmentVariableTarget]::Process
)
$locationWasPushed = $false
$runtime = $null
$nativeDialog = $null

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
        -Name 'frontend-native-dialog-contract' `
        -Executable 'npx.cmd' `
        -Arguments @(
            'vitest', 'run',
            'src/global/GlobalShell.test.tsx',
            'src/platform/tauriProjectFileDialog.test.ts',
            'src/platform/tauriBoundary.test.ts'
        )
    Invoke-CheckedCommand `
        -Name 'rust-windows-bundle-contract' `
        -Executable $script:CargoExecutable `
        -Arguments @(
            'test', '-p', 'myalbuns-desktop', '--lib',
            'tests::windows_bundle_uses_current_user_nsis_and_evergreen_webview2'
        )

    $buildWatch = [System.Diagnostics.Stopwatch]::StartNew()
    & (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1') build
    $buildExitCode = $LASTEXITCODE
    $buildWatch.Stop()
    if ($buildExitCode -ne 0) {
        throw "The NSIS release build failed with code $buildExitCode."
    }
    Add-Check `
        -Name 'tauri-nsis-release-build' `
        -ElapsedMs $buildWatch.ElapsedMilliseconds

    $installers = @(
        Get-ChildItem `
            -LiteralPath $bundleDirectory `
            -Filter '*_x64-setup.exe' `
            -File
    )
    if ($installers.Count -ne 1) {
        throw "Expected one x64 NSIS installer, found $($installers.Count)."
    }
    $installerPath = $installers[0].FullName

    $applicationPe = Get-PeEvidence -Path $applicationPath
    Add-Check -Name 'application-pe-x64' -ElapsedMs 0
    $builtSidecarPe = Get-PeEvidence -Path $builtSidecarPath
    $packagedSidecarPe = Get-PeEvidence -Path $packagedSidecarPath
    $builtSidecarHash = (
        Get-FileHash -LiteralPath $builtSidecarPath -Algorithm SHA256
    ).Hash
    $packagedSidecarHash = (
        Get-FileHash -LiteralPath $packagedSidecarPath -Algorithm SHA256
    ).Hash
    if ($builtSidecarHash -cne $packagedSidecarHash) {
        throw 'The sidecar prepared for the installer does not match the release sidecar.'
    }
    Add-Check -Name 'sidecar-pe-x64-and-origin' -ElapsedMs 0

    $port = Get-AvailableLoopbackPort
    $runId = "windows-distribution-$([DateTime]::UtcNow.Ticks)"
    $runDirectory = Join-Path $runRoot $runId
    $logDirectory = Join-Path $runDirectory 'logs'
    [System.IO.Directory]::CreateDirectory($logDirectory) | Out-Null
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new($applicationPath)
    $startInfo.UseShellExecute = $false
    $startInfo.Environment['MYALBUNS_PROCESS_ROLE'] = 'global'
    $startInfo.Environment['MYALBUNS_GLOBAL_SPIKE_ENDPOINT'] = "127.0.0.1:$port"
    $startInfo.Environment['MYALBUNS_TOPOLOGY_RUN_ID'] = $runId
    $startInfo.Environment['MYALBUNS_TOPOLOGY_SPIKE'] = 'independent'
    $startInfo.Environment['MYALBUNS_LOG_DIR'] = $logDirectory
    $startInfo.Environment['MYALBUNS_GLOBAL_SPIKE_WELCOME_VISIBLE'] = '1'
    $startedProcess = [System.Diagnostics.Process]::Start($startInfo)

    $readyMs = Wait-ForGlobalRuntime `
        -Process $startedProcess `
        -LogDirectory $logDirectory
    $webView2 = Get-WebView2Evidence `
        -RootProcessId ([uint32] $startedProcess.Id)
    Add-Check -Name 'global-welcome-and-webview2' -ElapsedMs $readyMs

    $checkpointPath = Join-Path $runDirectory 'computer-use-ready.json'
    $receiptPath = Join-Path $runDirectory 'computer-use-receipt.json'
    $checkpoint = [ordered]@{
        processId = [uint32] $startedProcess.Id
        windowTitle = $startedProcess.MainWindowTitle
        action = 'Click Open Project, observe the native dialog, write the receipt, then cancel.'
        logDirectory = $logDirectory
        receiptPath = $receiptPath
    } | ConvertTo-Json
    [System.IO.File]::WriteAllText(
        $checkpointPath,
        $checkpoint + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "COMPUTER_USE_READY=$checkpointPath"

    $nativeDialog = Wait-ForNativeDialogCancellation `
        -Process $startedProcess `
        -LogDirectory $logDirectory `
        -ReceiptPath $receiptPath `
        -TimeoutSeconds $InteractionTimeoutSeconds
    Add-Check `
        -Name 'native-dialog-cancelled' `
        -ElapsedMs $nativeDialog.elapsedMs

    $runtimeProcessId = [uint32] $startedProcess.Id
    $runtimeWindowTitle = $startedProcess.MainWindowTitle
    Start-Sleep -Milliseconds 500
    Stop-StartedProcessTree -RootProcess $startedProcess
    $startedProcess = $null

    $logEntries = @(Get-GlobalLogEntries -LogDirectory $logDirectory)
    $observedEvents = @(
        $logEntries |
            ForEach-Object { $_.event } |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
            Sort-Object -Unique
    )
    foreach ($requiredEvent in @(
        'start',
        'welcome_screen_ready',
        'project_file_selection_cancelled'
    )) {
        if ($observedEvents -cnotcontains $requiredEvent) {
            throw "The runtime log is missing '$requiredEvent'."
        }
    }
    $logFiles = @(
        Get-ChildItem `
            -LiteralPath $logDirectory `
            -Filter 'myalbuns-global*.jsonl' `
            -File |
            ForEach-Object { Get-FileEvidence -Path $_.FullName }
    )
    Add-Check -Name 'runtime-log-captured' -ElapsedMs 0

    $runtime = [ordered]@{
        source = 'unpacked_release'
        installedBinaryExercised = $false
        topology = 'independent'
        processId = $runtimeProcessId
        windowTitle = $runtimeWindowTitle
        readyMs = [long] $readyMs
        webView2 = $webView2
        nativeDialog = [ordered]@{
            checkpointEmitted = $true
            requestedDriver = 'computer_use'
            action = 'open_project_then_cancel'
            outcome = 'cancelled'
            evidenceEvent = 'project_file_selection_cancelled'
            externalReceipt = $nativeDialog.receipt
        }
        logCapture = [ordered]@{
            files = $logFiles
            observedEvents = $observedEvents
            matchingEntry = $nativeDialog.entry
        }
    }
}
finally {
    if ($null -ne $startedProcess) {
        Stop-StartedProcessTree -RootProcess $startedProcess
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
    $finalBuildInputs.fileCount -ne $initialBuildInputs.fileCount -or
    $finalBuildInputs.digestSha256 -cne $initialBuildInputs.digestSha256
) {
    throw 'Windows distribution checks changed source inputs or ran against dirty inputs.'
}

$gitCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $gitCommit -notmatch '^[0-9a-f]{40}$') {
    throw 'Could not identify the Windows distribution source commit.'
}
$workingTreeStatus = @(& git -C $script:WorkspaceRoot status --short)
if ($LASTEXITCODE -ne 0) {
    throw 'Could not inspect the working tree after the Windows distribution gate.'
}
$rustcExecutable = Join-Path $script:CargoHome 'bin\rustc.exe'
$rustHost = @(
    & $rustcExecutable -vV |
        Where-Object { $_ -like 'host:*' } |
        ForEach-Object { $_.Substring(5).Trim() }
)
if ($rustHost.Count -ne 1 -or $rustHost[0] -cne 'x86_64-pc-windows-msvc') {
    throw "Unexpected Rust host: $($rustHost -join ', ')."
}
$cleanEnvironment = Get-CleanEnvironmentEvidence
$operatingSystem = Get-CimInstance Win32_OperatingSystem

$applicationEvidence = Get-FileEvidence -Path $applicationPath
$applicationEvidence.pe = $applicationPe
$builtSidecarEvidence = Get-FileEvidence -Path $builtSidecarPath
$builtSidecarEvidence.pe = $builtSidecarPe
$packagedSidecarEvidence = Get-FileEvidence -Path $packagedSidecarPath
$packagedSidecarEvidence.pe = $packagedSidecarPe
$packagedSidecarEvidence.matchesBuiltSidecar = $true
$installerEvidence = Get-FileEvidence -Path $installerPath

$report = [ordered]@{
    schemaVersion = 1
    suite = 'windows_distribution_local_probe'
    collectedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
    gitCommit = $gitCommit
    sourceInputsDirty = $false
    platform = [ordered]@{
        operatingSystem = $operatingSystem.Caption
        operatingSystemVersion = $operatingSystem.Version
        osArchitecture = (
            [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        )
        processArchitecture = (
            [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
        )
        rustHost = $rustHost[0]
    }
    build = [ordered]@{
        profile = 'release'
        target = 'win-x64'
        cargoBuildJobs = $CargoBuildJobs
        targetDirectory = 'target/windows-distribution-gate'
        inputFileCount = $finalBuildInputs.fileCount
        inputDigestSha256 = $finalBuildInputs.digestSha256
        workingTreeDirty = $workingTreeStatus.Count -gt 0
        configuration = [ordered]@{
            bundleTargets = @($tauriConfig.bundle.targets)
            webView2 = [ordered]@{
                distribution = 'Evergreen'
                webviewInstallMode = $windowsBundle.webviewInstallMode
            }
            nsis = $windowsBundle.nsis
            externalBin = @($tauriConfig.bundle.externalBin)
        }
        outputs = [ordered]@{
            application = $applicationEvidence
            builtSidecar = $builtSidecarEvidence
            packagedSidecar = $packagedSidecarEvidence
            installer = $installerEvidence
        }
    }
    checks = @($checks)
    results = [ordered]@{
        runtime = $runtime
        cleanEnvironment = $cleanEnvironment
    }
    completion = [ordered]@{
        localProbePassed = $true
        cleanMachineE2ePassed = $false
        ticketCriterionSatisfied = $false
    }
    interpretation = [ordered]@{
        criterionClosed = $false
        reason = (
            'The bundle, x64 payloads, WebView2 and native dialog passed locally; ' +
            'installation and execution on a clean Windows machine were unavailable.'
        )
    }
}

$outputDirectory = Split-Path -Parent $OutputPath
[System.IO.Directory]::CreateDirectory($outputDirectory) | Out-Null
$json = $report | ConvertTo-Json -Depth 16
[System.IO.File]::WriteAllText(
    $OutputPath,
    $json + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)

Write-Output "Local Windows distribution probe passed: $OutputPath"
if ($cleanEnvironment.available) {
    Write-Output 'A disposable Windows exists, but its clean-machine E2E was not exercised.'
}
else {
    Write-Output 'Clean-machine installation remains pending because no disposable Windows exists.'
}
