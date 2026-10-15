[CmdletBinding()]
param(
    # The NSIS installer to measure (e.g. target\release\bundle\nsis\MyAlbuns_*-setup.exe).
    [Parameter(Mandatory)] [string] $Installer,
    # The project to open. It and its photos are copied into the VM at the same
    # absolute path, so they must live on C: or D: of the host.
    # Several projects are opened together, as Windows does when files are
    # selected and opened at once; the scenario drives the first editor shown.
    [Parameter(Mandatory)] [string[]] $ProjectFile,
    # Folders copied into the VM; by default the folder of each project.
    [string[]] $MediaFolder = @(),
    [string] $HardwareProfile = 'fraco',
    # Opening cycles in one boot: the first is cold, the others warm.
    [int] $Repeat = 1,
    [int] $IdleSeconds = 10,
    [int] $SettleSeconds = 5,
    # A weak machine can prepare images for many minutes before the Project shows.
    [int] $ReadyTimeoutSeconds = 3600,
    [string] $LabRoot = 'D:\HardwareLab',
    [string] $OutputRoot,
    # Leaves the VM running for inspection through Hyper-V Manager.
    [switch] $KeepRunning,
    # Measures a clean Windows without the Visual C++ runtime. By default the
    # host's runtime is copied in, as most user PCs have it installed and the
    # imaging sidecar cannot start without VCRUNTIME140.dll.
    [switch] $WithoutVcRuntime,
    # Records each opening with the Windows Performance Recorder (CPU samples
    # with stacks, disk and file I/O) into trace-N.etl, to open in Windows
    # Performance Analyzer with the build's .pdb files. Traces are large.
    [switch] $RecordEtw,
    # Saves a picture of the guest desktop every second for the first N
    # seconds of each opening (desktop-N\*.jpg), to see what the user sees
    # before DevTools can reach a page. Costs some guest CPU while it runs.
    [int] $DesktopSeconds = 0
)

# One measurement: a fresh system disk, the app installed silently, the guest
# rebooted so no file is cached, then the project opened in the interactive
# session and driven through DevTools while counters are sampled. Everything
# lands in a run folder with a summary.md.
. (Join-Path $PSScriptRoot 'HardwareLab.Common.ps1')
Assert-LabHyperVAccess

$GuestTools = "$HardwareLabGuestRoot\tools"
$GuestOut = "$HardwareLabGuestRoot\out"
$BrowserArguments = '--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection ' +
    '--autoplay-policy=no-user-gesture-required --remote-debugging-port=0'

$config = Get-LabConfig -LabRoot $LabRoot
$credential = Get-LabCredential -Config $config
$hardware = Get-LabProfile -Name $HardwareProfile
$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$projectPaths = @($ProjectFile | ForEach-Object { (Resolve-Path -LiteralPath $_).Path })
$projectPath = $projectPaths[0]
$folders = @(if ($MediaFolder.Count -gt 0) { $MediaFolder } else { $projectPaths | ForEach-Object { Split-Path $_ } }) |
    ForEach-Object { (Resolve-Path -LiteralPath $_).Path } | Select-Object -Unique
foreach ($path in @($projectPaths) + $folders) {
    if ($path -notmatch '^[CcDd]:\\') {
        throw "$path must be on C: or D: of the host: the VM has no network and only those drives."
    }
}
$node = (Get-Command node -ErrorAction Stop).Source
if (-not $OutputRoot) {
    $OutputRoot = Join-Path $LabRoot 'runs'
}
$runDirectory = Join-Path $OutputRoot ('{0}-{1}' -f (Get-Date -Format 'yyyyMMdd-HHmmss'), $HardwareProfile)
New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null
$run = [ordered] @{
    profile = $hardware
    installer = Split-Path $installerPath -Leaf
    installerSha256 = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash
    project = $projectPaths
    repeat = $Repeat
    lab = [ordered] @{ edition = $config.edition; windowsVersion = $config.windowsVersion; gpu = $config.gpu }
    startedAt = (Get-Date).ToString('o')
    iterations = @()
}

function Sync-LabFolder {
    param(
        [Parameter(Mandatory)] $Session,
        [Parameter(Mandatory)] [string] $Folder
    )
    $guestLengths = Invoke-Command -Session $Session -ArgumentList $Folder -ScriptBlock {
        param($folder)
        $lengths = @{}
        if (Test-Path -LiteralPath $folder) {
            Get-ChildItem -LiteralPath $folder -Recurse -File | ForEach-Object { $lengths[$_.FullName] = $_.Length }
        }
        $lengths
    }
    $pending = @(Get-ChildItem -LiteralPath $Folder -Recurse -File | Where-Object {
            -not $guestLengths.ContainsKey($_.FullName) -or $guestLengths[$_.FullName] -ne $_.Length
        })
    if ($pending.Count -eq 0) {
        return
    }
    $bytes = ($pending | Measure-Object Length -Sum).Sum
    Write-LabStep ("Copying {0} file(s), {1:N0} MB, of {2}" -f $pending.Count, ($bytes / 1MB), $Folder)
    $directories = @($pending | ForEach-Object DirectoryName | Sort-Object -Unique)
    Invoke-Command -Session $Session -ArgumentList (, $directories) -ScriptBlock {
        param($directories)
        foreach ($directory in $directories) {
            New-Item -ItemType Directory -Path $directory -Force | Out-Null
        }
    }
    foreach ($file in $pending) {
        Copy-Item -LiteralPath $file.FullName -Destination $file.FullName -ToSession $Session -Force
    }
}

function Restart-LabGuest {
    param([Parameter(Mandatory)] $Session)
    $bootedAt = Invoke-Command -Session $Session -ScriptBlock { (Get-CimInstance Win32_OperatingSystem).LastBootUpTime }
    try {
        Invoke-Command -Session $Session -ScriptBlock { Restart-Computer -Force }
    }
    catch {
        # The session may break while the guest goes down.
    }
    Remove-PSSession $Session -ErrorAction SilentlyContinue
    while ($true) {
        Start-Sleep -Seconds 10
        $next = Wait-LabGuest -Config $config -Credential $credential
        try {
            $current = Invoke-Command -Session $next -ErrorAction Stop -ScriptBlock { (Get-CimInstance Win32_OperatingSystem).LastBootUpTime }
            if ($current -ne $bootedAt) {
                return $next
            }
        }
        catch {
            # The old boot went down under this session; wait for the new one.
        }
        Remove-PSSession $next -ErrorAction SilentlyContinue
    }
}

function Wait-LabAppExit {
    param(
        [Parameter(Mandatory)] $Session,
        [int] $TimeoutSeconds = 60
    )
    Invoke-Command -Session $Session -ArgumentList $TimeoutSeconds -ScriptBlock {
        param($timeout)
        $deadline = (Get-Date).AddSeconds($timeout)
        while ((Get-Process -Name 'myalbuns-desktop' -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) {
            Start-Sleep -Milliseconds 500
        }
        $leftovers = @(Get-Process -Name 'myalbuns-desktop', 'myalbuns-imaging', 'msedgewebview2' -ErrorAction SilentlyContinue)
        $leftovers | Stop-Process -Force -ErrorAction SilentlyContinue
        $leftovers.Count
    }
}

Write-LabStep "Resetting the VM to the base image with profile '$HardwareProfile'"
Stop-LabVm -Config $config
Reset-LabSystemDisk -Config $config
Set-LabVmProfile -Config $config -HardwareProfile $hardware
Set-LabVmDiskLimit -Config $config -Unlimited
Start-VM -Name $config.vmName
$session = Wait-LabGuest -Config $config -Credential $credential
try {
    Write-LabStep 'Copying the tools, the installer and the project'
    Invoke-Command -Session $session -ArgumentList $GuestTools, $GuestOut -ScriptBlock {
        param($tools, $out)
        New-Item -ItemType Directory -Path $tools, $out -Force | Out-Null
    }
    Copy-Item -LiteralPath $node -Destination "$GuestTools\node.exe" -ToSession $session -Force
    Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'guest') -File |
        Where-Object { $_.Extension -in '.ps1', '.mjs' } |
        ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination "$GuestTools\$($_.Name)" -ToSession $session -Force }
    Copy-Item -LiteralPath $installerPath -Destination "$HardwareLabGuestRoot\installer.exe" -ToSession $session -Force
    foreach ($folder in $folders) {
        Sync-LabFolder -Session $session -Folder $folder
    }
    foreach ($path in $projectPaths) {
        Invoke-Command -Session $session -ArgumentList (Split-Path $path) -ScriptBlock {
            param($directory)
            New-Item -ItemType Directory -Path $directory -Force | Out-Null
        }
        Copy-Item -LiteralPath $path -Destination $path -ToSession $session -Force
    }

    $run.vcRuntime = $null
    if (-not $WithoutVcRuntime) {
        $runtimeFiles = @(Get-ChildItem -Path "$env:SystemRoot\System32\*" -Include 'vcruntime140*.dll', 'msvcp140*.dll' |
                Where-Object { $_.Name -notmatch 'd(_[a-z_]+)?\.dll$|_clr' })
        if ($runtimeFiles.Count -eq 0) {
            throw 'This computer has no Visual C++ runtime to copy; use -WithoutVcRuntime.'
        }
        Write-LabStep "Copying the Visual C++ runtime $($runtimeFiles[0].VersionInfo.FileVersion) into the guest"
        foreach ($file in $runtimeFiles) {
            Copy-Item -LiteralPath $file.FullName -Destination "C:\Windows\System32\$($file.Name)" -ToSession $session -Force
        }
        $run.vcRuntime = $runtimeFiles[0].VersionInfo.FileVersion
    }

    Write-LabStep 'Installing the app silently'
    $install = Invoke-Command -Session $session -ArgumentList $HardwareLabGuestRoot -ScriptBlock {
        param($root)
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $process = Start-Process -FilePath "$root\installer.exe" -ArgumentList '/S' -Wait -PassThru
        $executable = Join-Path $env:LOCALAPPDATA 'MyAlbuns\myalbuns-desktop.exe'
        [pscustomobject] @{
            exitCode = $process.ExitCode
            seconds = $watch.Elapsed.TotalSeconds
            executable = $executable
            installed = Test-Path -LiteralPath $executable
            version = if (Test-Path -LiteralPath $executable) { (Get-Item -LiteralPath $executable).VersionInfo.ProductVersion } else { $null }
            stateRoot = Join-Path $env:LOCALAPPDATA 'MyAlbuns2'
        }
    }
    if (-not $install.installed) {
        throw "The installer ended with code $($install.exitCode) and left no $($install.executable)."
    }
    $run.install = $install | Select-Object exitCode, seconds, version

    Invoke-Command -Session $session -ArgumentList $GuestTools -ScriptBlock {
        param($tools)
        $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero)
        $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive
        foreach ($task in @(
                @('MyAlbunsLabLaunch', 'Launch-App.ps1'),
                @('MyAlbunsLabClose', 'Close-App.ps1'),
                @('MyAlbunsLabRecord', 'Record-Desktop.ps1'))) {
            $action = New-ScheduledTaskAction -Execute 'powershell.exe' `
                -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$tools\$($task[1])`""
            Register-ScheduledTask -TaskName $task[0] -Action $action -Principal $principal -Settings $settings -Force | Out-Null
        }
    }

    Write-LabStep 'Capping the disks and rebooting so nothing stays in the file cache'
    Set-LabVmDiskLimit -Config $config -HardwareProfile $hardware
    $session = Restart-LabGuest -Session $session
    if (-not (Wait-LabGuestQuiet -Session $session)) {
        Write-Warning 'The guest CPU did not settle; the first opening may include boot work.'
    }

    Invoke-Command -Session $session -ArgumentList $GuestTools, $GuestOut -ScriptBlock {
        param($tools, $out)
        Remove-Item -Path "$out\*" -Recurse -Force -ErrorAction SilentlyContinue
        Start-Process -FilePath 'powershell.exe' -WindowStyle Hidden -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$tools\Sample-Counters.ps1`"",
            '-OutputPath', "`"$out\counters.csv`"", '-StopPath', "`"$out\stop-sampling`"")
    }

    for ($iteration = 1; $iteration -le $Repeat; $iteration += 1) {
        $kind = if ($iteration -eq 1) { 'fria' } else { 'morna' }
        Write-LabStep "Opening $iteration/$Repeat ($kind)"
        $launch = [pscustomobject] @{
            executable = $install.executable
            projects = $projectPaths
            browserArguments = $BrowserArguments
            markerPath = "$GuestOut\launch-$iteration.json"
        }
        $recording = if ($DesktopSeconds -gt 0) {
            [pscustomobject] @{ directory = "$GuestOut\desktop-$iteration"; seconds = $DesktopSeconds; intervalMs = 1000 }
        }
        $launched = Invoke-Command -Session $session -ArgumentList $launch, $recording -ScriptBlock {
            param($launch, $recording)
            if ($recording) {
                $recording | ConvertTo-Json | Set-Content -LiteralPath 'C:\Lab\record.json' -Encoding UTF8
                Start-ScheduledTask -TaskName 'MyAlbunsLabRecord'
                $deadline = (Get-Date).AddSeconds(30)
                while (-not (Test-Path -LiteralPath "$($recording.directory)\recording.json") -and (Get-Date) -lt $deadline) {
                    Start-Sleep -Milliseconds 200
                }
            }
            $launch | ConvertTo-Json | Set-Content -LiteralPath 'C:\Lab\launch.json' -Encoding UTF8
            Start-ScheduledTask -TaskName 'MyAlbunsLabLaunch'
            $deadline = (Get-Date).AddSeconds(60)
            while (-not (Test-Path -LiteralPath $launch.markerPath) -and (Get-Date) -lt $deadline) {
                Start-Sleep -Milliseconds 200
            }
            if (-not (Test-Path -LiteralPath $launch.markerPath)) {
                throw 'The launch task did not start the app within 60 s.'
            }
            Get-Content -LiteralPath $launch.markerPath -Raw -Encoding UTF8 | ConvertFrom-Json
        }
        if ($RecordEtw) {
            Invoke-Command -Session $session -ScriptBlock {
                & wpr.exe -start CPU -start DiskIO -start FileIO -filemode | Out-Null
                if ($LASTEXITCODE -ne 0) {
                    throw "wpr could not start recording (exit code $LASTEXITCODE)."
                }
            }
        }
        $scenario = [pscustomobject] @{
            launchedAt = $launched.launchedAt
            webviewStateRoot = "$($install.stateRoot)\State\WebView2"
            logDirectory = "$($install.stateRoot)\Logs"
            projectCount = $projectPaths.Count
            outputPath = "$GuestOut\scenario-$iteration.json"
            screenshotPath = "$GuestOut\screenshot-$iteration.png"
            readyTimeoutMs = $ReadyTimeoutSeconds * 1000
            idleMs = $IdleSeconds * 1000
            settleMs = $SettleSeconds * 1000
            progressIntervalMs = 15000
            profileDirectory = $GuestOut
            iteration = $iteration
        }
        Invoke-Command -Session $session -ArgumentList $scenario, $GuestTools -ScriptBlock {
            param($scenario, $tools)
            $scenario | ConvertTo-Json | Set-Content -LiteralPath 'C:\Lab\scenario.json' -Encoding UTF8
            & "$tools\node.exe" "$tools\measure-scenario.mjs" 'C:\Lab\scenario.json'
        }
        if ($RecordEtw) {
            Write-LabStep 'Saving the Windows performance trace'
            Invoke-Command -Session $session -ArgumentList "$GuestOut\trace-$iteration.etl" -ScriptBlock {
                param($path)
                & wpr.exe -stop $path | Out-Null
            }
        }
        Invoke-Command -Session $session -ScriptBlock { Start-ScheduledTask -TaskName 'MyAlbunsLabClose' }
        $killed = Wait-LabAppExit -Session $session
        $run.iterations += [ordered] @{ index = $iteration; kind = $kind; launchedAt = $launched.launchedAtIso; forcedProcesses = [int] $killed }
        Start-Sleep -Seconds 5
    }

    Write-LabStep 'Collecting counters, logs and guest details'
    $guest = Invoke-Command -Session $session -ArgumentList $GuestOut, $install.executable -ScriptBlock {
        param($out, $executable)
        New-Item -ItemType File -Path "$out\stop-sampling" -Force | Out-Null
        $deadline = (Get-Date).AddSeconds(30)
        while (-not (Test-Path -LiteralPath "$out\counters.csv.done") -and (Get-Date) -lt $deadline) {
            Start-Sleep -Milliseconds 250
        }
        $os = Get-CimInstance Win32_OperatingSystem
        $webview = Get-ItemProperty -Path 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue
        [pscustomobject] @{
            os = $os.Caption
            build = $os.BuildNumber
            logicalProcessors = [Environment]::ProcessorCount
            memoryMB = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1MB)
            webview2 = if ($webview) { $webview.pv } else { $null }
            videoControllers = @(Get-CimInstance Win32_VideoController | ForEach-Object { "$($_.Name) $($_.DriverVersion) [$($_.Status)]" })
            logDirectory = Join-Path $env:LOCALAPPDATA 'MyAlbuns2\Logs'
        }
    }
    $run.guest = $guest | Select-Object os, build, logicalProcessors, memoryMB, webview2, videoControllers
    Copy-Item -Path "$GuestOut\*" -Destination $runDirectory -FromSession $session -Recurse -Force
    $logs = Join-Path $runDirectory 'logs'
    New-Item -ItemType Directory -Path $logs -Force | Out-Null
    # A log still held open by a forced app exit is copied with shared reads.
    $stagedLogs = Invoke-Command -Session $session -ArgumentList $guest.logDirectory, "$HardwareLabGuestRoot\logs" -ScriptBlock {
        param($source, $staging)
        New-Item -ItemType Directory -Path $staging -Force | Out-Null
        foreach ($file in @(Get-ChildItem -LiteralPath $source -File -ErrorAction SilentlyContinue)) {
            $reader = [IO.File]::Open($file.FullName, 'Open', 'Read', 'ReadWrite')
            $writer = [IO.File]::Create((Join-Path $staging $file.Name))
            $reader.CopyTo($writer)
            $writer.Dispose()
            $reader.Dispose()
        }
        $staging
    }
    Copy-Item -Path "$stagedLogs\*" -Destination $logs -FromSession $session -Recurse -Force
}
finally {
    $run.finishedAt = (Get-Date).ToString('o')
    $run | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $runDirectory 'run.json') -Encoding UTF8
    if ($session) {
        Remove-PSSession $session -ErrorAction SilentlyContinue
    }
    if (-not $KeepRunning) {
        Stop-LabVm -Config $config
    }
}

& $node (Join-Path $PSScriptRoot 'summarize-run.mjs') $runDirectory
Write-LabStep "Run folder: $runDirectory"
