$script:BuildInputPathspecs = @(
    'Cargo.toml',
    'Cargo.lock',
    'crates',
    'global.html',
    'index.html',
    'package.json',
    'package-lock.json',
    'public',
    'scripts',
    'src',
    'src-tauri',
    'tests',
    'tsconfig.json',
    'tsconfig.node.json',
    'vite.config.ts',
    'vitest.config.ts'
)

function Get-BuildInputState {
    $relativeFiles = @(
        & git `
            -C $script:WorkspaceRoot `
            ls-files `
            --cached `
            --others `
            --exclude-standard `
            -- `
            @script:BuildInputPathspecs
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not enumerate evidence build inputs with Git.'
    }

    $inputHashes = @(
        $relativeFiles |
            Sort-Object -Unique |
            ForEach-Object {
                $relativePath = $_
                $fullPath = Join-Path $script:WorkspaceRoot $relativePath
                if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
                    throw "Evidence build input no longer exists: $relativePath"
                }
                $hash = (
                    Get-FileHash -LiteralPath $fullPath -Algorithm SHA256
                ).Hash.ToLowerInvariant()
                "$relativePath`0$hash"
            }
    )
    $payload = [System.Text.Encoding]::UTF8.GetBytes(
        $inputHashes -join "`n"
    )
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digest = -join (
            $sha256.ComputeHash($payload) |
                ForEach-Object { $_.ToString('x2') }
        )
    }
    finally {
        $sha256.Dispose()
    }

    $status = @(
        & git `
            -C $script:WorkspaceRoot `
            status `
            --short `
            -- `
            @script:BuildInputPathspecs
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not inspect evidence build input status with Git.'
    }

    return [ordered]@{
        fileCount = $inputHashes.Count
        digestSha256 = $digest
        dirty = $status.Count -gt 0
    }
}
