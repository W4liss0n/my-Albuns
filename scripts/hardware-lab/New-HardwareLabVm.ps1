[CmdletBinding()]
param(
    # A Windows 10 or 11 ISO from Microsoft, in the edition and build to imitate.
    [Parameter(Mandatory)] [string] $IsoPath,
    [string] $LabRoot = 'D:\HardwareLab',
    [string] $VmName = 'myalbuns-lab',
    # Matched against the end of the image name, e.g. 'Pro' or 'Enterprise Evaluation'.
    [string] $Edition = 'Pro',
    # The host GPU shared with the VM. Intel UHD is closer to an old integrated GPU.
    [ValidateSet('Intel', 'NVIDIA')] [string] $Gpu = 'Intel',
    [string] $HardwareProfile = 'fraco',
    [int] $SystemDiskGB = 64,
    [int] $DataDiskGB = 64,
    # Adds the current user to "Hyper-V Administrators" so the runs need no
    # elevation; takes effect after signing out and back in.
    [switch] $GrantHyperVAdministrators,
    # Deletes an existing lab VM with the same name and its disks first.
    [switch] $Force
)

# Builds the lab VM without Windows Setup: the image is applied to a VHDX with
# DISM, an answer file skips the first-run screens and logs the lab user on
# automatically, and the host GPU driver is copied in for GPU partitioning
# (the app refuses software WebGL2). The first boot finishes inside the base
# disk, which then becomes the read-only parent of every run.
. (Join-Path $PSScriptRoot 'HardwareLab.Common.ps1')
Assert-LabElevated

$EfiPartitionType = '{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}'
$GpuVendors = @{ Intel = 'VEN_8086'; NVIDIA = 'VEN_10DE' }
$hardware = Get-LabProfile -Name $HardwareProfile
$setupProfile = Get-LabProfile -Name 'referencia'

function New-LabPassword {
    $alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789'
    $bytes = [byte[]]::new(24)
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    -join ($bytes | ForEach-Object { $alphabet[$_ % $alphabet.Length] })
}

function Get-PartitionLetter {
    param([Parameter(Mandatory)] $Partition)
    $letter = (Get-Partition -DiskNumber $Partition.DiskNumber -PartitionNumber $Partition.PartitionNumber).DriveLetter
    if (-not $letter -or $letter -eq [char] 0) {
        throw "Partition $($Partition.PartitionNumber) of disk $($Partition.DiskNumber) has no drive letter."
    }
    [string] $letter
}

# Copies the files of the host GPU driver where a GPU partition expects them:
# driver-store packages under HostDriverStore, other files at their own path.
function Copy-HostGpuDriver {
    param(
        [Parameter(Mandatory)] [string] $Vendor,
        [Parameter(Mandatory)] [string] $WindowsLetter
    )
    $device = Get-CimInstance Win32_VideoController |
        Where-Object { $_.PNPDeviceID -like "*$Vendor*" } |
        Select-Object -First 1
    if (-not $device) {
        throw "The host has no video controller from $Vendor."
    }
    Write-LabStep "Copying the driver of $($device.Name) $($device.DriverVersion)"
    $files = Get-CimInstance Win32_PnPSignedDriverCIMDataFile |
        Where-Object { $_.Antecedent.DeviceID -eq $device.PNPDeviceID } |
        ForEach-Object { $_.Dependent.Name }
    $repository = Join-Path $env:SystemRoot 'System32\DriverStore\FileRepository'
    $hostDriverStore = "${WindowsLetter}:\Windows\System32\HostDriverStore\FileRepository"
    New-Item -ItemType Directory -Path $hostDriverStore -Force | Out-Null
    $packages = @{}
    $loose = 0
    foreach ($file in $files) {
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
            continue
        }
        if ($file -like "$repository\*") {
            $package = $file.Substring($repository.Length + 1).Split('\')[0]
            if (-not $packages.ContainsKey($package)) {
                Copy-Item -LiteralPath (Join-Path $repository $package) `
                    -Destination (Join-Path $hostDriverStore $package) -Recurse -Force
                $packages[$package] = $true
            }
        }
        else {
            # Never replace a file the guest Windows already ships.
            $target = "${WindowsLetter}:" + $file.Substring(2)
            if (-not (Test-Path -LiteralPath $target)) {
                New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
                Copy-Item -LiteralPath $file -Destination $target
                $loose += 1
            }
        }
    }
    if ($packages.Count -eq 0) {
        throw "No driver-store package was found for $($device.Name)."
    }
    Write-LabStep "Copied $($packages.Count) driver package(s) and $loose loose file(s)"
}

function New-DataDisk {
    param([Parameter(Mandatory)] [string] $Path)
    New-VHD -Path $Path -SizeBytes ([int64] $DataDiskGB * 1GB) -Dynamic | Out-Null
    $disk = Mount-VHD -Path $Path -Passthru | Get-Disk
    try {
        Initialize-Disk -Number $disk.Number -PartitionStyle GPT
        $partition = New-Partition -DiskNumber $disk.Number -UseMaximumSize
        Format-Volume -Partition $partition -FileSystem NTFS -NewFileSystemLabel 'LabData' -Confirm:$false -Force | Out-Null
    }
    finally {
        Dismount-VHD -Path $Path
    }
}

function New-BaseDisk {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $ImagePath,
        [Parameter(Mandatory)] [int] $ImageIndex,
        [Parameter(Mandatory)] [string] $UnattendXml
    )
    New-VHD -Path $Path -SizeBytes ([int64] $SystemDiskGB * 1GB) -Dynamic | Out-Null
    $disk = Mount-VHD -Path $Path -Passthru | Get-Disk
    try {
        Initialize-Disk -Number $disk.Number -PartitionStyle GPT
        $efi = New-Partition -DiskNumber $disk.Number -Size 260MB -GptType $EfiPartitionType
        Format-Volume -Partition $efi -FileSystem FAT32 -NewFileSystemLabel 'System' -Confirm:$false -Force | Out-Null
        $efi | Add-PartitionAccessPath -AssignDriveLetter
        $windows = New-Partition -DiskNumber $disk.Number -UseMaximumSize -AssignDriveLetter
        Format-Volume -Partition $windows -FileSystem NTFS -NewFileSystemLabel 'Windows' -Confirm:$false -Force | Out-Null
        $efiLetter = Get-PartitionLetter $efi
        $windowsLetter = Get-PartitionLetter $windows

        Write-LabStep "Applying the Windows image to ${windowsLetter}: (several minutes)"
        Expand-WindowsImage -ImagePath $ImagePath -Index $ImageIndex -ApplyPath "${windowsLetter}:\" | Out-Null
        & bcdboot "${windowsLetter}:\Windows" /s "${efiLetter}:" /f UEFI | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "bcdboot failed with exit code $LASTEXITCODE."
        }

        $panther = "${windowsLetter}:\Windows\Panther"
        New-Item -ItemType Directory -Path $panther -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $panther 'unattend.xml') -Value $UnattendXml -Encoding UTF8
        $setupScripts = "${windowsLetter}:\Windows\Setup\Scripts"
        New-Item -ItemType Directory -Path $setupScripts -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'guest\SetupComplete.cmd') -Destination $setupScripts
        New-Item -ItemType Directory -Path "${windowsLetter}:\Lab\out" -Force | Out-Null

        Copy-HostGpuDriver -Vendor $GpuVendors[$Gpu] -WindowsLetter $windowsLetter
        $efi | Remove-PartitionAccessPath -AccessPath "${efiLetter}:\"
    }
    finally {
        Dismount-VHD -Path $Path
    }
}

$vmFolder = Join-Path $LabRoot $VmName
$config = [pscustomobject] @{
    vmName = $VmName
    labRoot = $LabRoot
    baseDiskPath = Join-Path $vmFolder 'system-base.vhdx'
    systemDiskPath = Join-Path $vmFolder 'system-run.vhdx'
    dataDiskPath = Join-Path $vmFolder 'data.vhdx'
    credentialPath = Join-Path $LabRoot 'credential.xml'
    gpu = $Gpu
    edition = $null
    windowsVersion = $null
    guest = $null
    createdAt = (Get-Date).ToString('o')
}

$existing = Get-VM -Name $VmName -ErrorAction SilentlyContinue
if ($existing) {
    if (-not $Force) {
        throw "The VM $VmName already exists. Use -Force to delete and rebuild it."
    }
    Write-LabStep "Deleting the existing VM $VmName"
    if ($existing.State -ne 'Off') {
        Stop-VM -VM $existing -TurnOff -Force
    }
    Remove-VM -VM $existing -Force
}
foreach ($path in @($config.baseDiskPath, $config.systemDiskPath, $config.dataDiskPath)) {
    if (Test-Path -LiteralPath $path) {
        if (-not $Force) {
            throw "$path already exists. Use -Force to replace it."
        }
        Set-ItemProperty -LiteralPath $path -Name IsReadOnly -Value $false
        Remove-Item -LiteralPath $path -Force
    }
}
New-Item -ItemType Directory -Path $vmFolder -Force | Out-Null

$freeGB = (Get-PSDrive -Name $LabRoot.Substring(0, 1)).Free / 1GB
if ($freeGB -lt 40) {
    throw ("{0:N0} GB free on {1}; the lab needs about 40 GB." -f $freeGB, $LabRoot.Substring(0, 2))
}

$hostGpu = Get-VMHostPartitionableGpu | Where-Object { $_.Name -match $GpuVendors[$Gpu] } | Select-Object -First 1
if (-not $hostGpu) {
    $names = (Get-VMHostPartitionableGpu | ForEach-Object Name) -join '; '
    throw "The $Gpu GPU cannot be partitioned on this host. Partitionable GPUs: $names"
}

$password = New-LabPassword
$credential = [pscredential]::new(".\$HardwareLabGuestUser", (ConvertTo-SecureString $password -AsPlainText -Force))
$credential | Export-Clixml -LiteralPath $config.credentialPath
$unattend = (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'guest\unattend.xml') -Raw -Encoding UTF8).
    Replace('{{USER}}', $HardwareLabGuestUser).
    Replace('{{PASSWORD}}', $password)

$iso = Mount-DiskImage -ImagePath (Resolve-Path -LiteralPath $IsoPath).Path -PassThru
try {
    $isoLetter = ($iso | Get-Volume).DriveLetter
    $imagePath = @("${isoLetter}:\sources\install.wim", "${isoLetter}:\sources\install.esd") |
        Where-Object { Test-Path -LiteralPath $_ } |
        Select-Object -First 1
    if (-not $imagePath) {
        throw 'The ISO has no sources\install.wim or install.esd.'
    }
    $images = @(Get-WindowsImage -ImagePath $imagePath)
    $image = $images | Where-Object { $_.ImageName -match ('(^| )' + [regex]::Escape($Edition) + '$') } | Select-Object -First 1
    if (-not $image) {
        throw ("No image ends with '$Edition'. Images in the ISO: " + (($images | ForEach-Object ImageName) -join '; '))
    }
    $details = Get-WindowsImage -ImagePath $imagePath -Index $image.ImageIndex
    $config.edition = $details.ImageName
    $config.windowsVersion = $details.Version
    Write-LabStep "Using $($details.ImageName) $($details.Version)"
    New-BaseDisk -Path $config.baseDiskPath -ImagePath $imagePath -ImageIndex $image.ImageIndex -UnattendXml $unattend
}
finally {
    Dismount-DiskImage -ImagePath $iso.ImagePath | Out-Null
}
New-DataDisk -Path $config.dataDiskPath

Write-LabStep "Creating the VM $VmName"
New-VM -Name $VmName -Generation 2 -Path $LabRoot -MemoryStartupBytes 4GB -VHDPath $config.baseDiskPath | Out-Null
Get-VMNetworkAdapter -VMName $VmName | Remove-VMNetworkAdapter
Add-VMHardDiskDrive -VMName $VmName -Path $config.dataDiskPath
Set-VMFirmware -VMName $VmName -EnableSecureBoot On -SecureBootTemplate MicrosoftWindows
Set-VMKeyProtector -VMName $VmName -NewLocalKeyProtector
Enable-VMTPM -VMName $VmName
# GPU partitioning needs a turn-off stop action, guest-controlled caching and
# room for the GPU memory-mapped I/O.
Set-VM -Name $VmName `
    -AutomaticCheckpointsEnabled $false `
    -CheckpointType Disabled `
    -AutomaticStartAction Nothing `
    -AutomaticStopAction TurnOff `
    -GuestControlledCacheTypes $true `
    -LowMemoryMappedIoSpace 1GB `
    -HighMemoryMappedIoSpace 32GB
Add-VMGpuPartitionAdapter -VMName $VmName -InstancePath $hostGpu.Name
try {
    # Give the partition the whole range the host offers.
    Set-VMGpuPartitionAdapter -VMName $VmName `
        -MinPartitionVRAM $hostGpu.MinPartitionVRAM -MaxPartitionVRAM $hostGpu.MaxPartitionVRAM -OptimalPartitionVRAM $hostGpu.OptimalPartitionVRAM `
        -MinPartitionEncode $hostGpu.MinPartitionEncode -MaxPartitionEncode $hostGpu.MaxPartitionEncode -OptimalPartitionEncode $hostGpu.OptimalPartitionEncode `
        -MinPartitionDecode $hostGpu.MinPartitionDecode -MaxPartitionDecode $hostGpu.MaxPartitionDecode -OptimalPartitionDecode $hostGpu.OptimalPartitionDecode `
        -MinPartitionCompute $hostGpu.MinPartitionCompute -MaxPartitionCompute $hostGpu.MaxPartitionCompute -OptimalPartitionCompute $hostGpu.OptimalPartitionCompute
}
catch {
    Write-Warning "Kept the default GPU partition sizes: $($_.Exception.Message)"
}

# The first boot runs uncapped: it only prepares the base disk.
Set-LabVmProfile -Config $config -HardwareProfile $setupProfile
Set-LabVmDiskLimit -Config $config -Unlimited
Write-LabStep 'First boot: Windows finishes its setup and logs the lab user on (10-30 min)'
Start-VM -Name $VmName
$session = Wait-LabGuest -Config $config -Credential $credential -TimeoutSeconds 3600
try {
    Write-LabStep 'Preparing the guest session'
    $config.guest = Invoke-Command -Session $session -ScriptBlock {
        # Autologon from the answer file counts down; make it permanent.
        Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon' -Name AutoLogonCount -ErrorAction SilentlyContinue
        Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name ScreenSaveActive -Value 0
        $webview = Get-ItemProperty -Path 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue
        $os = Get-CimInstance Win32_OperatingSystem
        [pscustomobject] @{
            os = $os.Caption
            build = $os.BuildNumber
            dataDrive = (Get-Volume -FileSystemLabel 'LabData' -ErrorAction SilentlyContinue).DriveLetter
            webview2 = if ($webview) { $webview.pv } else { $null }
            videoControllers = @(Get-CimInstance Win32_VideoController | ForEach-Object {
                    [pscustomobject] @{ name = $_.Name; driverVersion = $_.DriverVersion; status = $_.Status }
                })
        }
    } | Select-Object os, build, dataDrive, webview2, videoControllers
    Write-LabStep 'Waiting for first-logon background work to settle'
    [void] (Wait-LabGuestQuiet -Session $session -TimeoutSeconds 1200)
}
finally {
    Remove-PSSession $session
}
Write-LabStep 'Shutting the base image down'
Stop-VM -Name $VmName -Force

Set-ItemProperty -LiteralPath $config.baseDiskPath -Name IsReadOnly -Value $true
New-VHD -Path $config.systemDiskPath -ParentPath $config.baseDiskPath -Differencing | Out-Null
Get-VMHardDiskDrive -VMName $VmName |
    Where-Object { $_.Path -eq $config.baseDiskPath } |
    Set-VMHardDiskDrive -Path $config.systemDiskPath
Set-LabVmProfile -Config $config -HardwareProfile $hardware
Save-LabConfig -LabRoot $LabRoot -Config $config

if ($GrantHyperVAdministrators) {
    $user = [Security.Principal.WindowsIdentity]::GetCurrent().Name
    $group = Get-LocalGroup -SID 'S-1-5-32-578'
    if (-not (Get-LocalGroupMember -Group $group | Where-Object { $_.Name -eq $user })) {
        Add-LocalGroupMember -Group $group -Member $user
        Write-LabStep "Added $user to $($group.Name); sign out and back in before the runs."
    }
}

Write-LabStep 'Lab ready'
$config.guest | Format-List | Out-Host
$gpuReady = $config.guest.videoControllers | Where-Object { $_.name -notmatch 'Hyper-V|Basic' -and $_.status -eq 'OK' }
if (-not $gpuReady) {
    Write-Warning 'The guest shows no working partitioned GPU; the app will refuse to draw the canvas.'
}
