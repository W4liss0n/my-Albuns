param(
    [ValidateSet('debug', 'release')]
    [string] $Profile = 'debug'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    $baseTargetDirectory = Resolve-MyAlbunsCargoTargetDirectory
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
    $targetDirectoryPrefix = $baseTargetDirectory.TrimEnd(
        [char[]] @('\', '/')
    ) + [System.IO.Path]::DirectorySeparatorChar
    $workspaceDirectoryPrefix = $script:WorkspaceRoot.TrimEnd(
        [char[]] @('\', '/')
    ) + [System.IO.Path]::DirectorySeparatorChar

    if (-not $source.StartsWith(
            $targetDirectoryPrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw "Unexpected sidecar source: $source"
    }
    if (-not $destination.StartsWith(
            $workspaceDirectoryPrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw "Unexpected sidecar destination: $destination"
    }
    if (-not $runtimeDestination.StartsWith(
            $targetDirectoryPrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw "Unexpected sidecar runtime destination: $runtimeDestination"
    }

    New-Item -ItemType Directory -Force -Path $binaryDirectory | Out-Null
    New-Item -ItemType Directory -Force -Path $runtimeDirectory | Out-Null
    Copy-Item -LiteralPath $source -Destination $destination -Force
    Copy-Item -LiteralPath $source -Destination $runtimeDestination -Force
}
finally {
    Pop-Location
}
