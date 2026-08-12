$ErrorActionPreference = 'Stop'

$workspaceRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$recoveryResearchPath = Join-Path `
    $workspaceRoot `
    'docs\research\0010-recuperacao-do-processador-de-imagens.md'
$pathsResearchPath = Join-Path `
    $workspaceRoot `
    'docs\research\0016-caminhos-windows-identidade-e-unc.md'
$recoveryArtifactPath = Join-Path `
    $workspaceRoot `
    'docs\research\artifacts\0004-imaging-recovery.json'
$pathsArtifactPath = Join-Path `
    $workspaceRoot `
    'docs\research\artifacts\0008-windows-path-gate.json'
$protocolSourcePath = Join-Path `
    $workspaceRoot `
    'crates\myalbuns-imaging-protocol\src\lib.rs'

$recoveryResearch = Get-Content -LiteralPath $recoveryResearchPath -Raw -Encoding UTF8
$pathsResearch = Get-Content -LiteralPath $pathsResearchPath -Raw -Encoding UTF8
$recoveryArtifact = Get-Content -LiteralPath $recoveryArtifactPath -Raw -Encoding UTF8 `
    | ConvertFrom-Json
$pathsArtifact = Get-Content -LiteralPath $pathsArtifactPath -Raw -Encoding UTF8 `
    | ConvertFrom-Json
$protocolSource = Get-Content -LiteralPath $protocolSourcePath -Raw -Encoding UTF8
$protocolMatch = [regex]::Match(
    $protocolSource,
    'IMAGING_PROTOCOL_VERSION:\s*u32\s*=\s*(\d+)'
)
if (-not $protocolMatch.Success) {
    throw 'The Imaging protocol version could not be read from its source.'
}
$protocolVersion = [int] $protocolMatch.Groups[1].Value

$expectations = @(
    [ordered]@{
        name = 'imaging recovery'
        research = $recoveryResearch
        artifact = $recoveryArtifact
        requiredText = @(
            "$(@($recoveryArtifact.checks).Count) verifica",
            "protocolo v$protocolVersion",
            'npm run test:imaging-recovery'
        )
    },
    [ordered]@{
        name = 'Windows paths'
        research = $pathsResearch
        artifact = $pathsArtifact
        requiredText = @(
            "$(@($pathsArtifact.checks).Count) checks",
            "protocolo $protocolVersion"
        )
    }
)

$assertionCount = 0
foreach ($expectation in $expectations) {
    foreach ($requiredText in $expectation.requiredText) {
        if (-not $expectation.research.Contains($requiredText)) {
            throw "The $($expectation.name) research does not identify '$requiredText'."
        }
        $assertionCount++
    }

    $recordedCommits = @(
        [regex]::Matches($expectation.research, '(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])') `
            | ForEach-Object Value `
            | Sort-Object -Unique
    )
    $divergentCommits = @(
        $recordedCommits `
            | Where-Object { $_ -ne $expectation.artifact.gitCommit }
    )
    if ($divergentCommits.Count -gt 0) {
        throw "The $($expectation.name) research names a different run: $divergentCommits"
    }
    $assertionCount++
}

Write-Output "Research/artifact coherence: $assertionCount assertions passed."
