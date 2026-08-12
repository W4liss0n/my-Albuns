function Get-TrackedGateSourceStatus {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $WorkspaceRoot,

        [Parameter(Mandatory = $true)]
        [string] $EvidencePath
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

    $status = @(
        & git `
            -C $gitRoot `
            status `
            --porcelain=v1 `
            --untracked-files=no `
            -- `
            @pathSpecs
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'Git could not inspect the tracked gate source inputs.'
    }
    return $status
}
