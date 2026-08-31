$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')

$workspaceRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$scratchRoot = [System.IO.Path]::GetFullPath((Join-Path $workspaceRoot '.scratch'))
$fixtureRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $scratchRoot "gate-source-provenance-$PID-$([Guid]::NewGuid().ToString('N'))")
)

if (-not [string]::Equals(
        [System.IO.Path]::GetDirectoryName($fixtureRoot),
        $scratchRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The provenance fixture escaped the workspace scratch root.'
}

try {
    New-Item -ItemType Directory -Force -Path $fixtureRoot | Out-Null
    $artifactDirectory = Join-Path $fixtureRoot 'docs\research\artifacts'
    New-Item -ItemType Directory -Force -Path $artifactDirectory | Out-Null
    [System.IO.File]::WriteAllText(
        (Join-Path $fixtureRoot 'vite.config.ts'),
        "export default {};`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    $evidencePath = Join-Path $artifactDirectory 'gate.json'
    [System.IO.File]::WriteAllText(
        $evidencePath,
        "{}`n",
        [System.Text.UTF8Encoding]::new($false)
    )

    & git -C $fixtureRoot init --quiet
    & git -C $fixtureRoot add -- .
    & git `
        -C $fixtureRoot `
        -c user.name='MyAlbuns Gate Test' `
        -c user.email='gate-test@myalbuns.invalid' `
        commit --quiet -m 'fixture'
    if ($LASTEXITCODE -ne 0) {
        throw 'The provenance fixture could not be committed.'
    }

    [System.IO.File]::WriteAllText(
        $evidencePath,
        "{`"generated`":true}`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    $evidenceOnlyStatus = @(
        Get-GateSourceStatus `
            -WorkspaceRoot $fixtureRoot `
            -EvidencePath $evidencePath
    )
    if ($evidenceOnlyStatus.Count -ne 0) {
        throw "Generated evidence was misclassified as a source input: $evidenceOnlyStatus"
    }

    $retainedEvidenceRoot = Join-Path `
        $fixtureRoot `
        '.scratch\focused-owned-dialog-evidence'
    foreach ($previousRun in @('previous-failure', 'previous-success')) {
        $previousRunRoot = Join-Path $retainedEvidenceRoot $previousRun
        New-Item -ItemType Directory -Force -Path $previousRunRoot | Out-Null
        [System.IO.File]::WriteAllText(
            (Join-Path $previousRunRoot 'evidence.json'),
            "{`"retained`":true}`n",
            [System.Text.UTF8Encoding]::new($false)
        )
    }
    $retainedEvidenceSnapshot = Get-GateSourceSnapshot `
        -WorkspaceRoot $fixtureRoot `
        -EvidencePath $evidencePath `
        -RetainedEvidenceRoot $retainedEvidenceRoot
    if ($retainedEvidenceSnapshot.sourceInputsDirty) {
        throw 'Prior evidence from the same gate dirtied its source inputs.'
    }

    $unrelatedOutputRoot = Join-Path $fixtureRoot '.scratch\unrelated-output'
    New-Item -ItemType Directory -Force -Path $unrelatedOutputRoot | Out-Null
    [System.IO.File]::WriteAllText(
        (Join-Path $unrelatedOutputRoot 'untracked.json'),
        "{`"unrelated`":true}`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    $unrelatedOutputStatus = @(
        Get-GateSourceStatus `
            -WorkspaceRoot $fixtureRoot `
            -EvidencePath $evidencePath `
            -RetainedEvidenceRoot $retainedEvidenceRoot
    )
    $unrelatedOutputSnapshot = Get-GateSourceSnapshot `
        -WorkspaceRoot $fixtureRoot `
        -EvidencePath $evidencePath `
        -RetainedEvidenceRoot $retainedEvidenceRoot
    if ($unrelatedOutputStatus.Count -ne 1 `
            -or $unrelatedOutputStatus[0] -notmatch '\?\? \.scratch/unrelated-output/untracked\.json$' `
            -or -not $unrelatedOutputSnapshot.sourceInputsDirty) {
        throw "An unrelated untracked output was hidden by the evidence exclusion: $unrelatedOutputStatus"
    }
    Remove-GateScratchDirectory `
        -Path $unrelatedOutputRoot `
        -AllowedParent (Split-Path -Parent $unrelatedOutputRoot)

    $escapedRootFailure = $null
    try {
        Get-GateSourceStatus `
            -WorkspaceRoot $fixtureRoot `
            -EvidencePath $evidencePath `
            -RetainedEvidenceRoot $scratchRoot |
            Out-Null
    }
    catch {
        $escapedRootFailure = $_.Exception.Message
    }
    if ($escapedRootFailure -notmatch 'inside the Git worktree') {
        throw 'A retained evidence root escaping the fixture did not fail closed.'
    }

    $junctionTarget = Join-Path `
        $scratchRoot `
        "gate-source-provenance-link-target-$PID-$([Guid]::NewGuid().ToString('N'))"
    $junctionPath = Join-Path $retainedEvidenceRoot 'linked-run'
    try {
        New-Item -ItemType Directory -Force -Path $junctionTarget | Out-Null
        [System.IO.File]::WriteAllText(
            (Join-Path $junctionTarget 'outside.json'),
            "{`"outside`":true}`n",
            [System.Text.UTF8Encoding]::new($false)
        )
        New-Item `
            -ItemType Junction `
            -Path $junctionPath `
            -Target $junctionTarget |
            Out-Null
        $junctionFailure = $null
        try {
            Get-GateSourceStatus `
                -WorkspaceRoot $fixtureRoot `
                -EvidencePath $evidencePath `
                -RetainedEvidenceRoot $retainedEvidenceRoot |
                Out-Null
        }
        catch {
            $junctionFailure = $_.Exception.Message
        }
        if ($junctionFailure -notmatch 'reparse point') {
            throw 'A junction below the retained evidence root did not fail closed.'
        }
    }
    finally {
        if (Test-Path -LiteralPath $junctionPath) {
            [System.IO.Directory]::Delete($junctionPath)
        }
        Remove-GateScratchDirectory `
            -Path $junctionTarget `
            -AllowedParent $scratchRoot
    }
    Remove-GateScratchDirectory `
        -Path $retainedEvidenceRoot `
        -AllowedParent (Split-Path -Parent $retainedEvidenceRoot)

    [System.IO.File]::WriteAllText(
        (Join-Path $fixtureRoot 'vite.config.ts'),
        "export default { base: '/dirty-root-input/' };`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    $dirtySourceStatus = @(
        Get-GateSourceStatus `
            -WorkspaceRoot $fixtureRoot `
            -EvidencePath $evidencePath
    )
    if ($dirtySourceStatus.Count -ne 1 `
            -or $dirtySourceStatus[0] -notmatch 'vite\.config\.ts$') {
        throw "A tracked root build input was not detected: $dirtySourceStatus"
    }

    [System.IO.File]::WriteAllText(
        (Join-Path $fixtureRoot 'vite.config.ts'),
        "export default {};`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    $untrackedSourceDirectory = Join-Path $fixtureRoot 'src'
    New-Item -ItemType Directory -Force -Path $untrackedSourceDirectory | Out-Null
    [System.IO.File]::WriteAllText(
        (Join-Path $untrackedSourceDirectory 'new-entry.ts'),
        "export const newEntry = true;`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    $untrackedSourceStatus = @(
        Get-GateSourceStatus `
            -WorkspaceRoot $fixtureRoot `
            -EvidencePath $evidencePath
    )
    if ($untrackedSourceStatus.Count -ne 1 `
            -or $untrackedSourceStatus[0] -notmatch '\?\? src/new-entry\.ts$') {
        throw "An untracked source input was not detected: $untrackedSourceStatus"
    }

    Remove-Item -LiteralPath $untrackedSourceDirectory -Recurse -Force
    $beforeSnapshot = Get-GateSourceSnapshot `
        -WorkspaceRoot $fixtureRoot `
        -EvidencePath $evidencePath
    if (Test-GateSourceSnapshotsDirty `
            -Before $beforeSnapshot `
            -After $beforeSnapshot) {
        throw 'An unchanged clean source snapshot was reported as dirty.'
    }
    [System.IO.File]::WriteAllText(
        (Join-Path $fixtureRoot 'vite.config.ts'),
        "export default { base: '/new-commit/' };`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    & git -C $fixtureRoot add -- vite.config.ts
    & git `
        -C $fixtureRoot `
        -c user.name='MyAlbuns Gate Test' `
        -c user.email='gate-test@myalbuns.invalid' `
        commit --quiet -m 'move fixture head'
    if ($LASTEXITCODE -ne 0) {
        throw 'The provenance fixture could not move HEAD.'
    }
    $afterSnapshot = Get-GateSourceSnapshot `
        -WorkspaceRoot $fixtureRoot `
        -EvidencePath $evidencePath
    if (-not (Test-GateSourceSnapshotsDirty `
                -Before $beforeSnapshot `
                -After $afterSnapshot)) {
        throw 'A HEAD change during a gate was not detected.'
    }

    $runnerOutputRoot = Join-Path $fixtureRoot '.scratch\runner-output'
    New-Item -ItemType Directory -Force -Path $runnerOutputRoot | Out-Null
    [System.IO.File]::WriteAllText(
        (Join-Path $runnerOutputRoot 'process-evidence.json'),
        "{`"temporary`":true}`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    $dirtyRunnerOutput = Get-GateSourceSnapshot `
        -WorkspaceRoot $fixtureRoot `
        -EvidencePath $evidencePath
    if (-not $dirtyRunnerOutput.sourceInputsDirty) {
        throw 'An untracked runner output was incorrectly reported as a clean input tree.'
    }
    $cleanupAttempts = [System.Collections.Generic.List[int]]::new()
    $transientCleanup = {
        param([string] $Candidate)
        $cleanupAttempts.Add(1)
        if ($cleanupAttempts.Count -lt 3) {
            throw 'simulated transient WebView file lock'
        }
        Remove-Item -LiteralPath $Candidate -Recurse -Force -ErrorAction Stop
    }
    Remove-GateScratchDirectory `
        -Path $runnerOutputRoot `
        -AllowedParent (Split-Path -Parent $runnerOutputRoot) `
        -MaximumAttempts 3 `
        -RetryDelayMilliseconds 0 `
        -RemoveOperation $transientCleanup
    if ($cleanupAttempts.Count -ne 3) {
        throw 'Gate scratch cleanup did not retry a transient file lock.'
    }

    $processProbeAttempts = [System.Collections.Generic.List[int]]::new()
    Wait-GatePathProcessesExit `
        -Path $runnerOutputRoot `
        -MaximumAttempts 3 `
        -RetryDelayMilliseconds 0 `
        -GetProcessesOperation {
            param([string] $Candidate)
            $processProbeAttempts.Add(1)
            if ($processProbeAttempts.Count -lt 3) {
                return @([pscustomobject]@{ ProcessId = 42; Path = $Candidate })
            }
            return @()
        }
    if ($processProbeAttempts.Count -ne 3) {
        throw 'Gate process cleanup did not await a transient scratch-bound process.'
    }

    $persistentProcessFailure = $null
    try {
        Wait-GatePathProcessesExit `
            -Path $runnerOutputRoot `
            -MaximumAttempts 2 `
            -RetryDelayMilliseconds 0 `
            -GetProcessesOperation {
                return @([pscustomobject]@{ ProcessId = 84 })
            }
    }
    catch {
        $persistentProcessFailure = $_.Exception.Message
    }
    if ($persistentProcessFailure -notmatch 'remained alive after 2 observations') {
        throw 'Gate process cleanup did not fail closed after exhausting observations.'
    }

    $persistentCleanupRoot = Join-Path $fixtureRoot '.scratch\persistent-lock'
    New-Item -ItemType Directory -Force -Path $persistentCleanupRoot | Out-Null
    $persistentFailure = $null
    try {
        Remove-GateScratchDirectory `
            -Path $persistentCleanupRoot `
            -AllowedParent (Split-Path -Parent $persistentCleanupRoot) `
            -MaximumAttempts 2 `
            -RetryDelayMilliseconds 0 `
            -RemoveOperation { throw 'simulated persistent WebView file lock' }
    }
    catch {
        $persistentFailure = $_.Exception.Message
    }
    if ($persistentFailure -notmatch 'failed after 2 attempts' `
            -or -not (Test-Path -LiteralPath $persistentCleanupRoot)) {
        throw 'Gate scratch cleanup did not fail closed after exhausting retries.'
    }
    Remove-GateScratchDirectory `
        -Path $persistentCleanupRoot `
        -AllowedParent (Split-Path -Parent $persistentCleanupRoot)
    $cleanAfterRunnerCleanup = Get-GateSourceSnapshot `
        -WorkspaceRoot $fixtureRoot `
        -EvidencePath $evidencePath
    if ($cleanAfterRunnerCleanup.sourceInputsDirty) {
        throw 'A runner-shaped cleanup did not restore a clean evidence input tree.'
    }

    Write-Output 'Gate source provenance: 15 assertions passed.'
}
finally {
    Remove-GateScratchDirectory `
        -Path $fixtureRoot `
        -AllowedParent $scratchRoot
}
