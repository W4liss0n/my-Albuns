$ErrorActionPreference = 'Stop'
$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$reportRoot = Join-Path $workspaceRoot '.tools\validation'
New-Item -ItemType Directory -Force -Path $reportRoot | Out-Null
$steps = @(
    @{ name = 'frontend-build'; arguments = @('run', 'build') },
    @{ name = 'frontend-tests'; arguments = @('test') },
    @{ name = 'automation-tests'; arguments = @('run', 'test:automation') },
    @{ name = 'rust-quality'; arguments = @('run', 'quality:rust') },
    @{ name = 'rust-tests'; arguments = @('run', 'test:rust') }
)
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
$sourceBefore = Get-GateSourceSnapshot -WorkspaceRoot $workspaceRoot -EvidencePath (Join-Path $reportRoot 'report.json')
$results = @()
$passed = $false
$previousNativeProbeTests = $env:MYALBUNS_NATIVE_PROBE_TESTS
$env:MYALBUNS_NATIVE_PROBE_TESTS = '0'
Push-Location $workspaceRoot
try {
    foreach ($step in $steps) {
        $timer = [Diagnostics.Stopwatch]::StartNew()
        $logPath = Join-Path $reportRoot ($step.name + '.log')
        Write-Output ('Running ' + $step.name + ' (no visible windows)')
        $previousPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = 'Continue'
            & npm.cmd @($step.arguments) *> $logPath
            $stepExitCode = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = $previousPreference
        }
        $timer.Stop()
        $results += [ordered]@{
            name = $step.name
            exitCode = $stepExitCode
            durationSeconds = [math]::Round($timer.Elapsed.TotalSeconds, 2)
            log = $step.name + '.log'
        }
        if ($stepExitCode -ne 0) {
            Get-Content -LiteralPath $logPath -Tail 60
            throw ($step.name + ' failed. Full log: ' + $logPath)
        }
        Write-Output ($step.name + ' PASS (' + $results[-1].durationSeconds + 's)')
    }
    $passed = $true
}
finally {
    $sourceAfter = Get-GateSourceSnapshot -WorkspaceRoot $workspaceRoot -EvidencePath (Join-Path $reportRoot 'report.json')
    $report = [ordered]@{
        schemaVersion = 1
        gate = 'headless-validation'
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        gitCommit = $sourceBefore.gitCommit
        sourceInputsDirty = [bool] (Test-GateSourceSnapshotsDirty -Before $sourceBefore -After $sourceAfter)
        passed = $passed
        visibleNativeTestsInvoked = $false
        steps = $results
    }
    [IO.File]::WriteAllText(
        (Join-Path $reportRoot 'report.json'),
        ($report | ConvertTo-Json -Depth 6) + [Environment]::NewLine,
        [Text.UTF8Encoding]::new($false)
    )
    $env:MYALBUNS_NATIVE_PROBE_TESTS = $previousNativeProbeTests
    Pop-Location
}
