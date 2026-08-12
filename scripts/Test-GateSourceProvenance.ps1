$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')

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
    Remove-Item -LiteralPath $runnerOutputRoot -Recurse -Force
    $cleanAfterRunnerCleanup = Get-GateSourceSnapshot `
        -WorkspaceRoot $fixtureRoot `
        -EvidencePath $evidencePath
    if ($cleanAfterRunnerCleanup.sourceInputsDirty) {
        throw 'A runner-shaped cleanup did not restore a clean evidence input tree.'
    }

    Write-Output 'Gate source provenance: 7 assertions passed.'
}
finally {
    if (Test-Path -LiteralPath $fixtureRoot) {
        $verifiedFixtureRoot = [System.IO.Path]::GetFullPath($fixtureRoot)
        if (-not [string]::Equals(
                [System.IO.Path]::GetDirectoryName($verifiedFixtureRoot),
                $scratchRoot,
                [System.StringComparison]::OrdinalIgnoreCase
            )) {
            throw 'Refusing to remove an unverified provenance fixture.'
        }
        Remove-Item -LiteralPath $verifiedFixtureRoot -Recurse -Force
    }
}
