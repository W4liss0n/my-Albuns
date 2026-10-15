$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

$script:HardwareLabScriptRoot = $PSScriptRoot
$script:HardwareLabWorkspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script:HardwareLabGuestRoot = 'C:\Lab'
$script:HardwareLabGuestUser = 'lab'

function Write-LabStep {
    param([Parameter(Mandatory)] [string] $Message)
    Write-Host ("[{0}] {1}" -f (Get-Date -Format 'HH:mm:ss'), $Message)
}

function Assert-LabElevated {
    $principal = [Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run this script from a PowerShell window opened as administrator.'
    }
}

function Assert-LabHyperVAccess {
    try {
        Get-VM -ErrorAction Stop | Out-Null
    }
    catch {
        throw ('Hyper-V refused access. Run as administrator, or run New-HardwareLabVm.ps1 ' +
            'with -GrantHyperVAdministrators once and sign out and back in. ' + $_.Exception.Message)
    }
}

function Get-LabConfigPath {
    param([Parameter(Mandatory)] [string] $LabRoot)
    Join-Path $LabRoot 'lab.json'
}

function Get-LabConfig {
    param([Parameter(Mandatory)] [string] $LabRoot)
    $path = Get-LabConfigPath -LabRoot $LabRoot
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "No hardware lab exists in $LabRoot. Create it with New-HardwareLabVm.ps1."
    }
    Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
}

function Save-LabConfig {
    param(
        [Parameter(Mandatory)] [string] $LabRoot,
        [Parameter(Mandatory)] $Config
    )
    $Config | ConvertTo-Json -Depth 5 |
        Set-Content -LiteralPath (Get-LabConfigPath -LabRoot $LabRoot) -Encoding UTF8
}

function Get-LabProfile {
    param([Parameter(Mandatory)] [string] $Name)
    $profiles = (Get-Content -LiteralPath (Join-Path $script:HardwareLabScriptRoot 'profiles.json') -Raw -Encoding UTF8 |
            ConvertFrom-Json).profiles
    $entry = $profiles.PSObject.Properties[$Name]
    if (-not $entry) {
        $names = ($profiles.PSObject.Properties | ForEach-Object Name) -join ', '
        throw "Unknown hardware profile '$Name'. Known profiles: $names."
    }
    $entry.Value | Add-Member -NotePropertyName name -NotePropertyValue $Name -PassThru
}

# Applies the processor and memory of a hardware profile; both can only change
# while the VM is off.
function Set-LabVmProfile {
    param(
        [Parameter(Mandatory)] $Config,
        [Parameter(Mandatory)] $HardwareProfile
    )
    $vm = Get-VM -Name $Config.vmName
    if ($vm.State -ne 'Off') {
        throw "The VM $($Config.vmName) must be off to change its hardware profile."
    }
    Set-VMProcessor -VMName $Config.vmName `
        -Count $HardwareProfile.processorCount `
        -Maximum $HardwareProfile.processorLimitPercent `
        -Reserve 0 `
        -RelativeWeight 100
    Set-VMMemory -VMName $Config.vmName `
        -DynamicMemoryEnabled $false `
        -StartupBytes ([int64] $HardwareProfile.memoryMB * 1MB)
}

# Caps the disks while the VM runs, so copies into the guest can run uncapped
# first. Hyper-V normalizes IOPS in 8 KB units, so the cap also limits
# sequential throughput (5000 IOPS ~ 40 MB/s); 0 removes the cap.
function Set-LabVmDiskLimit {
    param(
        [Parameter(Mandatory)] $Config,
        $HardwareProfile,
        [switch] $Unlimited
    )
    Get-VMHardDiskDrive -VMName $Config.vmName | ForEach-Object {
        $limit = if ($Unlimited) {
            0
        }
        elseif ($_.Path -eq $Config.dataDiskPath) {
            $HardwareProfile.dataDiskMaximumIops
        }
        else {
            $HardwareProfile.systemDiskMaximumIops
        }
        Set-VMHardDiskDrive -VMHardDiskDrive $_ -MaximumIOPS ([uint64] $limit)
    }
}

# Waits until the guest CPU stays below a threshold, so background work left by
# a boot (indexing, provisioning) does not pollute a measurement.
function Wait-LabGuestQuiet {
    param(
        [Parameter(Mandatory)] $Session,
        [int] $ThresholdPercent = 20,
        [int] $QuietSamples = 5,
        [int] $TimeoutSeconds = 300
    )
    Invoke-Command -Session $Session -ArgumentList $ThresholdPercent, $QuietSamples, $TimeoutSeconds -ScriptBlock {
        param($threshold, $needed, $timeout)
        $deadline = (Get-Date).AddSeconds($timeout)
        $quiet = 0
        while ($quiet -lt $needed -and (Get-Date) -lt $deadline) {
            Start-Sleep -Seconds 2
            $cpu = (Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'").PercentProcessorTime
            $quiet = if ($cpu -lt $threshold) { $quiet + 1 } else { 0 }
        }
        $quiet -ge $needed
    }
}

function Get-LabCredential {
    param([Parameter(Mandatory)] $Config)
    Import-Clixml -LiteralPath $Config.credentialPath
}

# Waits until PowerShell Direct answers and the lab user owns the interactive
# console session (autologon finished), so a GUI app can be started there.
function Wait-LabGuest {
    param(
        [Parameter(Mandatory)] $Config,
        [Parameter(Mandatory)] [pscredential] $Credential,
        [int] $TimeoutSeconds = 900
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ($true) {
        $session = $null
        try {
            $session = New-PSSession -VMName $Config.vmName -Credential $Credential -ErrorAction Stop
            # Explorer alone could belong to the setup account during the
            # first boot; the console user must be the lab user.
            $interactive = Invoke-Command -Session $session -ErrorAction Stop -ArgumentList $script:HardwareLabGuestUser -ScriptBlock {
                param($user)
                $console = (Get-CimInstance Win32_ComputerSystem).UserName
                [bool] ($console -and $console.EndsWith("\$user") -and
                    (Get-Process explorer -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -gt 0 }))
            }
            if ($interactive) {
                return $session
            }
            $reason = 'the lab user has not reached the desktop'
        }
        catch {
            # The guest is booting, or rebooted again and dropped the session.
            $reason = $_.Exception.Message
        }
        if ($session) {
            Remove-PSSession $session -ErrorAction SilentlyContinue
        }
        if ((Get-Date) -gt $deadline) {
            throw "The VM was not ready within $TimeoutSeconds s: $reason"
        }
        Start-Sleep -Seconds 5
    }
}

function Stop-LabVm {
    param([Parameter(Mandatory)] $Config)
    $vm = Get-VM -Name $Config.vmName
    if ($vm.State -ne 'Off') {
        Stop-VM -VM $vm -TurnOff -Force
    }
}

# Every run starts from the prepared golden image: the system disk is a fresh
# differencing child of the read-only base disk. The drive is detached while
# the file is replaced, so Hyper-V grants the VM access to the new file.
function Reset-LabSystemDisk {
    param([Parameter(Mandatory)] $Config)
    $drive = Get-VMHardDiskDrive -VMName $Config.vmName |
        Where-Object { $_.Path -eq $Config.systemDiskPath }
    if (-not $drive) {
        throw "The VM $($Config.vmName) has no disk at $($Config.systemDiskPath)."
    }
    $location = @{
        ControllerType = $drive.ControllerType
        ControllerNumber = $drive.ControllerNumber
        ControllerLocation = $drive.ControllerLocation
    }
    Remove-VMHardDiskDrive -VMHardDiskDrive $drive
    if (Test-Path -LiteralPath $Config.systemDiskPath) {
        Remove-Item -LiteralPath $Config.systemDiskPath -Force
    }
    New-VHD -Path $Config.systemDiskPath -ParentPath $Config.baseDiskPath -Differencing | Out-Null
    Add-VMHardDiskDrive -VMName $Config.vmName -Path $Config.systemDiskPath @location
    $added = Get-VMHardDiskDrive -VMName $Config.vmName | Where-Object { $_.Path -eq $Config.systemDiskPath }
    Set-VMFirmware -VMName $Config.vmName -FirstBootDevice $added
}
