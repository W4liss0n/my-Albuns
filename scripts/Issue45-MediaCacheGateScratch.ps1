function Assert-Issue45OwnedOutputsAbsent([string[]] $Paths) {
    $existing = @(
        $Paths | Where-Object { Test-Path -LiteralPath $_ }
    )
    if ($existing.Count -ne 0) {
        throw "The issue 45 gate requires its output paths to be absent before the run: $($existing -join ', ')."
    }
}

function New-Issue45MediaCacheScratchScope([string] $WorkspaceRoot) {
    $workspace = [System.IO.Path]::GetFullPath($WorkspaceRoot)
    $scratchRoot = [System.IO.Path]::GetFullPath(
        (Join-Path $workspace '.scratch\cargo-target-tests\issue-45-media-cache')
    )
    $scratchContainer = [System.IO.Path]::GetDirectoryName($scratchRoot)
    $workspaceScratch = Join-Path $workspace '.scratch'
    $scope = [pscustomobject]@{
        WorkspaceRoot = $workspace
        ScratchRoot = $scratchRoot
        ScratchContainer = $scratchContainer
        ScratchContainerExisted = Test-Path -LiteralPath $scratchContainer
        ScratchRootExisted = Test-Path -LiteralPath $scratchRoot
        RunRoot = $null
        DistPath = Join-Path $workspace 'dist'
        PreparedSidecarPath = Join-Path `
            $workspace `
            'src-tauri\binaries\myalbuns-imaging-x86_64-pc-windows-msvc.exe'
        SharedCargoTarget = Join-Path $workspace 'target'
        StandaloneWindowsPathScratch = Join-Path $workspaceScratch 'windows-path-gate'
        WindowsPathScratch = $null
        GateTarget = $null
    }
    $scope | Add-Member -NotePropertyName OwnedOutputPreflightPaths -NotePropertyValue @(
        $scope.PreparedSidecarPath
        $scope.DistPath
        $scope.SharedCargoTarget
        $scope.ScratchContainer
    )
    Assert-Issue45OwnedOutputsAbsent -Paths $scope.OwnedOutputPreflightPaths
    return $scope
}

function Initialize-Issue45MediaCacheScratch([psobject] $Scope) {
    New-Item -ItemType Directory -Force -Path $Scope.ScratchRoot | Out-Null
    & git -C $Scope.WorkspaceRoot check-ignore --quiet -- $Scope.ScratchRoot
    if ($LASTEXITCODE -ne 0) {
        throw 'The issue 45 gate scratch root must be excluded from source provenance.'
    }
    $Scope.RunRoot = [System.IO.Path]::GetFullPath(
        (Join-Path `
            $Scope.ScratchRoot `
            "run-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 8))")
    )
    if (-not [string]::Equals(
            [System.IO.Path]::GetDirectoryName($Scope.RunRoot),
            $Scope.ScratchRoot,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'The issue 45 gate scratch directory escaped its approved root.'
    }
    New-Item -ItemType Directory -Force -Path $Scope.RunRoot | Out-Null
    $Scope.WindowsPathScratch = [System.IO.Path]::GetFullPath(
        (Join-Path $Scope.RunRoot 'windows-path-scratch')
    )
    if (-not [string]::Equals(
            [System.IO.Path]::GetDirectoryName($Scope.WindowsPathScratch),
            $Scope.RunRoot,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'The nested Windows path scratch escaped the issue 45 run root.'
    }
    $Scope.GateTarget = [System.IO.Path]::GetFullPath(
        (Join-Path $Scope.RunRoot 'cargo-target')
    )
    if (-not [string]::Equals(
            [System.IO.Path]::GetDirectoryName($Scope.GateTarget),
            $Scope.RunRoot,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'The issue 45 Cargo target escaped its approved run scratch.'
    }
    return $Scope
}

function Clear-Issue45GateOutputs([psobject] $Scope) {
    $cleanupFailures = [System.Collections.Generic.List[string]]::new()

    try {
        if (Test-Path -LiteralPath $Scope.PreparedSidecarPath -PathType Leaf) {
            [System.IO.File]::Delete($Scope.PreparedSidecarPath)
        }
    }
    catch {
        $cleanupFailures.Add("prepared sidecar: $($_.Exception.Message)")
    }

    try {
        if (Test-Path -LiteralPath $Scope.DistPath) {
            Remove-GateScratchDirectory `
                -Path $Scope.DistPath `
                -AllowedParent $Scope.WorkspaceRoot
        }
    }
    catch {
        $cleanupFailures.Add("frontend distribution: $($_.Exception.Message)")
    }

    try {
        if (-not [string]::IsNullOrWhiteSpace($Scope.RunRoot) -and
                (Test-Path -LiteralPath $Scope.RunRoot)) {
            Remove-GateScratchDirectory `
                -Path $Scope.RunRoot `
                -AllowedParent $Scope.ScratchRoot
        }
    }
    catch {
        $cleanupFailures.Add("run scratch: $($_.Exception.Message)")
    }

    try {
        if (-not $Scope.ScratchRootExisted -and
                (Test-Path -LiteralPath $Scope.ScratchRoot) -and
                @(Get-ChildItem -LiteralPath $Scope.ScratchRoot -Force).Count -eq 0) {
            [System.IO.Directory]::Delete($Scope.ScratchRoot)
        }
    }
    catch {
        $cleanupFailures.Add("scratch root: $($_.Exception.Message)")
    }

    try {
        if (-not $Scope.ScratchContainerExisted -and
                (Test-Path -LiteralPath $Scope.ScratchContainer) -and
                @(Get-ChildItem -LiteralPath $Scope.ScratchContainer -Force).Count -eq 0) {
            [System.IO.Directory]::Delete($Scope.ScratchContainer)
        }
    }
    catch {
        $cleanupFailures.Add("scratch container: $($_.Exception.Message)")
    }

    if ($cleanupFailures.Count -ne 0) {
        throw "The issue 45 gate could not clean all owned outputs: $($cleanupFailures -join '; ')"
    }
}

function New-IndependentWindowsPathScratchProbe([psobject] $Scope) {
    $mutex = [System.Threading.Mutex]::new(
        $false,
        'Local\MyAlbuns.WindowsPathGateEvidence.v1'
    )
    $mutexHeld = $false
    $rootExisted = $false
    $path = $null
    try {
        try {
            $mutexHeld = $mutex.WaitOne(0)
        }
        catch [System.Threading.AbandonedMutexException] {
            $mutexHeld = $true
        }
        if (-not $mutexHeld) {
            throw 'An independent Windows path gate is already active.'
        }
        $rootExisted = Test-Path -LiteralPath $Scope.StandaloneWindowsPathScratch
        New-Item `
            -ItemType Directory `
            -Force `
            -Path $Scope.StandaloneWindowsPathScratch |
            Out-Null
        $root = Get-Item -LiteralPath $Scope.StandaloneWindowsPathScratch -Force
        if (($root.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw 'The independent Windows path scratch probe refuses a reparse root.'
        }
        $path = Join-Path `
            $Scope.StandaloneWindowsPathScratch `
            "issue-45-independent-$PID-$([guid]::NewGuid().ToString('N')).bin"
        $bytes = [byte[]] (11, 23, 47, 89, 131, 197)
        $stream = [System.IO.File]::Open(
            $path,
            [System.IO.FileMode]::CreateNew,
            [System.IO.FileAccess]::Write,
            [System.IO.FileShare]::Read
        )
        try {
            $stream.Write($bytes, 0, $bytes.Length)
            $stream.Flush($true)
        }
        finally {
            $stream.Dispose()
        }
        return [pscustomobject]@{
            path = $path
            root = $Scope.StandaloneWindowsPathScratch
            rootExisted = $rootExisted
            sha256 = Get-GateFileSha256 -Path $path
        }
    }
    catch {
        if (-not [string]::IsNullOrWhiteSpace($path) -and
                (Test-Path -LiteralPath $path -PathType Leaf)) {
            [System.IO.File]::Delete($path)
        }
        if (-not $rootExisted -and
                (Test-Path `
                    -LiteralPath $Scope.StandaloneWindowsPathScratch `
                    -PathType Container) -and
                @(Get-ChildItem `
                    -LiteralPath $Scope.StandaloneWindowsPathScratch `
                    -Force).Count -eq 0) {
            [System.IO.Directory]::Delete($Scope.StandaloneWindowsPathScratch)
        }
        throw
    }
    finally {
        if ($mutexHeld) {
            $mutex.ReleaseMutex()
        }
        $mutex.Dispose()
    }
}

function Test-IndependentWindowsPathScratchProbe([object] $Probe) {
    if ($null -eq $Probe -or
            -not (Test-Path -LiteralPath $Probe.path -PathType Leaf) -or
            (Get-GateFileSha256 -Path $Probe.path) -ne $Probe.sha256) {
        throw 'The issue 45 cleanup changed an independent Windows path scratch sentinel.'
    }
    return 2
}

function Remove-IndependentWindowsPathScratchProbe([object] $Probe) {
    if ($null -eq $Probe) {
        return
    }
    if (Test-Path -LiteralPath $Probe.path -PathType Leaf) {
        [System.IO.File]::Delete($Probe.path)
    }
    if (-not $Probe.rootExisted) {
        $mutex = [System.Threading.Mutex]::new(
            $false,
            'Local\MyAlbuns.WindowsPathGateEvidence.v1'
        )
        $mutexHeld = $false
        try {
            try {
                $mutexHeld = $mutex.WaitOne(0)
            }
            catch [System.Threading.AbandonedMutexException] {
                $mutexHeld = $true
            }
            if ($mutexHeld -and
                    (Test-Path -LiteralPath $Probe.root -PathType Container) -and
                    @(Get-ChildItem -LiteralPath $Probe.root -Force).Count -eq 0) {
                [System.IO.Directory]::Delete($Probe.root)
            }
        }
        finally {
            if ($mutexHeld) {
                $mutex.ReleaseMutex()
            }
            $mutex.Dispose()
        }
    }
}

function Test-Issue45OwnedOutputPreflightContracts([psobject] $Scope) {
    $sentinelFile = Join-Path $Scope.RunRoot 'preexisting-output.bin'
    $sentinelDirectory = Join-Path $Scope.RunRoot 'preexisting-output-directory'
    $sentinelChild = Join-Path $sentinelDirectory 'sentinel.bin'
    $missing = Join-Path $Scope.RunRoot 'absent-output'
    New-Item -ItemType Directory -Path $sentinelDirectory | Out-Null
    [System.IO.File]::WriteAllBytes($sentinelFile, [byte[]] (1, 3, 5, 7))
    [System.IO.File]::WriteAllBytes($sentinelChild, [byte[]] (2, 4, 6, 8))
    $fileHash = Get-GateFileSha256 -Path $sentinelFile
    $childHash = Get-GateFileSha256 -Path $sentinelChild
    $rejected = $false
    try {
        Assert-Issue45OwnedOutputsAbsent -Paths @(
            $sentinelFile
            $sentinelDirectory
            $missing
        )
    }
    catch {
        if ($_.Exception.Message -notlike
                'The issue 45 gate requires its output paths to be absent*') {
            throw
        }
        $rejected = $true
    }
    if (-not $rejected -or
            (Get-GateFileSha256 -Path $sentinelFile) -ne $fileHash -or
            (Get-GateFileSha256 -Path $sentinelChild) -ne $childHash) {
        throw 'The output preflight did not reject and preserve its byte sentinels.'
    }
    Assert-Issue45OwnedOutputsAbsent -Paths @($missing)
    return 4
}
