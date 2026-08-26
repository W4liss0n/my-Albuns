function Get-MyAlbunsRustToolchain {
    param(
        [Parameter(Mandatory = $true)]
        [string] $WorkspaceRoot
    )

    $toolchainFile = Join-Path $WorkspaceRoot 'rust-toolchain.toml'
    if (-not (Test-Path -LiteralPath $toolchainFile)) {
        throw "The pinned Rust toolchain file was not found: $toolchainFile"
    }

    $toolchainContents = Get-Content -Raw -LiteralPath $toolchainFile
    $channelMatches = [regex]::Matches(
        $toolchainContents,
        '(?m)^\s*channel\s*=\s*"(?<channel>[^"]+)"\s*$'
    )
    if ($channelMatches.Count -ne 1) {
        throw 'rust-toolchain.toml must declare exactly one toolchain channel.'
    }

    return $channelMatches[0].Groups['channel'].Value
}
