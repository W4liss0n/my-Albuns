function Resolve-GateRetainedEvidenceRoot {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $GitRoot,

        [Parameter(Mandatory = $true)]
        [string] $RetainedEvidenceRoot
    )

    $resolvedRoot = [System.IO.Path]::GetFullPath($RetainedEvidenceRoot)
    $gitRootPrefix = $GitRoot.TrimEnd('\', '/') `
        + [System.IO.Path]::DirectorySeparatorChar
    if (-not $resolvedRoot.StartsWith(
            $gitRootPrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'The retained gate evidence root must stay inside the Git worktree.'
    }
    if (-not (Test-Path -LiteralPath $resolvedRoot -PathType Container)) {
        throw 'The retained gate evidence root must be an existing directory.'
    }

    $relativeRoot = $resolvedRoot.Substring($gitRootPrefix.Length)
    $cursor = $GitRoot
    foreach ($segment in $relativeRoot.Split(
            [char[]]@(
                [System.IO.Path]::DirectorySeparatorChar,
                [System.IO.Path]::AltDirectorySeparatorChar
            ),
            [System.StringSplitOptions]::RemoveEmptyEntries
        )) {
        $cursor = Join-Path $cursor $segment
        $item = Get-Item -LiteralPath $cursor -Force
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw 'The retained gate evidence root cannot traverse a reparse point.'
        }
    }

    $pendingDirectories = [System.Collections.Generic.Stack[System.IO.DirectoryInfo]]::new()
    $pendingDirectories.Push((Get-Item -LiteralPath $resolvedRoot -Force))
    while ($pendingDirectories.Count -gt 0) {
        $directory = $pendingDirectories.Pop()
        foreach ($entry in $directory.EnumerateFileSystemInfos()) {
            if (($entry.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw 'The retained gate evidence root cannot contain a reparse point.'
            }
            if ($entry -is [System.IO.DirectoryInfo]) {
                $pendingDirectories.Push($entry)
            }
        }
    }

    return $relativeRoot.Replace('\', '/')
}

function Get-GateSourceStatus {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $WorkspaceRoot,

        [Parameter(Mandatory = $true)]
        [string] $EvidencePath,

        [string] $RetainedEvidenceRoot
    )

    $resolvedWorkspaceRoot = (Resolve-Path -LiteralPath $WorkspaceRoot).Path
    $gitRoot = (& git -C $resolvedWorkspaceRoot rev-parse --show-toplevel).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw 'The gate source root is not a readable Git worktree.'
    }
    $gitRoot = [System.IO.Path]::GetFullPath($gitRoot)
    if (-not [string]::Equals(
            $resolvedWorkspaceRoot,
            $gitRoot,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'The gate source root must be the Git worktree root.'
    }

    $pathSpecs = [System.Collections.Generic.List[string]]::new()
    $pathSpecs.Add('.')
    $resolvedEvidencePath = [System.IO.Path]::GetFullPath($EvidencePath)
    $gitRootPrefix = $gitRoot.TrimEnd('\', '/') `
        + [System.IO.Path]::DirectorySeparatorChar
    $evidenceIsInsideWorktree = $resolvedEvidencePath.StartsWith(
        $gitRootPrefix,
        [System.StringComparison]::OrdinalIgnoreCase
    )
    if ($evidenceIsInsideWorktree) {
        $relativeEvidencePath = $resolvedEvidencePath.Substring($gitRootPrefix.Length)
        $gitRelativeEvidencePath = $relativeEvidencePath.Replace('\', '/')
        $pathSpecs.Add(":(top,exclude,literal)$gitRelativeEvidencePath")
    }
    if (-not [string]::IsNullOrWhiteSpace($RetainedEvidenceRoot)) {
        $gitRelativeEvidenceRoot = Resolve-GateRetainedEvidenceRoot `
            -GitRoot $gitRoot `
            -RetainedEvidenceRoot $RetainedEvidenceRoot
        $pathSpecs.Add(":(top,exclude,literal)$gitRelativeEvidenceRoot")
    }

    $status = @(
        & git `
            -C $gitRoot `
            status `
            --porcelain=v1 `
            --untracked-files=all `
            -- `
            @pathSpecs
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'Git could not inspect the gate source inputs.'
    }
    return $status
}

function Get-GateSourceSnapshot {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $WorkspaceRoot,

        [Parameter(Mandatory = $true)]
        [string] $EvidencePath,

        [string] $RetainedEvidenceRoot
    )

    $headBeforeStatus = (& git -C $WorkspaceRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $headBeforeStatus -notmatch '^[0-9a-f]{40}$') {
        throw 'Git could not capture the gate source commit.'
    }
    $status = @(
        Get-GateSourceStatus `
            -WorkspaceRoot $WorkspaceRoot `
            -EvidencePath $EvidencePath `
            -RetainedEvidenceRoot $RetainedEvidenceRoot
    )
    $headAfterStatus = (& git -C $WorkspaceRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $headAfterStatus -notmatch '^[0-9a-f]{40}$') {
        throw 'Git could not verify the gate source commit.'
    }

    return [pscustomobject]@{
        gitCommit = $headBeforeStatus
        sourceInputsDirty = $status.Count -gt 0 `
            -or $headBeforeStatus -ne $headAfterStatus
    }
}

function Test-GateSourceSnapshotsDirty {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [psobject] $Before,

        [Parameter(Mandatory = $true)]
        [psobject] $After
    )

    return $Before.sourceInputsDirty `
        -or $After.sourceInputsDirty `
        -or $Before.gitCommit -ne $After.gitCommit
}
