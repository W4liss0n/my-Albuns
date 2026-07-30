param([string] $OutputPath)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0004-imaging-recovery.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

$checks = @(
    [ordered]@{
        name = 'protocol'
        arguments = @(
            'test',
            '-p',
            'myalbuns-imaging-protocol',
            '--test',
            'protocol'
        )
    },
    [ordered]@{
        name = 'cache-temporary-cleanup'
        arguments = @(
            'test',
            '-p',
            'myalbuns-paths',
            '--test',
            'app_paths',
            'discards_only_cache_temporaries_left_by_a_terminated_processor'
        )
    },
    [ordered]@{
        name = 'imaging-sidecar-build'
        arguments = @(
            'build',
            '-p',
            'myalbuns-imaging'
        )
    },
    [ordered]@{
        name = 'production-recovery-integration'
        arguments = @(
            'test',
            '-p',
            'myalbuns-desktop',
            '--lib',
            'imaging_recovery_integration::real_processor_recovery_flows_through_production_modules',
            '--',
            '--ignored',
            '--exact',
            '--nocapture'
        )
    }
)

$scratchRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot '.scratch')
)
$evidenceDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path `
        $scratchRoot `
        "imaging-recovery-evidence-$PID-$([DateTime]::UtcNow.Ticks)")
)
$evidenceParent = [System.IO.Path]::GetDirectoryName($evidenceDirectory)
if (-not [string]::Equals(
        $evidenceParent,
        $scratchRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The recovery evidence directory escaped the workspace scratch root.'
}

New-Item -ItemType Directory -Force -Path $evidenceDirectory | Out-Null
$evidenceEnvironmentName = 'MYALBUNS_RECOVERY_EVIDENCE_DIR'
$processorEnvironmentName = 'MYALBUNS_REAL_IMAGING_PROCESSOR'
$processorPath = Join-Path $script:WorkspaceRoot 'target\debug\myalbuns-imaging.exe'
$previousEvidenceDirectory = [System.Environment]::GetEnvironmentVariable(
    $evidenceEnvironmentName,
    [System.EnvironmentVariableTarget]::Process
)
$previousProcessorPath = [System.Environment]::GetEnvironmentVariable(
    $processorEnvironmentName,
    [System.EnvironmentVariableTarget]::Process
)
[System.Environment]::SetEnvironmentVariable(
    $evidenceEnvironmentName,
    $evidenceDirectory,
    [System.EnvironmentVariableTarget]::Process
)
[System.Environment]::SetEnvironmentVariable(
    $processorEnvironmentName,
    $processorPath,
    [System.EnvironmentVariableTarget]::Process
)

$results = [System.Collections.Generic.List[object]]::new()
$cacheEvidence = $null
$exportEvidence = $null
$locationWasPushed = $false
try {
    Push-Location $script:WorkspaceRoot
    $locationWasPushed = $true
    foreach ($check in $checks) {
        $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        & $script:CargoExecutable @($check.arguments)
        $exitCode = $LASTEXITCODE
        $stopwatch.Stop()
        if ($exitCode -ne 0) {
            throw "Imaging recovery check '$($check.name)' failed with exit code $exitCode."
        }
        $results.Add([ordered]@{
            name = $check.name
            passed = $true
            elapsedMs = $stopwatch.ElapsedMilliseconds
        })
    }

    $cacheEvidencePath = Join-Path $evidenceDirectory 'cache.json'
    $exportEvidencePath = Join-Path $evidenceDirectory 'export.json'
    if (-not (Test-Path -LiteralPath $cacheEvidencePath -PathType Leaf)) {
        throw 'The real Cache crash test did not produce evidence.'
    }
    if (-not (Test-Path -LiteralPath $exportEvidencePath -PathType Leaf)) {
        throw 'The real Export crash test did not produce evidence.'
    }
    $cacheEvidence = Get-Content -LiteralPath $cacheEvidencePath -Raw | ConvertFrom-Json
    $exportEvidence = Get-Content -LiteralPath $exportEvidencePath -Raw | ConvertFrom-Json

    if ($cacheEvidence.failedProcessId -eq $cacheEvidence.restartedProcessId `
            -or -not $cacheEvidence.temporaryObservedAfterFailure `
            -or $cacheEvidence.removedTemporaryCount -lt 1 `
            -or $cacheEvidence.temporaryExistedAfterCleanup `
            -or $cacheEvidence.metadataExistedAfterFailure `
            -or -not $cacheEvidence.metadataExistedAfterRestart `
            -or $cacheEvidence.generatedCountAfterRestart -lt 1) {
        throw 'The observed Cache recovery evidence does not satisfy the gate.'
    }
    if ($exportEvidence.failedProcessId -eq $exportEvidence.retryProcessId `
            -or $exportEvidence.sourcePolicy -ne 'linkedOriginals' `
            -or $exportEvidence.processCountBeforeExplicitRetry -ne 1 `
            -or $exportEvidence.successResponseBeforeExplicitRetry `
            -or -not $exportEvidence.partialPreparationObserved `
            -or $exportEvidence.previousOutputSha256BeforeFailure `
                -ne $exportEvidence.previousOutputSha256AfterFailure `
            -or $exportEvidence.projectSha256BeforeFailure `
                -ne $exportEvidence.projectSha256AfterFailure `
            -or $exportEvidence.finalOutputSha256AfterExplicitRetry `
                -eq $exportEvidence.previousOutputSha256BeforeFailure) {
        throw 'The observed Export recovery evidence does not satisfy the gate.'
    }
}
finally {
    if ($locationWasPushed) {
        Pop-Location
    }
    [System.Environment]::SetEnvironmentVariable(
        $evidenceEnvironmentName,
        $previousEvidenceDirectory,
        [System.EnvironmentVariableTarget]::Process
    )
    [System.Environment]::SetEnvironmentVariable(
        $processorEnvironmentName,
        $previousProcessorPath,
        [System.EnvironmentVariableTarget]::Process
    )
    if (Test-Path -LiteralPath $evidenceDirectory) {
        $verifiedEvidenceDirectory = [System.IO.Path]::GetFullPath($evidenceDirectory)
        $verifiedEvidenceParent = [System.IO.Path]::GetDirectoryName(
            $verifiedEvidenceDirectory
        )
        if (-not [string]::Equals(
                $verifiedEvidenceParent,
                $scratchRoot,
                [System.StringComparison]::OrdinalIgnoreCase
            )) {
            throw 'Refusing to remove an unverified recovery evidence directory.'
        }
        Remove-Item -LiteralPath $verifiedEvidenceDirectory -Recurse -Force
    }
}

$sourceStatus = @(
    & git status --porcelain -- `
        Cargo.toml `
        Cargo.lock `
        crates `
        package.json `
        package-lock.json `
        scripts `
        src `
        src-tauri `
        tests
)
$report = [ordered]@{
    schemaVersion = 1
    collectedAtUtc = [DateTime]::UtcNow.ToString('o')
    gitCommit = (& git rev-parse HEAD).Trim()
    sourceInputsDirty = $sourceStatus.Count -gt 0
    platform = [ordered]@{
        operatingSystem = [System.Environment]::OSVersion.VersionString
        architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    }
    checks = @($results)
    evidence = [ordered]@{
        cache = $cacheEvidence
        export = $exportEvidence
    }
}
$json = $report | ConvertTo-Json -Depth 6
$outputDirectory = Split-Path -Parent $OutputPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
[System.IO.File]::WriteAllText(
    $OutputPath,
    $json + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)
Write-Output "Imaging recovery report: $OutputPath"
Write-Output $json
