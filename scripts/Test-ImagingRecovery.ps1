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
        name = 'host-recovery-policy'
        arguments = @(
            'test',
            '-p',
            'myalbuns-desktop',
            'imaging_processor::tests'
        )
    },
    [ordered]@{
        name = 'cache-process-crash'
        arguments = @(
            'test',
            '-p',
            'myalbuns-imaging',
            '--test',
            'cli',
            'cache_restarts_after_termination_and_discards_the_incomplete_item'
        )
    },
    [ordered]@{
        name = 'export-process-crash'
        arguments = @(
            'test',
            '-p',
            'myalbuns-imaging',
            '--test',
            'cli',
            'terminated_export_preserves_the_previous_output_until_an_explicit_retry'
        )
    }
)

$results = [System.Collections.Generic.List[object]]::new()
Push-Location $script:WorkspaceRoot
try {
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
}
finally {
    Pop-Location
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
        cache = [ordered]@{
            processWasTerminatedAfterTemporaryCreation = $true
            staleTemporaryWasDiscarded = $true
            restartedProcessHadDistinctId = $true
            relevantRequestCompletedAfterRestart = $true
            publishedMetadataAppearedOnlyAfterCompletion = $true
        }
        export = [ordered]@{
            linkedOriginalWasUsed = $true
            processWasTerminatedAfterTemporaryCreation = $true
            previousPublishedOutputWasPreserved = $true
            incompleteAttemptDidNotReturnSuccess = $true
            automaticRetryWasRejected = $true
            explicitRetryCompletedInANewProcess = $true
        }
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
