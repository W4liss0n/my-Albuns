[CmdletBinding()]
param(
    # Processor limits to try, as Hyper-V's per-VM maximum percentage.
    [int[]] $ProcessorLimitPercent = @(25, 100),
    [int] $ProcessorCount = 2,
    # Memory comes from this profile; processors from the parameters above.
    [string] $HardwareProfile = 'cliente-ssd',
    [string] $LabRoot = 'D:\HardwareLab'
)

# Whether two CPU-bound processes in the guest finish in the time of one, i.e.
# whether its virtual processors add up like real cores under a given limit.
# Each process hashes the same buffer for a fixed amount of work; the result is
# the time of one process alone and of two at once.
. (Join-Path $PSScriptRoot 'HardwareLab.Common.ps1')
Assert-LabHyperVAccess
$config = Get-LabConfig -LabRoot $LabRoot
$credential = Get-LabCredential -Config $config
$work = {
    $buffer = [byte[]]::new(64MB)
    $hash = [Security.Cryptography.SHA256]::Create()
    $watch = [Diagnostics.Stopwatch]::StartNew()
    # About 20 s on the host: Hyper-V averages the limit over time, so a short
    # burst would run at full speed under any limit.
    for ($index = 0; $index -lt 600; $index++) { [void] $hash.ComputeHash($buffer) }
    # Whole milliseconds: the guest's culture may print decimals with a comma.
    $watch.ElapsedMilliseconds
}
foreach ($limit in $ProcessorLimitPercent) {
    # Each boot starts from the golden image, as a lab run does: a disk left by
    # an earlier boot may not reach the desktop again.
    Stop-LabVm -Config $config
    Reset-LabSystemDisk -Config $config
    $hardware = Get-LabProfile -Name $HardwareProfile
    $hardware.processorCount = $ProcessorCount
    $hardware.processorLimitPercent = $limit
    Set-LabVmProfile -Config $config -HardwareProfile $hardware
    Set-LabVmDiskLimit -Config $config -Unlimited
    Start-VM -Name $config.vmName
    $session = Wait-LabGuest -Config $config -Credential $credential
    try {
        [void] (Wait-LabGuestQuiet -Session $session)
        $result = Invoke-Command -Session $session -ArgumentList $work.ToString() -ScriptBlock {
            param($work)
            $run = {
                param($count)
                $jobs = 1..$count | ForEach-Object {
                    Start-Process powershell.exe -ArgumentList '-NoProfile', '-Command', $work -NoNewWindow -PassThru -RedirectStandardOutput "$env:TEMP\cpu-$_.txt"
                }
                $jobs | Wait-Process
                1..$count | ForEach-Object { [long] (Get-Content "$env:TEMP\cpu-$_.txt" | Select-Object -Last 1) / 1000 }
            }
            [pscustomobject] @{
                alone = (& $run 1)
                together = (& $run 2)
            }
        }
        $alone = $result.alone | Measure-Object -Average
        $together = $result.together | Measure-Object -Average
        [pscustomobject] @{
            limitPercent = $limit
            aloneSeconds = [math]::Round($alone.Average, 2)
            togetherSeconds = [math]::Round($together.Average, 2)
            # 1.0: two processes add up like two cores; 0.5: no gain at all.
            scaling = [math]::Round($alone.Average / $together.Average, 2)
        }
    }
    finally {
        Remove-PSSession $session -ErrorAction SilentlyContinue
        Stop-LabVm -Config $config
    }
}
