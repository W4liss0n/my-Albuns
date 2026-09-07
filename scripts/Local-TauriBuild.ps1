function Invoke-MyAlbunsTauriBuild {
    param(
        [string[]] $TauriArguments = @()
    )

    $sidecarProfile = if ($TauriArguments -contains '--debug') {
        'debug'
    }
    else {
        'release'
    }
    & (Join-Path $script:WorkspaceRoot 'scripts\Prepare-Sidecar.ps1') `
        -Profile $sidecarProfile
    if ($LASTEXITCODE -ne 0) {
        return
    }

    $tauriCommand = Join-Path `
        $script:WorkspaceRoot `
        'node_modules\.bin\tauri.cmd'
    if (-not (Test-Path -LiteralPath $tauriCommand -PathType Leaf)) {
        throw 'The local Tauri CLI does not exist. Run npm run setup:local.'
    }

    & $tauriCommand build @TauriArguments
}
