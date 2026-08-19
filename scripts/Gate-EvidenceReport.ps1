function Assert-GateSourceUnchanged {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [psobject] $Before,

        [Parameter(Mandatory = $true)]
        [string] $WorkspaceRoot,

        [Parameter(Mandatory = $true)]
        [string] $EvidencePath,

        [Parameter(Mandatory = $true)]
        [string] $Stage
    )

    $after = Get-GateSourceSnapshot `
        -WorkspaceRoot $WorkspaceRoot `
        -EvidencePath $EvidencePath
    if (Test-GateSourceSnapshotsDirty -Before $Before -After $after) {
        throw "The gate source changed $Stage."
    }
    return $after
}

function Test-GatePostProofSourceMutationContract([string] $FixtureRoot) {
    New-Item -ItemType Directory -Path $FixtureRoot | Out-Null
    $inputPath = Join-Path $FixtureRoot 'behavior.txt'
    $evidencePath = Join-Path $FixtureRoot 'evidence.json'
    $encoding = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($inputPath, "baseline`n", $encoding)
    & git -C $FixtureRoot init --quiet
    & git -C $FixtureRoot add -- behavior.txt
    & git `
        -C $FixtureRoot `
        -c user.name='MyAlbuns Gate' `
        -c user.email='gate@myalbuns.invalid' `
        commit --quiet -m baseline
    if ($LASTEXITCODE -ne 0) {
        throw 'The post-proof provenance fixture could not create its clean input commit.'
    }

    $before = Get-GateSourceSnapshot `
        -WorkspaceRoot $FixtureRoot `
        -EvidencePath $evidencePath
    [void] [System.IO.File]::ReadAllText($inputPath)
    [System.IO.File]::WriteAllText($inputPath, "mutated after proof`n", $encoding)

    $rejected = $false
    try {
        [void] (Assert-GateSourceUnchanged `
            -Before $before `
            -WorkspaceRoot $FixtureRoot `
            -EvidencePath $evidencePath `
            -Stage 'after the post-proof mutation fixture')
    }
    catch {
        if ($_.Exception.Message -ne
                'The gate source changed after the post-proof mutation fixture.') {
            throw
        }
        $rejected = $true
    }
    if (-not $rejected) {
        throw 'The gate accepted a behavioral input mutation after proof collection.'
    }
    return 1
}

function Get-GateFileSha256([string] $Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString(
                $sha.ComputeHash($stream)
            )).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
        $stream.Dispose()
    }
}

function Get-GateReleaseArtifact([string] $Name, [string] $Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "The release artifact '$Name' was not produced."
    }
    $file = Get-Item -LiteralPath $Path
    return [ordered]@{
        name = $Name
        bytes = [long] $file.Length
        sha256 = Get-GateFileSha256 -Path $file.FullName
    }
}

function Test-GateExclusiveRead([string] $Path) {
    $stream = [System.IO.File]::Open(
        $Path,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::None
    )
    $stream.Dispose()
}

function Publish-GateEvidenceReport {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $Json,

        [Parameter(Mandatory = $true)]
        [psobject] $SourceSnapshot,

        [Parameter(Mandatory = $true)]
        [string] $WorkspaceRoot,

        [Parameter(Mandatory = $true)]
        [string] $EvidencePath
    )

    [void] (Assert-GateSourceUnchanged `
        -Before $SourceSnapshot `
        -WorkspaceRoot $WorkspaceRoot `
        -EvidencePath $EvidencePath `
        -Stage 'before terminal evidence publication')
    $evidenceExisted = Test-Path -LiteralPath $EvidencePath -PathType Leaf
    $previousEvidence = if ($evidenceExisted) {
        [System.IO.File]::ReadAllBytes($EvidencePath)
    }
    else {
        $null
    }
    try {
        New-Item `
            -ItemType Directory `
            -Force `
            -Path (Split-Path -Parent $EvidencePath) |
            Out-Null
        [System.IO.File]::WriteAllText(
            $EvidencePath,
            $Json + [System.Environment]::NewLine,
            [System.Text.UTF8Encoding]::new($false)
        )
        [void] (Assert-GateSourceUnchanged `
            -Before $SourceSnapshot `
            -WorkspaceRoot $WorkspaceRoot `
            -EvidencePath $EvidencePath `
            -Stage 'during terminal evidence publication')
    }
    catch {
        $publicationFailure = $_.Exception
        if ($evidenceExisted) {
            [System.IO.File]::WriteAllBytes($EvidencePath, $previousEvidence)
        }
        else {
            [System.IO.File]::Delete($EvidencePath)
        }
        throw $publicationFailure
    }
}
