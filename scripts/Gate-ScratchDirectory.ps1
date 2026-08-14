function Remove-GateScratchDirectory {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path,

        [Parameter(Mandatory = $true)]
        [string] $AllowedParent,

        [ValidateRange(1, 100)]
        [int] $MaximumAttempts = 50,

        [ValidateRange(0, 1000)]
        [int] $RetryDelayMilliseconds = 100,

        [scriptblock] $RemoveOperation = {
            param([string] $Candidate)
            Remove-Item -LiteralPath $Candidate -Recurse -Force -ErrorAction Stop
        }
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }
    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $resolvedAllowedParent = (Resolve-Path -LiteralPath $AllowedParent).Path
    if (-not [string]::Equals(
            [System.IO.Path]::GetDirectoryName($resolvedPath),
            $resolvedAllowedParent,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'Refusing to remove a gate scratch directory outside its allowed parent.'
    }

    $lastFailure = $null
    for ($attempt = 1; $attempt -le $MaximumAttempts; $attempt++) {
        try {
            & $RemoveOperation $resolvedPath
            if (-not (Test-Path -LiteralPath $resolvedPath)) {
                return
            }
            $lastFailure = 'the directory still exists after the removal operation'
        }
        catch {
            $lastFailure = $_.Exception.Message
        }
        if ($attempt -lt $MaximumAttempts -and $RetryDelayMilliseconds -gt 0) {
            Start-Sleep -Milliseconds $RetryDelayMilliseconds
        }
    }

    throw "Gate scratch cleanup failed after $MaximumAttempts attempts: $lastFailure"
}

function Wait-GatePathProcessesExit {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path,

        [ValidateRange(1, 100)]
        [int] $MaximumAttempts = 50,

        [ValidateRange(0, 1000)]
        [int] $RetryDelayMilliseconds = 100,

        [scriptblock] $GetProcessesOperation = {
            param([string] $Candidate)
            return @(
                Get-CimInstance Win32_Process |
                    Where-Object {
                        -not [string]::IsNullOrWhiteSpace($_.CommandLine) -and
                        $_.CommandLine.IndexOf(
                            $Candidate,
                            [System.StringComparison]::OrdinalIgnoreCase
                        ) -ge 0
                    }
            )
        }
    )

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    for ($attempt = 1; $attempt -le $MaximumAttempts; $attempt++) {
        $remainingProcesses = @(& $GetProcessesOperation $resolvedPath)
        if ($remainingProcesses.Count -eq 0) {
            return
        }
        if ($attempt -lt $MaximumAttempts -and $RetryDelayMilliseconds -gt 0) {
            Start-Sleep -Milliseconds $RetryDelayMilliseconds
        }
    }

    throw "Gate path processes remained alive after $MaximumAttempts observations."
}
