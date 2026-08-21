param(
    [ValidateSet('debug', 'release')]
    [string] $Profile = 'debug'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    $baseTargetDirectory = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        Join-Path $script:WorkspaceRoot 'target'
    }
    elseif ([System.IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
        [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    }
    else {
        [System.IO.Path]::GetFullPath((Join-Path $script:WorkspaceRoot $env:CARGO_TARGET_DIR))
    }
    $sidecarTargetDirectory = Join-Path $baseTargetDirectory 'sidecar-build'
    $buildArguments = @(
        'build',
        '-p',
        'myalbuns-imaging',
        '--bin',
        'myalbuns-imaging',
        '--target-dir',
        $sidecarTargetDirectory
    )
    if ($Profile -eq 'release') {
        $buildArguments += '--release'
    }
    & $script:CargoExecutable @buildArguments
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $source = (
        Resolve-Path (
            Join-Path $sidecarTargetDirectory "$Profile\myalbuns-imaging.exe"
        )
    ).Path
    $binaryDirectory = Join-Path $script:WorkspaceRoot 'src-tauri\binaries'
    $destination = Join-Path $binaryDirectory 'myalbuns-imaging-x86_64-pc-windows-msvc.exe'
    $runtimeDirectory = Join-Path $baseTargetDirectory $Profile
    $runtimeDestination = Join-Path $runtimeDirectory 'myalbuns-imaging.exe'

    if (-not $source.StartsWith($script:WorkspaceRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Origem inesperada do sidecar: $source"
    }
    if (-not $destination.StartsWith($script:WorkspaceRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Destino inesperado do sidecar: $destination"
    }
    if (-not $runtimeDestination.StartsWith($script:WorkspaceRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Destino de runtime inesperado do sidecar: $runtimeDestination"
    }

    New-Item -ItemType Directory -Force -Path $binaryDirectory | Out-Null
    New-Item -ItemType Directory -Force -Path $runtimeDirectory | Out-Null
    Copy-Item -LiteralPath $source -Destination $destination -Force
    Copy-Item -LiteralPath $source -Destination $runtimeDestination -Force
}
finally {
    Pop-Location
}
