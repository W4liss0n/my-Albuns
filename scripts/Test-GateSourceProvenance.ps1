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
        Get-TrackedGateSourceStatus `
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
        Get-TrackedGateSourceStatus `
            -WorkspaceRoot $fixtureRoot `
            -EvidencePath $evidencePath
    )
    if ($dirtySourceStatus.Count -ne 1 `
            -or $dirtySourceStatus[0] -notmatch 'vite\.config\.ts$') {
        throw "A tracked root build input was not detected: $dirtySourceStatus"
    }

    Write-Output 'Gate source provenance: 2 assertions passed.'
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
